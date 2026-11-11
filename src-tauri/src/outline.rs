//! Bookmarks (the document outline): read as a tree of titles and target
//! pages, edited in the app, and written back as a fresh outline.

use crate::util::*;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub title: String,
    /// Target page index; `None` when the target is not a page in this document.
    pub page: Option<usize>,
    /// Where on the page (user-space y of the top), when known.
    #[serde(default)]
    pub top: Option<f32>,
    #[serde(default)]
    pub open: bool,
    #[serde(default)]
    pub children: Vec<Bookmark>,
}

/// Looks a named destination up in /Dests or the /Names/Dests name tree.
fn named(doc: &Document, name: &[u8]) -> Option<Object> {
    let cat = doc.catalog().ok()?;
    if let Some(d) = cat.get(b"Dests").ok().and_then(|o| dict(doc, o)) {
        if let Ok(v) = d.get(name) {
            return Some(deref(doc, v).clone());
        }
    }
    let tree = cat.get(b"Names").ok().and_then(|o| dict(doc, o))?.get(b"Dests").ok().and_then(|o| dict(doc, o))?;
    find_in_tree(doc, tree, name, 0)
}

fn find_in_tree(doc: &Document, node: &Dictionary, name: &[u8], depth: usize) -> Option<Object> {
    if depth > 32 {
        return None;
    }
    if let Some(names) = get_array(doc, node, b"Names") {
        for pair in names.chunks(2) {
            if pair.first().map(|k| deref(doc, k)).and_then(|k| k.as_str().ok()) == Some(name) {
                return pair.get(1).map(|v| deref(doc, v).clone());
            }
        }
    }
    for k in get_array(doc, node, b"Kids")? {
        if let Some(d) = dict(doc, k) {
            if let Some(v) = find_in_tree(doc, d, name, depth + 1) {
                return Some(v);
            }
        }
    }
    None
}

/// (page index, top) of a destination.
fn resolve(doc: &Document, dest: &Object, pages: &[ObjectId]) -> (Option<usize>, Option<f32>) {
    let mut d = deref(doc, dest).clone();
    for _ in 0..4 {
        d = match &d {
            Object::Name(n) | Object::String(n, _) => match named(doc, n) {
                Some(v) => v,
                None => return (None, None),
            },
            Object::Dictionary(x) => match x.get(b"D") {
                Ok(v) => deref(doc, v).clone(),
                Err(_) => return (None, None),
            },
            _ => break,
        };
    }
    let Object::Array(a) = d else { return (None, None) };
    let page = a.first().and_then(|p| match p {
        Object::Reference(id) => pages.iter().position(|x| x == id),
        Object::Integer(i) => Some(*i as usize),
        _ => None,
    });
    let kind = a.get(1).and_then(|o| o.as_name().ok()).unwrap_or(b"");
    let top = match kind {
        b"XYZ" => a.get(3).and_then(num),
        b"FitH" | b"FitBH" => a.get(2).and_then(num),
        _ => None,
    };
    (page, top)
}

pub fn get_outline(doc: &Document) -> Vec<Bookmark> {
    let pages = crate::pages::page_ids(doc);
    let Some(root) = doc.catalog().ok().and_then(|c| c.get(b"Outlines").ok()).and_then(|o| dict(doc, o)) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    children(doc, root, &pages, &mut seen, 0)
}

fn children(doc: &Document, parent: &Dictionary, pages: &[ObjectId], seen: &mut HashSet<ObjectId>, depth: usize) -> Vec<Bookmark> {
    let mut out = Vec::new();
    if depth > 24 {
        return out;
    }
    let mut next = parent.get(b"First").ok().and_then(|o| o.as_reference().ok());
    while let Some(id) = next {
        if !seen.insert(id) || out.len() > 10_000 {
            break;
        }
        let Ok(d) = doc.get_dictionary(id) else { break };
        let title = get(doc, d, b"Title").and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default();
        let target = d.get(b"Dest").ok().or_else(|| {
            let a = d.get(b"A").ok().and_then(|o| dict(doc, o))?;
            (get_name(doc, a, b"S").as_deref() == Some("GoTo")).then(|| a.get(b"D").ok()).flatten()
        });
        let (page, top) = target.map(|t| resolve(doc, t, pages)).unwrap_or((None, None));
        let open = get_num(doc, d, b"Count").map(|c| c > 0.0).unwrap_or(false);
        out.push(Bookmark { title, page, top, open, children: children(doc, d, pages, seen, depth + 1) });
        next = d.get(b"Next").ok().and_then(|o| o.as_reference().ok());
    }
    out
}

/// Writes a new outline; the old outline objects are left for pruning on save.
pub fn set_outline(doc: &mut Document, items: &[Bookmark]) -> Result<(), String> {
    let pages = crate::pages::page_ids(doc);
    if items.is_empty() {
        doc.catalog_mut().map_err(err)?.remove(b"Outlines");
        return Ok(());
    }
    let root = doc.new_object_id();
    let (first, last, count) = write_level(doc, items, root, &pages)?;
    doc.objects.insert(
        root,
        Object::Dictionary(dictionary! { "Type" => "Outlines", "First" => first, "Last" => last, "Count" => count }),
    );
    let cat = doc.catalog_mut().map_err(err)?;
    cat.set("Outlines", Object::Reference(root));
    if !cat.has(b"PageMode") {
        cat.set("PageMode", "UseOutlines");
    }
    Ok(())
}

/// Writes siblings; returns (first, last, visible descendant count).
fn write_level(doc: &mut Document, items: &[Bookmark], parent: ObjectId, pages: &[ObjectId]) -> Result<(ObjectId, ObjectId, i64), String> {
    let ids: Vec<ObjectId> = items.iter().map(|_| doc.new_object_id()).collect();
    let mut visible = 0i64;
    for (i, b) in items.iter().enumerate() {
        let mut d = Dictionary::new();
        d.set("Title", lopdf::text_string(&b.title));
        d.set("Parent", Object::Reference(parent));
        if i > 0 {
            d.set("Prev", Object::Reference(ids[i - 1]));
        }
        if i + 1 < ids.len() {
            d.set("Next", Object::Reference(ids[i + 1]));
        }
        if let Some(p) = b.page.and_then(|p| pages.get(p)) {
            let top = b.top.map(real).unwrap_or(Object::Null);
            d.set("Dest", vec![Object::Reference(*p), "XYZ".into(), Object::Null, top, Object::Null]);
        }
        visible += 1;
        if !b.children.is_empty() {
            let (f, l, c) = write_level(doc, &b.children, ids[i], pages)?;
            d.set("First", f);
            d.set("Last", l);
            d.set("Count", if b.open { c } else { -c });
            if b.open {
                visible += c;
            }
        }
        doc.objects.insert(ids[i], Object::Dictionary(d));
    }
    Ok((ids[0], *ids.last().unwrap(), visible))
}
