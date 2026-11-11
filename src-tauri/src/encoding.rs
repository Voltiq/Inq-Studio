//! Single-byte font encodings and glyph names.
//!
//! Simple fonts (Type 1, TrueType, Type 3) map one byte to one glyph. Which
//! character that glyph is comes from a base encoding (WinAnsi, MacRoman,
//! Standard), patched by a `/Differences` array of glyph names. Both are
//! needed in two directions: bytes to text when reading a page, and text to
//! bytes when writing in the document's own font.

/// The four base encodings a simple font can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseEncoding {
    WinAnsi,
    MacRoman,
    Standard,
    PdfDoc,
}

impl BaseEncoding {
    pub fn from_name(name: &[u8]) -> Option<Self> {
        match name {
            b"WinAnsiEncoding" => Some(Self::WinAnsi),
            b"MacRomanEncoding" => Some(Self::MacRoman),
            b"StandardEncoding" => Some(Self::Standard),
            b"PDFDocEncoding" => Some(Self::PdfDoc),
            _ => None,
        }
    }

    /// The character a byte stands for in this encoding.
    pub fn decode(self, code: u8) -> Option<char> {
        match self {
            Self::WinAnsi | Self::PdfDoc => win_ansi(code),
            Self::MacRoman => mac_roman(code),
            Self::Standard => standard(code),
        }
    }

    /// The table as 256 optional characters.
    pub fn table(self) -> [Option<char>; 256] {
        let mut out = [None; 256];
        for (code, slot) in out.iter_mut().enumerate() {
            *slot = self.decode(code as u8);
        }
        out
    }
}

fn win_ansi(code: u8) -> Option<char> {
    const HIGH: [u32; 32] = [
        0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017D, 0,
        0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E,
        0x0178,
    ];
    match code {
        0x20..=0x7E => Some(code as char),
        0x80..=0x9F => char::from_u32(HIGH[(code - 0x80) as usize]).filter(|c| *c != '\0'),
        0xA0..=0xFF => Some(code as char),
        // Many producers put tabs and line breaks through WinAnsi unchanged.
        0x09 | 0x0A | 0x0D => Some(code as char),
        _ => None,
    }
}

fn mac_roman(code: u8) -> Option<char> {
    const HIGH: [u32; 128] = [
        0xC4, 0xC5, 0xC7, 0xC9, 0xD1, 0xD6, 0xDC, 0xE1, 0xE0, 0xE2, 0xE4, 0xE3, 0xE5, 0xE7, 0xE9, 0xE8, // 80
        0xEA, 0xEB, 0xED, 0xEC, 0xEE, 0xEF, 0xF1, 0xF3, 0xF2, 0xF4, 0xF6, 0xF5, 0xFA, 0xF9, 0xFB, 0xFC, // 90
        0x2020, 0xB0, 0xA2, 0xA3, 0xA7, 0x2022, 0xB6, 0xDF, 0xAE, 0xA9, 0x2122, 0xB4, 0xA8, 0x2260, 0xC6, 0xD8, // A0
        0x221E, 0xB1, 0x2264, 0x2265, 0xA5, 0xB5, 0x2202, 0x2211, 0x220F, 0x3C0, 0x222B, 0xAA, 0xBA, 0x3A9, 0xE6,
        0xF8, // B0
        0xBF, 0xA1, 0xAC, 0x221A, 0x192, 0x2248, 0x2206, 0xAB, 0xBB, 0x2026, 0xA0, 0xC0, 0xC3, 0xD5, 0x152, 0x153, // C0
        0x2013, 0x2014, 0x201C, 0x201D, 0x2018, 0x2019, 0xF7, 0x25CA, 0xFF, 0x178, 0x2044, 0xA4, 0x2039, 0x203A,
        0xFB01, 0xFB02, // D0
        0x2021, 0xB7, 0x201A, 0x201E, 0x2030, 0xC2, 0xCA, 0xC1, 0xCB, 0xC8, 0xCD, 0xCE, 0xCF, 0xCC, 0xD3, 0xD4, // E0
        0xF8FF, 0xD2, 0xDA, 0xDB, 0xD9, 0x131, 0x2C6, 0x2DC, 0xAF, 0x2D8, 0x2D9, 0x2DA, 0xB8, 0x2DD, 0x2DB, 0x2C7, // F0
    ];
    match code {
        0x20..=0x7E => Some(code as char),
        0x80..=0xFF => char::from_u32(HIGH[(code - 0x80) as usize]),
        _ => None,
    }
}

fn standard(code: u8) -> Option<char> {
    let u: u32 = match code {
        0x27 => 0x2019,
        0x60 => 0x2018,
        0x20..=0x7E => code as u32,
        0xA1 => 0xA1,
        0xA2 => 0xA2,
        0xA3 => 0xA3,
        0xA4 => 0x2044,
        0xA5 => 0xA5,
        0xA6 => 0x192,
        0xA7 => 0xA7,
        0xA8 => 0xA4,
        0xA9 => 0x27,
        0xAA => 0x201C,
        0xAB => 0xAB,
        0xAC => 0x2039,
        0xAD => 0x203A,
        0xAE => 0xFB01,
        0xAF => 0xFB02,
        0xB1 => 0x2013,
        0xB2 => 0x2020,
        0xB3 => 0x2021,
        0xB4 => 0xB7,
        0xB6 => 0xB6,
        0xB7 => 0x2022,
        0xB8 => 0x201A,
        0xB9 => 0x201E,
        0xBA => 0x201D,
        0xBB => 0xBB,
        0xBC => 0x2026,
        0xBD => 0x2030,
        0xBF => 0xBF,
        0xC1 => 0x60,
        0xC2 => 0xB4,
        0xC3 => 0x2C6,
        0xC4 => 0x2DC,
        0xC5 => 0xAF,
        0xC6 => 0x2D8,
        0xC7 => 0x2D9,
        0xC8 => 0xA8,
        0xCA => 0x2DA,
        0xCB => 0xB8,
        0xCD => 0x2DD,
        0xCE => 0x2DB,
        0xCF => 0x2C7,
        0xD0 => 0x2014,
        0xE1 => 0xC6,
        0xE3 => 0xAA,
        0xE8 => 0x141,
        0xE9 => 0xD8,
        0xEA => 0x152,
        0xEB => 0xBA,
        0xF1 => 0xE6,
        0xF5 => 0x131,
        0xF8 => 0x142,
        0xF9 => 0xF8,
        0xFA => 0x153,
        0xFB => 0xDF,
        _ => return None,
    };
    char::from_u32(u)
}

/// Adobe glyph names for the characters PDF producers use most. Anything
/// else is reached through the `uniXXXX` / `uXXXXX` forms.
const GLYPH_NAMES: &[(&str, u32)] = &[
    ("space", 0x20), ("exclam", 0x21), ("quotedbl", 0x22), ("numbersign", 0x23), ("dollar", 0x24),
    ("percent", 0x25), ("ampersand", 0x26), ("quotesingle", 0x27), ("quoteright", 0x2019),
    ("parenleft", 0x28), ("parenright", 0x29), ("asterisk", 0x2A), ("plus", 0x2B), ("comma", 0x2C),
    ("hyphen", 0x2D), ("period", 0x2E), ("slash", 0x2F), ("zero", 0x30), ("one", 0x31), ("two", 0x32),
    ("three", 0x33), ("four", 0x34), ("five", 0x35), ("six", 0x36), ("seven", 0x37), ("eight", 0x38),
    ("nine", 0x39), ("colon", 0x3A), ("semicolon", 0x3B), ("less", 0x3C), ("equal", 0x3D), ("greater", 0x3E),
    ("question", 0x3F), ("at", 0x40), ("bracketleft", 0x5B), ("backslash", 0x5C), ("bracketright", 0x5D),
    ("asciicircum", 0x5E), ("underscore", 0x5F), ("grave", 0x60), ("quoteleft", 0x2018), ("braceleft", 0x7B),
    ("bar", 0x7C), ("braceright", 0x7D), ("asciitilde", 0x7E),
    ("exclamdown", 0xA1), ("cent", 0xA2), ("sterling", 0xA3), ("currency", 0xA4), ("yen", 0xA5),
    ("brokenbar", 0xA6), ("section", 0xA7), ("dieresis", 0xA8), ("copyright", 0xA9), ("ordfeminine", 0xAA),
    ("guillemotleft", 0xAB), ("logicalnot", 0xAC), ("sfthyphen", 0xAD), ("registered", 0xAE), ("macron", 0xAF),
    ("degree", 0xB0), ("plusminus", 0xB1), ("twosuperior", 0xB2), ("threesuperior", 0xB3), ("acute", 0xB4),
    ("mu", 0xB5), ("paragraph", 0xB6), ("periodcentered", 0xB7), ("cedilla", 0xB8), ("onesuperior", 0xB9),
    ("ordmasculine", 0xBA), ("guillemotright", 0xBB), ("onequarter", 0xBC), ("onehalf", 0xBD),
    ("threequarters", 0xBE), ("questiondown", 0xBF),
    ("Agrave", 0xC0), ("Aacute", 0xC1), ("Acircumflex", 0xC2), ("Atilde", 0xC3), ("Adieresis", 0xC4),
    ("Aring", 0xC5), ("AE", 0xC6), ("Ccedilla", 0xC7), ("Egrave", 0xC8), ("Eacute", 0xC9), ("Ecircumflex", 0xCA),
    ("Edieresis", 0xCB), ("Igrave", 0xCC), ("Iacute", 0xCD), ("Icircumflex", 0xCE), ("Idieresis", 0xCF),
    ("Eth", 0xD0), ("Ntilde", 0xD1), ("Ograve", 0xD2), ("Oacute", 0xD3), ("Ocircumflex", 0xD4), ("Otilde", 0xD5),
    ("Odieresis", 0xD6), ("multiply", 0xD7), ("Oslash", 0xD8), ("Ugrave", 0xD9), ("Uacute", 0xDA),
    ("Ucircumflex", 0xDB), ("Udieresis", 0xDC), ("Yacute", 0xDD), ("Thorn", 0xDE), ("germandbls", 0xDF),
    ("agrave", 0xE0), ("aacute", 0xE1), ("acircumflex", 0xE2), ("atilde", 0xE3), ("adieresis", 0xE4),
    ("aring", 0xE5), ("ae", 0xE6), ("ccedilla", 0xE7), ("egrave", 0xE8), ("eacute", 0xE9), ("ecircumflex", 0xEA),
    ("edieresis", 0xEB), ("igrave", 0xEC), ("iacute", 0xED), ("icircumflex", 0xEE), ("idieresis", 0xEF),
    ("eth", 0xF0), ("ntilde", 0xF1), ("ograve", 0xF2), ("oacute", 0xF3), ("ocircumflex", 0xF4), ("otilde", 0xF5),
    ("odieresis", 0xF6), ("divide", 0xF7), ("oslash", 0xF8), ("ugrave", 0xF9), ("uacute", 0xFA),
    ("ucircumflex", 0xFB), ("udieresis", 0xFC), ("yacute", 0xFD), ("thorn", 0xFE), ("ydieresis", 0xFF),
    ("Euro", 0x20AC), ("quotesinglbase", 0x201A), ("florin", 0x192), ("quotedblbase", 0x201E),
    ("ellipsis", 0x2026), ("dagger", 0x2020), ("daggerdbl", 0x2021), ("circumflex", 0x2C6),
    ("perthousand", 0x2030), ("Scaron", 0x160), ("guilsinglleft", 0x2039), ("OE", 0x152), ("Zcaron", 0x17D),
    ("quotedblleft", 0x201C), ("quotedblright", 0x201D), ("bullet", 0x2022), ("endash", 0x2013),
    ("emdash", 0x2014), ("tilde", 0x2DC), ("trademark", 0x2122), ("scaron", 0x161), ("guilsinglright", 0x203A),
    ("oe", 0x153), ("zcaron", 0x17E), ("Ydieresis", 0x178), ("fi", 0xFB01), ("fl", 0xFB02), ("ff", 0xFB00),
    ("ffi", 0xFB03), ("ffl", 0xFB04), ("fraction", 0x2044), ("dotlessi", 0x131), ("Lslash", 0x141),
    ("lslash", 0x142), ("breve", 0x2D8), ("dotaccent", 0x2D9), ("ring", 0x2DA), ("hungarumlaut", 0x2DD),
    ("ogonek", 0x2DB), ("caron", 0x2C7), ("minus", 0x2212), ("notequal", 0x2260), ("infinity", 0x221E),
    ("lessequal", 0x2264), ("greaterequal", 0x2265), ("partialdiff", 0x2202), ("summation", 0x2211),
    ("product", 0x220F), ("pi", 0x3C0), ("integral", 0x222B), ("Omega", 0x3A9), ("radical", 0x221A),
    ("approxequal", 0x2248), ("Delta", 0x2206), ("lozenge", 0x25CA), ("nbspace", 0xA0),
    ("nonbreakingspace", 0xA0), ("Aogonek", 0x104), ("aogonek", 0x105), ("Cacute", 0x106), ("cacute", 0x107),
    ("Ccaron", 0x10C), ("ccaron", 0x10D), ("Dcaron", 0x10E), ("dcaron", 0x10F), ("Dcroat", 0x110),
    ("dcroat", 0x111), ("Eogonek", 0x118), ("eogonek", 0x119), ("Ecaron", 0x11A), ("ecaron", 0x11B),
    ("Gbreve", 0x11E), ("gbreve", 0x11F), ("Idotaccent", 0x130), ("Nacute", 0x143), ("nacute", 0x144),
    ("Ncaron", 0x147), ("ncaron", 0x148), ("Odblacute", 0x150), ("odblacute", 0x151), ("Rcaron", 0x158),
    ("rcaron", 0x159), ("Sacute", 0x15A), ("sacute", 0x15B), ("Scedilla", 0x15E), ("scedilla", 0x15F),
    ("Tcaron", 0x164), ("tcaron", 0x165), ("Uring", 0x16E), ("uring", 0x16F), ("Udblacute", 0x170),
    ("udblacute", 0x171), ("Zacute", 0x179), ("zacute", 0x17A), ("Zdotaccent", 0x17B), ("zdotaccent", 0x17C),
    ("Amacron", 0x100), ("amacron", 0x101), ("Emacron", 0x112), ("emacron", 0x113), ("Imacron", 0x12A),
    ("imacron", 0x12B), ("Omacron", 0x14C), ("omacron", 0x14D), ("Umacron", 0x16A), ("umacron", 0x16B),
    ("arrowleft", 0x2190), ("arrowup", 0x2191), ("arrowright", 0x2192), ("arrowdown", 0x2193),
    ("checkmark", 0x2713), ("periodcentered.loclCAT", 0xB7), ("figuredash", 0x2012), ("afii00208", 0x2015),
    ("quotereversed", 0x201B), ("onethird", 0x2153), ("twothirds", 0x2154), ("oneeighth", 0x215B),
    ("degreesign", 0xB0), ("micro", 0xB5), ("estimated", 0x212E), ("numero", 0x2116),
];

/// The text a glyph name stands for, following the Adobe Glyph List rules:
/// suffixes after a period are variants, underscores join ligatures, and
/// `uniXXXX` / `uXXXX` spell code points directly.
pub fn glyph_to_unicode(name: &str) -> Option<String> {
    if name == ".notdef" || name.is_empty() {
        return None;
    }
    let base = name.split('.').next().unwrap_or(name);
    if base.is_empty() {
        return None;
    }
    if base.contains('_') {
        let parts: Option<String> = base.split('_').map(glyph_to_unicode).collect();
        return parts;
    }
    if let Some(hex) = base.strip_prefix("uni") {
        if hex.len() >= 4 && hex.len() % 4 == 0 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut out = String::new();
            for chunk in hex.as_bytes().chunks(4) {
                let v = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
                out.push(char::from_u32(v)?);
            }
            return Some(out);
        }
    }
    if let Some(hex) = base.strip_prefix('u') {
        if (4..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return char::from_u32(u32::from_str_radix(hex, 16).ok()?).map(String::from);
        }
    }
    if let Some((_, u)) = GLYPH_NAMES.iter().find(|(n, _)| *n == base) {
        return char::from_u32(*u).map(String::from);
    }
    if base.len() == 1 && base.is_ascii() {
        return Some(base.to_string());
    }
    None
}

/// The WinAnsi byte for a character, for text set in the standard fonts.
pub fn win_ansi_byte(c: char) -> Option<u8> {
    let c = if c == '\u{A0}' || c == '\t' { ' ' } else { c };
    (0u16..256).find(|b| win_ansi(*b as u8) == Some(c)).map(|b| b as u8)
}

/// The glyph name for a character, for writing `/Differences`.
pub fn unicode_to_glyph(c: char) -> String {
    if c.is_ascii_alphabetic() {
        return c.to_string();
    }
    if let Some((n, _)) = GLYPH_NAMES.iter().find(|(_, u)| *u == c as u32) {
        return (*n).to_string();
    }
    format!("uni{:04X}", c as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_names() {
        assert_eq!(glyph_to_unicode("A").as_deref(), Some("A"));
        assert_eq!(glyph_to_unicode("eacute").as_deref(), Some("é"));
        assert_eq!(glyph_to_unicode("uni20AC").as_deref(), Some("€"));
        assert_eq!(glyph_to_unicode("f_i").as_deref(), Some("fi"));
        assert_eq!(glyph_to_unicode("a.sc").as_deref(), Some("a"));
        assert_eq!(glyph_to_unicode("g123"), None);
        assert_eq!(unicode_to_glyph('é'), "eacute");
    }

    #[test]
    fn encodings() {
        assert_eq!(BaseEncoding::WinAnsi.decode(0x80), Some('€'));
        assert_eq!(BaseEncoding::WinAnsi.decode(0xE9), Some('é'));
        assert_eq!(BaseEncoding::MacRoman.decode(0x8E), Some('é'));
        assert_eq!(BaseEncoding::Standard.decode(0x27), Some('’'));
    }
}
