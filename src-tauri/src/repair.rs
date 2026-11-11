//! Streams the PDF library could not read.
//!
//! Some producers (scanner software, mostly) indent their files, so that
//! white space stands between a stream's data and the `endstream` keyword
//! where only a line end is allowed. Readers accept that; the PDF library
//! does not, and silently leaves the whole object out. For a scanned page
//! that object is the page's content, and the page comes out blank.
//!
//! After loading, every object the cross-reference table lists but the
//! library did not produce is read again here, leniently.

use lopdf::xref::XrefEntry;
use lopdf::{Dictionary, Document, Object, Stream};

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Parses a dictionary with the library's own parser, by wrapping it in a minimal file.
fn parse_dict(text: &[u8]) -> Option<Dictionary> {
    let mut file = b"%PDF-1.4\n1 0 obj\n".to_vec();
    file.extend_from_slice(text);
    file.extend_from_slice(b"\nendobj\n2 0 obj\n<</Type/Catalog>>\nendobj\n");
    let second = find(&file, b"2 0 obj")?;
    let xref = file.len();
    file.extend_from_slice(format!("xref\n0 3\n0000000000 65535 f \n{:010} 00000 n \n{:010} 00000 n \ntrailer\n<</Size 3/Root 2 0 R>>\nstartxref\n{xref}\n%%EOF\n", 9, second).as_bytes());
    match Document::load_mem(&file).ok()?.objects.remove(&(1, 0))? {
        Object::Dictionary(d) => Some(d),
        _ => None,
    }
}

/// Reads the stream object that starts at `offset`, tolerating white space before `endstream`.
fn read_stream(bytes: &[u8], offset: usize, id: u32, generation: u16, doc: &Document) -> Option<Stream> {
    let rest = bytes.get(offset..)?;
    let head = String::from_utf8_lossy(&rest[..rest.len().min(40)]).to_string();
    let mut words = head.split_whitespace();
    if words.next()?.parse::<u32>().ok()? != id || words.next()?.parse::<u16>().ok()? != generation || !words.next()?.starts_with("obj") {
        return None;
    }
    let obj = find(rest, b"obj")? + 3;
    let end_obj = find(rest, b"endobj").unwrap_or(rest.len());
    let keyword = obj + find(&rest[obj..end_obj.max(obj)], b"stream")?;
    let dict = parse_dict(&rest[obj..keyword])?;
    // Data starts after the line end that follows the keyword.
    let mut start = keyword + 6;
    if rest.get(start) == Some(&b'\r') {
        start += 1;
    }
    if rest.get(start) == Some(&b'\n') {
        start += 1;
    }
    let declared = match dict.get(b"Length").ok()? {
        Object::Integer(n) => Some(*n),
        Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_i64().ok()),
        _ => None,
    }
    .filter(|n| *n >= 0)
    .map(|n| n as usize);
    // The declared length is right when only white space separates the data from the keyword.
    let by_length = declared.filter(|len| {
        rest.get(start + len..)
            .map(|tail| {
                let skipped = tail.iter().take_while(|b| b.is_ascii_whitespace()).count();
                tail[skipped..].starts_with(b"endstream")
            })
            .unwrap_or(false)
    });
    let data = match by_length {
        Some(len) => &rest[start..start + len],
        None => {
            // Otherwise the data runs up to the keyword, less the line end before it.
            let end = start + find(&rest[start..], b"endstream")?;
            let mut data = &rest[start..end];
            while let [head @ .., b' ' | b'\t'] = data {
                data = head;
            }
            if let [head @ .., b'\n'] = data {
                data = head;
            }
            if let [head @ .., b'\r'] = data {
                data = head;
            }
            data
        }
    };
    Some(Stream::new(dict, data.to_vec()))
}

/// Puts back the stream objects the library left out. Returns how many.
pub fn missing_streams(bytes: &[u8], doc: &mut Document) -> usize {
    // What an encrypted file holds cannot be read without the key the library used.
    if doc.encryption_state.is_some() || doc.is_encrypted() {
        return 0;
    }
    let wanted: Vec<(u32, u16, usize)> = doc
        .reference_table
        .entries
        .iter()
        .filter_map(|(id, e)| match e {
            XrefEntry::Normal { offset, generation } if !doc.objects.contains_key(&(*id, *generation)) => Some((*id, *generation, *offset as usize)),
            _ => None,
        })
        .collect();
    let mut repaired = 0;
    for (id, generation, offset) in wanted {
        if let Some(stream) = read_stream(bytes, offset, id, generation, doc) {
            doc.objects.insert((id, generation), Object::Stream(stream));
            doc.max_id = doc.max_id.max(id);
            repaired += 1;
        }
    }
    repaired
}
