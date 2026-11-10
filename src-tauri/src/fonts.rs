//! The font model.
//!
//! Every font a page uses is read into a [`PdfFont`]: how its byte codes map
//! to text (ToUnicode, base encoding, `/Differences`), how wide each glyph is,
//! which characters the embedded program can actually draw, and whether new
//! glyphs can be added to it. That is what lets Inq Studio write new text in
//! the font the document already uses, byte for byte the way the original
//! producer would have.

use crate::cmap::{self, CMap};
use crate::encoding::{glyph_to_unicode, BaseEncoding};
use crate::util::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FontKind {
    Type1,
    TrueType,
    Type0,
    Type3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgramKind {
    /// FontFile: a Type 1 (PostScript) program.
    Type1,
    /// FontFile2: TrueType outlines.
    TrueType,
    /// FontFile3 /Type1C or /CIDFontType0C: bare CFF.
    Cff,
    /// FontFile3 /OpenType.
    OpenType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Std14 {
    Helvetica,
    HelveticaBold,
    Times,
    TimesBold,
    Courier,
    Symbol,
    Dingbats,
}

/// One piece of encoded text: a character code, or a gap standing in for a
/// space the font has no glyph for (in thousandths of an em).
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Code(u32),
    Gap(f32),
}

/// A glyph found in the embedded program that the font does not map yet.
#[derive(Clone, Debug)]
pub struct Addition {
    pub code: u32,
    pub text: String,
    /// Advance in glyph space.
    pub width: f32,
    /// For simple fonts: the glyph name to put into `/Differences`.
    pub glyph_name: Option<String>,
}

/// How a font can gain glyphs it does not map yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Growth {
    None,
    /// Composite font with glyph-id codes: new codes come from the program's cmap.
    Cid,
    /// Simple font with a CFF program: unused codes get new glyphs through `/Differences`.
    Simple,
}

pub struct Encoded {
    pub pieces: Vec<Piece>,
    /// Glyphs found in the embedded program that need mapping entries
    /// before their codes can be used.
    pub additions: Vec<Addition>,
    /// Characters the font cannot draw at all.
    pub missing: Vec<char>,
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub code: u32,
    pub text: String,
    /// Advance in glyph space (thousandths of an em for everything but Type 3).
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct PdfFont {
    /// Stable identity: the object id of the font dictionary.
    pub key: String,
    pub obj: Option<ObjectId>,
    /// BaseFont as written, subset tag included.
    pub raw_name: String,
    /// BaseFont without the subset tag.
    pub ps_name: String,
    pub family: String,
    pub style: String,
    pub bold: bool,
    pub italic: bool,
    pub serif: bool,
    pub fixed: bool,
    pub kind: FontKind,
    pub cid_type2: bool,
    pub code_len: u8,
    pub vertical: bool,
    pub encoding_name: String,
    pub to_unicode: BTreeMap<u32, String>,
    pub has_to_unicode: bool,
    /// Text to code, for codes whose glyph exists.
    pub reverse: HashMap<String, u32>,
    widths: HashMap<u32, f32>,
    pub default_width: f32,
    /// Glyph space to text space: 0.001, or FontMatrix[0] for Type 3.
    pub width_scale: f32,
    pub ascent: f32,
    pub descent: f32,
    pub italic_angle: f32,
    pub program: Option<(ProgramKind, ObjectId)>,
    pub subset: bool,
    pub std14: Option<Std14>,
    cid_map: Option<BTreeMap<u32, u32>>,
    pub identity: bool,
    pub cid_to_gid_identity: bool,
    pub descendant: Option<ObjectId>,
    pub to_unicode_obj: Option<ObjectId>,
    /// New glyphs can be pulled from the embedded program.
    pub extendable: bool,
    pub growth: Growth,
}

fn strip_subset(name: &str) -> (&str, bool) {
    let b = name.as_bytes();
    if b.len() > 7 && b[6] == b'+' && b[..6].iter().all(|c| c.is_ascii_uppercase()) {
        (&name[7..], true)
    } else {
        (name, false)
    }
}

/// "TimesNewRomanPS-BoldItalicMT" → ("Times New Roman", "Bold Italic").
pub fn split_name(ps: &str) -> (String, String) {
    let (fam, style) = match ps.find(['-', ',']) {
        Some(i) => (&ps[..i], &ps[i + 1..]),
        None => (ps, ""),
    };
    let mut fam = fam.to_string();
    for suffix in ["PSMT", "MT", "PS"] {
        if fam.len() > suffix.len() + 2 && fam.ends_with(suffix) {
            fam.truncate(fam.len() - suffix.len());
            break;
        }
    }
    let style = style.trim_end_matches("MT").trim_end_matches("PS");
    (spaced(&fam), spaced(style))
}

/// Inserts spaces at lower→upper case changes: "TimesNewRoman" → "Times New Roman".
fn spaced(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && chars[i - 1].is_lowercase() {
            out.push(' ');
        }
        out.push(*c);
    }
    out
}

pub fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

fn detect_std14(ps: &str) -> Option<Std14> {
    let n = norm(ps);
    let bold = n.contains("bold") || n.contains("black") || n.contains("heavy");
    if n.starts_with("helvetica") || n.starts_with("arial") {
        Some(if bold { Std14::HelveticaBold } else { Std14::Helvetica })
    } else if n.starts_with("times") {
        Some(if bold { Std14::TimesBold } else { Std14::Times })
    } else if n.starts_with("courier") {
        Some(Std14::Courier)
    } else if n == "symbol" {
        Some(Std14::Symbol)
    } else if n.starts_with("zapfdingbats") {
        Some(Std14::Dingbats)
    } else {
        None
    }
}

const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556,
    556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334,
    260, 334, 584,
];
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611,
    556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389,
    280, 389, 584,
];
const TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500, 500, 500, 500, 500,
    500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889,
    722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500,
    444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480,
    200, 480, 541,
];

/// Approximate advance of a character in a standard font, for PDFs that
/// leave out `/Widths` (allowed for the standard 14 only).
fn std14_width(f: Std14, c: char) -> f32 {
    let table = match f {
        Std14::Courier => return 600.0,
        Std14::Helvetica => &HELVETICA,
        Std14::HelveticaBold => &HELVETICA_BOLD,
        Std14::Times | Std14::TimesBold => &TIMES,
        Std14::Symbol | Std14::Dingbats => return 600.0,
    };
    let u = c as u32;
    if (0x20..=0x7E).contains(&u) {
        table[(u - 0x20) as usize] as f32
    } else if c.is_alphabetic() {
        if matches!(f, Std14::Times | Std14::TimesBold) { 500.0 } else { 556.0 }
    } else {
        table[0] as f32 * 1.5
    }
}

impl PdfFont {
    /// Reads a font from its dictionary (`font` may be a reference).
    pub fn load(doc: &Document, font: &Object, key: String) -> PdfFont {
        let obj = font.as_reference().ok();
        let empty = Dictionary::new();
        let d = dict(doc, font).unwrap_or(&empty);
        let subtype = get_name(doc, d, b"Subtype").unwrap_or_default();
        let kind = match subtype.as_str() {
            "Type0" => FontKind::Type0,
            "TrueType" => FontKind::TrueType,
            "Type3" => FontKind::Type3,
            _ => FontKind::Type1,
        };
        let raw_name = get_name(doc, d, b"BaseFont")
            .or_else(|| get_name(doc, d, b"Name"))
            .unwrap_or_else(|| if kind == FontKind::Type3 { "Type 3 font".into() } else { "Unnamed font".into() });
        let (ps, subset) = strip_subset(&raw_name);
        let ps_name = ps.to_string();
        let (family, style) = split_name(&ps_name);

        let mut font = PdfFont {
            key,
            obj,
            raw_name: raw_name.clone(),
            ps_name: ps_name.clone(),
            family,
            style: style.clone(),
            bold: false,
            italic: false,
            serif: false,
            fixed: false,
            kind,
            cid_type2: false,
            code_len: 1,
            vertical: false,
            encoding_name: String::new(),
            to_unicode: BTreeMap::new(),
            has_to_unicode: false,
            reverse: HashMap::new(),
            widths: HashMap::new(),
            default_width: 500.0,
            width_scale: 0.001,
            ascent: 0.0,
            descent: 0.0,
            italic_angle: 0.0,
            program: None,
            subset,
            std14: None,
            cid_map: None,
            identity: false,
            cid_to_gid_identity: false,
            descendant: None,
            to_unicode_obj: ref_id(d, b"ToUnicode"),
            extendable: false,
            growth: Growth::None,
        };

        // The dictionary holding the descriptor and widths: the CIDFont for composite fonts.
        let mut descriptor_owner = d;
        if kind == FontKind::Type0 {
            font.code_len = 2;
            match get(doc, d, b"Encoding") {
                Some(Object::Name(n)) => {
                    let name = String::from_utf8_lossy(n).to_string();
                    font.identity = name == "Identity-H" || name == "Identity-V";
                    font.vertical = name.ends_with("-V");
                    font.encoding_name = name;
                }
                Some(Object::Stream(s)) => {
                    font.encoding_name = "Embedded CMap".into();
                    let bytes = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
                    let cm = CMap::parse(&bytes);
                    font.code_len = cm.fixed_code_length().unwrap_or(2);
                    font.cid_map = Some(cm.cids);
                }
                _ => {}
            }
            if let Some(Object::Array(desc)) = get(doc, d, b"DescendantFonts") {
                if let Some(first) = desc.first() {
                    font.descendant = first.as_reference().ok();
                    if let Some(cid) = dict(doc, first) {
                        descriptor_owner = cid;
                        font.cid_type2 = get_name(doc, cid, b"Subtype").as_deref() == Some("CIDFontType2");
                        font.cid_to_gid_identity = match get(doc, cid, b"CIDToGIDMap") {
                            None => true,
                            Some(Object::Name(n)) => n == b"Identity",
                            _ => false,
                        };
                        font.default_width = get_num(doc, cid, b"DW").unwrap_or(1000.0);
                        if let Some(w) = get_array(doc, cid, b"W") {
                            font.read_cid_widths(doc, w);
                        }
                    }
                }
            }
        } else {
            let first = get_num(doc, d, b"FirstChar").unwrap_or(0.0) as u32;
            if let Some(ws) = get_array(doc, d, b"Widths") {
                for (i, w) in ws.iter().enumerate() {
                    if let Some(w) = num(deref(doc, w)) {
                        font.widths.insert(first + i as u32, w);
                    }
                }
            }
            if kind == FontKind::Type3 {
                if let Some(m) = get_array(doc, d, b"FontMatrix") {
                    if let Some(a) = m.first().and_then(|x| num(deref(doc, x))) {
                        font.width_scale = a;
                    }
                }
            }
        }

        // Descriptor: metrics, style flags and the embedded program.
        let mut flags = 0i64;
        if let Some(fd) = get(doc, descriptor_owner, b"FontDescriptor").and_then(|o| match o {
            Object::Dictionary(d) => Some(d),
            _ => None,
        }) {
            flags = get_num(doc, fd, b"Flags").unwrap_or(0.0) as i64;
            font.ascent = get_num(doc, fd, b"Ascent").unwrap_or(0.0);
            font.descent = get_num(doc, fd, b"Descent").unwrap_or(0.0);
            font.italic_angle = get_num(doc, fd, b"ItalicAngle").unwrap_or(0.0);
            if let Some(mw) = get_num(doc, fd, b"MissingWidth") {
                if kind != FontKind::Type0 {
                    font.default_width = mw;
                }
            }
            let weight = get_num(doc, fd, b"FontWeight").unwrap_or(0.0);
            font.bold = weight >= 600.0 || flags & (1 << 18) != 0;
            if let Some(id) = ref_id(fd, b"FontFile2") {
                font.program = Some((ProgramKind::TrueType, id));
            } else if let Some(id) = ref_id(fd, b"FontFile") {
                font.program = Some((ProgramKind::Type1, id));
            } else if let Some(id) = ref_id(fd, b"FontFile3") {
                let sub = doc
                    .get_object(id)
                    .ok()
                    .and_then(|o| o.as_stream().ok())
                    .and_then(|s| s.dict.get(b"Subtype").ok())
                    .and_then(|n| n.as_name().ok())
                    .map(|n| n.to_vec())
                    .unwrap_or_default();
                let pk = if sub == b"OpenType" { ProgramKind::OpenType } else { ProgramKind::Cff };
                font.program = Some((pk, id));
            }
        }
        let lower = ps_name.to_lowercase();
        let style_l = style.to_lowercase();
        font.bold |= ["bold", "black", "heavy", "semibold", "demi"].iter().any(|w| style_l.contains(w) || lower.contains(w));
        font.italic = flags & (1 << 6) != 0
            || font.italic_angle.abs() > 0.5
            || style_l.contains("italic")
            || style_l.contains("oblique")
            || lower.contains("italic");
        font.fixed = flags & 1 != 0 || lower.contains("mono") || lower.contains("courier");
        font.serif = flags & 2 != 0 || lower.contains("times") || lower.contains("serif") && !lower.contains("sans");
        if font.program.is_none() {
            font.std14 = detect_std14(&ps_name);
        }
        if font.ascent == 0.0 {
            font.ascent = 750.0;
        }
        if font.descent == 0.0 {
            font.descent = -250.0;
        }

        // Text mapping.
        let mut base_table: [Option<char>; 256] = [None; 256];
        let mut differences: HashMap<u32, String> = HashMap::new();
        if kind != FontKind::Type0 {
            let symbolic_std = matches!(font.std14, Some(Std14::Symbol | Std14::Dingbats));
            let mut base = if symbolic_std {
                None
            } else if kind == FontKind::TrueType || font.std14.is_some() {
                Some(BaseEncoding::WinAnsi)
            } else {
                Some(BaseEncoding::Standard)
            };
            font.encoding_name = "Built-in".into();
            match get(doc, d, b"Encoding") {
                Some(Object::Name(n)) => {
                    if let Some(b) = BaseEncoding::from_name(n) {
                        base = Some(b);
                    }
                    font.encoding_name = String::from_utf8_lossy(n).to_string();
                }
                Some(Object::Dictionary(ed)) => {
                    if let Some(n) = get_name(doc, ed, b"BaseEncoding") {
                        if let Some(b) = BaseEncoding::from_name(n.as_bytes()) {
                            base = Some(b);
                        }
                        font.encoding_name = format!("{n} + Differences");
                    } else {
                        font.encoding_name = "Differences".into();
                    }
                    if let Some(diffs) = get_array(doc, ed, b"Differences") {
                        let mut code = 0u32;
                        for item in diffs {
                            match deref(doc, item) {
                                Object::Integer(i) => code = *i as u32,
                                Object::Name(n) => {
                                    differences.insert(code, String::from_utf8_lossy(n).to_string());
                                    code += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
            if let Some(b) = base {
                base_table = b.table();
            }
        }

        if let Some(tu) = get(doc, d, b"ToUnicode") {
            if let Some(bytes) = stream_bytes(doc, tu) {
                let cm = CMap::parse(&bytes);
                font.has_to_unicode = !cm.unicode.is_empty();
                font.to_unicode = cm.unicode;
            }
        }
        if kind != FontKind::Type0 {
            for code in 0..256u32 {
                if font.to_unicode.contains_key(&code) {
                    continue;
                }
                let text = differences
                    .get(&code)
                    .and_then(|n| glyph_to_unicode(n))
                    .or_else(|| base_table[code as usize].map(String::from));
                if let Some(t) = text {
                    font.to_unicode.insert(code, t);
                }
            }
        }

        // Which codes can be drawn: everything mapped, except that a subset
        // only has the glyphs it was cut down to (zero widths mark the rest).
        let has_widths = !font.widths.is_empty();
        let codes: Vec<(u32, String)> = font.to_unicode.iter().map(|(c, t)| (*c, t.clone())).collect();
        for (code, text) in codes {
            let available = if kind == FontKind::Type0 {
                true
            } else if has_widths {
                match font.widths.get(&code) {
                    Some(w) => *w > 0.0 || !font.subset,
                    None => false,
                }
            } else {
                true
            };
            if available && !text.is_empty() {
                font.reverse.entry(text).or_insert(code);
            }
        }

        // New glyphs can come from the embedded program when codes are glyph ids
        // (or CIDs the program resolves), or through /Differences for CFF simple fonts.
        font.growth = if kind == FontKind::Type0 && font.identity && font.obj.is_some() {
            match font.program {
                Some((ProgramKind::TrueType, _)) if font.cid_type2 && font.cid_to_gid_identity && font.face_check(doc) => {
                    Growth::Cid
                }
                Some((ProgramKind::OpenType, _)) if (!font.cid_type2 || font.cid_to_gid_identity) && font.face_check(doc) => {
                    Growth::Cid
                }
                _ => Growth::None,
            }
        } else if matches!(kind, FontKind::Type1 | FontKind::TrueType)
            && font.obj.is_some()
            && matches!(font.program, Some((ProgramKind::Cff, _)))
        {
            Growth::Simple
        } else {
            Growth::None
        };
        font.extendable = font.growth != Growth::None;
        font
    }

    fn face_check(&self, doc: &Document) -> bool {
        self.program_bytes(doc)
            .map(|(_, b)| ttf_parser::Face::parse(&b, 0).map(|f| f.tables().cmap.is_some()).unwrap_or(false))
            .unwrap_or(false)
    }

    fn read_cid_widths(&mut self, doc: &Document, w: &[Object]) {
        let mut i = 0;
        while i < w.len() {
            let Some(first) = num(deref(doc, &w[i])) else { break };
            match w.get(i + 1).map(|o| deref(doc, o)) {
                Some(Object::Array(list)) => {
                    for (k, v) in list.iter().enumerate() {
                        if let Some(v) = num(deref(doc, v)) {
                            self.widths.insert(first as u32 + k as u32, v);
                        }
                    }
                    i += 2;
                }
                Some(o) => {
                    let (Some(last), Some(v)) = (num(o), w.get(i + 2).and_then(|x| num(deref(doc, x)))) else { break };
                    let (a, b) = (first as u32, last as u32);
                    if b >= a && b - a < 70000 {
                        for c in a..=b {
                            self.widths.insert(c, v);
                        }
                    }
                    i += 3;
                }
                None => break,
            }
        }
    }

    pub fn cid(&self, code: u32) -> u32 {
        self.cid_map.as_ref().and_then(|m| m.get(&code).copied()).unwrap_or(code)
    }

    /// Advance of a code in glyph space.
    pub fn width(&self, code: u32) -> f32 {
        let key = if self.kind == FontKind::Type0 { self.cid(code) } else { code };
        if let Some(w) = self.widths.get(&key) {
            return *w;
        }
        if let Some(s) = self.std14 {
            if let Some(c) = self.to_unicode.get(&code).and_then(|t| t.chars().next()) {
                return std14_width(s, c);
            }
        }
        self.default_width
    }

    /// Splits a shown string into glyphs.
    pub fn decode(&self, bytes: &[u8]) -> Vec<Glyph> {
        let step = self.code_len.max(1) as usize;
        bytes
            .chunks(step)
            .map(|chunk| {
                let code = chunk.iter().fold(0u32, |a, b| (a << 8) | *b as u32);
                let text = self.to_unicode.get(&code).cloned().unwrap_or_else(|| {
                    if self.kind == FontKind::Type0 || self.has_to_unicode {
                        "\u{FFFD}".to_string()
                    } else {
                        (code as u8 as char).to_string()
                    }
                });
                Glyph { code, text, width: self.width(code) }
            })
            .collect()
    }

    /// Width in glyph space that a gap for a missing space should take.
    fn space_gap(&self) -> f32 {
        if let Some(code) = self.reverse.get(" ") {
            return self.width(*code);
        }
        match self.std14 {
            Some(s) => std14_width(s, ' '),
            None if self.fixed => 600.0,
            None => 260.0,
        }
    }

    /// Encodes text in this font, reporting what it cannot draw.
    /// Encodes text; without ligatures every piece stands for exactly one character.
    pub fn encode_with(&self, doc: &Document, text: &str, ligatures: bool) -> Encoded {
        let mut out = Encoded { pieces: Vec::new(), additions: Vec::new(), missing: Vec::new() };
        let program = if self.growth != Growth::None { self.program_bytes(doc).map(|(_, b)| b) } else { None };
        let face = match self.growth {
            Growth::Cid => program.as_ref().and_then(|b| ttf_parser::Face::parse(b, 0).ok()),
            _ => None,
        };
        let cff = match self.growth {
            Growth::Simple => program.as_ref().and_then(|b| ttf_parser::cff::Table::parse(b)),
            _ => None,
        };
        let mut added: HashMap<char, u32> = HashMap::new();
        let mut free_codes = self.free_codes();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            // Ligatures first ("fi" as one glyph), then single characters.
            if ligatures && i + 1 < chars.len() {
                let pair: String = chars[i..i + 2].iter().collect();
                if let Some(code) = self.reverse.get(&pair) {
                    if !pair.chars().all(|c| c.is_ascii_alphabetic()) || self.reverse.get(&chars[i].to_string()).is_none() {
                        out.pieces.push(Piece::Code(*code));
                        i += 2;
                        continue;
                    }
                }
            }
            let c = chars[i];
            i += 1;
            let s = c.to_string();
            if let Some(code) = self.reverse.get(&s) {
                out.pieces.push(Piece::Code(*code));
                continue;
            }
            if c == '\u{A0}' {
                if let Some(code) = self.reverse.get(" ") {
                    out.pieces.push(Piece::Code(*code));
                    continue;
                }
            }
            if let Some(code) = added.get(&c) {
                out.pieces.push(Piece::Code(*code));
                continue;
            }
            if let Some(face) = &face {
                if let Some(gid) = face.glyph_index(c).filter(|g| g.0 != 0) {
                    // In an OpenType program with CID-keyed CFF the code is the glyph's CID.
                    let code = face
                        .tables()
                        .cff
                        .and_then(|cff| cff.glyph_cid(gid))
                        .map(|cid| cid as u32)
                        .unwrap_or(gid.0 as u32);
                    let upem = face.units_per_em().max(1) as f32;
                    let width = face.glyph_hor_advance(gid).unwrap_or(0) as f32 * 1000.0 / upem;
                    out.additions.push(Addition { code, text: s.clone(), width, glyph_name: None });
                    added.insert(c, code);
                    out.pieces.push(Piece::Code(code));
                    continue;
                }
            }
            if let Some(cff) = &cff {
                let name = crate::encoding::unicode_to_glyph(c);
                let gid = cff
                    .glyph_index_by_name(&name)
                    .or_else(|| cff.glyph_index_by_name(&format!("uni{:04X}", c as u32)))
                    .filter(|g| g.0 != 0);
                if let (Some(gid), false) = (gid, free_codes.is_empty()) {
                    let code = free_codes.remove(0);
                    let m = cff.matrix();
                    let width = cff.glyph_width(gid).unwrap_or(0) as f32 * m.sx * 1000.0;
                    out.additions.push(Addition { code, text: s.clone(), width, glyph_name: Some(name) });
                    added.insert(c, code);
                    out.pieces.push(Piece::Code(code));
                    continue;
                }
            }
            if c == ' ' || c == '\u{A0}' || c == '\t' {
                let g = self.space_gap() * if c == '\t' { 4.0 } else { 1.0 };
                out.pieces.push(Piece::Gap(g));
                continue;
            }
            if !out.missing.contains(&c) {
                out.missing.push(c);
            }
        }
        out
    }

    /// Makes glyphs added during an edit known to this (working) copy, so later
    /// text in the same edit reuses their codes.
    pub fn register(&mut self, additions: &[Addition]) {
        for a in additions {
            self.to_unicode.insert(a.code, a.text.clone());
            self.reverse.entry(a.text.clone()).or_insert(a.code);
            self.widths.insert(a.code, a.width);
        }
    }

    /// Single-byte codes no glyph uses yet, preferring the upper half.
    fn free_codes(&self) -> Vec<u32> {
        if self.growth != Growth::Simple {
            return Vec::new();
        }
        let used: std::collections::HashSet<u32> = self.reverse.values().copied().collect();
        (128..=255u32)
            .chain(33..=127)
            .filter(|c| !used.contains(c) && self.widths.get(c).map(|w| *w == 0.0).unwrap_or(true))
            .collect()
    }

    pub fn program_bytes(&self, doc: &Document) -> Option<(ProgramKind, Vec<u8>)> {
        let (kind, id) = self.program?;
        let bytes = stream_bytes(doc, &Object::Reference(id))?;
        Some((kind, bytes))
    }

    /// The characters this font can draw today, in code order.
    pub fn charset(&self) -> String {
        let mut v: Vec<(&u32, &String)> = self.reverse.iter().map(|(t, c)| (c, t)).collect();
        v.sort();
        v.into_iter().map(|(_, t)| t.as_str()).collect()
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            FontKind::Type0 if self.cid_type2 => "CID TrueType",
            FontKind::Type0 => "CID CFF",
            FontKind::TrueType => "TrueType",
            FontKind::Type3 => "Type 3",
            FontKind::Type1 => match self.program {
                Some((ProgramKind::Cff, _)) => "Type 1 (CFF)",
                Some((ProgramKind::OpenType, _)) => "OpenType",
                _ => "Type 1",
            },
        }
    }
}

/// Records glyphs pulled from an embedded program: ToUnicode entries, widths
/// and, for simple fonts, `/Differences` entries, so the new codes draw,
/// copy, search and lay out correctly.
pub fn apply_additions(doc: &mut Document, font: &PdfFont, additions: &[Addition]) -> Result<(), String> {
    if additions.is_empty() {
        return Ok(());
    }
    let font_id = font.obj.ok_or("This font is stored inline and cannot be extended")?;
    // ToUnicode: rewrite with the old and new entries (simple fonts only when they have one).
    if font.kind == FontKind::Type0 || font.to_unicode_obj.is_some() {
        let mut map = font.to_unicode.clone();
        for a in additions {
            map.insert(a.code, a.text.clone());
        }
        let mut s = Stream::new(Dictionary::new(), cmap::write_to_unicode(&map, font.code_len));
        let _ = s.compress();
        match font.to_unicode_obj {
            Some(id) => {
                doc.objects.insert(id, Object::Stream(s));
            }
            None => {
                let id = doc.add_object(s);
                doc.get_dictionary_mut(font_id).map_err(err)?.set("ToUnicode", Object::Reference(id));
            }
        }
    }
    if font.kind == FontKind::Type0 {
        // Widths: append `cid [w]` entries to the CIDFont's W array.
        let mut entries: Vec<Object> = Vec::new();
        for a in additions {
            entries.push(Object::Integer(a.code as i64));
            entries.push(Object::Array(vec![real(a.width)]));
        }
        if let Some(cid_id) = font.descendant {
            let w_obj = doc.get_dictionary(cid_id).ok().and_then(|d| d.get(b"W").ok().cloned());
            match w_obj {
                Some(Object::Reference(wid)) => {
                    if let Ok(Object::Array(arr)) = doc.get_object_mut(wid) {
                        arr.extend(entries);
                    }
                }
                Some(Object::Array(mut arr)) => {
                    arr.extend(entries);
                    doc.get_dictionary_mut(cid_id).map_err(err)?.set("W", Object::Array(arr));
                }
                _ => {
                    doc.get_dictionary_mut(cid_id).map_err(err)?.set("W", Object::Array(entries));
                }
            }
        }
        return Ok(());
    }

    // Simple font: name the glyphs in /Differences ...
    let mut diffs: Vec<Object> = Vec::new();
    for a in additions {
        diffs.push(Object::Integer(a.code as i64));
        diffs.push(Object::Name(a.glyph_name.clone().unwrap_or_else(|| ".notdef".into()).into_bytes()));
    }
    let enc = doc.get_dictionary(font_id).map_err(err)?.get(b"Encoding").ok().cloned();
    match enc {
        Some(Object::Reference(eid)) if doc.get_dictionary(eid).is_ok() => {
            let d = doc.get_dictionary_mut(eid).map_err(err)?;
            let mut arr = d.get(b"Differences").and_then(Object::as_array).cloned().unwrap_or_default();
            arr.extend(diffs);
            d.set("Differences", arr);
        }
        Some(Object::Dictionary(mut d)) => {
            let mut arr = d.get(b"Differences").and_then(Object::as_array).cloned().unwrap_or_default();
            arr.extend(diffs);
            d.set("Differences", arr);
            doc.get_dictionary_mut(font_id).map_err(err)?.set("Encoding", d);
        }
        other => {
            let mut d = Dictionary::new();
            d.set("Type", "Encoding");
            if let Some(Object::Name(n)) = other {
                d.set("BaseEncoding", Object::Name(n));
            }
            d.set("Differences", diffs);
            doc.get_dictionary_mut(font_id).map_err(err)?.set("Encoding", d);
        }
    }
    // ... and give them widths, growing /Widths when a code falls outside it.
    let fd = doc.get_dictionary(font_id).map_err(err)?;
    let first = fd.get(b"FirstChar").ok().and_then(num).unwrap_or(0.0) as i64;
    let widths_obj = fd.get(b"Widths").ok().cloned();
    let mut widths: Vec<Object> = match &widths_obj {
        Some(Object::Reference(id)) => doc.get_object(*id).ok().and_then(|o| o.as_array().ok()).cloned().unwrap_or_default(),
        Some(Object::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    let mut first = if widths.is_empty() { additions.iter().map(|a| a.code as i64).min().unwrap_or(0) } else { first };
    for a in additions {
        let c = a.code as i64;
        while c < first {
            widths.insert(0, Object::Integer(0));
            first -= 1;
        }
        while c >= first + widths.len() as i64 {
            widths.push(Object::Integer(0));
        }
        widths[(c - first) as usize] = real(a.width);
    }
    let last = first + widths.len() as i64 - 1;
    match widths_obj {
        Some(Object::Reference(id)) => {
            doc.objects.insert(id, Object::Array(widths));
        }
        _ => {
            doc.get_dictionary_mut(font_id).map_err(err)?.set("Widths", widths);
        }
    }
    let fd = doc.get_dictionary_mut(font_id).map_err(err)?;
    fd.set("FirstChar", first);
    fd.set("LastChar", last);
    Ok(())
}

/// Helvetica advance of a character (for tests that lay out text by hand).
#[cfg(test)]
pub fn tests_width(c: char) -> f32 {
    std14_width(Std14::Helvetica, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(split_name("TimesNewRomanPS-BoldItalicMT"), ("Times New Roman".into(), "Bold Italic".into()));
        assert_eq!(split_name("Calibri"), ("Calibri".into(), "".into()));
        assert_eq!(split_name("Arial,Bold"), ("Arial".into(), "Bold".into()));
        assert_eq!(strip_subset("ABCDEF+Calibri-Light"), ("Calibri-Light", true));
    }
}
