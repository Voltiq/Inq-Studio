//! CMaps: the `/ToUnicode` streams that say which text a character code
//! stands for, and the encoding CMaps of composite fonts that turn byte
//! sequences into CIDs. Inq Studio reads both, and writes ToUnicode CMaps
//! whenever it adds glyphs to a font, so copied text stays correct.

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct CMap {
    /// Code to text, from `bfchar` / `bfrange`.
    pub unicode: BTreeMap<u32, String>,
    /// Code to CID, from `cidchar` / `cidrange`.
    pub cids: BTreeMap<u32, u32>,
    /// Byte lengths of codes, from `codespacerange` (and failing that, the
    /// lengths of the source codes seen).
    pub code_lengths: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Hex(Vec<u8>),
    Int(i64),
    Word(String),
    Open,
    Close,
}

fn tokenize(data: &[u8]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        match c {
            b'%' => {
                while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                    i += 1;
                }
            }
            b'<' if data.get(i + 1) == Some(&b'<') => i += 2,
            b'>' if data.get(i + 1) == Some(&b'>') => i += 2,
            b'<' => {
                i += 1;
                let mut hex = Vec::new();
                while i < data.len() && data[i] != b'>' {
                    if data[i].is_ascii_hexdigit() {
                        hex.push(data[i]);
                    }
                    i += 1;
                }
                i += 1;
                if hex.len() % 2 == 1 {
                    hex.push(b'0');
                }
                let bytes = hex
                    .chunks(2)
                    .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap_or("00"), 16).unwrap_or(0))
                    .collect();
                out.push(Tok::Hex(bytes));
            }
            b'[' => {
                out.push(Tok::Open);
                i += 1;
            }
            b']' => {
                out.push(Tok::Close);
                i += 1;
            }
            b'(' => {
                // Literal strings appear in the CMap header only; skip them.
                let mut depth = 0;
                while i < data.len() {
                    match data[i] {
                        b'\\' => i += 1,
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < data.len() && !data[i].is_ascii_whitespace() && !b"<>[]()%".contains(&data[i]) {
                    i += 1;
                }
                if i == start {
                    i += 1;
                    continue;
                }
                let word = String::from_utf8_lossy(&data[start..i]).to_string();
                match word.parse::<i64>() {
                    Ok(n) => out.push(Tok::Int(n)),
                    Err(_) => out.push(Tok::Word(word)),
                }
            }
        }
    }
    out
}

fn code_of(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, b| (acc << 8) | *b as u32)
}

/// UTF-16BE bytes to text. Lone surrogates turn into U+FFFD.
fn utf16_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks(2).map(|p| ((p[0] as u16) << 8) | *p.get(1).unwrap_or(&0) as u16).collect();
    char::decode_utf16(units).map(|r| r.unwrap_or('\u{FFFD}')).collect()
}

/// Adds `offset` to the last code unit of a UTF-16BE destination, as `bfrange` does.
fn bump_utf16(bytes: &[u8], offset: u32) -> String {
    let mut b = bytes.to_vec();
    if b.len() >= 2 {
        let n = b.len();
        let last = (((b[n - 2] as u32) << 8) | b[n - 1] as u32) + offset;
        b[n - 2] = (last >> 8) as u8;
        b[n - 1] = last as u8;
    }
    utf16_text(&b)
}

impl CMap {
    pub fn parse(data: &[u8]) -> CMap {
        let toks = tokenize(data);
        let mut map = CMap::default();
        let mut i = 0;
        let note_len = |map: &mut CMap, n: usize| {
            let n = n.clamp(1, 4) as u8;
            if !map.code_lengths.contains(&n) {
                map.code_lengths.push(n);
            }
        };
        while i < toks.len() {
            let Tok::Word(w) = &toks[i] else {
                i += 1;
                continue;
            };
            match w.as_str() {
                "begincodespacerange" => {
                    i += 1;
                    while i + 1 < toks.len() {
                        match (&toks[i], &toks[i + 1]) {
                            (Tok::Hex(lo), Tok::Hex(_)) => {
                                note_len(&mut map, lo.len());
                                i += 2;
                            }
                            _ => break,
                        }
                    }
                }
                "beginbfchar" => {
                    i += 1;
                    while i + 1 < toks.len() {
                        match (&toks[i], &toks[i + 1]) {
                            (Tok::Hex(src), Tok::Hex(dst)) => {
                                map.unicode.insert(code_of(src), utf16_text(dst));
                                i += 2;
                            }
                            (Tok::Hex(src), Tok::Word(name)) => {
                                // A glyph name as destination (rare, allowed by the spec).
                                if let Some(t) = crate::encoding::glyph_to_unicode(name.trim_start_matches('/')) {
                                    map.unicode.insert(code_of(src), t);
                                }
                                i += 2;
                            }
                            _ => break,
                        }
                    }
                }
                "beginbfrange" => {
                    i += 1;
                    while i + 2 < toks.len() {
                        let (Tok::Hex(lo), Tok::Hex(hi)) = (&toks[i], &toks[i + 1]) else { break };
                        let (lo, hi) = (code_of(lo), code_of(hi));
                        if hi < lo || hi - lo > 0xFFFF {
                            break;
                        }
                        match &toks[i + 2] {
                            Tok::Hex(dst) => {
                                for code in lo..=hi {
                                    map.unicode.insert(code, bump_utf16(dst, code - lo));
                                }
                                i += 3;
                            }
                            Tok::Open => {
                                let mut j = i + 3;
                                let mut code = lo;
                                while j < toks.len() && toks[j] != Tok::Close {
                                    if let Tok::Hex(dst) = &toks[j] {
                                        if code <= hi {
                                            map.unicode.insert(code, utf16_text(dst));
                                        }
                                        code += 1;
                                    }
                                    j += 1;
                                }
                                i = j + 1;
                            }
                            _ => break,
                        }
                    }
                }
                "begincidchar" => {
                    i += 1;
                    while i + 1 < toks.len() {
                        match (&toks[i], &toks[i + 1]) {
                            (Tok::Hex(src), Tok::Int(cid)) => {
                                note_len(&mut map, src.len());
                                map.cids.insert(code_of(src), *cid as u32);
                                i += 2;
                            }
                            _ => break,
                        }
                    }
                }
                "begincidrange" => {
                    i += 1;
                    while i + 2 < toks.len() {
                        match (&toks[i], &toks[i + 1], &toks[i + 2]) {
                            (Tok::Hex(lo), Tok::Hex(hi), Tok::Int(cid)) => {
                                note_len(&mut map, lo.len());
                                let (l, h) = (code_of(lo), code_of(hi));
                                if h >= l && h - l <= 0xFFFF {
                                    for code in l..=h {
                                        map.cids.insert(code, *cid as u32 + (code - l));
                                    }
                                }
                                i += 3;
                            }
                            _ => break,
                        }
                    }
                }
                _ => i += 1,
            }
        }
        map
    }

    /// The code length when all codes are the same length.
    pub fn fixed_code_length(&self) -> Option<u8> {
        match self.code_lengths.as_slice() {
            [n] => Some(*n),
            _ => None,
        }
    }
}

fn hex(code: u32, bytes: u8) -> String {
    format!("{:0width$X}", code, width = bytes as usize * 2)
}

fn utf16_hex(text: &str) -> String {
    text.encode_utf16().map(|u| format!("{u:04X}")).collect()
}

/// Writes a ToUnicode CMap for codes of `bytes` length.
pub fn write_to_unicode(map: &BTreeMap<u32, String>, bytes: u8) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n");
    s.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    s.push_str("/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n");
    s.push_str(&format!(
        "1 begincodespacerange\n<{}> <{}>\nendcodespacerange\n",
        hex(0, bytes),
        hex(if bytes == 1 { 0xFF } else { 0xFFFF }, bytes)
    ));
    let entries: Vec<_> = map.iter().filter(|(_, t)| !t.is_empty()).collect();
    for chunk in entries.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (code, text) in chunk {
            s.push_str(&format!("<{}> <{}>\n", hex(**code, bytes), utf16_hex(text)));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bfchar_and_bfrange() {
        let src = b"1 begincodespacerange <0000> <FFFF> endcodespacerange
            2 beginbfchar <0003> <0020> <0011> <00660069> endbfchar
            2 beginbfrange <0024> <0026> <0041> <0030> <0031> [<00E9> <D83DDE00>] endbfrange";
        let m = CMap::parse(src);
        assert_eq!(m.fixed_code_length(), Some(2));
        assert_eq!(m.unicode[&3], " ");
        assert_eq!(m.unicode[&0x11], "fi");
        assert_eq!(m.unicode[&0x26], "C");
        assert_eq!(m.unicode[&0x30], "é");
        assert_eq!(m.unicode[&0x31], "😀");
    }

    #[test]
    fn round_trips() {
        let mut map = BTreeMap::new();
        map.insert(5u32, "Q".to_string());
        map.insert(6u32, "😀".to_string());
        let parsed = CMap::parse(&write_to_unicode(&map, 2));
        assert_eq!(parsed.unicode, map);
    }
}
