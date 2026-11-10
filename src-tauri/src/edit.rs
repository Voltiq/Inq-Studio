//! Text editing.
//!
//! Edits rewrite the page's own content stream rather than painting over it:
//! a changed line is re-encoded in its original font and put back into the
//! operator that drew it, so the result is real, selectable, searchable text,
//! indistinguishable from what the original producer wrote.
//!
//! When the embedded font cannot draw a character, the same font is taken
//! from this computer and embedded as a subset; only when it is not installed
//! does the edit fall back to the nearest standard font, and it says so.

use crate::encoding::BaseEncoding;
use crate::fonts::{apply_additions, Piece, PdfFont};
use crate::interp::{page_resources, run_at, scan_page};
use crate::sysfonts;
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
    pub dx: f32,
    pub dy: f32,
    pub delete: bool,
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

/// A stretch of encoded text in one font, or a line break (when adding text).
#[derive(Debug)]
enum Seg {
    Text { res: Vec<u8>, code_len: u8, pieces: Vec<Piece> },
    Break,
}

/// Encoded text ready to go into show operators.
struct Prepared {
    segs: Vec<Seg>,
    notice: Option<String>,
}

/// Writes a page's operators back as one fresh content stream.
pub fn set_page_ops(doc: &mut Document, page_id: ObjectId, ops: Vec<Operation>) -> Result<(), String> {
    let bytes = Content { operations: ops }.encode().map_err(err)?;
    let mut stream = Stream::new(Dictionary::new(), bytes);
    let _ = stream.compress();
    let id = doc.add_object(stream);
    doc.get_dictionary_mut(page_id).map_err(err)?.set("Contents", Object::Reference(id));
    Ok(())
}

/// Makes a font object available on a page and returns its resource name.
pub fn add_page_font(doc: &mut Document, page_id: ObjectId, font_id: ObjectId) -> Result<Vec<u8>, String> {
    // The page gets its own Resources entry if it only inherits one.
    let page = doc.get_dictionary(page_id).map_err(err)?;
    let res_ref = match page.get(b"Resources") {
        Ok(Object::Reference(id)) => Some(*id),
        Ok(_) => None,
        Err(_) => {
            let inherited = page_resources(doc, page_id);
            doc.get_dictionary_mut(page_id).map_err(err)?.set("Resources", Object::Dictionary(inherited));
            None
        }
    };
    // Where the /Font dictionary lives: its own object, or inside Resources.
    let res_dict: &Dictionary = match res_ref {
        Some(id) => doc.get_dictionary(id).map_err(err)?,
        None => doc.get_dictionary(page_id).map_err(err)?.get(b"Resources").and_then(Object::as_dict).map_err(err)?,
    };
    let font_ref = match res_dict.get(b"Font") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    };
    let existing: Dictionary = match font_ref {
        Some(id) => doc.get_dictionary(id).cloned().unwrap_or_default(),
        None => res_dict.get(b"Font").and_then(Object::as_dict).cloned().unwrap_or_default(),
    };
    for (name, obj) in existing.iter() {
        if obj.as_reference().ok() == Some(font_id) {
            return Ok(name.clone());
        }
    }
    let mut n = existing.len() + 1;
    let name = loop {
        let candidate = format!("InqF{n}").into_bytes();
        if !existing.has(&candidate) {
            break candidate;
        }
        n += 1;
    };
    match font_ref {
        Some(id) => {
            doc.get_dictionary_mut(id).map_err(err)?.set(name.clone(), Object::Reference(font_id));
        }
        None => {
            let mut fonts = existing;
            fonts.set(name.clone(), Object::Reference(font_id));
            let res = match res_ref {
                Some(id) => doc.get_dictionary_mut(id).map_err(err)?,
                None => doc
                    .get_dictionary_mut(page_id)
                    .map_err(err)?
                    .get_mut(b"Resources")
                    .and_then(Object::as_dict_mut)
                    .map_err(err)?,
            };
            res.set("Font", Object::Dictionary(fonts));
        }
    }
    Ok(name)
}

fn list_chars(chars: &[char]) -> String {
    let shown: Vec<String> = chars.iter().take(8).map(|c| format!("‘{c}’")).collect();
    let more = if chars.len() > 8 { format!(" and {} more", chars.len() - 8) } else { String::new() };
    format!("{}{more}", shown.join(" "))
}

fn win_ansi_pieces(text: &str) -> Result<Vec<Piece>, Vec<char>> {
    let table = BaseEncoding::WinAnsi.table();
    let mut rev: HashMap<char, u32> = HashMap::new();
    for (code, c) in table.iter().enumerate() {
        if let Some(c) = c {
            rev.entry(*c).or_insert(code as u32);
        }
    }
    let mut missing = Vec::new();
    let pieces = text
        .chars()
        .filter_map(|c| {
            let c = if c == '\u{A0}' || c == '\t' { ' ' } else { c };
            match rev.get(&c) {
                Some(code) => Some(Piece::Code(*code)),
                None => {
                    missing.push(c);
                    None
                }
            }
        })
        .collect();
    if missing.is_empty() { Ok(pieces) } else { Err(missing) }
}

/// A font standing in for characters the document's font cannot draw.
struct Substitute {
    res: Vec<u8>,
    code_len: u8,
    codes: HashMap<char, u32>,
    name: String,
}

/// Finds a font for `chars`: a broad installed font of the same kind
/// (Arial, Times New Roman, Courier New), else a standard font.
fn substitute(
    doc: &mut Document,
    page_id: ObjectId,
    chars: &[char],
    serif: bool,
    fixed: bool,
    bold: bool,
    italic: bool,
) -> Result<Substitute, String> {
    let wanted: String = chars.iter().collect();
    let family = if fixed { "Courier New" } else if serif { "Times New Roman" } else { "Arial" };
    if let Some(loaded) = sysfonts::find_family(family, None, bold, italic) {
        if let Ok(emb) = sysfonts::embed(doc, &loaded, &wanted) {
            let res = add_page_font(doc, page_id, emb.font_id)?;
            let codes = emb.codes.iter().map(|(c, g)| (*c, *g as u32)).collect();
            return Ok(Substitute { res, code_len: 2, codes, name: loaded.ps_name });
        }
    }
    let base = sysfonts::standard_for(serif, fixed, bold, italic);
    match win_ansi_pieces(&wanted) {
        Ok(pieces) => {
            let id = sysfonts::standard_font(doc, base);
            let res = add_page_font(doc, page_id, id)?;
            let codes = chars
                .iter()
                .zip(pieces)
                .filter_map(|(c, p)| match p {
                    Piece::Code(code) => Some((*c, code)),
                    Piece::Gap(_) => None,
                })
                .collect();
            Ok(Substitute { res, code_len: 1, codes, name: base.to_string() })
        }
        Err(missing) => Err(format!(
            "No font on this computer can draw {}. Install a font that has these characters.",
            list_chars(&missing)
        )),
    }
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

/// Encodes `text` for a page, in this order of preference:
/// 1. the document's own font (adding glyphs from its embedded program if needed);
/// 2. the same typeface installed on this computer, embedded as a subset;
/// 3. the document's font for every character it has, and a substitute only
///    for the characters it lacks.
fn prepare(
    doc: &mut Document,
    page_id: ObjectId,
    font: Option<(&PdfFont, Option<Vec<u8>>)>,
    style: (&str, &str, bool, bool, bool, bool),
    text: &str,
) -> Result<Prepared, String> {
    let (ps_name, family, bold, italic, serif, fixed) = style;
    let text = text.replace('\r', "");
    let lines: Vec<&str> = text.split('\n').collect();
    let flat: String = lines.concat();

    // 1. The document's font as it is.
    let mut doc_font: Option<(&PdfFont, Vec<u8>)> = None;
    let mut missing: Vec<char> = Vec::new();
    if let Some((font, res)) = font {
        let enc = font.encode(doc, &flat);
        missing = enc.missing;
        let res = match res {
            Some(r) => r,
            None => add_page_font(doc, page_id, font.obj.ok_or("This font cannot be shared with other pages")?)?,
        };
        if missing.is_empty() {
            apply_additions(doc, font, &enc.additions)?;
            let mut segs = Vec::new();
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    segs.push(Seg::Break);
                }
                if !l.is_empty() {
                    segs.push(Seg::Text { res: res.clone(), code_len: font.code_len, pieces: font.encode(doc, l).pieces });
                }
            }
            let notice = (!enc.additions.is_empty()).then(|| {
                let n = enc.additions.len();
                format!("Added {n} glyph{} to {} from its embedded font program.", if n == 1 { "" } else { "s" }, font.ps_name)
            });
            return Ok(Prepared { segs, notice });
        }
        doc_font = Some((font, res));
    }

    // 2. The same typeface, installed on this computer.
    let loaded = if ps_name.is_empty() {
        sysfonts::find_family(family, None, bold, italic)
    } else {
        sysfonts::find(ps_name, bold, italic)
    };
    if let Some(loaded) = loaded {
        if let Ok(emb) = sysfonts::embed(doc, &loaded, &flat) {
            let res = add_page_font(doc, page_id, emb.font_id)?;
            let mut segs = Vec::new();
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    segs.push(Seg::Break);
                }
                if !l.is_empty() {
                    let pieces = l.chars().map(|c| Piece::Code(*emb.codes.get(&c).unwrap_or(&0) as u32)).collect();
                    segs.push(Seg::Text { res: res.clone(), code_len: 2, pieces });
                }
            }
            let notice = (!missing.is_empty()).then(|| {
                format!(
                    "The embedded subset of {ps_name} has no {}, so this text was set with {} from your system (embedded as a subset).",
                    list_chars(&missing),
                    loaded.ps_name
                )
            });
            return Ok(Prepared { segs, notice });
        }
    }

    // 3. Keep the document's font where it can draw, substitute the rest.
    let needed = match &doc_font {
        Some(_) => dedup(missing.iter().copied()),
        None => dedup(flat.chars()),
    };
    let sub = substitute(doc, page_id, &needed, serif, fixed, bold, italic)?;
    let mut segs: Vec<Seg> = Vec::new();
    let mut additions = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            segs.push(Seg::Break);
        }
        // Runs of characters that go to the same font.
        let chars: Vec<char> = l.chars().collect();
        let mut k = 0;
        while k < chars.len() {
            let use_sub = doc_font.is_none() || needed.contains(&chars[k]);
            let mut j = k;
            while j < chars.len() && (doc_font.is_none() || needed.contains(&chars[j])) == use_sub {
                j += 1;
            }
            let part: String = chars[k..j].iter().collect();
            if use_sub {
                let pieces = part.chars().filter_map(|c| sub.codes.get(&c).map(|code| Piece::Code(*code))).collect();
                segs.push(Seg::Text { res: sub.res.clone(), code_len: sub.code_len, pieces });
            } else if let Some((font, res)) = &doc_font {
                let enc = font.encode(doc, &part);
                additions.extend(enc.additions);
                segs.push(Seg::Text { res: res.clone(), code_len: font.code_len, pieces: enc.pieces });
            }
            k = j;
        }
    }
    let notice = match &doc_font {
        Some((font, _)) => {
            apply_additions(doc, font, &additions)?;
            format!(
                "{} is not installed and its embedded subset has no {}. Those characters were set in {}; the rest stays in {}.",
                if family.is_empty() { ps_name } else { family },
                list_chars(&missing),
                sub.name,
                font.ps_name
            )
        }
        None => format!("{} is not installed; the text was set in {} instead.", family, sub.name),
    };
    Ok(Prepared { segs, notice: Some(notice) })
}

/// Show operators for prepared text, switching fonts between segments.
/// Returns the operators and the font resource left selected at the end.
fn show_ops(segs: &[Seg], start_res: &[u8], size: f32, force_tf: bool, leading: f32) -> (Vec<Operation>, Vec<u8>) {
    let mut out = Vec::new();
    let mut current = start_res.to_vec();
    let mut first = true;
    for seg in segs {
        match seg {
            Seg::Break => out.push(Operation::new("Td", vec![Object::Integer(0), real(-leading)])),
            Seg::Text { res, code_len, pieces } => {
                if *res != current || (first && force_tf) {
                    out.push(Operation::new("Tf", vec![Object::Name(res.clone()), real(size)]));
                    current = res.clone();
                }
                first = false;
                if pieces.is_empty() {
                    continue;
                }
                let (operand, is_array) = show_operand(pieces, *code_len);
                out.push(Operation::new(if is_array { "TJ" } else { "Tj" }, vec![operand]));
            }
        }
    }
    if out.iter().all(|o| o.operator != "Tj" && o.operator != "TJ") {
        // Keep one show operator so the line still exists for positioning.
        out.push(Operation::new("Tj", vec![Object::String(vec![], StringFormat::Literal)]));
    }
    (out, current)
}

/// The operand of a show operator: one string, or an array with gaps.
fn show_operand(pieces: &[Piece], code_len: u8) -> (Object, bool) {
    let format = if code_len == 2 { StringFormat::Hexadecimal } else { StringFormat::Literal };
    let mut items: Vec<Object> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut has_gap = false;
    for p in pieces {
        match p {
            Piece::Code(c) => {
                if code_len == 2 {
                    current.push((c >> 8) as u8);
                }
                current.push(*c as u8);
            }
            Piece::Gap(g) => {
                has_gap = true;
                if !current.is_empty() {
                    items.push(Object::String(std::mem::take(&mut current), format));
                }
                items.push(real(-g.round()));
            }
        }
    }
    if !has_gap {
        return (Object::String(current, format), false);
    }
    if !current.is_empty() {
        items.push(Object::String(current, format));
    }
    (Object::Array(items), true)
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
        other => Operation::new(other, vec![Object::String(vec![], StringFormat::Literal)]),
    }
}

/// Edits one text line on a page. Returns a notice when the edit had to
/// change fonts, or did anything else worth telling.
pub fn edit_line(doc: &mut Document, page_id: ObjectId, line_id: usize, edit: &LineEdit) -> Result<Option<String>, String> {
    let scan = scan_page(doc, page_id)?;
    let line = scan.lines.get(line_id).ok_or("This line no longer exists. Refresh and try again.")?.clone();
    if !line.editable {
        return Err(line.note.unwrap_or_else(|| "This text cannot be edited.".into()));
    }
    let first_op = line.ops[0];
    let run = run_at(&scan.runs, first_op).ok_or("The text could not be located.")?.clone();
    let mut ops = scan.ops.clone();

    if edit.delete {
        for &i in &line.ops {
            ops[i] = emptied(&ops[i]);
        }
        set_page_ops(doc, page_id, ops)?;
        return Ok(None);
    }

    let text = edit.text.clone().unwrap_or_else(|| line.text.clone()).replace(['\r', '\n'], " ");
    let font: Arc<PdfFont> = run.font.clone();
    let prepared = prepare(
        doc,
        page_id,
        Some((&font, Some(run.res_name.clone()))),
        (&font.ps_name, &font.family, font.bold, font.italic, font.serif, font.fixed),
        &text,
    )?;

    let src = &scan.ops[first_op];
    let mut out: Vec<Operation> = Vec::new();
    // Positioning the original operator did before showing.
    match src.operator.as_str() {
        "'" => out.push(Operation::new("T*", vec![])),
        "\"" => {
            out.push(Operation::new("Tw", vec![src.operands.first().cloned().unwrap_or(Object::Integer(0))]));
            out.push(Operation::new("Tc", vec![src.operands.get(1).cloned().unwrap_or(Object::Integer(0))]));
            out.push(Operation::new("T*", vec![]));
        }
        _ => {}
    }
    let size = edit.size.unwrap_or(run.size);
    let size_changed = (size - run.size).abs() > 1e-4;
    if let Some([r, g, b]) = edit.color {
        out.push(Operation::new("rg", vec![real(r), real(g), real(b)]));
    }
    let moved = edit.dx.abs() > 1e-3 || edit.dy.abs() > 1e-3;
    if moved {
        let lin = [run.ctm[0], run.ctm[1], run.ctm[2], run.ctm[3], 0.0, 0.0];
        let inv = invert(&lin).ok_or("The text is drawn with a degenerate transform.")?;
        let (ddx, ddy) = apply_vec(&inv, edit.dx, edit.dy);
        let mut m = run.tm;
        m[4] += ddx;
        m[5] += ddy;
        out.push(Operation::new("Tm", m.iter().map(|v| real(*v)).collect()));
    }
    let (shows, end_res) = show_ops(&prepared.segs, &run.res_name, size, size_changed, 0.0);
    out.extend(shows);
    // Put the state back for whatever the page draws next.
    if end_res != run.res_name || size_changed {
        out.push(Operation::new("Tf", vec![Object::Name(run.res_name.clone()), real(run.size)]));
    }
    if edit.color.is_some() {
        out.extend(run.fill.ops());
    }
    if moved {
        out.push(Operation::new("Tm", run.tlm.iter().map(|v| real(*v)).collect()));
    }

    for &i in &line.ops[1..] {
        ops[i] = emptied(&ops[i]);
    }
    let mut new_ops = Vec::with_capacity(ops.len() + out.len());
    for (i, op) in ops.into_iter().enumerate() {
        if i == first_op {
            new_ops.extend(out.drain(..));
        } else {
            new_ops.push(op);
        }
    }
    set_page_ops(doc, page_id, new_ops)?;
    Ok(prepared.notice)
}

pub fn page_rotation(doc: &Document, page_id: ObjectId) -> i64 {
    inherited(doc, page_id, b"Rotate").and_then(num).map(|r| (r as i64).rem_euclid(360)).unwrap_or(0)
}

fn find_font(doc: &Document, key: &str) -> Option<PdfFont> {
    let mut parts = key.split(' ');
    let n: u32 = parts.next()?.parse().ok()?;
    let g: u16 = parts.next()?.parse().ok()?;
    doc.get_object((n, g)).ok()?;
    Some(PdfFont::load(doc, &Object::Reference((n, g)), key.to_string()))
}

/// Adds new text at a point in user space, upright as the page is shown.
pub fn add_text(doc: &mut Document, page_id: ObjectId, spec: &NewText) -> Result<Option<String>, String> {
    if spec.text.trim().is_empty() {
        return Err("There is no text to add.".into());
    }
    let scan = scan_page(doc, page_id)?;
    let text = spec.text.replace("\r\n", "\n");

    let prepared = match &spec.font {
        FontChoice::Document { key } => {
            let font = find_font(doc, key).ok_or("That font is not in this document.")?;
            prepare(
                doc,
                page_id,
                Some((&font, None)),
                (&font.ps_name, &font.family, font.bold, font.italic, font.serif, font.fixed),
                &text,
            )?
        }
        FontChoice::System { family, bold, italic } => {
            let l = family.to_lowercase();
            let serif = l.contains("times") || (l.contains("serif") && !l.contains("sans"));
            let fixed = l.contains("mono") || l.contains("courier") || l.contains("consol");
            prepare(doc, page_id, None, ("", family, *bold, *italic, serif, fixed), &text)?
        }
        FontChoice::Standard { name } => {
            let id = sysfonts::standard_font(doc, name);
            let res = add_page_font(doc, page_id, id)?;
            let mut segs = Vec::new();
            for (i, l) in text.split('\n').enumerate() {
                if i > 0 {
                    segs.push(Seg::Break);
                }
                let pieces = win_ansi_pieces(l)
                    .map_err(|m| format!("{name} cannot draw {}. Pick an installed font instead.", list_chars(&m)))?;
                segs.push(Seg::Text { res: res.clone(), code_len: 1, pieces });
            }
            Prepared { segs, notice: None }
        }
    };

    let rot = page_rotation(doc, page_id) as f32;
    let (s, c) = rot.to_radians().sin_cos();
    let mut add = vec![
        Operation::new("q", vec![]),
        Operation::new("BT", vec![]),
        Operation::new("rg", vec![real(spec.color[0]), real(spec.color[1]), real(spec.color[2])]),
        Operation::new("Tm", vec![real(c), real(s), real(-s), real(c), real(spec.x), real(spec.y)]),
    ];
    let (shows, _) = show_ops(&prepared.segs, &[], spec.size, true, spec.size * spec.line_height);
    add.extend(shows);
    add.push(Operation::new("ET", vec![]));
    add.push(Operation::new("Q", vec![]));

    let mut ops = scan.ops;
    if !scan.clean_end {
        ops.insert(0, Operation::new("q", vec![]));
        ops.push(Operation::new("Q", vec![]));
    }
    ops.extend(add);
    set_page_ops(doc, page_id, ops)?;
    Ok(prepared.notice)
}
