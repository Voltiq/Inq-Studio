//! Text editing.
//!
//! Edits rewrite the content streams themselves rather than painting over
//! them: changed text is re-encoded in its original fonts and put back where
//! it was drawn, so the result is real, selectable, searchable text.
//!
//! * A line edit keeps the line in place. Each new character takes the style
//!   (font, size, colour) of the old character it replaces, so a bold word in
//!   a sentence stays bold when the sentence changes.
//! * A paragraph edit reflows: the text is wrapped to the paragraph's column
//!   with the fonts' own advance widths, keeping its alignment and leading.
//! * Characters a font cannot draw come, in this order, from the font's
//!   embedded program, from the same typeface installed on this computer, from
//!   a broad installed font of the same kind, and last from a standard font.
//!   Only the characters that need it are moved to another font.

use crate::encoding::BaseEncoding;
use crate::fonts::{apply_additions, Addition, Piece, PdfFont};
use crate::interp::{page_resources, scan_page, Fill, Scan, Src};
use crate::sysfonts::{self, LoadedFont};
use crate::util::*;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct LineEdit {
    pub text: Option<String>,
    pub size: Option<f32>,
    pub color: Option<[f32; 3]>,
    pub font: Option<FontChoice>,
    pub dx: f32,
    pub dy: f32,
    pub delete: bool,
    /// For rewriting recognised (invisible) text on a scan: the colour of the
    /// paper around the line and of its ink, as the window sees them.
    pub cover: Option<[f32; 3]>,
    pub ink: Option<[f32; 3]>,
    /// Keep the paragraph around the line in shape: a rewritten line is
    /// wrapped again together with the rest of its paragraph, and the lines
    /// below a deleted line move up.
    pub reflow: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FontChoice {
    /// A font already in the document, by key ("12 0").
    Document { key: String },
    /// An installed font, by family.
    System { family: String, bold: bool, italic: bool },
    /// One of the standard 14.
    Standard { name: String },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewText {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font: FontChoice,
    pub size: f32,
    pub color: [f32; 3],
    #[serde(default = "default_leading")]
    pub line_height: f32,
}

fn default_leading() -> f32 {
    1.2
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

enum ResLoc {
    Obj(ObjectId),
    PageInline(ObjectId),
    FormInline(ObjectId),
}

fn res_dict_mut<'a>(doc: &'a mut Document, loc: &ResLoc) -> Result<&'a mut Dictionary, String> {
    match loc {
        ResLoc::Obj(id) => doc.get_dictionary_mut(*id).map_err(err),
        ResLoc::PageInline(p) => doc
            .get_dictionary_mut(*p)
            .map_err(err)?
            .get_mut(b"Resources")
            .and_then(Object::as_dict_mut)
            .map_err(err),
        ResLoc::FormInline(f) => doc
            .get_object_mut(*f)
            .and_then(Object::as_stream_mut)
            .map_err(err)?
            .dict
            .get_mut(b"Resources")
            .and_then(Object::as_dict_mut)
            .map_err(err),
    }
}

fn page_res_loc(doc: &mut Document, page_id: ObjectId) -> Result<ResLoc, String> {
    let page = doc.get_dictionary(page_id).map_err(err)?;
    Ok(match page.get(b"Resources") {
        Ok(Object::Reference(id)) => ResLoc::Obj(*id),
        Ok(Object::Dictionary(_)) => ResLoc::PageInline(page_id),
        _ => {
            let inherited = page_resources(doc, page_id);
            doc.get_dictionary_mut(page_id).map_err(err)?.set("Resources", Object::Dictionary(inherited));
            ResLoc::PageInline(page_id)
        }
    })
}

/// Makes a font object available to a content stream and returns its resource name.
pub fn add_font_resource(doc: &mut Document, page_id: ObjectId, src: Src, font_id: ObjectId) -> Result<Vec<u8>, String> {
    add_resource(doc, page_id, src, "Font", font_id, "InqF")
}

/// Makes an object available to a content stream under a resource category
/// (Font, XObject, ExtGState, …) and returns its name; an existing entry for
/// the same object is reused.
pub fn add_resource(
    doc: &mut Document,
    page_id: ObjectId,
    src: Src,
    category: &str,
    obj_id: ObjectId,
    prefix: &str,
) -> Result<Vec<u8>, String> {
    let loc = match src {
        Src::Page => page_res_loc(doc, page_id)?,
        Src::Form(fid) => {
            let dict = &doc.get_object(fid).and_then(Object::as_stream).map_err(err)?.dict;
            match dict.get(b"Resources") {
                Ok(Object::Reference(id)) => ResLoc::Obj(*id),
                Ok(Object::Dictionary(_)) => ResLoc::FormInline(fid),
                _ => page_res_loc(doc, page_id)?,
            }
        }
    };
    let cat = category.as_bytes();
    let sub_ref = match res_dict_mut(doc, &loc)?.get(cat) {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    };
    let existing: Dictionary = match sub_ref {
        Some(id) => doc.get_dictionary(id).cloned().unwrap_or_default(),
        None => res_dict_mut(doc, &loc)?.get(cat).and_then(Object::as_dict).cloned().unwrap_or_default(),
    };
    for (name, obj) in existing.iter() {
        if obj.as_reference().ok() == Some(obj_id) {
            return Ok(name.clone());
        }
    }
    let mut n = existing.len() + 1;
    let name = loop {
        let candidate = format!("{prefix}{n}").into_bytes();
        if !existing.has(&candidate) {
            break candidate;
        }
        n += 1;
    };
    match sub_ref {
        Some(id) => doc.get_dictionary_mut(id).map_err(err)?.set(name.clone(), Object::Reference(obj_id)),
        None => {
            let mut d = existing;
            d.set(name.clone(), Object::Reference(obj_id));
            res_dict_mut(doc, &loc)?.set(category, Object::Dictionary(d));
        }
    }
    Ok(name)
}

/// A graphics state with the given fill and stroke opacity, as a page resource name.
pub fn opacity_state(doc: &mut Document, page_id: ObjectId, opacity: f32) -> Result<Vec<u8>, String> {
    let id = doc.add_object(lopdf::dictionary! { "Type" => "ExtGState", "ca" => real(opacity), "CA" => real(opacity) });
    add_resource(doc, page_id, Src::Page, "ExtGState", id, "InqGS")
}

/// Adds operators to a page, over everything (or under, with `behind`).
/// The existing content is wrapped in q/Q so its state cannot leak.
pub fn add_page_ops(doc: &mut Document, page_id: ObjectId, ops: Vec<Operation>, behind: bool) -> Result<(), String> {
    if ops.is_empty() {
        return Ok(());
    }
    let existing = crate::interp::page_content_ops(doc, page_id)?;
    let mut out = Vec::with_capacity(existing.len() + ops.len() + 2);
    if behind {
        out.extend(ops);
        out.push(Operation::new("q", vec![]));
        out.extend(existing);
        out.push(Operation::new("Q", vec![]));
    } else {
        out.push(Operation::new("q", vec![]));
        out.extend(existing);
        out.push(Operation::new("Q", vec![]));
        out.extend(ops);
    }
    set_ops(doc, page_id, Src::Page, out)
}

/// Writes operators back to their stream: a page gets one fresh content stream,
/// a form gets its stream content replaced.
pub fn set_ops(doc: &mut Document, page_id: ObjectId, src: Src, ops: Vec<Operation>) -> Result<(), String> {
    let bytes = Content { operations: ops }.encode().map_err(err)?;
    match src {
        Src::Page => {
            let mut stream = Stream::new(Dictionary::new(), bytes);
            let _ = stream.compress();
            let id = doc.add_object(stream);
            doc.get_dictionary_mut(page_id).map_err(err)?.set("Contents", Object::Reference(id));
        }
        Src::Form(fid) => {
            let s = doc.get_object_mut(fid).and_then(Object::as_stream_mut).map_err(err)?;
            s.dict.remove(b"Filter");
            s.dict.remove(b"DecodeParms");
            s.set_plain_content(bytes);
            let _ = s.compress();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Encoding with fallbacks
// ---------------------------------------------------------------------------

/// One encoded character.
#[derive(Clone, Debug)]
struct Enc {
    res: Vec<u8>,
    code_len: u8,
    piece: Piece,
    /// Advance in glyph space (thousandths of an em).
    w: f32,
}

/// What text should be set in.
#[derive(Clone)]
enum Target {
    /// A document font; with its resource name when it is already in the stream.
    Doc(Arc<PdfFont>, Option<Vec<u8>>),
    Choice(FontChoice),
}

#[derive(Clone)]
struct StyleHint {
    ps_name: String,
    family: String,
    bold: bool,
    italic: bool,
    serif: bool,
    fixed: bool,
}

impl StyleHint {
    fn of(f: &PdfFont) -> StyleHint {
        StyleHint {
            ps_name: f.ps_name.clone(),
            family: f.family.clone(),
            bold: f.bold,
            italic: f.italic,
            serif: f.serif,
            fixed: f.fixed,
        }
    }
}

fn list_chars(chars: &[char]) -> String {
    let shown: Vec<String> = chars.iter().take(8).map(|c| format!("‘{c}’")).collect();
    let more = if chars.len() > 8 { format!(" and {} more", chars.len() - 8) } else { String::new() };
    format!("{}{more}", shown.join(" "))
}

fn dedup(chars: impl Iterator<Item = char>) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    for c in chars {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

fn win_ansi_code(c: char) -> Option<u32> {
    let c = if c == '\u{A0}' || c == '\t' { ' ' } else { c };
    BaseEncoding::WinAnsi.table().iter().position(|x| *x == Some(c)).map(|p| p as u32)
}

/// Encodes text across one edit, keeping track of glyphs added to fonts so
/// several pieces of text can grow the same font without clashing.
struct Encoder {
    page_id: ObjectId,
    notices: Vec<String>,
    /// Per document font: the font as loaded, a working copy that knows the
    /// glyphs added so far, and those additions.
    fonts: HashMap<String, (Arc<PdfFont>, PdfFont, Vec<Addition>)>,
    loaded: HashMap<ObjectId, PdfFont>,
}

impl Encoder {
    fn new(page_id: ObjectId) -> Encoder {
        Encoder { page_id, notices: Vec::new(), fonts: HashMap::new(), loaded: HashMap::new() }
    }

    fn note(&mut self, s: String) {
        if !self.notices.contains(&s) {
            self.notices.push(s);
        }
    }

    /// The width of a code in a font object, loading it once.
    fn width_in(&mut self, doc: &Document, font_id: ObjectId, code: u32) -> f32 {
        self.loaded
            .entry(font_id)
            .or_insert_with(|| PdfFont::load(doc, &Object::Reference(font_id), format!("{} {}", font_id.0, font_id.1)))
            .width(code)
    }

    /// Encodes `text` (no line breaks) for stream `src`: one entry per character.
    fn encode(&mut self, doc: &mut Document, src: Src, target: &Target, text: &str) -> Result<Vec<Enc>, String> {
        let chars: Vec<char> = text.chars().collect();
        let mut out: Vec<Option<Enc>> = vec![None; chars.len()];
        let hint = match target {
            Target::Doc(f, _) => StyleHint::of(f),
            Target::Choice(FontChoice::Document { key }) => {
                let f = find_font(doc, key).ok_or("That font is not in this document.")?;
                StyleHint::of(&f)
            }
            Target::Choice(FontChoice::System { family, bold, italic }) => {
                let l = family.to_lowercase();
                StyleHint {
                    ps_name: String::new(),
                    family: family.clone(),
                    bold: *bold,
                    italic: *italic,
                    serif: l.contains("times") || l.contains("georgia") || (l.contains("serif") && !l.contains("sans")),
                    fixed: l.contains("mono") || l.contains("courier") || l.contains("consol"),
                }
            }
            Target::Choice(FontChoice::Standard { name }) => StyleHint {
                ps_name: name.clone(),
                family: name.split('-').next().unwrap_or(name).to_string(),
                bold: name.contains("Bold"),
                italic: name.contains("Italic") || name.contains("Oblique"),
                serif: name.starts_with("Times"),
                fixed: name.starts_with("Courier"),
            },
        };

        // 1. The chosen or original font itself.
        let doc_font = match target {
            Target::Doc(f, res) => Some((f.clone(), res.clone())),
            Target::Choice(FontChoice::Document { key }) => Some((Arc::new(find_font(doc, key).unwrap()), None)),
            _ => None,
        };
        if let Some((font, known_res)) = doc_font {
            let entry = self
                .fonts
                .entry(font.key.clone())
                .or_insert_with(|| (font.clone(), (*font).clone(), Vec::new()));
            let enc = entry.1.encode_with(doc, text, false);
            let mut pieces = enc.pieces.into_iter();
            let mut slots: Vec<Option<Piece>> = Vec::new();
            for c in &chars {
                slots.push(if enc.missing.contains(c) { None } else { pieces.next() });
            }
            if !enc.additions.is_empty() {
                entry.1.register(&enc.additions);
                entry.2.extend(enc.additions.iter().cloned());
            }
            if slots.iter().any(|s| s.is_some()) {
                let res = match (known_res, font.obj) {
                    (Some(r), _) => r,
                    (None, Some(id)) => add_font_resource(doc, self.page_id, src, id)?,
                    (None, None) => return Err("This font is stored inside the page and cannot be reused.".into()),
                };
                let working = &self.fonts[&font.key].1;
                for (i, s) in slots.into_iter().enumerate() {
                    if let Some(piece) = s {
                        let w = match &piece {
                            Piece::Code(c) => working.width(*c),
                            Piece::Gap(g) => *g,
                        };
                        out[i] = Some(Enc { res: res.clone(), code_len: font.code_len, piece, w });
                    }
                }
            }
        } else if let Target::Choice(FontChoice::Standard { name }) = target {
            let id = sysfonts::standard_font(doc, name);
            let res = add_font_resource(doc, self.page_id, src, id)?;
            for (i, c) in chars.iter().enumerate() {
                if let Some(code) = win_ansi_code(*c) {
                    let w = self.width_in(doc, id, code);
                    out[i] = Some(Enc { res: res.clone(), code_len: 1, piece: Piece::Code(code), w });
                }
            }
            let missing = dedup(chars.iter().enumerate().filter(|(i, _)| out[*i].is_none()).map(|(_, c)| *c));
            if !missing.is_empty() {
                return Err(format!("{name} cannot draw {}. Pick an installed font instead.", list_chars(&missing)));
            }
        }

        // 2. The same typeface installed on this computer (or the chosen installed family).
        let missing = dedup(chars.iter().enumerate().filter(|(i, _)| out[*i].is_none()).map(|(_, c)| *c));
        if missing.is_empty() {
            return Ok(out.into_iter().map(|e| e.unwrap()).collect());
        }
        let preferred: Option<LoadedFont> = if hint.ps_name.is_empty() {
            sysfonts::find_family(&hint.family, None, hint.bold, hint.italic)
        } else {
            sysfonts::find(&hint.ps_name, hint.bold, hint.italic)
        };
        let mut remaining = missing.clone();
        if let Some(face) = preferred {
            let covered = covered_by(&face, &remaining);
            if !covered.is_empty() {
                let wanted: String = covered.iter().collect();
                if let Ok(emb) = sysfonts::embed(doc, &face, &wanted) {
                    self.loaded.remove(&emb.font_id);
                    let res = add_font_resource(doc, self.page_id, src, emb.font_id)?;
                    for (i, c) in chars.iter().enumerate() {
                        if out[i].is_none() {
                            if let Some(code) = emb.codes.get(c) {
                                let w = self.width_in(doc, emb.font_id, *code);
                                out[i] = Some(Enc { res: res.clone(), code_len: 2, piece: Piece::Code(*code), w });
                            }
                        }
                    }
                    remaining.retain(|c| !covered.contains(c));
                    if matches!(target, Target::Doc(..)) || matches!(target, Target::Choice(FontChoice::Document { .. })) {
                        self.note(format!(
                            "{} came from {} installed on this computer, because the embedded subset of {} lacks {}.",
                            if covered.len() == 1 { "One character" } else { "Some characters" },
                            face.ps_name,
                            hint.ps_name,
                            list_chars(&covered)
                        ));
                    }
                }
            }
        }

        // 3. A broad installed font of the same kind, then a standard font.
        if !remaining.is_empty() {
            let family = if hint.fixed { "Courier New" } else if hint.serif { "Times New Roman" } else { "Arial" };
            let mut used = None;
            if let Some(face) = sysfonts::find_family(family, None, hint.bold, hint.italic) {
                let covered = covered_by(&face, &remaining);
                if !covered.is_empty() {
                    let wanted: String = covered.iter().collect();
                    if let Ok(emb) = sysfonts::embed(doc, &face, &wanted) {
                        self.loaded.remove(&emb.font_id);
                        let res = add_font_resource(doc, self.page_id, src, emb.font_id)?;
                        for (i, c) in chars.iter().enumerate() {
                            if out[i].is_none() {
                                if let Some(code) = emb.codes.get(c) {
                                    let w = self.width_in(doc, emb.font_id, *code);
                                    out[i] = Some(Enc { res: res.clone(), code_len: 2, piece: Piece::Code(*code), w });
                                }
                            }
                        }
                        remaining.retain(|c| !covered.contains(c));
                        used = Some((face.ps_name.clone(), covered));
                    }
                }
            }
            if !remaining.is_empty() {
                let base = sysfonts::standard_for(hint.serif, hint.fixed, hint.bold, hint.italic);
                if remaining.iter().all(|c| win_ansi_code(*c).is_some()) {
                    let id = sysfonts::standard_font(doc, base);
                    let res = add_font_resource(doc, self.page_id, src, id)?;
                    for (i, c) in chars.iter().enumerate() {
                        if out[i].is_none() {
                            let code = win_ansi_code(*c).unwrap();
                            let w = self.width_in(doc, id, code);
                            out[i] = Some(Enc { res: res.clone(), code_len: 1, piece: Piece::Code(code), w });
                        }
                    }
                    used = Some((base.to_string(), remaining.clone()));
                    remaining.clear();
                } else {
                    let bad: Vec<char> = remaining.iter().copied().filter(|c| win_ansi_code(*c).is_none()).collect();
                    return Err(format!(
                        "No font on this computer can draw {}. Install a font that has these characters.",
                        list_chars(&bad)
                    ));
                }
            }
            if let Some((name, chars)) = used {
                self.note(format!(
                    "{} is not available for {}, so {} set in {name}; everything else keeps its font.",
                    if hint.family.is_empty() { &hint.ps_name } else { &hint.family },
                    list_chars(&chars),
                    if chars.len() == 1 { "it is" } else { "they are" },
                ));
            }
        }
        Ok(out.into_iter().map(|e| e.expect("every character encoded")).collect())
    }

    /// Records glyph additions in the fonts they grew.
    fn finish(self, doc: &mut Document) -> Result<Vec<String>, String> {
        let mut notices = self.notices;
        for (_, (original, _, mut additions)) in self.fonts {
            if additions.is_empty() {
                continue;
            }
            additions.sort_by_key(|a| a.code);
            additions.dedup_by_key(|a| a.code);
            apply_additions(doc, &original, &additions)?;
            notices.push(format!(
                "Added {} glyph{} to {} from its embedded font program.",
                additions.len(),
                if additions.len() == 1 { "" } else { "s" },
                original.ps_name
            ));
        }
        Ok(notices)
    }
}

fn covered_by(face: &LoadedFont, chars: &[char]) -> Vec<char> {
    let Ok(f) = ttf_parser::Face::parse(&face.data, face.index) else { return Vec::new() };
    chars
        .iter()
        .copied()
        .filter(|c| {
            let c2 = if *c == '\t' || *c == '\u{A0}' { ' ' } else { *c };
            f.glyph_index(c2).is_some()
        })
        .collect()
}

pub fn find_font(doc: &Document, key: &str) -> Option<PdfFont> {
    let mut parts = key.split(' ');
    let n: u32 = parts.next()?.parse().ok()?;
    let g: u16 = parts.next()?.parse().ok()?;
    doc.get_object((n, g)).ok()?;
    Some(PdfFont::load(doc, &Object::Reference((n, g)), key.to_string()))
}

// ---------------------------------------------------------------------------
// Laying out and writing text
// ---------------------------------------------------------------------------

/// How a stretch of text is drawn.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    size: f32,
    fill: Fill,
    tc: f32,
}

/// One character ready to write.
#[derive(Clone, Debug)]
struct Item {
    ch: char,
    enc: Enc,
    look: usize,
    /// Extra advance after this character, in TJ units (thousandths of the font size, positive moves left).
    tj_after: f32,
}

/// Show operators for a run of items, switching font, size and colour as needed.
/// `state` is (resource, size, fill) currently selected; it is updated.
fn shows(items: &[Item], looks: &[Look], state: &mut (Vec<u8>, f32, Fill), force_tf: bool, out: &mut Vec<Operation>) {
    let mut i = 0;
    let mut first = true;
    while i < items.len() {
        let it = &items[i];
        let look = &looks[it.look];
        if it.enc.res != state.0 || (look.size - state.1).abs() > 1e-4 || (first && force_tf) {
            out.push(Operation::new("Tf", vec![Object::Name(it.enc.res.clone()), real(look.size)]));
            state.0 = it.enc.res.clone();
            state.1 = look.size;
        }
        first = false;
        if look.fill != state.2 {
            out.extend(look.fill.ops());
            state.2 = look.fill.clone();
        }
        let format = if it.enc.code_len == 2 { StringFormat::Hexadecimal } else { StringFormat::Literal };
        let mut arr: Vec<Object> = Vec::new();
        let mut cur: Vec<u8> = Vec::new();
        let mut j = i;
        while j < items.len() && items[j].enc.res == it.enc.res && items[j].look == it.look {
            let x = &items[j];
            match x.enc.piece {
                Piece::Code(c) => {
                    if x.enc.code_len == 2 {
                        cur.push((c >> 8) as u8);
                    }
                    cur.push(c as u8);
                }
                Piece::Gap(g) => {
                    if !cur.is_empty() {
                        arr.push(Object::String(std::mem::take(&mut cur), format));
                    }
                    arr.push(real(-g.round()));
                }
            }
            if x.tj_after.abs() > 0.01 {
                if !cur.is_empty() {
                    arr.push(Object::String(std::mem::take(&mut cur), format));
                }
                arr.push(real((x.tj_after * 100.0).round() / 100.0));
            }
            j += 1;
        }
        if !cur.is_empty() {
            arr.push(Object::String(cur, format));
        }
        if arr.len() == 1 && matches!(arr[0], Object::String(..)) {
            out.push(Operation::new("Tj", arr));
        } else if !arr.is_empty() {
            out.push(Operation::new("TJ", vec![Object::Array(arr)]));
        }
        i = j;
    }
}

/// For each character of `new`, the style it should take from `old`.
///
/// The two texts are aligned (longest common subsequence). Kept characters
/// keep their style; text that replaces old text takes the replaced text's
/// styles proportionally ("bold" → "BOLD" stays bold); purely inserted text
/// takes the style of the character before it.
fn map_styles(old: &[char], old_style: &[usize], new: &[char]) -> Vec<usize> {
    let fallback = old_style.first().copied().unwrap_or(0);
    // Case is ignored when aligning, so retyping a word in capitals keeps its style.
    let same = |a: char, b: char| a == b || a.to_lowercase().eq(b.to_lowercase());
    if old.is_empty() {
        return vec![fallback; new.len()];
    }
    // Trim the common prefix and suffix, then align the middle.
    let mut p = 0;
    while p < old.len() && p < new.len() && same(old[p], new[p]) {
        p += 1;
    }
    let mut s = 0;
    while s < old.len() - p && s < new.len() - p && same(old[old.len() - 1 - s], new[new.len() - 1 - s]) {
        s += 1;
    }
    let (om, nm) = (&old[p..old.len() - s], &new[p..new.len() - s]);
    // Matched pairs (old index, new index) in the middle, by LCS when it is small enough.
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    if !om.is_empty() && !nm.is_empty() && om.len() * nm.len() <= 4_000_000 {
        let (n, m) = (om.len(), nm.len());
        let mut dp = vec![0u16; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[i * (m + 1) + j] = if same(om[i], nm[j]) {
                    dp[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    dp[(i + 1) * (m + 1) + j].max(dp[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if same(om[i], nm[j]) {
                pairs.push((p + i, p + j));
                i += 1;
                j += 1;
            } else if dp[(i + 1) * (m + 1) + j] >= dp[i * (m + 1) + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    let mut anchors: Vec<(usize, usize)> = (0..p).map(|i| (i, i)).collect();
    anchors.extend(pairs);
    anchors.extend((0..s).map(|k| (old.len() - s + k, new.len() - s + k)));

    let mut out = vec![usize::MAX; new.len()];
    for &(o, n) in &anchors {
        out[n] = old_style[o];
    }
    // Fill the gaps between anchors.
    let mut prev: Option<(usize, usize)> = None;
    let mut bounds = anchors.clone();
    bounds.push((old.len(), new.len()));
    for &(o, n) in &bounds {
        let (o0, n0) = prev.map(|(a, b)| (a + 1, b + 1)).unwrap_or((0, 0));
        if n > n0 {
            let gap_new = n - n0;
            for t in 0..gap_new {
                out[n0 + t] = if o > o0 {
                    old_style[o0 + t * (o - o0) / gap_new]
                } else if n0 > 0 {
                    out[n0 - 1]
                } else if o < old.len() {
                    old_style[o]
                } else {
                    fallback
                };
            }
        }
        prev = Some((o, n));
    }
    out.into_iter().map(|s| if s == usize::MAX { fallback } else { s }).collect()
}

/// Changes to apply to a page's streams in one go.
#[derive(Default)]
struct Plan {
    replace: HashMap<(Src, usize), Vec<Operation>>,
    append: Vec<Operation>,
}

impl Plan {
    fn empty_op(&mut self, scan: &Scan, src: Src, i: usize) {
        if let Some(op) = scan.ops.get(&src).and_then(|o| o.get(i)) {
            self.replace.entry((src, i)).or_insert_with(|| vec![emptied(op)]);
        }
    }

    fn apply(self, doc: &mut Document, page_id: ObjectId, scan: &Scan) -> Result<(), String> {
        let mut srcs: Vec<Src> = self.replace.keys().map(|k| k.0).collect();
        if !self.append.is_empty() {
            srcs.push(Src::Page);
        }
        srcs.sort_by_key(|s| match s {
            Src::Page => (0, 0, 0),
            Src::Form((n, g)) => (1, *n, *g),
        });
        srcs.dedup();
        for src in srcs {
            let ops = scan.ops.get(&src).cloned().unwrap_or_default();
            let mut out = Vec::with_capacity(ops.len() + 8);
            for (i, op) in ops.into_iter().enumerate() {
                match self.replace.get(&(src, i)) {
                    Some(r) => out.extend(r.iter().cloned()),
                    None => out.push(op),
                }
            }
            if src == Src::Page && !self.append.is_empty() {
                if !scan.clean_end {
                    out.insert(0, Operation::new("q", vec![]));
                    out.push(Operation::new("Q", vec![]));
                }
                out.extend(self.append.iter().cloned());
            }
            set_ops(doc, page_id, src, out)?;
        }
        Ok(())
    }
}

/// The same operator with its text removed (positioning kept).
fn emptied(op: &Operation) -> Operation {
    match op.operator.as_str() {
        "TJ" => Operation::new("TJ", vec![Object::Array(vec![])]),
        "\"" => {
            let mut o = op.operands.clone();
            if o.len() == 3 {
                o[2] = Object::String(vec![], StringFormat::Literal);
            }
            Operation::new("\"", o)
        }
        "Tj" | "'" => Operation::new(&op.operator, vec![Object::String(vec![], StringFormat::Literal)]),
        _ => op.clone(),
    }
}

fn src_of(label: &str) -> Src {
    let parts: Vec<&str> = label.split(' ').collect();
    if parts.len() == 3 && parts[0] == "form" {
        if let (Ok(n), Ok(g)) = (parts[1].parse(), parts[2].parse()) {
            return Src::Form((n, g));
        }
    }
    Src::Page
}

/// Encodes characters that each carry a style index into items.
fn encode_styled(
    enc: &mut Encoder,
    doc: &mut Document,
    src: Src,
    chars: &[char],
    styles: &[usize],
    targets: &[Target],
) -> Result<Vec<Item>, String> {
    let mut items = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        while j < chars.len() && styles[j] == styles[i] {
            j += 1;
        }
        let text: String = chars[i..j].iter().collect();
        let encs = enc.encode(doc, src, &targets[styles[i]], &text)?;
        for (k, e) in encs.into_iter().enumerate() {
            items.push(Item { ch: chars[i + k], enc: e, look: styles[i], tj_after: 0.0 });
        }
        i = j;
    }
    Ok(items)
}

// ---------------------------------------------------------------------------
// Line edits (in place)
// ---------------------------------------------------------------------------

fn plan_line(
    doc: &mut Document,
    _page_id: ObjectId,
    scan: &Scan,
    line_id: usize,
    edit: &LineEdit,
    enc: &mut Encoder,
    plan: &mut Plan,
) -> Result<(), String> {
    let line = scan.lines.get(line_id).ok_or("This line no longer exists. Refresh and try again.")?;
    if !line.editable {
        return Err(line.note.clone().unwrap_or_else(|| "This text cannot be edited.".into()));
    }
    let src = src_of(&line.src);
    if edit.delete {
        for &ri in &line.runs {
            plan.empty_op(scan, src, scan.runs[ri].op);
        }
        return Ok(());
    }
    let first = &scan.runs[line.runs[0]];
    let new_text: Vec<char> = edit.text.clone().unwrap_or_else(|| line.text.clone()).replace(['\r', '\n'], " ").chars().collect();
    let old_text: Vec<char> = line.text.chars().collect();

    // Styles are the line's runs (local indexes).
    let run_ids: Vec<usize> = line.runs.clone();
    let old_styles: Vec<usize> = line.char_runs.iter().map(|r| run_ids.iter().position(|x| x == r).unwrap_or(0)).collect();
    let styles = map_styles(&old_text, &old_styles, &new_text);
    let factor = edit.size.map(|s| s / first.size.max(0.01)).unwrap_or(1.0);
    let targets: Vec<Target> = run_ids
        .iter()
        .map(|&ri| {
            let r = &scan.runs[ri];
            match &edit.font {
                Some(choice) => Target::Choice(choice.clone()),
                None => Target::Doc(r.font.clone(), Some(r.res_name.clone())),
            }
        })
        .collect();
    let looks: Vec<Look> = run_ids
        .iter()
        .map(|&ri| {
            let r = &scan.runs[ri];
            Look {
                size: r.size * factor,
                fill: match edit.color {
                    Some([cr, cg, cb]) => Fill::Rgb(cr, cg, cb),
                    None => r.fill.clone(),
                },
                tc: r.tc,
            }
        })
        .collect();
    let items = encode_styled(enc, doc, src, &new_text, &styles, &targets)?;

    let src_ops = &scan.ops[&src];
    let op = &src_ops[first.op];
    let mut out: Vec<Operation> = Vec::new();
    // Positioning the original operator did before showing.
    match op.operator.as_str() {
        "'" => out.push(Operation::new("T*", vec![])),
        "\"" => {
            out.push(Operation::new("Tw", vec![op.operands.first().cloned().unwrap_or(Object::Integer(0))]));
            out.push(Operation::new("Tc", vec![op.operands.get(1).cloned().unwrap_or(Object::Integer(0))]));
            out.push(Operation::new("T*", vec![]));
        }
        _ => {}
    }
    let moved = edit.dx.abs() > 1e-3 || edit.dy.abs() > 1e-3;
    if moved {
        let lin = [first.ctm[0], first.ctm[1], first.ctm[2], first.ctm[3], 0.0, 0.0];
        let inv = invert(&lin).ok_or("The text is drawn with a degenerate transform.")?;
        let (ddx, ddy) = apply_vec(&inv, edit.dx, edit.dy);
        let mut m = first.tm;
        m[4] += ddx;
        m[5] += ddy;
        out.push(Operation::new("Tm", m.iter().map(|v| real(*v)).collect()));
    }
    let mut state = (first.res_name.clone(), first.size, first.fill.clone());
    shows(&items, &looks, &mut state, false, &mut out);
    if out.iter().all(|o| o.operator != "Tj" && o.operator != "TJ") {
        out.push(Operation::new("Tj", vec![Object::String(vec![], StringFormat::Literal)]));
    }
    // Put the state back for whatever the stream draws next.
    if state.0 != first.res_name || (state.1 - first.size).abs() > 1e-4 {
        out.push(Operation::new("Tf", vec![Object::Name(first.res_name.clone()), real(first.size)]));
    }
    if state.2 != first.fill {
        out.extend(first.fill.ops());
    }
    if moved {
        out.push(Operation::new("Tm", first.tlm.iter().map(|v| real(*v)).collect()));
    }
    plan.replace.insert((src, first.op), out);
    for &ri in &line.runs[1..] {
        let r = &scan.runs[ri];
        if r.op != first.op {
            plan.empty_op(scan, src, r.op);
        }
    }
    Ok(())
}

/// Edits one or more lines on a page in a single change.
pub fn edit_lines(doc: &mut Document, page_id: ObjectId, ids: &[usize], edit: &LineEdit) -> Result<Vec<String>, String> {
    let scan = scan_page(doc, page_id)?;
    // Recognised text on a scan is invisible: rewriting it must change what the page shows.
    let hidden = |id: &usize| scan.lines.get(*id).map(|l| l.invisible).unwrap_or(false);
    if edit.text.is_some() && !ids.is_empty() && ids.iter().all(hidden) {
        return rewrite_scanned(doc, page_id, &scan, ids, edit);
    }
    if edit.reflow && ids.len() == 1 && edit.size.is_none() && edit.font.is_none() && edit.color.is_none() {
        if let (Some(text), Some(line)) = (&edit.text, scan.lines.get(ids[0])) {
            if let Some(notices) = reflow_line(doc, page_id, &scan, line.block, ids[0], text)? {
                return Ok(notices);
            }
        }
    }
    let mut enc = Encoder::new(page_id);
    let mut plan = Plan::default();
    for &id in ids {
        plan_line(doc, page_id, &scan, id, edit, &mut enc, &mut plan)?;
    }
    if edit.reflow && edit.delete {
        close_gaps(doc, page_id, &scan, ids, &mut enc, &mut plan)?;
    }
    let notices = enc.finish(doc)?;
    plan.apply(doc, page_id, &scan)?;
    Ok(notices)
}

/// After lines were deleted: the lines below them in the same paragraph move up.
fn close_gaps(doc: &mut Document, page_id: ObjectId, scan: &Scan, deleted: &[usize], enc: &mut Encoder, plan: &mut Plan) -> Result<(), String> {
    for block in &scan.blocks {
        if block.lines.len() < 2 {
            continue;
        }
        let up = scan.runs[scan.lines[block.lines[0]].runs[0]].up;
        let mut gone = 0.0f32;
        for &li in &block.lines {
            if deleted.contains(&li) {
                gone += 1.0;
            } else if gone > 0.0 {
                let shift = LineEdit { dx: up.0 * gone * block.leading, dy: up.1 * gone * block.leading, ..Default::default() };
                plan_line(doc, page_id, scan, li, &shift, enc, plan)?;
            }
        }
    }
    Ok(())
}

/// Where a line starts and ends along the baseline, and how far below the
/// paragraph's first line it sits, in the paragraph's own frame.
fn line_place(scan: &Scan, block: &crate::interp::Block, li: usize) -> (f32, f32, f32) {
    let first = &scan.lines[block.lines[0]];
    let fr = &scan.runs[first.runs[0]];
    let l = &scan.lines[li];
    let v = (l.origin[0] - first.origin[0], l.origin[1] - first.origin[1]);
    let x = v.0 * fr.dir.0 + v.1 * fr.dir.1;
    (x, x + l.chars.last().copied().unwrap_or(0.0), v.0 * fr.up.0 + v.1 * fr.up.1)
}

/// The last line (index within the paragraph block) of the sentence flow that
/// line `from` belongs to: the first line at or after it that was broken on
/// purpose. A line was broken on purpose when the first word of the next line
/// would still have fitted on it, or when the next line is indented.
fn flow_end(scan: &Scan, block: &crate::interp::Block, from: usize) -> usize {
    let base = block.left.min(block.first_x);
    let right = base + block.width;
    let flush_left = block.align == "left" || block.align == "justify";
    for j in from..block.lines.len() - 1 {
        let l = &scan.lines[block.lines[j]];
        let next = &scan.lines[block.lines[j + 1]];
        let (x0, x1, _) = line_place(scan, block, block.lines[j]);
        let (n0, _, _) = line_place(scan, block, block.lines[j + 1]);
        let word = next.text.chars().position(|c| c == ' ').unwrap_or(next.text.chars().count());
        let word_w = next.chars.get(word).copied().unwrap_or(0.0);
        let space = 0.25 * l.em;
        let room = if flush_left { right - x1 } else { block.width - (x1 - x0) };
        if word_w + space <= room - 0.5 || (flush_left && n0 > block.left + 0.5 * l.em) {
            return j;
        }
    }
    block.lines.len() - 1
}

/// Rewrites one line of a paragraph and wraps it again together with the
/// lines that continue its sentence; what follows in the paragraph moves up
/// or down by the lines that were won or lost. `None` when the line stands
/// alone, so there is no column to wrap to.
fn reflow_line(doc: &mut Document, page_id: ObjectId, scan: &Scan, block_id: usize, line_id: usize, text: &str) -> Result<Option<Vec<String>>, String> {
    let Some(block) = scan.blocks.get(block_id) else { return Ok(None) };
    let Some(from) = block.lines.iter().position(|&l| l == line_id) else { return Ok(None) };
    if block.lines.len() < 2 || block.lines.iter().any(|&l| !scan.lines[l].editable) {
        return Ok(None);
    }
    let to = flow_end(scan, block, from);
    // The paragraph's text after this line, up to the end of the flow.
    let of_line = |li: usize| -> Option<(usize, usize)> {
        let first = block.char_src.iter().position(|c| matches!(c, Some((l, _)) if *l == li))?;
        let last = block.char_src.iter().rposition(|c| matches!(c, Some((l, _)) if *l == li))?;
        Some((first, last + 1))
    };
    let (Some((_, own_end)), Some((_, end))) = (of_line(line_id), of_line(block.lines[to])) else { return Ok(None) };
    let all: Vec<char> = block.text.chars().collect();
    let rest: String = all[own_end..end].iter().collect();
    let mut new = text.replace(['\r', '\n'], " ");
    // A word that was split with a hyphen at the line end is whole in the paragraph's text.
    let was_split = from < to && !rest.starts_with(' ') && scan.lines[line_id].text.ends_with('-');
    if was_split && new.ends_with('-') {
        new.pop();
    }
    new.push_str(&rest);
    let new = new.trim_start().to_string();
    rewrap(doc, page_id, scan, block, from, to, &new).map(Some)
}

#[cfg(test)]
/// Edits one line. Returns a notice when the edit had to change fonts, or did
/// anything else worth telling.
pub fn edit_line(doc: &mut Document, page_id: ObjectId, line_id: usize, edit: &LineEdit) -> Result<Option<String>, String> {
    let notices = edit_lines(doc, page_id, &[line_id], edit)?;
    Ok((!notices.is_empty()).then(|| notices.join(" ")))
}

// ---------------------------------------------------------------------------
// Paragraph edits (reflow)
// ---------------------------------------------------------------------------

/// Writes laid-out lines as a new text object on the page.
fn emit_lines(
    lines: &[(Vec<Item>, (f32, f32))],
    looks: &[Look],
    ctm: Matrix,
    tm_lin: [f32; 4],
    th: f32,
) -> Result<Vec<Operation>, String> {
    let inv = invert(&ctm).ok_or("The text is drawn with a degenerate transform.")?;
    let mut out = vec![
        Operation::new("q", vec![]),
        Operation::new("cm", ctm.iter().map(|v| real(*v)).collect()),
        Operation::new("BT", vec![]),
        Operation::new("Tr", vec![Object::Integer(0)]),
        Operation::new("Ts", vec![Object::Integer(0)]),
        Operation::new("Tw", vec![Object::Integer(0)]),
        Operation::new("Tz", vec![real(th * 100.0)]),
    ];
    let mut state = (Vec::new(), -1.0f32, Fill::Gray(-1.0));
    let mut tc = f32::NAN;
    for (items, point) in lines {
        if items.is_empty() {
            continue;
        }
        let (e, f) = apply(&inv, point.0, point.1);
        out.push(Operation::new(
            "Tm",
            vec![real(tm_lin[0]), real(tm_lin[1]), real(tm_lin[2]), real(tm_lin[3]), real(e), real(f)],
        ));
        // Character spacing follows the look of each stretch.
        let mut i = 0;
        while i < items.len() {
            let mut j = i;
            while j < items.len() && looks[items[j].look].tc == looks[items[i].look].tc {
                j += 1;
            }
            let want = looks[items[i].look].tc;
            if want != tc {
                out.push(Operation::new("Tc", vec![real(want)]));
                tc = want;
            }
            shows(&items[i..j], looks, &mut state, false, &mut out);
            i = j;
        }
    }
    out.push(Operation::new("ET", vec![]));
    out.push(Operation::new("Q", vec![]));
    Ok(out)
}

/// Breaks styled items into lines no wider than `limit(i)` (points), at spaces
/// and hard breaks. Returns each line with its width, and whether it ends a paragraph.
fn wrap(items: Vec<Item>, adv: &dyn Fn(&Item) -> f32, limit: &dyn Fn(usize) -> f32) -> Vec<(Vec<Item>, f32, bool)> {
    let mut lines: Vec<(Vec<Item>, f32, bool)> = Vec::new();
    let mut line: Vec<Item> = Vec::new();
    let mut width = 0.0;
    let mut word: Vec<Item> = Vec::new();
    let mut word_w = 0.0;
    let trimmed = |l: &[Item]| -> f32 {
        let mut w: f32 = l.iter().map(adv).sum();
        for it in l.iter().rev() {
            if it.ch == ' ' {
                w -= adv(it);
            } else {
                break;
            }
        }
        w
    };
    let flush_word = |line: &mut Vec<Item>, width: &mut f32, word: &mut Vec<Item>, word_w: &mut f32, lines: &mut Vec<(Vec<Item>, f32, bool)>| {
        let word_core: f32 = {
            let mut w = *word_w;
            for it in word.iter().rev() {
                if it.ch == ' ' {
                    w -= adv(it);
                } else {
                    break;
                }
            }
            w
        };
        if !line.is_empty() && *width + word_core > limit(lines.len()) + 0.01 {
            let w = trimmed(line);
            lines.push((std::mem::take(line), w, false));
            *width = 0.0;
        }
        *width += *word_w;
        line.append(word);
        *word_w = 0.0;
    };
    for it in items {
        if it.ch == '\n' {
            flush_word(&mut line, &mut width, &mut word, &mut word_w, &mut lines);
            let w = trimmed(&line);
            lines.push((std::mem::take(&mut line), w, true));
            width = 0.0;
            continue;
        }
        let is_space = it.ch == ' ';
        word_w += adv(&it);
        word.push(it);
        if is_space {
            flush_word(&mut line, &mut width, &mut word, &mut word_w, &mut lines);
        }
    }
    flush_word(&mut line, &mut width, &mut word, &mut word_w, &mut lines);
    let w = trimmed(&line);
    lines.push((line, w, true));
    // Trailing spaces are not drawn.
    for l in lines.iter_mut() {
        while l.0.last().map(|i| i.ch == ' ').unwrap_or(false) {
            l.0.pop();
        }
    }
    lines
}

/// Rewrites a paragraph with new text, wrapped to its column.
pub fn edit_block(doc: &mut Document, page_id: ObjectId, block_id: usize, text: &str) -> Result<Vec<String>, String> {
    let scan = scan_page(doc, page_id)?;
    let block = scan.blocks.get(block_id).ok_or("This paragraph no longer exists. Refresh and try again.")?.clone();
    let last = block.lines.len() - 1;
    rewrap(doc, page_id, &scan, &block, 0, last, text)
}

/// Replaces the lines `from..=to` of a paragraph (indexes within the block)
/// with `text`, wrapped to the paragraph's column. Lines of the paragraph
/// below `to` keep their text and move by the lines won or lost.
fn rewrap(doc: &mut Document, page_id: ObjectId, scan: &Scan, block: &crate::interp::Block, from: usize, to: usize, text: &str) -> Result<Vec<String>, String> {
    for &li in &block.lines {
        if !scan.lines[li].editable {
            return Err(scan.lines[li].note.clone().unwrap_or_else(|| "This text cannot be edited.".into()));
        }
    }
    let whole = from == 0 && to + 1 == block.lines.len();
    let part = &block.lines[from..=to];
    let first_line = &scan.lines[part[0]];
    let fr = &scan.runs[first_line.runs[0]];

    // Styles: every run of the lines being replaced.
    let mut run_ids: Vec<usize> = Vec::new();
    for &li in part {
        for &ri in &scan.lines[li].runs {
            if !run_ids.contains(&ri) {
                run_ids.push(ri);
            }
        }
    }
    let local = |ri: usize| run_ids.iter().position(|x| *x == ri).unwrap_or(0);
    // The stretch of the paragraph's text those lines hold.
    let lo = block.char_src.iter().position(|c| matches!(c, Some((l, _)) if part.contains(l))).unwrap_or(0);
    let hi = block.char_src.iter().rposition(|c| matches!(c, Some((l, _)) if part.contains(l))).map(|i| i + 1).unwrap_or(0);
    let mut old_styles: Vec<usize> = Vec::new();
    for cs in &block.char_src[lo..hi] {
        let s = match cs {
            Some((li, ci)) => local(scan.lines[*li].char_runs[*ci]),
            None => old_styles.last().copied().unwrap_or(0),
        };
        old_styles.push(s);
    }
    let old: Vec<char> = block.text.chars().skip(lo).take(hi - lo).collect();
    let new: Vec<char> = text.replace("\r\n", "\n").replace('\r', "\n").chars().collect();
    let styles = map_styles(&old, &old_styles, &new);

    // Sizes are normalised to the first run's text matrix, which the new text uses.
    let k = fr.em / fr.size.max(0.01);
    let targets: Vec<Target> = run_ids.iter().map(|&ri| Target::Doc(scan.runs[ri].font.clone(), None)).collect();
    let looks: Vec<Look> = run_ids
        .iter()
        .map(|&ri| {
            let r = &scan.runs[ri];
            Look { size: r.em / k, fill: r.fill.portable(), tc: r.tc * (r.em / r.size.max(0.01)) / k }
        })
        .collect();

    let mut enc = Encoder::new(page_id);
    // Encode everything but the line breaks, then put the breaks back.
    let mut items: Vec<Item> = Vec::new();
    let mut i = 0;
    while i < new.len() {
        let mut j = i;
        while j < new.len() && new[j] != '\n' {
            j += 1;
        }
        items.extend(encode_styled(&mut enc, doc, Src::Page, &new[i..j], &styles[i..j], &targets)?);
        if j < new.len() {
            items.push(Item {
                ch: '\n',
                enc: Enc { res: Vec::new(), code_len: 1, piece: Piece::Gap(0.0), w: 0.0 },
                look: styles[j],
                tj_after: 0.0,
            });
        }
        i = j + 1;
    }

    let th = fr.th;
    let adv = |it: &Item| -> f32 {
        let l = &looks[it.look];
        (it.enc.w / 1000.0 * l.size + l.tc) * th * k
    };
    let single = block.lines.len() == 1;
    let indent = block.first_x - block.left;
    let width = block.width.max(1.0);
    // Where the first replaced line starts, in the frame of the paragraph's first line.
    let (start_x, _, start_y) = line_place(scan, block, part[0]);
    let right = block.left.min(block.first_x) + width;
    let limit = |n: usize| -> f32 {
        if single {
            f32::MAX
        } else if n == 0 && from > 0 {
            (right - start_x).max(1.0)
        } else if n == 0 {
            width - indent
        } else {
            width
        }
    };
    let mut laid = wrap(items, &adv, &limit);

    // Place lines in the frame of the paragraph's first line.
    let (d, u) = (fr.dir, fr.up);
    let o = scan.lines[block.lines[0]].origin;
    let mut placed: Vec<(Vec<Item>, (f32, f32))> = Vec::new();
    let mut new_para = true;
    let count = laid.len();
    for (n, (line, w, ends_para)) in laid.iter_mut().enumerate() {
        let continued = n == 0 && from > 0;
        let x0 = if continued { start_x } else if new_para { block.first_x } else { block.left };
        let room = if single { *w } else if continued { (right - start_x).max(1.0) } else if new_para { width - indent } else { width };
        let free = (room - *w).max(0.0);
        let mut x = x0;
        match block.align.as_str() {
            "center" if !single => x += free / 2.0,
            "right" if !single => x += free,
            "justify" if !*ends_para && n + 1 < count => {
                let spaces = line.iter().filter(|it| it.ch == ' ').count();
                if spaces > 0 && free < width * 0.35 {
                    let per = free / spaces as f32;
                    for it in line.iter_mut().filter(|it| it.ch == ' ') {
                        let l = &looks[it.look];
                        it.tj_after = -(per / (k * th) / l.size.max(0.01)) * 1000.0;
                    }
                }
            }
            _ => {}
        }
        let y = start_y - (n as f32) * block.leading;
        let p = (o[0] + d.0 * x + u.0 * y, o[1] + d.1 * x + u.1 * y);
        placed.push((std::mem::take(line), p));
        new_para = *ends_para;
    }

    let mut plan = Plan::default();
    plan.append = emit_lines(&placed, &looks, fr.ctm, [fr.tm[0], fr.tm[1], fr.tm[2], fr.tm[3]], th)?;
    for &li in part {
        let l = &scan.lines[li];
        let src = src_of(&l.src);
        for &ri in &l.runs {
            plan.empty_op(scan, src, scan.runs[ri].op);
        }
    }
    // The rest of the paragraph follows: up when lines were won, down when the text grew.
    let n_new = placed.iter().filter(|p| !p.0.is_empty()).count().max(1);
    let won = part.len() as f32 - n_new as f32;
    if !whole && won != 0.0 {
        let shift = LineEdit { dx: u.0 * won * block.leading, dy: u.1 * won * block.leading, ..Default::default() };
        for &li in &block.lines[to + 1..] {
            plan_line(doc, page_id, scan, li, &shift, &mut enc, &mut plan)?;
        }
    }
    let mut notices = enc.finish(doc)?;
    plan.apply(doc, page_id, scan)?;
    if !single && whole {
        let n = placed.iter().filter(|p| !p.0.is_empty()).count();
        if n != block.lines.len() {
            notices.push(format!("The paragraph now runs over {n} lines instead of {}.", block.lines.len()));
        }
    }
    Ok(notices)
}

// ---------------------------------------------------------------------------
// New text
// ---------------------------------------------------------------------------

pub fn page_rotation(doc: &Document, page_id: ObjectId) -> i64 {
    inherited(doc, page_id, b"Rotate").and_then(num).map(|r| (r as i64).rem_euclid(360)).unwrap_or(0)
}

/// Adds new text at a point in user space, upright as the page is shown.
pub fn add_text(doc: &mut Document, page_id: ObjectId, spec: &NewText) -> Result<Option<String>, String> {
    if spec.text.trim().is_empty() {
        return Err("There is no text to add.".into());
    }
    let scan = scan_page(doc, page_id)?;
    let text = spec.text.replace("\r\n", "\n");
    let target = Target::Choice(spec.font.clone());
    let looks = vec![Look { size: spec.size, fill: Fill::Rgb(spec.color[0], spec.color[1], spec.color[2]), tc: 0.0 }];
    let mut enc = Encoder::new(page_id);
    let rot = page_rotation(doc, page_id) as f32;
    let (s, c) = rot.to_radians().sin_cos();
    let up = (-s, c);
    let mut lines = Vec::new();
    for (n, l) in text.split('\n').enumerate() {
        let chars: Vec<char> = l.chars().collect();
        let items = encode_styled(&mut enc, doc, Src::Page, &chars, &vec![0; chars.len()], std::slice::from_ref(&target))?;
        let lead = n as f32 * spec.size * spec.line_height;
        lines.push((items, (spec.x - up.0 * lead, spec.y - up.1 * lead)));
    }
    let mut plan = Plan::default();
    plan.append = emit_lines(&lines, &looks, IDENTITY, [c, s, -s, c], 1.0)?;
    let notices = enc.finish(doc)?;
    plan.apply(doc, page_id, &scan)?;
    Ok((!notices.is_empty()).then(|| notices.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::map_styles;

    #[test]
    fn styles_follow_the_text() {
        let old: Vec<char> = "Hello bold world".chars().collect();
        // "bold" (6..10) is style 1.
        let st: Vec<usize> = (0..old.len()).map(|i| if (6..10).contains(&i) { 1 } else { 0 }).collect();
        let new: Vec<char> = "Hello very bold world!".chars().collect();
        let m = map_styles(&old, &st, &new);
        let bold: String = new.iter().zip(&m).filter(|(_, s)| **s == 1).map(|(c, _)| *c).collect();
        assert_eq!(bold, "bold");
        let new2: Vec<char> = "Hello BOLD world".chars().collect();
        let m2 = map_styles(&old, &st, &new2);
        let bold2: String = new2.iter().zip(&m2).filter(|(_, s)| **s == 1).map(|(c, _)| *c).collect();
        assert_eq!(bold2, "BOLD");
    }
}

/// Sets one line of text on a page: in any font choice, anchored at a point
/// in user space with `align` 0 (start), 0.5 (centre) or 1 (end) along an
/// angle in degrees, optionally see-through and behind the page content.
/// Returns notices about font substitutions.
#[allow(clippy::too_many_arguments)]
pub fn place_text(
    doc: &mut Document,
    page_id: ObjectId,
    text: &str,
    font: &FontChoice,
    size: f32,
    color: [f32; 3],
    anchor: (f32, f32),
    angle: f32,
    align: f32,
    opacity: f32,
    behind: bool,
) -> Result<Vec<String>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let target = Target::Choice(font.clone());
    let looks = vec![Look { size, fill: Fill::Rgb(color[0], color[1], color[2]), tc: 0.0 }];
    let mut enc = Encoder::new(page_id);
    let chars: Vec<char> = text.replace(['\r', '\n'], " ").chars().collect();
    let items = encode_styled(&mut enc, doc, Src::Page, &chars, &vec![0; chars.len()], std::slice::from_ref(&target))?;
    let width: f32 = items.iter().map(|it| it.enc.w / 1000.0 * size).sum();
    let (s, c) = angle.to_radians().sin_cos();
    let start = (anchor.0 - c * width * align, anchor.1 - s * width * align);
    let mut ops = emit_lines(&[(items, start)], &looks, IDENTITY, [c, s, -s, c], 1.0)?;
    if opacity < 1.0 {
        let gs = opacity_state(doc, page_id, opacity.clamp(0.02, 1.0))?;
        ops.insert(1, Operation::new("gs", vec![Object::Name(gs)]));
    }
    let notices = enc.finish(doc)?;
    add_page_ops(doc, page_id, ops, behind)?;
    Ok(notices)
}

/// One recognised word for an invisible text layer: where its baseline
/// starts, which way it runs, and how wide it must come out.
pub struct HiddenWord {
    pub text: String,
    pub origin: (f32, f32),
    /// Unit vectors along and up the baseline, in user space.
    pub dir: (f32, f32),
    pub up: (f32, f32),
    pub size: f32,
    pub width: f32,
}

/// Lays invisible text over a page (rendering mode 3): it is not drawn, but
/// it can be searched, selected and copied. Every word is stretched to the
/// width it was recognised with, so a selection covers the scanned word.
/// Returns how many words were placed, and notices about fonts.
pub fn place_hidden_words(doc: &mut Document, page_id: ObjectId, words: &[HiddenWord], font: &FontChoice) -> Result<(usize, Vec<String>), String> {
    let target = Target::Choice(font.clone());
    let mut enc = Encoder::new(page_id);
    let mut ops = vec![Operation::new("q", vec![]), Operation::new("BT", vec![]), Operation::new("Tr", vec![Object::Integer(3)])];
    let mut state = (Vec::new(), -1.0f32, Fill::Gray(0.0));
    let mut placed = 0;
    for w in words {
        let chars: Vec<char> = w.text.chars().filter(|c| !c.is_control()).collect();
        if chars.is_empty() || w.size < 0.5 || w.width <= 0.0 {
            continue;
        }
        // A word no available font can write is left out rather than failing the page.
        let Ok(items) = encode_styled(&mut enc, doc, Src::Page, &chars, &vec![0; chars.len()], std::slice::from_ref(&target)) else { continue };
        let natural: f32 = items.iter().map(|it| it.enc.w / 1000.0 * w.size).sum();
        if natural <= 0.0 {
            continue;
        }
        let stretch = (w.width / natural * 100.0).clamp(5.0, 2000.0);
        let looks = [Look { size: w.size, fill: Fill::Gray(0.0), tc: 0.0 }];
        ops.push(Operation::new("Tz", vec![real(stretch)]));
        ops.push(Operation::new("Tm", vec![real(w.dir.0), real(w.dir.1), real(w.up.0), real(w.up.1), real(w.origin.0), real(w.origin.1)]));
        shows(&items, &looks, &mut state, false, &mut ops);
        placed += 1;
    }
    ops.push(Operation::new("ET", vec![]));
    ops.push(Operation::new("Q", vec![]));
    let notices = enc.finish(doc)?;
    if placed > 0 {
        add_page_ops(doc, page_id, ops, false)?;
    }
    Ok((placed, notices))
}

/// Rewrites recognised text on a scanned page. The words of the scan are
/// pixels, so the old line is covered in the colour of the paper and the new
/// text is drawn over it, visibly, at the size the line was recognised at.
fn rewrite_scanned(doc: &mut Document, page_id: ObjectId, scan: &crate::interp::Scan, ids: &[usize], edit: &LineEdit) -> Result<Vec<String>, String> {
    let text = edit.text.clone().unwrap_or_default();
    let cover = edit.cover.unwrap_or([1.0, 1.0, 1.0]);
    let ink = edit.ink.unwrap_or([0.0, 0.0, 0.0]);
    let lines: Vec<([[f32; 2]; 4], [f32; 2], f32, f32)> = ids.iter().filter_map(|id| scan.lines.get(*id)).map(|l| (l.quad, l.origin, l.angle, l.em)).collect();
    // The invisible words go: the new text replaces them for searching and copying too.
    edit_lines(doc, page_id, ids, &LineEdit { delete: true, ..Default::default() })?;
    let mut notices = Vec::new();
    for (quad, origin, angle, em) in lines {
        // A little more than the line's box, so no edge of the old letters shows.
        let centre = [(quad[0][0] + quad[2][0]) / 2.0, (quad[0][1] + quad[2][1]) / 2.0];
        let grown: Vec<[f32; 2]> = quad
            .iter()
            .map(|p| {
                let (dx, dy) = (p[0] - centre[0], p[1] - centre[1]);
                let d = (dx * dx + dy * dy).sqrt().max(1e-3);
                [p[0] + dx / d * 0.12 * em, p[1] + dy / d * 0.12 * em]
            })
            .collect();
        let mut ops = vec![Operation::new("q", vec![]), Operation::new("rg", cover.iter().map(|v| real(*v)).collect())];
        for (i, p) in grown.iter().enumerate() {
            ops.push(Operation::new(if i == 0 { "m" } else { "l" }, vec![real(p[0]), real(p[1])]));
        }
        ops.push(Operation::new("h", vec![]));
        ops.push(Operation::new("f", vec![]));
        ops.push(Operation::new("Q", vec![]));
        add_page_ops(doc, page_id, ops, false)?;
        if text.trim().is_empty() {
            continue;
        }
        let plain = text.chars().all(|c| crate::encoding::win_ansi_byte(c).is_some());
        let font = if plain { FontChoice::Standard { name: "Helvetica".into() } } else { FontChoice::System { family: "Arial".into(), bold: false, italic: false } };
        notices.extend(place_text(doc, page_id, &text, &font, em, ink, (origin[0], origin[1]), angle, 0.0, 1.0, false)?);
    }
    notices.insert(0, "This is a scan: the old line was covered in the colour of the paper and the new text drawn over it.".to_string());
    notices.dedup();
    Ok(notices)
}
