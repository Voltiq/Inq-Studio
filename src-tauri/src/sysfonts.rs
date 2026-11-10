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
                best = Some((score, LoadedFont { data: data.clone(), index, ps_name: ps, family: fam_name }));
            }
        }
    }
    best.map(|(_, f)| f)
}

/// The result of embedding: the new font object and how to encode with it.
pub struct EmbeddedFont {
    pub font_id: ObjectId,
    pub codes: BTreeMap<char, u16>,
}

fn subset_tag(seed: &[u16]) -> String {
    let mut h: u32 = 2166136261;
    for g in seed {
        h = (h ^ *g as u32).wrapping_mul(16777619);
    }
    (0..6)
        .map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char)
        .collect()
}

/// Embeds the glyphs `text` needs from an installed font as a composite
/// (Type 0, Identity-H) font. Codes are the glyph ids of the subset.
pub fn embed(doc: &mut Document, font: &LoadedFont, text: &str) -> Result<EmbeddedFont, String> {
    let face = ttf_parser::Face::parse(&font.data, font.index).map_err(err)?;
    let upem = face.units_per_em().max(1) as f32;
    let scale = 1000.0 / upem;
    let mut missing = Vec::new();
    let mut remapper = subsetter::GlyphRemapper::new();
    remapper.remap(0);
    let mut chars: BTreeMap<char, u16> = BTreeMap::new();
    for c in text.chars() {
        if c == '\n' || c == '\r' || chars.contains_key(&c) {
            continue;
        }
        let c2 = if c == '\t' || c == '\u{A0}' { ' ' } else { c };
        match face.glyph_index(c2) {
            Some(g) => {
                chars.insert(c, g.0);
                remapper.remap(g.0);
            }
            None => missing.push(c),
        }
    }
    if !missing.is_empty() {
        let list: String = missing.iter().map(|c| format!("‘{c}’ ")).collect();
        return Err(format!("{} has no glyphs for {}", font.family, list.trim_end()));
    }
    let data = subsetter::subset(&font.data, font.index, &remapper).map_err(|e| format!("Subsetting failed: {e:?}"))?;
    let is_cff = face.tables().cff.is_some();

    let mut codes = BTreeMap::new();
    let mut widths = BTreeMap::new();
    let mut to_unicode = BTreeMap::new();
    for (c, old) in &chars {
        let new = remapper.get(*old).unwrap_or(0);
        codes.insert(*c, new);
        let adv = face.glyph_hor_advance(ttf_parser::GlyphId(*old)).unwrap_or(0) as f32 * scale;
        widths.insert(new, adv);
        to_unicode.insert(new as u32, c.to_string());
    }

    let gids: Vec<u16> = codes.values().copied().collect();
    let base_font = format!("{}+{}", subset_tag(&gids), font.ps_name.replace(' ', ""));

    let mut file = if is_cff {
        Stream::new(dictionary! { "Subtype" => "OpenType" }, data)
    } else {
        let l = data.len() as i64;
        Stream::new(dictionary! { "Length1" => l }, data)
    };
    let _ = file.compress();
    let file_id = doc.add_object(file);

    let bbox = face.global_bounding_box();
    let mut flags = 4i64; // symbolic: glyphs are addressed by id, not a standard encoding
    if face.is_monospaced() {
        flags |= 1;
    }
    if face.is_italic() {
        flags |= 64;
    }
    let descriptor = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => Object::Name(base_font.clone().into_bytes()),
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
        (if is_cff { "FontFile3" } else { "FontFile2" }) => file_id,
    };
    let descriptor_id = doc.add_object(descriptor);

    let mut w = Vec::new();
    for (gid, adv) in &widths {
        w.push(Object::Integer(*gid as i64));
        w.push(Object::Array(vec![real(*adv)]));
    }
    let mut cid_font = dictionary! {
        "Type" => "Font",
        "Subtype" => if is_cff { "CIDFontType0" } else { "CIDFontType2" },
        "BaseFont" => Object::Name(base_font.clone().into_bytes()),
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => descriptor_id,
        "DW" => real(face.glyph_hor_advance(ttf_parser::GlyphId(0)).unwrap_or(0) as f32 * scale),
        "W" => w,
    };
    if !is_cff {
        cid_font.set("CIDToGIDMap", "Identity");
    }
    let cid_id = doc.add_object(cid_font);

    let mut tu = Stream::new(lopdf::Dictionary::new(), cmap::write_to_unicode(&to_unicode, 2));
    let _ = tu.compress();
    let tu_id = doc.add_object(tu);

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => Object::Name(base_font.clone().into_bytes()),
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(cid_id)],
        "ToUnicode" => tu_id,
    });
    Ok(EmbeddedFont { font_id, codes })
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
