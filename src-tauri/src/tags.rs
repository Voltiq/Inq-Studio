//! Accessibility: the structure tree of tagged PDFs.
//!
//! A tagged PDF describes its content twice: the page streams draw it, and
//! the structure tree says what it is (heading, paragraph, figure, table)
//! and in which order it is read. Screen readers follow the tree. This
//! module reads the tree, finds where each tagged piece of content sits on
//! its page (so the reading order can be drawn over the page), and edits
//! what assistive technology needs most: alternative text for figures, the
//! document language and the document title.

use crate::interp::{scan_page, Src};
use crate::pages;
use crate::util::*;
use lopdf::content::Operation;
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TagNode {
    /// Object id of the structure element ("12 0"); empty for one that is written inline.
    pub id: String,
    /// The standard type the element maps to (P, H1, Figure, Table …).
    pub role: String,
    /// The type as the document names it, when it differs.
    pub raw: String,
    pub title: String,
    pub alt: String,
    pub actual_text: String,
    pub lang: String,
    pub page: Option<usize>,
    /// The start of the text this element holds.
    pub text: String,
    /// Bounds of its own content on `page`, in user space.
    pub bbox: Option<[f32; 4]>,
    pub children: Vec<TagNode>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    /// The catalog says the document is tagged.
    pub marked: bool,
    pub has_tree: bool,
    pub lang: String,
    pub title: String,
    /// Readers show the title instead of the file name.
    pub display_title: bool,
    pub nodes: Vec<TagNode>,
    pub elements: usize,
    pub figures: usize,
    pub figures_without_alt: usize,
    /// The tree was cut off because it is very large.
    pub truncated: bool,
    /// What assistive technology will stumble over.
    pub issues: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderItem {
    pub n: usize,
    pub role: String,
    pub bbox: [f32; 4],
    pub text: String,
    /// Structure element id, when the order comes from tags.
    pub id: String,
}

/// One piece of marked content on a page.
#[derive(Clone, Default)]
struct Marked {
    text: String,
    bbox: Option<[f32; 4]>,
}

fn grow(b: &mut Option<[f32; 4]>, r: [f32; 4]) {
    *b = Some(match *b {
        Some(o) => [o[0].min(r[0]), o[1].min(r[1]), o[2].max(r[2]), o[3].max(r[3])],
        None => r,
    });
}

fn mcid_of(doc: &Document, res: &Dictionary, op: &Operation) -> Option<i64> {
    let props = op.operands.get(1)?;
    let d = match props {
        Object::Dictionary(d) => Some(d),
        Object::Name(n) => match get(doc, res, b"Properties") {
            Some(Object::Dictionary(p)) => p.get(n).ok().and_then(|o| dict(doc, o)),
            _ => None,
        },
        _ => None,
    }?;
    get_num(doc, d, b"MCID").map(|v| v as i64)
}

/// Marked content of a page by MCID: its text and where it is drawn.
fn marked_content(doc: &Document, page_id: ObjectId) -> HashMap<i64, Marked> {
    let mut out: HashMap<i64, Marked> = HashMap::new();
    let Ok(scan) = scan_page(doc, page_id) else { return out };
    let Some(ops) = scan.ops.get(&Src::Page) else { return out };
    let res = crate::interp::page_resources(doc, page_id);
    // For every operator, the marked-content sequence it is inside.
    let mut owner: Vec<Option<i64>> = Vec::with_capacity(ops.len());
    let mut stack: Vec<Option<i64>> = Vec::new();
    for op in ops {
        match op.operator.as_str() {
            "BDC" => stack.push(mcid_of(doc, &res, op)),
            "BMC" => stack.push(None),
            "EMC" => {
                stack.pop();
            }
            _ => {}
        }
        owner.push(stack.iter().rev().find_map(|m| *m));
    }
    // Lines are already joined across operators; split each by the marked content its characters belong to.
    for l in scan.lines.iter().filter(|l| l.src == "page") {
        let chars: Vec<char> = l.text.chars().collect();
        let width = l.chars.last().copied().unwrap_or(0.0).max(1e-3);
        let owner_of = |i: usize| -> Option<i64> { l.char_runs.get(i).and_then(|&r| scan.runs.get(r)).and_then(|r| owner.get(r.op).copied().flatten()) };
        let mut i = 0;
        while i < chars.len() {
            let id = owner_of(i);
            let mut j = i + 1;
            while j < chars.len() && owner_of(j) == id {
                j += 1;
            }
            if let Some(id) = id {
                let m = out.entry(id).or_default();
                let piece: String = chars[i..j].iter().collect();
                if m.text.len() < 4000 {
                    if !m.text.is_empty() && !m.text.ends_with(' ') && !piece.starts_with(' ') {
                        m.text.push(' ');
                    }
                    m.text.push_str(piece.trim_end());
                }
                let a = l.chars.get(i).copied().unwrap_or(0.0) / width;
                let b = l.chars.get(j).copied().unwrap_or(width) / width;
                let at = |p: [f32; 2], q: [f32; 2], t: f32| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                let pts = [at(l.quad[0], l.quad[1], a), at(l.quad[0], l.quad[1], b), at(l.quad[3], l.quad[2], b), at(l.quad[3], l.quad[2], a)];
                let xs = pts.iter().map(|p| p[0]);
                let ys = pts.iter().map(|p| p[1]);
                grow(&mut m.bbox, [xs.clone().fold(f32::MAX, f32::min), ys.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max), ys.fold(f32::MIN, f32::max)]);
            }
            i = j;
        }
    }
    for im in scan.images.iter().filter(|i| i.src == Src::Page) {
        if let Some(Some(id)) = owner.get(im.op) {
            grow(&mut out.entry(*id).or_default().bbox, im.bbox());
        }
    }
    for p in scan.paths.iter().filter(|p| p.src == Src::Page) {
        if let Some(Some(id)) = owner.get(p.first) {
            if p.bbox[0] <= p.bbox[2] {
                grow(&mut out.entry(*id).or_default().bbox, p.bbox);
            }
        }
    }
    out
}

fn text_of(doc: &Document, d: &Dictionary, key: &[u8]) -> String {
    get(doc, d, key).and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default()
}

const STANDARD: [&str; 49] = [
    "Document", "Part", "Art", "Sect", "Div", "BlockQuote", "Caption", "TOC", "TOCI", "Index", "NonStruct", "Private", "P", "H", "H1", "H2", "H3",
    "H4", "H5", "H6", "L", "LI", "Lbl", "LBody", "Table", "TR", "TH", "TD", "THead", "TBody", "TFoot", "Span", "Quote", "Note", "Reference",
    "BibEntry", "Code", "Link", "Annot", "Ruby", "Warichu", "Figure", "Formula", "Form", "DocumentFragment", "Aside", "Title", "FENote", "Sub",
];

struct Reader<'a> {
    doc: &'a Document,
    role_map: Option<&'a Dictionary>,
    page_index: HashMap<ObjectId, usize>,
    marked: HashMap<usize, HashMap<i64, Marked>>,
    seen: HashSet<ObjectId>,
    count: usize,
    figures: usize,
    figures_without_alt: usize,
    truncated: bool,
    limit: usize,
}

impl Reader<'_> {
    fn role(&self, raw: &str) -> String {
        let mut cur = raw.to_string();
        for _ in 0..8 {
            if STANDARD.contains(&cur.as_str()) {
                break;
            }
            match self.role_map.and_then(|m| get_name(self.doc, m, cur.as_bytes())) {
                Some(next) if next != cur => cur = next,
                _ => break,
            }
        }
        cur
    }

    fn content(&mut self, page: Option<usize>, mcid: i64) -> Marked {
        let Some(p) = page else { return Marked::default() };
        if !self.marked.contains_key(&p) {
            let found = pages::page_id(self.doc, p).map(|pid| marked_content(self.doc, pid)).unwrap_or_default();
            self.marked.insert(p, found);
        }
        self.marked.get(&p).and_then(|m| m.get(&mcid)).cloned().unwrap_or_default()
    }

    /// Reads a structure element (`o` is a reference to it or the dictionary itself).
    fn element(&mut self, o: &Object, inherited_page: Option<usize>, depth: usize) -> Option<TagNode> {
        let doc = self.doc;
        let id = o.as_reference().ok();
        if let Some(id) = id {
            if !self.seen.insert(id) {
                return None;
            }
        }
        let d = dict(doc, o)?;
        if self.count >= self.limit || depth > 60 {
            self.truncated = true;
            return None;
        }
        self.count += 1;
        let raw = get_name(doc, d, b"S").unwrap_or_default();
        let role = self.role(&raw);
        let page = ref_id(d, b"Pg").and_then(|p| self.page_index.get(&p).copied()).or(inherited_page);
        let mut node = TagNode {
            id: id.map(|(n, g)| format!("{n} {g}")).unwrap_or_default(),
            raw: if raw == role { String::new() } else { raw },
            role,
            title: text_of(doc, d, b"T"),
            alt: text_of(doc, d, b"Alt"),
            actual_text: text_of(doc, d, b"ActualText"),
            lang: text_of(doc, d, b"Lang"),
            page,
            text: String::new(),
            bbox: None,
            children: Vec::new(),
        };
        let kids: Vec<Object> = match d.get(b"K") {
            Ok(Object::Array(a)) => a.clone(),
            Ok(Object::Reference(r)) => match doc.get_object(*r) {
                Ok(Object::Array(a)) => a.clone(),
                _ => vec![Object::Reference(*r)],
            },
            Ok(other) => vec![other.clone()],
            Err(_) => Vec::new(),
        };
        for k in &kids {
            match k {
                Object::Integer(mcid) => {
                    let m = self.content(page, *mcid);
                    self.take(&mut node, m);
                }
                _ => {
                    let Some(kd) = dict(doc, k) else { continue };
                    match get_name(doc, kd, b"Type").as_deref() {
                        Some("MCR") => {
                            let p = ref_id(kd, b"Pg").and_then(|p| self.page_index.get(&p).copied()).or(page);
                            if let Some(mcid) = get_num(doc, kd, b"MCID") {
                                let m = self.content(p, mcid as i64);
                                if node.page.is_none() {
                                    node.page = p;
                                }
                                self.take(&mut node, m);
                            }
                        }
                        Some("OBJR") => {
                            // An annotation or form field: use its rectangle.
                            if let Some(r) = kd.get(b"Obj").ok().and_then(|o| dict(doc, o)).and_then(|a| a.get(b"Rect").ok()).and_then(|r| rect(doc, r)) {
                                grow(&mut node.bbox, r);
                            }
                        }
                        _ => {
                            if let Some(child) = self.element(k, page, depth + 1) {
                                node.children.push(child);
                            }
                        }
                    }
                }
            }
        }
        if node.page.is_none() {
            node.page = node.children.iter().find_map(|c| c.page);
        }
        if node.text.is_empty() {
            let mut t = String::new();
            for c in &node.children {
                if t.chars().count() >= 80 {
                    break;
                }
                if !c.text.is_empty() {
                    if !t.is_empty() {
                        t.push(' ');
                    }
                    t.push_str(&c.text);
                }
            }
            node.text = t.chars().take(80).collect();
        }
        if node.role == "Figure" || node.role == "Formula" {
            self.figures += 1;
            if node.alt.trim().is_empty() && node.actual_text.trim().is_empty() {
                self.figures_without_alt += 1;
            }
        }
        Some(node)
    }

    fn take(&mut self, node: &mut TagNode, m: Marked) {
        if let Some(b) = m.bbox {
            grow(&mut node.bbox, b);
        }
        if node.text.chars().count() < 80 && !m.text.is_empty() {
            if !node.text.is_empty() && !node.text.ends_with(' ') {
                node.text.push(' ');
            }
            node.text.push_str(&m.text);
            node.text = node.text.chars().take(80).collect();
        }
    }
}

fn catalog_lang(doc: &Document) -> String {
    doc.catalog().ok().map(|c| text_of(doc, c, b"Lang")).unwrap_or_default()
}

fn display_title(doc: &Document) -> bool {
    doc.catalog()
        .ok()
        .and_then(|c| get(doc, c, b"ViewerPreferences"))
        .and_then(|o| dict(doc, o))
        .and_then(|v| get(doc, v, b"DisplayDocTitle"))
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(false)
}

fn read_with(doc: &Document, limit: usize) -> (Vec<TagNode>, Reader<'_>, bool) {
    let cat = doc.catalog().ok();
    let root = cat.and_then(|c| get(doc, c, b"StructTreeRoot")).and_then(|o| dict(doc, o));
    let mut reader = Reader {
        doc,
        role_map: root.and_then(|r| get(doc, r, b"RoleMap")).and_then(|o| dict(doc, o)),
        page_index: pages::page_ids(doc).into_iter().enumerate().map(|(i, p)| (p, i)).collect(),
        marked: HashMap::new(),
        seen: HashSet::new(),
        count: 0,
        figures: 0,
        figures_without_alt: 0,
        truncated: false,
        limit,
    };
    let mut nodes = Vec::new();
    if let Some(root) = root {
        let kids: Vec<Object> = match root.get(b"K") {
            Ok(Object::Array(a)) => a.clone(),
            Ok(Object::Reference(r)) => match doc.get_object(*r) {
                Ok(Object::Array(a)) => a.clone(),
                _ => vec![Object::Reference(*r)],
            },
            Ok(o) => vec![o.clone()],
            Err(_) => Vec::new(),
        };
        for k in &kids {
            if let Some(n) = reader.element(k, None, 0) {
                nodes.push(n);
            }
        }
    }
    (nodes, reader, root.is_some())
}

pub fn read(doc: &Document) -> TagInfo {
    let (nodes, reader, has_tree) = read_with(doc, 6000);
    let cat = doc.catalog().ok();
    let marked = cat
        .and_then(|c| get(doc, c, b"MarkInfo"))
        .and_then(|o| dict(doc, o))
        .and_then(|m| get(doc, m, b"Marked"))
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(false);
    let lang = catalog_lang(doc);
    let title = crate::store::read_meta(doc).title;
    let show_title = display_title(doc);
    let mut issues = Vec::new();
    if !has_tree || nodes.is_empty() {
        issues.push("The document is not tagged: screen readers get the text in the order the pages draw it, without headings, lists or tables.".to_string());
    } else if !marked {
        issues.push("The document has tags but does not say so (MarkInfo is missing), so some readers ignore them.".to_string());
    }
    if lang.trim().is_empty() {
        issues.push("No document language is set, so a screen reader has to guess the pronunciation.".to_string());
    }
    if title.trim().is_empty() {
        issues.push("The document has no title.".to_string());
    } else if !show_title {
        issues.push("Readers show the file name instead of the title.".to_string());
    }
    if reader.figures_without_alt > 0 {
        issues.push(format!(
            "{} of {} figure{} without alternative text.",
            reader.figures_without_alt,
            reader.figures,
            if reader.figures == 1 { " is" } else { "s are" }
        ));
    }
    TagInfo {
        marked,
        has_tree,
        lang,
        title,
        display_title: show_title,
        elements: reader.count,
        figures: reader.figures,
        figures_without_alt: reader.figures_without_alt,
        truncated: reader.truncated,
        nodes,
        issues,
    }
}

fn parse_id(id: &str) -> Result<ObjectId, String> {
    let mut it = id.split_whitespace();
    let n = it.next().and_then(|s| s.parse().ok()).ok_or("That element cannot be changed: it has no object of its own.")?;
    let g = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Ok((n, g))
}

/// Sets or clears text properties of a structure element.
pub fn set_element(doc: &mut Document, id: &str, alt: Option<&str>, actual_text: Option<&str>, title: Option<&str>) -> Result<(), String> {
    let oid = parse_id(id)?;
    let d = doc.get_dictionary_mut(oid).map_err(|_| "That element is no longer in the document.")?;
    if !d.has(b"S") {
        return Err("That object is not a structure element.".into());
    }
    for (key, value) in [("Alt", alt), ("ActualText", actual_text), ("T", title)] {
        match value {
            Some(v) if v.trim().is_empty() => {
                d.remove(key.as_bytes());
            }
            Some(v) => d.set(key, lopdf::text_string(v)),
            None => {}
        }
    }
    Ok(())
}

/// Sets the document language (a BCP 47 tag like "en-GB") and whether
/// readers show the title in their title bar.
pub fn set_document(doc: &mut Document, lang: Option<&str>, show_title: Option<bool>) -> Result<(), String> {
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(err)?;
    if let Some(lang) = lang {
        let l = lang.trim();
        if !l.is_empty() && !l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("A language is written like en, en-GB or de-CH.".into());
        }
        let cat = doc.get_dictionary_mut(root).map_err(err)?;
        if l.is_empty() {
            cat.remove(b"Lang");
        } else {
            cat.set("Lang", Object::string_literal(l));
        }
    }
    if let Some(show) = show_title {
        let existing = doc.get_dictionary(root).map_err(err)?.get(b"ViewerPreferences").ok().cloned();
        match existing {
            Some(Object::Reference(id)) if doc.get_dictionary(id).is_ok() => {
                doc.get_dictionary_mut(id).map_err(err)?.set("DisplayDocTitle", show);
            }
            other => {
                let mut prefs = match other {
                    Some(Object::Dictionary(d)) => d,
                    _ => Dictionary::new(),
                };
                prefs.set("DisplayDocTitle", show);
                doc.get_dictionary_mut(root).map_err(err)?.set("ViewerPreferences", Object::Dictionary(prefs));
            }
        }
    }
    Ok(())
}

/// The order assistive technology reads a page in: the structure tree's
/// order for a tagged document, the order the page draws its paragraphs
/// otherwise.
pub fn reading_order(doc: &Document, page: usize) -> Result<Vec<OrderItem>, String> {
    let (nodes, _, has_tree) = read_with(doc, 40_000);
    let mut out = Vec::new();
    fn walk(n: &TagNode, page: usize, out: &mut Vec<OrderItem>) {
        if n.page == Some(page) || n.children.iter().any(|c| c.page == Some(page)) || n.page.is_none() {
            if let (Some(b), Some(p)) = (n.bbox, n.page) {
                if p == page {
                    out.push(OrderItem { n: out.len() + 1, role: n.role.clone(), bbox: b, text: n.text.clone(), id: n.id.clone() });
                }
            }
        }
        for c in &n.children {
            walk(c, page, out);
        }
    }
    if has_tree {
        for n in &nodes {
            walk(n, page, &mut out);
        }
    }
    if out.is_empty() {
        let pid = pages::page_id(doc, page)?;
        let scan = scan_page(doc, pid)?;
        for b in &scan.blocks {
            let xs = b.quad.iter().map(|p| p[0]);
            let ys = b.quad.iter().map(|p| p[1]);
            out.push(OrderItem {
                n: out.len() + 1,
                role: String::new(),
                bbox: [xs.clone().fold(f32::MAX, f32::min), ys.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max), ys.fold(f32::MIN, f32::max)],
                text: b.text.chars().take(80).collect(),
                id: String::new(),
            });
        }
    }
    Ok(out)
}
