//! Installed fonts, and embedding them.
//!
//! Embedded fonts are usually subsets: only the glyphs the original text
//! needed. When an edit needs a character the subset lacks, Inq Studio looks
//! for the same font installed on this computer (by PostScript name, then
//! family and style), cuts it down to the glyphs the edit needs, and embeds
//! that as a new composite font. The edit then still reads in the document's
//! typeface instead of falling back to something generic.

use crate::cmap;
use crate::fonts::{norm, split_name};
use crate::util::*;
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFont {
    /// The name Windows lists it under, e.g. "Calibri Bold".
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
}

/// A parsed installed font, ready to embed.
pub struct LoadedFont {
    pub data: Arc<Vec<u8>>,
    pub index: u32,
    pub ps_name: String,
    pub family: String,
    /// "file|face index": identifies the face across edits, so one document
    /// keeps one growing subset per installed face.
    pub source: String,
}

impl LoadedFont {
    /// Loads a font file directly (fonts that are not installed, and tests).
    #[cfg(test)]
    pub fn from_file(path: &std::path::Path, index: u32) -> Option<LoadedFont> {
        let data = Arc::new(std::fs::read(path).ok()?);
        let face = ttf_parser::Face::parse(&data, index).ok()?;
        let ps = face_name(&face, ttf_parser::name_id::POST_SCRIPT_NAME).unwrap_or_default();
        let family = face_name(&face, ttf_parser::name_id::FAMILY).unwrap_or_default();
        drop(face);
        Some(LoadedFont { data, index, ps_name: ps, family, source: format!("{}|{index}", path.display()) })
    }
}

static INDEX: OnceLock<Vec<InstalledFont>> = OnceLock::new();
static FILES: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>> = OnceLock::new();

fn strip_kind(name: &str) -> String {
    let mut n = name.trim().to_string();
    for suffix in [" (TrueType)", " (OpenType)", " (All res)"] {
        if let Some(i) = n.find(suffix) {
            n.truncate(i);
        }
    }
    n
}

#[cfg(windows)]
fn registry() -> Vec<InstalledFont> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let fonts_dir = PathBuf::from(windir).join("Fonts");
    let mut out = Vec::new();
    for (hive, base) in [(HKEY_LOCAL_MACHINE, Some(fonts_dir.clone())), (HKEY_CURRENT_USER, None)] {
        let Ok(key) = RegKey::predef(hive).open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts") else {
            continue;
        };
        for (name, value) in key.enum_values().flatten() {
            let file = value.to_string().trim_matches('"').to_string();
            let lower = file.to_lowercase();
            if !(lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".ttc")) {
                continue;
            }
            let path = PathBuf::from(&file);
            let path = if path.is_absolute() {
                path
            } else {
                base.clone().unwrap_or_else(|| fonts_dir.clone()).join(path)
            };
            out.push(InstalledFont { name: strip_kind(&name), path });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

#[cfg(not(windows))]
fn registry() -> Vec<InstalledFont> {
    Vec::new()
}

pub fn installed() -> &'static [InstalledFont] {
    INDEX.get_or_init(registry)
}

const STYLE_WORDS: [&str; 16] = [
    "Bold", "Italic", "Oblique", "Light", "Semibold", "SemiBold", "Black", "Medium", "Thin", "Heavy", "Regular",
    "Condensed", "Narrow", "Extra", "Semilight", "Demibold",
];

/// Family names of installed fonts, for the font picker.
pub fn families() -> Vec<String> {
    let mut fams: Vec<String> = installed()
        .iter()
        .map(|f| {
            let first = f.name.split(" & ").next().unwrap_or(&f.name);
            let mut words: Vec<&str> = first.split_whitespace().collect();
            while words.len() > 1 && STYLE_WORDS.contains(words.last().unwrap()) {
                words.pop();
            }
            words.join(" ")
        })
        .collect();
    fams.sort_by_key(|a| a.to_lowercase());
    fams.dedup();
    fams
}

fn read_file(path: &PathBuf) -> Option<Arc<Vec<u8>>> {
    let cache = FILES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(d) = cache.lock().ok()?.get(path) {
        return Some(d.clone());
    }
    let data = Arc::new(std::fs::read(path).ok()?);
    let mut c = cache.lock().ok()?;
    if c.len() > 24 {
        c.clear();
    }
    c.insert(path.clone(), data.clone());
    Some(data)
}

fn face_name(face: &ttf_parser::Face, id: u16) -> Option<String> {
    face.names().into_iter().filter(|n| n.name_id == id).find_map(|n| n.to_string())
}

/// Finds an installed font for a PDF font name (subset tag already removed).
pub fn find(ps_name: &str, bold: bool, italic: bool) -> Option<LoadedFont> {
    let (family, _) = split_name(ps_name);
    find_family(&family, Some(ps_name), bold, italic)
}

/// Finds an installed font by family and style, preferring an exact PostScript name.
pub fn find_family(family: &str, ps_name: Option<&str>, bold: bool, italic: bool) -> Option<LoadedFont> {
    let fam = norm(family);
    if fam.len() < 2 {
        return None;
    }
    let want_ps = ps_name.map(norm);
    let mut best: Option<(i32, LoadedFont)> = None;
    let candidates = installed().iter().filter(|f| f.name.split(" & ").any(|n| norm(n).starts_with(&fam))).take(24);
    for cand in candidates {
        let Some(data) = read_file(&cand.path) else { continue };
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        for index in 0..count {
            let Ok(face) = ttf_parser::Face::parse(&data, index) else { continue };
            if face.tables().glyf.is_none() && face.tables().cff.is_none() {
                continue;
            }
            let ps = face_name(&face, ttf_parser::name_id::POST_SCRIPT_NAME).unwrap_or_default();
            let fam_name = face_name(&face, ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
                .or_else(|| face_name(&face, ttf_parser::name_id::FAMILY))
                .unwrap_or_default();
            let mut score = 0;
            if want_ps.as_deref() == Some(norm(&ps).as_str()) {
                score += 1000;
            }
            if norm(&fam_name) == fam {
                score += 100;
            } else if norm(&fam_name).starts_with(&fam) {
                score += 20;
            } else {
                continue;
            }
            let f_bold = face.is_bold() || face.weight().to_number() >= 600;
            let f_italic = face.is_italic() || face.is_oblique();
            if f_bold == bold {
                score += 30;
            }
            if f_italic == italic {
                score += 30;
            }
            if face.weight().to_number() == 400 && !bold {
                score += 5;
            }
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((
                    score,
                    LoadedFont {
                        data: data.clone(),
                        index,
                        ps_name: ps,
                        family: fam_name,
                        source: format!("{}|{index}", cand.path.display()),
                    },
                ));
            }
        }
    }
    best.map(|(_, f)| f)
}

/// The result of embedding: the font object and how to encode with it.
pub struct EmbeddedFont {
    pub font_id: ObjectId,
    /// Character to code (two bytes, Identity-H).
    pub codes: BTreeMap<char, u32>,
}

fn subset_tag(seed: &str) -> String {
    let mut h: u32 = 2166136261;
    for b in seed.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    (0..6).map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char).collect()
}

/// Fonts this app embedded earlier from the same installed face.
fn inq_fonts(doc: &Document, source: &str) -> Vec<ObjectId> {
    doc.objects
        .iter()
        .filter_map(|(id, o)| {
            let d = o.as_dict().ok()?;
            let s = d.get(b"InqSource").ok()?.as_str().ok()?;
            (s == source.as_bytes()).then_some(*id)
        })
        .collect()
}

/// Code → text of an embedded font's ToUnicode map.
fn font_map(doc: &Document, font_id: ObjectId) -> BTreeMap<u32, String> {
    doc.get_dictionary(font_id)
        .ok()
        .and_then(|d| d.get(b"ToUnicode").ok())
        .and_then(|o| stream_bytes(doc, o))
        .map(|b| cmap::CMap::parse(&b).unicode)
        .unwrap_or_default()
}

fn compressed(dict: lopdf::Dictionary, data: Vec<u8>) -> Stream {
    let mut s = Stream::new(dict, data);
    let _ = s.compress();
    s
}

/// Makes `text` drawable with an installed font, as a composite (Type 0,
/// Identity-H) font embedded as a subset.
///
/// TrueType faces get one embedded font per document that grows as edits
/// need more glyphs: codes are the face's own glyph ids, mapped to the subset
/// through a `/CIDToGIDMap`, so earlier text keeps its codes when the subset
/// is rebuilt. CFF (OpenType) faces cannot remap CIDs that way, so an
/// existing subset is reused when it already has every glyph, and a new one
/// is made otherwise.
pub fn embed(doc: &mut Document, font: &LoadedFont, text: &str) -> Result<EmbeddedFont, String> {
    let face = ttf_parser::Face::parse(&font.data, font.index).map_err(err)?;
    let upem = face.units_per_em().max(1) as f32;
    let scale = 1000.0 / upem;
    let is_cff = face.tables().cff.is_some();

    let mut missing = Vec::new();
    let mut wanted: BTreeMap<char, u16> = BTreeMap::new();
    for c in text.chars() {
        if c == '\n' || c == '\r' || wanted.contains_key(&c) {
            continue;
        }
        let c2 = if c == '\t' || c == '\u{A0}' { ' ' } else { c };
        match face.glyph_index(c2) {
            Some(g) => {
                wanted.insert(c, g.0);
            }
            None => missing.push(c),
        }
    }
    if !missing.is_empty() {
        let list: String = missing.iter().map(|c| format!("‘{c}’ ")).collect();
        return Err(format!("{} has no glyphs for {}", font.family, list.trim_end()));
    }
    let advance = |gid: u16| face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0) as f32 * scale;

    // Reuse (and for TrueType, grow) what this document already has from this face.
    for id in inq_fonts(doc, &font.source) {
        let map = font_map(doc, id);
        let by_text: HashMap<String, u32> = map.iter().map(|(c, t)| (t.clone(), *c)).collect();
        if is_cff {
            if wanted.keys().all(|c| by_text.contains_key(&c.to_string())) {
                let codes: BTreeMap<char, u32> = wanted.keys().map(|c| (*c, by_text[&c.to_string()])).collect();
                return Ok(EmbeddedFont { font_id: id, codes });
            }
            continue;
        }
        let mut all: BTreeMap<u32, String> = map.clone();
        let mut grew = false;
        for (c, g) in &wanted {
            if !all.contains_key(&(*g as u32)) {
                all.insert(*g as u32, c.to_string());
                grew = true;
            }
        }
        if grew {
            write_truetype_subset(doc, Some(id), font, &face, &all)?;
        }
        let codes = wanted.iter().map(|(c, g)| (*c, *g as u32)).collect();
        return Ok(EmbeddedFont { font_id: id, codes });
    }

    if !is_cff {
        let all: BTreeMap<u32, String> = wanted.iter().map(|(c, g)| (*g as u32, c.to_string())).collect();
        let id = write_truetype_subset(doc, None, font, &face, &all)?;
        let codes = wanted.iter().map(|(c, g)| (*c, *g as u32)).collect();
        return Ok(EmbeddedFont { font_id: id, codes });
    }

    // CFF: the subset's glyph ids are the CIDs.
    let mut remapper = subsetter::GlyphRemapper::new();
    remapper.remap(0);
    for g in wanted.values() {
        remapper.remap(*g);
    }
    let data = subsetter::subset(&font.data, font.index, &remapper).map_err(|e| format!("Subsetting failed: {e:?}"))?;
    let mut codes = BTreeMap::new();
    let mut widths = HashMap::new();
    let mut to_unicode = BTreeMap::new();
    for (c, old) in &wanted {
        let new = remapper.get(*old).unwrap_or(0) as u32;
        codes.insert(*c, new);
        widths.insert(new, advance(*old));
        to_unicode.entry(new).or_insert_with(|| c.to_string());
    }
    let seed: String = to_unicode.values().cloned().collect();
    let base_font = format!("{}+{}", subset_tag(&seed), font.ps_name.replace(' ', ""));
    let file_id = doc.add_object(compressed(dictionary! { "Subtype" => "OpenType" }, data));
    let descriptor_id = doc.add_object(descriptor(&face, &base_font, "FontFile3", file_id));
    let w = w_array(&widths);
    let cid_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType0",
        "BaseFont" => Object::Name(base_font.clone().into_bytes()),
        "CIDSystemInfo" => cid_system_info(),
        "FontDescriptor" => descriptor_id,
        "DW" => real(advance(0)),
        "W" => w,
    });
    let tu_id = doc.add_object(compressed(lopdf::Dictionary::new(), cmap::write_to_unicode(&to_unicode, 2)));
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => Object::Name(base_font.into_bytes()),
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(cid_id)],
        "ToUnicode" => tu_id,
        "InqSource" => Object::string_literal(font.source.clone()),
    });
    Ok(EmbeddedFont { font_id, codes })
}

fn cid_system_info() -> lopdf::Dictionary {
    dictionary! {
        "Registry" => Object::string_literal("Adobe"),
        "Ordering" => Object::string_literal("Identity"),
        "Supplement" => 0,
    }
}

fn w_array(widths: &HashMap<u32, f32>) -> Vec<Object> {
    let mut keys: Vec<&u32> = widths.keys().collect();
    keys.sort();
    let mut w = Vec::new();
    for k in keys {
        w.push(Object::Integer(*k as i64));
        w.push(Object::Array(vec![real(widths[k])]));
    }
    w
}

fn descriptor(face: &ttf_parser::Face, base_font: &str, file_key: &str, file_id: ObjectId) -> lopdf::Dictionary {
    let scale = 1000.0 / face.units_per_em().max(1) as f32;
    let bbox = face.global_bounding_box();
    let mut flags = 4i64; // symbolic: glyphs are addressed by id, not a standard encoding
    if face.is_monospaced() {
        flags |= 1;
    }
    if face.is_italic() {
        flags |= 64;
    }
    let mut d = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => Object::Name(base_font.as_bytes().to_vec()),
        "Flags" => flags,
        "FontBBox" => vec![
            real(bbox.x_min as f32 * scale), real(bbox.y_min as f32 * scale),
            real(bbox.x_max as f32 * scale), real(bbox.y_max as f32 * scale),
        ],
        "ItalicAngle" => real(face.italic_angle()),
        "Ascent" => real(face.ascender() as f32 * scale),
        "Descent" => real(face.descender() as f32 * scale),
        "CapHeight" => real(face.capital_height().unwrap_or(face.ascender()) as f32 * scale),
        "StemV" => 80,
        "FontWeight" => face.weight().to_number() as i64,
    };
    d.set(file_key, Object::Reference(file_id));
    d
}

/// Writes (or rewrites in place) a TrueType subset holding the glyphs in
/// `glyphs` (face glyph id → text). Codes stay the face's glyph ids.
fn write_truetype_subset(
    doc: &mut Document,
    existing: Option<ObjectId>,
    font: &LoadedFont,
    face: &ttf_parser::Face,
    glyphs: &BTreeMap<u32, String>,
) -> Result<ObjectId, String> {
    let scale = 1000.0 / face.units_per_em().max(1) as f32;
    let mut remapper = subsetter::GlyphRemapper::new();
    remapper.remap(0);
    for g in glyphs.keys() {
        remapper.remap(*g as u16);
    }
    let data = subsetter::subset(&font.data, font.index, &remapper).map_err(|e| format!("Subsetting failed: {e:?}"))?;
    let max = glyphs.keys().max().copied().unwrap_or(0) as usize;
    let mut map = vec![0u8; (max + 1) * 2];
    for g in glyphs.keys() {
        let new = remapper.get(*g as u16).unwrap_or(0);
        map[*g as usize * 2] = (new >> 8) as u8;
        map[*g as usize * 2 + 1] = new as u8;
    }
    let widths: HashMap<u32, f32> = glyphs
        .keys()
        .map(|g| (*g, face.glyph_hor_advance(ttf_parser::GlyphId(*g as u16)).unwrap_or(0) as f32 * scale))
        .collect();
    let len1 = data.len() as i64;
    let file = compressed(dictionary! { "Length1" => len1 }, data);
    let cid_map = compressed(lopdf::Dictionary::new(), map);
    let tu = compressed(lopdf::Dictionary::new(), cmap::write_to_unicode(glyphs, 2));

    if let Some(font_id) = existing {
        // Swap the program, glyph map, widths and ToUnicode inside the same objects.
        let f = doc.get_dictionary(font_id).map_err(err)?;
        let tu_id = f.get(b"ToUnicode").and_then(Object::as_reference).map_err(err)?;
        let cid_id = f
            .get(b"DescendantFonts")
            .and_then(Object::as_array)
            .ok()
            .and_then(|a| a.first())
            .and_then(|o| o.as_reference().ok())
            .ok_or("Embedded font has no descendant")?;
        let cd = doc.get_dictionary(cid_id).map_err(err)?;
        let map_id = cd.get(b"CIDToGIDMap").and_then(Object::as_reference).map_err(err)?;
        let fd_id = cd.get(b"FontDescriptor").and_then(Object::as_reference).map_err(err)?;
        let file_id = doc.get_dictionary(fd_id).map_err(err)?.get(b"FontFile2").and_then(Object::as_reference).map_err(err)?;
        doc.objects.insert(file_id, Object::Stream(file));
        doc.objects.insert(map_id, Object::Stream(cid_map));
        doc.objects.insert(tu_id, Object::Stream(tu));
        doc.get_dictionary_mut(cid_id).map_err(err)?.set("W", w_array(&widths));
        return Ok(font_id);
    }

    let base_font = format!("{}+{}", subset_tag(&font.source), font.ps_name.replace(' ', ""));
    let file_id = doc.add_object(file);
    let descriptor_id = doc.add_object(descriptor(face, &base_font, "FontFile2", file_id));
    let map_id = doc.add_object(cid_map);
    let cid_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => Object::Name(base_font.clone().into_bytes()),
        "CIDSystemInfo" => cid_system_info(),
        "FontDescriptor" => descriptor_id,
        "DW" => real(face.glyph_hor_advance(ttf_parser::GlyphId(0)).unwrap_or(0) as f32 * scale),
        "W" => w_array(&widths),
        "CIDToGIDMap" => map_id,
    });
    let tu_id = doc.add_object(tu);
    Ok(doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => Object::Name(base_font.into_bytes()),
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(cid_id)],
        "ToUnicode" => tu_id,
        "InqSource" => Object::string_literal(font.source.clone()),
    }))
}

/// One of the 14 standard fonts every PDF reader has, with WinAnsi encoding.
pub fn standard_font(doc: &mut Document, base: &str) -> ObjectId {
    doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => Object::Name(base.as_bytes().to_vec()),
        "Encoding" => "WinAnsiEncoding",
    })
}

pub const STANDARD_FONTS: [&str; 12] = [
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
];

/// The standard font closest to a font's style.
pub fn standard_for(serif: bool, fixed: bool, bold: bool, italic: bool) -> &'static str {
    let i = match (bold, italic) {
        (false, false) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (true, true) => 3,
    };
    let base = if fixed { 8 } else if serif { 4 } else { 0 };
    STANDARD_FONTS[base + i]
}
