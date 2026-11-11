//! Change tracking for large documents.
//!
//! Three things come from knowing exactly which objects a change touched:
//!
//! * **Undo as differences.** A history step keeps only the objects a change
//!   replaced, added or removed, not a copy of the whole document.
//! * **Appended rendering.** The bytes the window renders from are the document
//!   as first serialised plus one small incremental update per change, so an
//!   edit costs the size of the edit, not the size of the file.
//! * **Changed pages.** Following references upwards from the touched objects
//!   finds the pages whose picture can have changed; the others keep their
//!   canvas.

use crate::util::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Writer};
use std::collections::{HashMap, HashSet};
use std::io::Write;

/// What turns one state of a document into another.
pub struct Delta {
    /// Objects to put back (`Some`) or take out (`None`).
    pub objects: Vec<(ObjectId, Option<Object>)>,
    pub trailer: Dictionary,
    pub max_id: u32,
    pub version: String,
}

impl Delta {
    /// Rough memory held by this step, for the history budget.
    pub fn weight(&self) -> usize {
        self.objects
            .iter()
            .map(|(_, o)| match o {
                Some(Object::Stream(s)) => s.content.len() + 256,
                Some(_) => 256,
                None => 16,
            })
            .sum::<usize>()
            + 256
    }

    pub fn ids(&self) -> Vec<ObjectId> {
        self.objects.iter().map(|(id, _)| *id).collect()
    }
}

/// The step that takes `after` back to `before`.
pub fn diff(before: &Document, after: &Document) -> Delta {
    let mut objects = Vec::new();
    for (id, old) in &before.objects {
        if after.objects.get(id) != Some(old) {
            objects.push((*id, Some(old.clone())));
        }
    }
    for id in after.objects.keys() {
        if !before.objects.contains_key(id) {
            objects.push((*id, None));
        }
    }
    Delta { objects, trailer: before.trailer.clone(), max_id: before.max_id, version: before.version.clone() }
}

/// Applies a step and returns the one that undoes it.
pub fn apply(doc: &mut Document, delta: Delta) -> Delta {
    let mut back = Vec::with_capacity(delta.objects.len());
    for (id, value) in delta.objects {
        let current = match value {
            Some(o) => doc.objects.insert(id, o),
            None => doc.objects.remove(&id),
        };
        back.push((id, current));
    }
    Delta {
        objects: back,
        trailer: std::mem::replace(&mut doc.trailer, delta.trailer),
        max_id: std::mem::replace(&mut doc.max_id, delta.max_id),
        version: std::mem::replace(&mut doc.version, delta.version),
    }
}

// ---------------------------------------------------------------------------
// Rendering bytes that grow by appended updates
// ---------------------------------------------------------------------------

/// The document as bytes for the renderer.
pub struct Render {
    /// Changes when the bytes are rebuilt from scratch; appended updates keep it.
    pub generation: u64,
    pub bytes: Vec<u8>,
    /// Offset of the last cross-reference section.
    startxref: usize,
    /// Updates appended since the last full serialisation.
    pub updates: usize,
}

fn last_startxref(bytes: &[u8]) -> Option<usize> {
    let tail_from = bytes.len().saturating_sub(2048);
    let tail = &bytes[tail_from..];
    let key = b"startxref";
    let at = tail.windows(key.len()).rposition(|w| w == key)?;
    let rest = &tail[at + key.len()..];
    let digits: Vec<u8> = rest.iter().skip_while(|b| b.is_ascii_whitespace()).take_while(|b| b.is_ascii_digit()).copied().collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

impl Render {
    /// Serialises the whole document.
    pub fn full(doc: &mut Document, generation: u64) -> Result<Render, String> {
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).map_err(err)?;
        Render::from_bytes(bytes, generation)
    }

    /// Takes bytes that already are this document (a file just written).
    pub fn from_bytes(bytes: Vec<u8>, generation: u64) -> Result<Render, String> {
        let startxref = last_startxref(&bytes).ok_or("The serialised document has no cross-reference pointer.")?;
        Ok(Render { generation, bytes, startxref, updates: 0 })
    }

    /// Appends the current values of `ids` as an incremental update.
    pub fn append(&mut self, doc: &Document, ids: &[ObjectId]) -> Result<(), String> {
        let mut ids: Vec<ObjectId> = ids.to_vec();
        ids.sort();
        ids.dedup();
        let out = &mut self.bytes;
        if out.last() != Some(&b'\n') {
            out.push(b'\n');
        }
        // (object number, generation, offset or None when the object is gone)
        let mut entries: Vec<(u32, u16, Option<usize>)> = Vec::with_capacity(ids.len());
        for id in ids {
            match doc.objects.get(&id) {
                Some(obj) => {
                    entries.push((id.0, id.1, Some(out.len())));
                    write!(out, "{} {} obj\n", id.0, id.1).map_err(err)?;
                    Writer::write_object(out, obj).map_err(err)?;
                    out.extend_from_slice(b"\nendobj\n");
                }
                None => entries.push((id.0, id.1, None)),
            }
        }
        let xref_at = out.len();
        out.extend_from_slice(b"xref\n");
        let mut i = 0;
        while i < entries.len() {
            let mut j = i + 1;
            while j < entries.len() && entries[j].0 == entries[j - 1].0 + 1 {
                j += 1;
            }
            write!(out, "{} {}\n", entries[i].0, j - i).map_err(err)?;
            for (_, generation, offset) in &entries[i..j] {
                match offset {
                    Some(o) => write!(out, "{o:010} {generation:05} n \n").map_err(err)?,
                    None => write!(out, "{:010} {:05} f \n", 0, 65535).map_err(err)?,
                }
            }
            i = j;
        }
        let mut trailer = doc.trailer.clone();
        for key in [&b"Prev"[..], b"XRefStm", b"Type", b"W", b"Index", b"Length", b"Filter", b"DecodeParms", b"Encrypt"] {
            trailer.remove(key);
        }
        trailer.set("Size", (doc.max_id + 1) as i64);
        trailer.set("Prev", self.startxref as i64);
        out.extend_from_slice(b"trailer\n");
        Writer::write_object(out, &Object::Dictionary(trailer)).map_err(err)?;
        write!(out, "\nstartxref\n{xref_at}\n%%EOF\n").map_err(err)?;
        self.startxref = xref_at;
        self.updates += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Which pages a change can have repainted
// ---------------------------------------------------------------------------

/// Keys that point across the document rather than at something a page draws.
const NOT_DRAWN: [&[u8]; 16] = [
    b"Parent", b"P", b"Dest", b"A", b"AA", b"IRT", b"Popup", b"Prev", b"Next", b"First", b"Last", b"K", b"Pg", b"Obj", b"B", b"Thumb",
];

fn collect_refs(o: &Object, out: &mut Vec<ObjectId>) {
    match o {
        Object::Reference(id) => out.push(*id),
        Object::Array(a) => a.iter().for_each(|x| collect_refs(x, out)),
        Object::Dictionary(d) => d.iter().for_each(|(_, v)| collect_refs(v, out)),
        Object::Stream(s) => s.dict.iter().for_each(|(_, v)| collect_refs(v, out)),
        _ => {}
    }
}

fn type_of(o: &Object) -> Option<&[u8]> {
    let d = match o {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &s.dict,
        _ => return None,
    };
    d.get(b"Type").ok().and_then(|t| t.as_name().ok())
}

/// The pages (indices) whose rendering a change to `ids` can affect, or
/// `None` when it may be all of them. `pages_before` is the page list before
/// the change.
pub fn changed_pages(doc: &Document, ids: &[ObjectId], pages_before: &[ObjectId]) -> Option<Vec<usize>> {
    let pages: Vec<ObjectId> = crate::pages::page_ids(doc);
    if pages != pages_before {
        return None;
    }
    if ids.is_empty() {
        return Some(Vec::new());
    }
    if ids.len() > 4000 {
        return None;
    }
    let root = doc.trailer.get(b"Root").ok().and_then(|o| o.as_reference().ok());
    let index: HashMap<ObjectId, usize> = pages.iter().enumerate().map(|(i, p)| (*p, i)).collect();

    // Who draws with what: referrers by object, skipping links that only navigate.
    let mut referrers: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
    let mut buf = Vec::new();
    for (id, obj) in &doc.objects {
        let dict = match obj {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        };
        buf.clear();
        match dict {
            Some(d) => {
                let is_catalog = Some(*id) == root;
                let is_pages = type_of(obj) == Some(b"Pages");
                for (k, v) in d.iter() {
                    if NOT_DRAWN.contains(&k.as_slice()) {
                        continue;
                    }
                    // The page tree's own structure and the catalog's tables are not drawing.
                    if is_pages && k == b"Kids" {
                        continue;
                    }
                    if is_catalog && k != b"OCProperties" {
                        continue;
                    }
                    collect_refs(v, &mut buf);
                }
            }
            None => collect_refs(obj, &mut buf),
        }
        for r in &buf {
            referrers.entry(*r).or_default().push(*id);
        }
    }

    let mut out: HashSet<usize> = HashSet::new();
    let mut seen: HashSet<ObjectId> = HashSet::new();
    let mut queue: Vec<ObjectId> = ids.to_vec();
    while let Some(id) = queue.pop() {
        if !seen.insert(id) {
            continue;
        }
        if let Some(i) = index.get(&id) {
            out.insert(*i);
            continue;
        }
        if Some(id) == root {
            return None;
        }
        if let Some(obj) = doc.objects.get(&id) {
            // Something every page below inherits (resources, boxes, rotation).
            if type_of(obj) == Some(b"Pages") {
                return None;
            }
        }
        if let Some(up) = referrers.get(&id) {
            queue.extend(up.iter().copied());
        }
        if seen.len() > 200_000 {
            return None;
        }
    }
    let mut v: Vec<usize> = out.into_iter().collect();
    v.sort();
    Some(v)
}

/// Keeps rendering honest in tests: the appended bytes must read back as the document.
#[cfg(test)]
pub fn same_objects(a: &Document, b: &Document) -> Result<(), String> {
    for (id, obj) in &a.objects {
        match b.objects.get(id) {
            Some(other) if other == obj => {}
            Some(_) => return Err(format!("object {} {} differs", id.0, id.1)),
            None => return Err(format!("object {} {} is missing", id.0, id.1)),
        }
    }
    let _ = get_name(a, &Dictionary::new(), b"x");
    Ok(())
}
