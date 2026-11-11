//! Digital signatures: checking the ones a document carries, and signing.
//!
//! A PDF signature covers a byte range of the file: everything except the
//! signature value itself. Checking one means hashing exactly those bytes
//! and verifying the CMS signature over that hash, then looking at who
//! signed (the certificate and whether this computer trusts its issuer) and
//! whether anything was appended to the file afterwards.
//!
//! Signing writes a signature dictionary with room for the value, saves the
//! file, and fills in the byte range and the value in place. Signatures are
//! PAdES baseline (`ETSI.CAdES.detached`, SHA-256, with the signing
//! certificate bound into the signed attributes).
//!
//! The cryptography is done by Windows (CryptoAPI / CNG), so certificates in
//! the Windows store, smart cards and tokens work, as do `.pfx` files.

use crate::pages;
use crate::util::*;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use serde::{Deserialize, Serialize};

/// Room reserved for the signature value, in bytes.
pub(crate) const CONTENTS_BYTES: usize = 24576;
const RANGE_PLACEHOLDER: i64 = 9_999_999_999;

// ---------------------------------------------------------------------------
// Reading signatures
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SigInfo {
    pub field: String,
    pub page: Option<usize>,
    pub rect: Option<[f32; 4]>,
    /// "signature", "certification" or "timestamp"
    pub kind: String,
    pub sub_filter: String,
    /// Who signed, from the certificate (or the dictionary's Name when there is no certificate).
    pub signer: String,
    pub subject: String,
    pub issuer: String,
    pub serial: String,
    pub valid_from: String,
    pub valid_to: String,
    pub reason: String,
    pub location: String,
    pub contact: String,
    /// When it was signed, as the signature says (ISO 8601).
    pub time: String,
    pub digest: String,
    /// The signed bytes are unchanged. `None` when it could not be checked.
    pub intact: Option<bool>,
    /// The signature covers the file as it is on disk; otherwise content was appended after it.
    pub covers_whole: bool,
    /// Which revision of the file it signs, and how many there are.
    pub revision: usize,
    pub revisions: usize,
    /// "trusted", "untrusted", "expired", "revoked", "unknown"
    pub trust: String,
    pub trust_detail: String,
    /// The signature carries a timestamp from a timestamp authority.
    pub timestamped: bool,
    /// Why it could not be checked, in words.
    pub problem: String,
}

struct RawSig {
    field: String,
    page: Option<usize>,
    rect: Option<[f32; 4]>,
    dict: Dictionary,
}

fn text(doc: &Document, d: &Dictionary, key: &[u8]) -> String {
    get(doc, d, key).and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default()
}

fn signed_fields(doc: &Document) -> Vec<RawSig> {
    let mut out = Vec::new();
    let Ok(cat) = doc.catalog() else { return out };
    let Some(form) = get(doc, cat, b"AcroForm").and_then(|o| dict(doc, o)) else { return out };
    let page_ids = pages::page_ids(doc);
    let mut stack: Vec<(Object, String)> = get_array(doc, form, b"Fields").map(|a| a.iter().map(|o| (o.clone(), String::new())).collect()).unwrap_or_default();
    stack.reverse();
    let mut seen = 0;
    while let Some((o, prefix)) = stack.pop() {
        seen += 1;
        if seen > 20_000 {
            break;
        }
        let Some(f) = dict(doc, &o) else { continue };
        let own = text(doc, f, b"T");
        let name = match (prefix.is_empty(), own.is_empty()) {
            (true, _) => own,
            (false, true) => prefix.clone(),
            (false, false) => format!("{prefix}.{own}"),
        };
        if let Some(kids) = get_array(doc, f, b"Kids") {
            for k in kids.iter().rev() {
                stack.push((k.clone(), name.clone()));
            }
        }
        if get_name(doc, f, b"FT").as_deref() != Some("Sig") {
            continue;
        }
        let Some(v) = get(doc, f, b"V").and_then(|o| dict(doc, o)) else { continue };
        // The widget is the field itself or its first kid.
        let widget = if f.has(b"Rect") { Some(f) } else { get_array(doc, f, b"Kids").and_then(|k| k.first()).and_then(|k| dict(doc, k)) };
        let page = widget.and_then(|w| ref_id(w, b"P")).and_then(|p| page_ids.iter().position(|x| *x == p)).or_else(|| {
            // No /P: find the page whose annotations hold the widget.
            let id = o.as_reference().ok()?;
            page_ids.iter().position(|pid| crate::annots::annots_array(doc, *pid).iter().any(|a| a.as_reference().ok() == Some(id)))
        });
        let rect = widget.and_then(|w| w.get(b"Rect").ok()).and_then(|r| rect(doc, r)).filter(|r| r[2] - r[0] > 0.5 && r[3] - r[1] > 0.5);
        out.push(RawSig { field: name, page, rect, dict: v.clone() });
    }
    out
}

/// Signature fields that are still empty, by name.
pub fn empty_fields(doc: &Document) -> Vec<String> {
    let signed: Vec<String> = signed_fields(doc).into_iter().map(|s| s.field).collect();
    crate::forms::fields(doc).into_iter().filter(|f| f.kind == "signature" && !signed.contains(&f.name)).map(|f| f.name).collect()
}

fn byte_range(doc: &Document, d: &Dictionary) -> Option<[usize; 4]> {
    let a = get_array(doc, d, b"ByteRange")?;
    let v: Vec<usize> = a.iter().filter_map(|o| deref(doc, o).as_i64().ok()).filter(|n| *n >= 0).map(|n| n as usize).collect();
    (a.len() == 4 && v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
}

/// The signature value as it stands in the file, between the two signed ranges.
fn contents_from_file(file: &[u8], range: [usize; 4]) -> Option<Vec<u8>> {
    let (from, to) = (range[0] + range[1], range[2]);
    if from >= to || to > file.len() {
        return None;
    }
    let gap = &file[from..to];
    let hex: Vec<u8> = gap.iter().copied().filter(|b| b.is_ascii_hexdigit()).collect();
    let mut out = Vec::with_capacity(hex.len() / 2);
    for pair in hex.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        let v = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
        out.push(v(pair[0]) << 4 | v(pair[1]));
    }
    Some(out)
}

/// A DER element: (tag, header length, content length).
fn der_head(b: &[u8]) -> Option<(u8, usize, usize)> {
    let tag = *b.first()?;
    let first = *b.get(1)? as usize;
    if first < 0x80 {
        return Some((tag, 2, first));
    }
    let n = first & 0x7f;
    if n == 0 || n > 4 || b.len() < 2 + n {
        return None;
    }
    let len = b[2..2 + n].iter().fold(0usize, |acc, x| acc << 8 | *x as usize);
    Some((tag, 2 + n, len))
}

/// The DER object at the start of `b`, without the zero padding after it.
fn der_trim(b: &[u8]) -> Option<&[u8]> {
    let (_, head, len) = der_head(b)?;
    b.get(..head + len)
}

/// The elements that follow each other in `rest`: (tag, content).
fn der_items(mut rest: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    while let Some((tag, h, l)) = der_head(rest) {
        let Some(value) = rest.get(h..h + l) else { break };
        out.push((tag, value));
        rest = &rest[h + l..];
    }
    out
}

/// The children of a constructed DER element.
fn der_children(b: &[u8]) -> Vec<(u8, &[u8])> {
    match der_head(b) {
        Some((_, head, len)) => b.get(head..head + len).map(der_items).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// "D:20260102030405+01'00'" → "2026-01-02T03:04:05+01:00"
fn iso_from_pdf(pdf: &str) -> String {
    let s = pdf.trim().trim_start_matches("D:");
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 {
        return String::new();
    }
    let part = |from: usize, default: &str| digits.get(from..from + 2).unwrap_or(default).to_string();
    let mut out = format!("{}-{}-{}T{}:{}:{}", &digits[0..4], part(4, "01"), part(6, "01"), part(8, "00"), part(10, "00"), part(12, "00"));
    let zone = &s[digits.len()..];
    match zone.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let nums: String = zone[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            out.push_str(&format!("{sign}{}:{}", nums.get(0..2).unwrap_or("00"), nums.get(2..4).unwrap_or("00")));
        }
        _ => out.push('Z'),
    }
    out
}

/// Which revision of the file ends at `end` (each revision closes with `%%EOF`), and how many there are.
fn revision_at(file: &[u8], end: usize) -> (usize, usize) {
    let key = b"%%EOF";
    let (mut total, mut upto) = (0, 0);
    let mut i = 0;
    while i + key.len() <= file.len() {
        if &file[i..i + key.len()] == key {
            total += 1;
            if i + key.len() <= end {
                upto += 1;
            }
            i += key.len();
        } else {
            i += 1;
        }
    }
    (upto.max(1), total.max(1))
}

/// The signatures of a document, checked against the file as it is on disk.
pub fn list(doc: &Document, file: Option<&[u8]>) -> Vec<SigInfo> {
    let mut out = Vec::new();
    for raw in signed_fields(doc) {
        let d = &raw.dict;
        let sub_filter = get_name(doc, d, b"SubFilter").unwrap_or_default();
        let is_timestamp = get_name(doc, d, b"Type").as_deref() == Some("DocTimeStamp") || sub_filter == "ETSI.RFC3161";
        let certifies = get_array(doc, d, b"Reference")
            .map(|a| a.iter().filter_map(|r| dict(doc, r)).any(|r| get_name(doc, r, b"TransformMethod").as_deref() == Some("DocMDP")))
            .unwrap_or(false);
        let mut info = SigInfo {
            field: raw.field.clone(),
            page: raw.page,
            rect: raw.rect,
            kind: if is_timestamp { "timestamp" } else if certifies { "certification" } else { "signature" }.into(),
            sub_filter: sub_filter.clone(),
            signer: text(doc, d, b"Name"),
            reason: text(doc, d, b"Reason"),
            location: text(doc, d, b"Location"),
            contact: text(doc, d, b"ContactInfo"),
            time: iso_from_pdf(&text(doc, d, b"M")),
            trust: "unknown".into(),
            revision: 1,
            revisions: 1,
            ..Default::default()
        };
        let range = byte_range(doc, d);
        match (file, range) {
            (None, _) => info.problem = "Save the document first: a signature is checked against the file.".into(),
            (_, None) => info.problem = "The signature does not say which bytes it covers.".into(),
            (Some(file), Some(r)) => {
                let end = r[2].saturating_add(r[3]);
                if r[0] != 0 || r[0] + r[1] > r[2] || end > file.len() {
                    info.problem = "The signed byte range does not fit the file.".into();
                    info.intact = Some(false);
                } else {
                    info.covers_whole = file.len() - end <= 2 && file[end..].iter().all(|b| b.is_ascii_whitespace());
                    let (rev, total) = revision_at(file, end);
                    info.revision = rev;
                    info.revisions = total;
                    match contents_from_file(file, r).as_deref().and_then(der_trim) {
                        None => info.problem = "The signature value is empty or damaged.".into(),
                        Some(der) => {
                            let parts = [&file[..r[1]], &file[r[2]..end]];
                            verify::check(der, &parts, &sub_filter, &mut info);
                        }
                    }
                }
            }
        }
        out.push(info);
    }
    out
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SignOptions {
    pub reason: String,
    pub location: String,
    pub contact: String,
    /// Where the visible signature goes; without a rectangle the signature is invisible.
    pub page: usize,
    pub rect: Option<[f32; 4]>,
    /// Sign into this existing empty signature field instead of adding one.
    pub field: Option<String>,
    /// RFC 3161 timestamp authority to get a timestamp from; empty for none.
    pub timestamp_url: String,
}

fn win_ansi(text: &str) -> Vec<u8> {
    text.chars().map(|c| crate::encoding::win_ansi_byte(c).unwrap_or(b'?')).collect()
}

fn escaped(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.push(b'(');
    for b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(*b);
    }
    out.push(b')');
    out
}

fn helv_width(text: &str, bold: bool, size: f32) -> f32 {
    let f = if bold { crate::fonts::Std14::HelveticaBold } else { crate::fonts::Std14::Helvetica };
    text.chars().map(|c| crate::fonts::std14_width(f, c)).sum::<f32>() / 1000.0 * size
}

/// The picture of a visible signature: the signer's name, then the facts.
fn appearance(doc: &mut Document, w: f32, h: f32, name: &str, lines: &[String]) -> ObjectId {
    let font = |base: &str| dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => base, "Encoding" => "WinAnsiEncoding" };
    let (regular, bold) = (doc.add_object(font("Helvetica")), doc.add_object(font("Helvetica-Bold")));
    let pad = (h * 0.1).clamp(2.0, 6.0);
    let rows = lines.len() as f32 + 1.6;
    let mut small = ((h - 2.0 * pad) / (rows * 1.25)).clamp(4.0, 9.0);
    let widest = lines.iter().map(|l| helv_width(l, false, 1.0)).fold(0.0f32, f32::max);
    if widest * small > w - 2.0 * pad {
        small = ((w - 2.0 * pad) / widest).max(3.0);
    }
    let mut big = small * 1.5;
    let name_w = helv_width(name, true, 1.0);
    if name_w * big > w - 2.0 * pad {
        big = ((w - 2.0 * pad) / name_w).max(3.0);
    }
    let mut c: Vec<u8> = Vec::new();
    c.extend(format!("q 0.12 0.32 0.78 RG 0.75 w 0.375 0.375 {:.2} {:.2} re S Q\n", w - 0.75, h - 0.75).bytes());
    c.extend(format!("q 0.12 0.32 0.78 rg 0 0 {:.2} {:.2} re f Q\n", pad * 0.6, h).bytes());
    let x = pad * 1.6;
    let mut y = h - pad - big * 0.85;
    c.extend(format!("BT 0.07 0.08 0.1 rg /F2 {big:.2} Tf {x:.2} {y:.2} Td ").bytes());
    c.extend(escaped(&win_ansi(name)));
    c.extend(b" Tj ET\n");
    y -= big * 0.5;
    for l in lines {
        y -= small * 1.25;
        c.extend(format!("BT 0.25 0.27 0.32 rg /F1 {small:.2} Tf {x:.2} {y:.2} Td ").bytes());
        c.extend(escaped(&win_ansi(l)));
        c.extend(b" Tj ET\n");
    }
    let mut s = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "FormType" => 1,
            "BBox" => vec![0.into(), 0.into(), real(w), real(h)],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => regular, "F2" => bold } },
        },
        c,
    );
    let _ = s.compress();
    doc.add_object(s)
}

fn find_empty_field(doc: &Document, name: &str) -> Option<ObjectId> {
    let cat = doc.catalog().ok()?;
    let form = get(doc, cat, b"AcroForm").and_then(|o| dict(doc, o))?;
    let mut stack: Vec<(Object, String)> = get_array(doc, form, b"Fields")?.iter().map(|o| (o.clone(), String::new())).collect();
    while let Some((o, prefix)) = stack.pop() {
        let Some(f) = dict(doc, &o) else { continue };
        let own = text(doc, f, b"T");
        let full = if prefix.is_empty() { own } else if own.is_empty() { prefix.clone() } else { format!("{prefix}.{own}") };
        if let Some(kids) = get_array(doc, f, b"Kids") {
            stack.extend(kids.iter().map(|k| (k.clone(), full.clone())));
        }
        if get_name(doc, f, b"FT").as_deref() == Some("Sig") && !f.has(b"V") && full == name {
            return o.as_reference().ok();
        }
    }
    None
}

fn acroform_id(doc: &mut Document) -> Result<ObjectId, String> {
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(err)?;
    let existing = doc.get_dictionary(root).map_err(err)?.get(b"AcroForm").ok().cloned();
    Ok(match existing {
        Some(Object::Reference(id)) if doc.get_dictionary(id).is_ok() => id,
        other => {
            let d = match other {
                Some(Object::Dictionary(d)) => d,
                _ => dictionary! { "Fields" => Vec::<Object>::new() },
            };
            let id = doc.add_object(d);
            doc.get_dictionary_mut(root).map_err(err)?.set("AcroForm", Object::Reference(id));
            id
        }
    })
}

/// Adds a signature to the document with room for its value. `signer` is the
/// name on the certificate and `date` a PDF date.
pub fn add_signature(doc: &mut Document, opts: &SignOptions, signer: &str, date: &str) -> Result<(), String> {
    let mut sig = dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => "ETSI.CAdES.detached",
        "ByteRange" => vec![Object::Integer(0), Object::Integer(RANGE_PLACEHOLDER), Object::Integer(RANGE_PLACEHOLDER), Object::Integer(RANGE_PLACEHOLDER)],
        "Contents" => Object::String(vec![0u8; CONTENTS_BYTES], StringFormat::Hexadecimal),
        "M" => Object::string_literal(date),
    };
    for (key, value) in [("Name", signer), ("Reason", opts.reason.as_str()), ("Location", opts.location.as_str()), ("ContactInfo", opts.contact.as_str())] {
        if !value.trim().is_empty() {
            sig.set(key, lopdf::text_string(value.trim()));
        }
    }
    let sig_id = doc.add_object(sig);
    let form_id = acroform_id(doc)?;

    let existing = opts.field.as_deref().filter(|n| !n.is_empty()).map(|n| find_empty_field(doc, n).ok_or(format!("There is no empty signature field named {n}."))).transpose()?;
    let (widget_id, rect) = match existing {
        Some(id) => {
            let f = doc.get_dictionary(id).map_err(err)?;
            // The widget is the field itself or its first kid.
            let widget = if f.has(b"Rect") { id } else { get_array(doc, f, b"Kids").and_then(|k| k.first()).and_then(|k| k.as_reference().ok()).unwrap_or(id) };
            let r = doc.get_dictionary(widget).ok().and_then(|w| w.get(b"Rect").ok()).and_then(|r| rect(doc, r));
            doc.get_dictionary_mut(id).map_err(err)?.set("V", Object::Reference(sig_id));
            (widget, r)
        }
        None => {
            let page_id = pages::page_id(doc, opts.page)?;
            let taken: Vec<String> = crate::forms::fields(doc).into_iter().map(|f| f.name).collect();
            let name = (1..).map(|n| format!("Signature{n}")).find(|n| !taken.contains(n)).unwrap();
            let r = opts.rect.map(|r| [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])]);
            let widget = doc.add_object(dictionary! {
                "Type" => "Annot", "Subtype" => "Widget", "FT" => "Sig",
                "T" => lopdf::text_string(&name),
                "F" => 132,
                "P" => page_id,
                "Rect" => r.map(|r| r.iter().map(|v| real(*v)).collect::<Vec<_>>()).unwrap_or_else(|| vec![0.into(), 0.into(), 0.into(), 0.into()]),
                "V" => sig_id,
            });
            let mut annots = crate::annots::annots_array(doc, page_id);
            annots.push(Object::Reference(widget));
            crate::annots::set_annots(doc, page_id, annots)?;
            let form = doc.get_dictionary_mut(form_id).map_err(err)?;
            match form.get_mut(b"Fields") {
                Ok(Object::Array(a)) => a.push(Object::Reference(widget)),
                Ok(Object::Reference(fields)) => {
                    let fields = *fields;
                    if let Ok(Object::Array(a)) = doc.get_object_mut(fields) {
                        a.push(Object::Reference(widget));
                    }
                }
                _ => form.set("Fields", vec![Object::Reference(widget)]),
            }
            (widget, r)
        }
    };
    doc.get_dictionary_mut(form_id).map_err(err)?.set("SigFlags", 3);

    // The appearance: a picture for a visible signature, an empty one otherwise.
    let ap = match rect.filter(|r| r[2] - r[0] > 2.0 && r[3] - r[1] > 2.0) {
        Some(r) => {
            let mut lines = vec![format!("Date: {}", iso_from_pdf(date).replace('T', " "))];
            if !opts.reason.trim().is_empty() {
                lines.push(format!("Reason: {}", opts.reason.trim()));
            }
            if !opts.location.trim().is_empty() {
                lines.push(format!("Location: {}", opts.location.trim()));
            }
            lines.insert(0, "Digitally signed".to_string());
            appearance(doc, r[2] - r[0], r[3] - r[1], signer, &lines)
        }
        None => doc.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 0.into(), 0.into()] }, Vec::new())),
    };
    doc.get_dictionary_mut(widget_id).map_err(err)?.set("AP", dictionary! { "N" => ap });
    Ok(())
}

fn find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// Fills in the byte range and the signature value of the signature that
/// `add_signature` left open. `sign` gets the two signed parts of the file
/// and returns the CMS signature over them.
pub fn finish(mut bytes: Vec<u8>, sign: impl FnOnce(&[u8], &[u8]) -> Result<Vec<u8>, String>) -> Result<Vec<u8>, String> {
    let mut hole = vec![b'<'];
    hole.extend(std::iter::repeat(b'0').take(CONTENTS_BYTES * 2));
    hole.push(b'>');
    let lt = find_last(&bytes, &hole).ok_or("The signature could not be placed in the saved file.")?;
    let gt = lt + hole.len() - 1;
    let ph = RANGE_PLACEHOLDER.to_string();
    let pattern = format!("{ph} {ph} {ph}");
    let at = find_last(&bytes, pattern.as_bytes()).ok_or("The signed range could not be placed in the saved file.")?;
    // The array as written: "[0 P P P]".
    let open = bytes[..at].iter().rposition(|b| *b == b'[').ok_or("The signed range is malformed.")?;
    let close = at + pattern.len() + bytes[at + pattern.len()..].iter().position(|b| *b == b']').ok_or("The signed range is malformed.")?;
    let range = [0, lt, gt + 1, bytes.len() - (gt + 1)];
    let mut written = format!("[{} {} {} {}", range[0], range[1], range[2], range[3]).into_bytes();
    let room = close - open;
    if written.len() > room {
        return Err("The signed range does not fit.".into());
    }
    written.resize(room, b' ');
    bytes[open..close].copy_from_slice(&written);

    let cms = sign(&bytes[..lt], &bytes[gt + 1..])?;
    if cms.len() > CONTENTS_BYTES {
        return Err(format!("The signature is larger than the room reserved for it ({} bytes).", cms.len()));
    }
    let hex: Vec<u8> = cms.iter().flat_map(|b| format!("{b:02X}").into_bytes()).collect();
    bytes[lt + 1..lt + 1 + hex.len()].copy_from_slice(&hex);
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Identities
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    /// SHA-1 thumbprint of the certificate, in hex.
    pub thumbprint: String,
    pub name: String,
    pub subject: String,
    pub issuer: String,
    pub valid_to: String,
    pub expired: bool,
    /// Issued by itself: readers will show it as unverified until they trust it.
    pub self_signed: bool,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IdentityRef {
    /// A certificate with its key in the Windows certificate store.
    Store { thumbprint: String },
    /// A PKCS #12 file.
    File { path: String, password: String },
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewIdentity {
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub organisation: String,
    #[serde(default)]
    pub country: String,
    pub years: u32,
    pub path: String,
    pub password: String,
}

pub use verify::{create_identity, file_identity, sign_with, store_identities, timestamp};

// ---------------------------------------------------------------------------
// Windows cryptography
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod verify {
    use super::*;
    use std::ffi::c_void;
    use windows::core::{HSTRING, PCSTR, PCWSTR, PSTR, PWSTR};
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::Security::Cryptography::*;

    const ENCODING: CERT_QUERY_ENCODING_TYPE = CERT_QUERY_ENCODING_TYPE(X509_ASN_ENCODING.0 | PKCS_7_ASN_ENCODING.0);
    const OID_SHA256: &[u8] = b"2.16.840.1.101.3.4.2.1\0";
    const OID_SIGNING_CERT_V2: &[u8] = b"1.2.840.113549.1.9.16.2.47\0";
    const OID_SIGNING_TIME: &str = "1.2.840.113549.1.9.5";
    const OID_TIMESTAMP_TOKEN: &str = "1.2.840.113549.1.9.16.2.14";
    const OID_RSA_SHA256: &[u8] = b"1.2.840.113549.1.1.11\0";

    struct Msg(*mut c_void);
    impl Drop for Msg {
        fn drop(&mut self) {
            unsafe {
                let _ = CryptMsgClose(Some(self.0));
            }
        }
    }
    struct StoreHandle(HCERTSTORE);
    impl Drop for StoreHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CertCloseStore(Some(self.0), 0);
            }
        }
    }
    struct Cert(*const CERT_CONTEXT);
    impl Drop for Cert {
        fn drop(&mut self) {
            unsafe {
                let _ = CertFreeCertificateContext(Some(self.0));
            }
        }
    }

    fn msg_param(msg: &Msg, param: u32, index: u32) -> Option<Vec<u8>> {
        unsafe {
            let mut size = 0u32;
            CryptMsgGetParam(msg.0, param, index, None, &mut size).ok()?;
            // Align for the structures some parameters return.
            let mut buf: Vec<u64> = vec![0; (size as usize).div_ceil(8) + 1];
            CryptMsgGetParam(msg.0, param, index, Some(buf.as_mut_ptr() as *mut c_void), &mut size).ok()?;
            let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, size as usize).to_vec();
            Some(bytes)
        }
    }

    /// Like `msg_param`, for parameters that are structures with pointers into the same buffer.
    fn msg_struct(msg: &Msg, param: u32) -> Option<Vec<u64>> {
        unsafe {
            let mut size = 0u32;
            CryptMsgGetParam(msg.0, param, 0, None, &mut size).ok()?;
            let mut buf: Vec<u64> = vec![0; (size as usize).div_ceil(8) + 1];
            CryptMsgGetParam(msg.0, param, 0, Some(buf.as_mut_ptr() as *mut c_void), &mut size).ok()?;
            Some(buf)
        }
    }

    fn cert_name(cert: *const CERT_CONTEXT, kind: u32, flags: u32) -> String {
        unsafe {
            let para = CERT_X500_NAME_STR.0;
            let para_ptr = if kind == CERT_NAME_RDN_TYPE { Some(&para as *const u32 as *const c_void) } else { None };
            let n = CertGetNameStringW(cert, kind, flags, para_ptr, None);
            if n <= 1 {
                return String::new();
            }
            let mut buf = vec![0u16; n as usize];
            CertGetNameStringW(cert, kind, flags, para_ptr, Some(&mut buf));
            String::from_utf16_lossy(&buf[..buf.len().saturating_sub(1)])
        }
    }

    /// Days since 1970-01-01 to a calendar date.
    fn civil(days: i64) -> (i64, u32, u32) {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (yoe + era * 400 + (m <= 2) as i64, m, d)
    }

    fn filetime_iso(ft: FILETIME) -> String {
        let ticks = ((ft.dwHighDateTime as u64) << 32 | ft.dwLowDateTime as u64) as i64;
        let secs = ticks / 10_000_000 - 11_644_473_600;
        let (y, m, d) = civil(secs.div_euclid(86_400));
        let s = secs.rem_euclid(86_400);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
    }

    fn thumbprint(cert: *const CERT_CONTEXT) -> String {
        unsafe {
            let mut hash = [0u8; 20];
            let mut size = 20u32;
            if CertGetCertificateContextProperty(cert, CERT_SHA1_HASH_PROP_ID, Some(hash.as_mut_ptr() as *mut c_void), &mut size).is_err() {
                return String::new();
            }
            hash.iter().map(|b| format!("{b:02X}")).collect()
        }
    }

    fn hash(alg: PCWSTR, data: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let mut out = [0u8; 64];
            let mut size = 64u32;
            CryptHashCertificate2(alg, 0, None, Some(data), Some(out.as_mut_ptr()), &mut size).ok()?;
            Some(out[..size as usize].to_vec())
        }
    }

    fn digest_name(oid: &str) -> (&'static str, Option<PCWSTR>) {
        match oid {
            "1.3.14.3.2.26" => ("SHA-1", Some(BCRYPT_SHA1_ALGORITHM)),
            "2.16.840.1.101.3.4.2.1" => ("SHA-256", Some(BCRYPT_SHA256_ALGORITHM)),
            "2.16.840.1.101.3.4.2.2" => ("SHA-384", Some(BCRYPT_SHA384_ALGORITHM)),
            "2.16.840.1.101.3.4.2.3" => ("SHA-512", Some(BCRYPT_SHA512_ALGORITHM)),
            "1.2.840.113549.2.5" => ("MD5", Some(BCRYPT_MD5_ALGORITHM)),
            _ => ("unknown", None),
        }
    }

    fn oid_string(der: &[u8]) -> String {
        let mut parts: Vec<u64> = Vec::new();
        let mut value = 0u64;
        for b in der {
            value = value << 7 | (*b & 0x7f) as u64;
            if b & 0x80 == 0 {
                if parts.is_empty() {
                    let first = (value / 40).min(2);
                    parts.push(first);
                    parts.push(value - first * 40);
                } else {
                    parts.push(value);
                }
                value = 0;
            }
        }
        parts.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(".")
    }

    /// "YYMMDDHHMMSSZ" or "YYYYMMDDHHMMSSZ" → ISO 8601.
    fn asn1_time(tag: u8, v: &[u8]) -> String {
        let s = String::from_utf8_lossy(v);
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        let full = if tag == 0x17 && digits.len() >= 12 {
            let yy: u32 = digits[0..2].parse().unwrap_or(0);
            format!("{}{}", if yy >= 50 { 1900 + yy } else { 2000 + yy }, &digits[2..])
        } else {
            digits
        };
        if full.len() < 14 {
            return String::new();
        }
        format!("{}-{}-{}T{}:{}:{}Z", &full[0..4], &full[4..6], &full[6..8], &full[8..10], &full[10..12], &full[12..14])
    }

    unsafe fn attributes(buf: &[u64]) -> Vec<(String, Vec<Vec<u8>>)> {
        let attrs = &*(buf.as_ptr() as *const CRYPT_ATTRIBUTES);
        let mut out = Vec::new();
        for i in 0..attrs.cAttr as usize {
            let a = &*attrs.rgAttr.add(i);
            let oid = a.pszObjId.to_string().unwrap_or_default();
            let mut values = Vec::new();
            for k in 0..a.cValue as usize {
                let v = &*a.rgValue.add(k);
                values.push(std::slice::from_raw_parts(v.pbData, v.cbData as usize).to_vec());
            }
            out.push((oid, values));
        }
        out
    }

    /// Whether this computer trusts the certificate, at `when` (now when `None`).
    unsafe fn chain_status(cert: *const CERT_CONTEXT, extra: Option<HCERTSTORE>, when: Option<FILETIME>) -> Option<(u32, Vec<*const CERT_CONTEXT>, *mut CERT_CHAIN_CONTEXT)> {
        let para = CERT_CHAIN_PARA { cbSize: std::mem::size_of::<CERT_CHAIN_PARA>() as u32, ..Default::default() };
        let mut chain: *mut CERT_CHAIN_CONTEXT = std::ptr::null_mut();
        CertGetCertificateChain(None, cert, when.as_ref().map(|t| t as *const FILETIME), extra, &para, 0, None, &mut chain).ok()?;
        if chain.is_null() {
            return None;
        }
        let mut certs = Vec::new();
        if (*chain).cChain > 0 {
            let simple = &**(*chain).rgpChain;
            for i in 0..simple.cElement as usize {
                certs.push((**simple.rgpElement.add(i)).pCertContext);
            }
        }
        Some(((*chain).TrustStatus.dwErrorStatus, certs, chain))
    }

    fn describe_trust(cert: *const CERT_CONTEXT, extra: Option<HCERTSTORE>, signed_at: Option<FILETIME>, info: &mut SigInfo) {
        unsafe {
            let Some((status, _, chain)) = chain_status(cert, extra, None) else {
                info.trust = "unknown".into();
                info.trust_detail = "Windows could not build a certificate chain.".into();
                return;
            };
            CertFreeCertificateChain(chain);
            let self_signed = cert_name(cert, CERT_NAME_RDN_TYPE, 0) == cert_name(cert, CERT_NAME_RDN_TYPE, CERT_NAME_ISSUER_FLAG);
            let (trust, detail) = if status & CERT_TRUST_IS_REVOKED != 0 {
                ("revoked", "The certificate has been revoked by its issuer.".to_string())
            } else if status & (CERT_TRUST_IS_UNTRUSTED_ROOT | CERT_TRUST_IS_PARTIAL_CHAIN) != 0 {
                if self_signed {
                    ("untrusted", "A self-signed certificate: it proves the document is unchanged, but nobody vouches for who the signer is.".to_string())
                } else {
                    ("untrusted", "The certificate's issuer is not among the authorities this computer trusts.".to_string())
                }
            } else if status & CERT_TRUST_IS_NOT_TIME_VALID != 0 {
                // Valid when it was used?
                let then = signed_at.and_then(|t| chain_status(cert, extra, Some(t)));
                let ok_then = then.as_ref().map(|(s, _, _)| *s == 0).unwrap_or(false);
                if let Some((_, _, c)) = then {
                    CertFreeCertificateChain(c);
                }
                if ok_then {
                    ("trusted", "The certificate has expired since, but was valid at the time the signature states.".to_string())
                } else {
                    ("expired", "The certificate is outside its validity period.".to_string())
                }
            } else if status != 0 {
                ("untrusted", format!("Windows reports a problem with the certificate chain (status 0x{status:X})."))
            } else {
                ("trusted", "Issued by an authority this computer trusts. Revocation was not checked online.".to_string())
            };
            info.trust = trust.into();
            info.trust_detail = detail;
        }
    }

    fn iso_filetime(iso: &str) -> Option<FILETIME> {
        // "YYYY-MM-DDTHH:MM:SS…"
        let n = |a: usize, b: usize| iso.get(a..b).and_then(|s| s.parse::<i64>().ok());
        let (y, m, d, hh, mm, ss) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
        // Days from civil date.
        let yy = if m <= 2 { y - 1 } else { y };
        let era = yy.div_euclid(400);
        let yoe = yy.rem_euclid(400);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let secs = days * 86_400 + hh * 3600 + mm * 60 + ss;
        let ticks = ((secs + 11_644_473_600) * 10_000_000) as u64;
        Some(FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 })
    }

    /// Verifies a signature value over the signed parts of the file and fills in `info`.
    pub fn check(der: &[u8], parts: &[&[u8]], sub_filter: &str, info: &mut SigInfo) {
        let detached = matches!(sub_filter, "adbe.pkcs7.detached" | "ETSI.CAdES.detached");
        let sha1_wrapped = sub_filter == "adbe.pkcs7.sha1";
        let timestamp = sub_filter == "ETSI.RFC3161";
        if !detached && !sha1_wrapped && !timestamp {
            info.problem = format!("Signatures of the kind {sub_filter} cannot be checked yet.");
            return;
        }
        unsafe {
            let handle = CryptMsgOpenToDecode(ENCODING.0, if detached { CMSG_DETACHED_FLAG } else { 0 }, 0, None, None, None);
            if handle.is_null() {
                info.problem = "Windows could not read the signature.".into();
                return;
            }
            let msg = Msg(handle);
            if CryptMsgUpdate(msg.0, Some(der), true).is_err() {
                info.problem = "The signature value is not a valid CMS message.".into();
                return;
            }
            if detached {
                // The signed content, in pieces small enough for a 32-bit length.
                let chunks: Vec<&[u8]> = parts.iter().flat_map(|p| p.chunks(1 << 30)).collect();
                for (i, c) in chunks.iter().enumerate() {
                    if CryptMsgUpdate(msg.0, Some(c), i + 1 == chunks.len()).is_err() {
                        info.problem = "Windows could not hash the signed content.".into();
                        return;
                    }
                }
            }

            if let Some(alg) = msg_struct(&msg, CMSG_SIGNER_HASH_ALGORITHM_PARAM) {
                let a = &*(alg.as_ptr() as *const CRYPT_ALGORITHM_IDENTIFIER);
                let oid = a.pszObjId.to_string().unwrap_or_default();
                info.digest = digest_name(&oid).0.to_string();
            }
            if let Some(buf) = msg_struct(&msg, CMSG_SIGNER_AUTH_ATTR_PARAM) {
                for (oid, values) in attributes(&buf) {
                    if oid == OID_SIGNING_TIME && info.time.is_empty() {
                        if let Some(v) = values.first() {
                            if let Some((tag, h, l)) = der_head(v) {
                                info.time = asn1_time(tag, v.get(h..h + l).unwrap_or(&[]));
                            }
                        }
                    }
                }
            }
            if let Some(buf) = msg_struct(&msg, CMSG_SIGNER_UNAUTH_ATTR_PARAM) {
                info.timestamped = attributes(&buf).iter().any(|(oid, _)| oid == OID_TIMESTAMP_TOKEN);
            }

            // The signer's certificate travels in the message.
            let Some(cert_info) = msg_struct(&msg, CMSG_SIGNER_CERT_INFO_PARAM) else {
                info.problem = "The signature does not identify its signer.".into();
                return;
            };
            let Ok(store) = CertOpenStore(CERT_STORE_PROV_MSG, ENCODING, None, CERT_OPEN_STORE_FLAGS(0), Some(msg.0 as *const c_void)) else {
                info.problem = "The certificates in the signature could not be read.".into();
                return;
            };
            let store = StoreHandle(store);
            let found = CertGetSubjectCertificateFromStore(store.0, ENCODING, cert_info.as_ptr() as *const CERT_INFO);
            if found.is_null() {
                info.problem = "The signer's certificate is not included in the signature.".into();
                return;
            }
            let cert = Cert(found);
            let ci = &*(*cert.0).pCertInfo;
            info.signer = cert_name(cert.0, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0);
            info.subject = cert_name(cert.0, CERT_NAME_RDN_TYPE, 0);
            info.issuer = cert_name(cert.0, CERT_NAME_SIMPLE_DISPLAY_TYPE, CERT_NAME_ISSUER_FLAG);
            info.valid_from = filetime_iso(ci.NotBefore);
            info.valid_to = filetime_iso(ci.NotAfter);
            let serial = std::slice::from_raw_parts(ci.SerialNumber.pbData, ci.SerialNumber.cbData as usize);
            info.serial = serial.iter().rev().map(|b| format!("{b:02X}")).collect();

            let mut intact = CryptMsgControl(msg.0, 0, CMSG_CTRL_VERIFY_SIGNATURE, Some((*cert.0).pCertInfo as *const c_void)).is_ok();
            if intact && !detached {
                // The message signs a digest of the file (or a timestamp of one): compare it.
                let mut content = msg_param(&msg, CMSG_CONTENT_PARAM, 0).unwrap_or_default();
                let whole: Vec<u8> = parts.concat();
                if sha1_wrapped {
                    if content.len() != 20 {
                        // Some writers wrap the digest in an OCTET STRING once more.
                        if let Some((0x04, h, l)) = der_head(&content) {
                            content = content.get(h..h + l).unwrap_or(&[]).to_vec();
                        }
                    }
                    intact = hash(BCRYPT_SHA1_ALGORITHM, &whole).map(|h| h == content).unwrap_or(false);
                } else {
                    if let Some((0x04, h, l)) = der_head(&content) {
                        content = content.get(h..h + l).unwrap_or(&[]).to_vec();
                    }
                    // TSTInfo ::= SEQUENCE { version, policy, messageImprint { algorithm, hashedMessage }, serial, genTime, … }
                    let fields = der_children(&content);
                    let imprint = der_items(fields.get(2).map(|(_, v)| *v).unwrap_or(&[]));
                    let oid = imprint.first().and_then(|(_, alg)| der_items(alg).first().map(|(_, o)| oid_string(o))).unwrap_or_default();
                    let hashed = imprint.get(1).map(|(_, v)| v.to_vec()).unwrap_or_default();
                    let (name, alg) = digest_name(&oid);
                    info.digest = name.to_string();
                    intact = alg.and_then(|a| hash(a, &whole)).map(|h| h == hashed).unwrap_or(false);
                    if let Some((tag, v)) = fields.get(4) {
                        info.time = asn1_time(*tag, v);
                    }
                    info.timestamped = true;
                }
            }
            info.intact = Some(intact);
            if !intact {
                info.problem = "The document was changed after it was signed, or the signature is damaged.".into();
            }
            describe_trust(cert.0, Some(store.0), iso_filetime(&info.time), info);
        }
    }

    unsafe fn has_key(cert: *const CERT_CONTEXT) -> bool {
        [CERT_KEY_PROV_INFO_PROP_ID, CERT_KEY_CONTEXT_PROP_ID, CERT_NCRYPT_KEY_HANDLE_PROP_ID].iter().any(|prop| {
            let mut size = 0u32;
            CertGetCertificateContextProperty(cert, *prop, None, &mut size).is_ok()
        })
    }

    unsafe fn identity_of(cert: *const CERT_CONTEXT) -> Identity {
        let ci = &*(*cert).pCertInfo;
        let subject = cert_name(cert, CERT_NAME_RDN_TYPE, 0);
        let issuer_full = cert_name(cert, CERT_NAME_RDN_TYPE, CERT_NAME_ISSUER_FLAG);
        Identity {
            thumbprint: thumbprint(cert),
            name: cert_name(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0),
            self_signed: subject == issuer_full,
            subject,
            issuer: cert_name(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, CERT_NAME_ISSUER_FLAG),
            valid_to: filetime_iso(ci.NotAfter),
            expired: CertVerifyTimeValidity(None, ci) != 0,
        }
    }

    /// Whether a certificate may sign documents: no key usage restriction, or one that allows signing.
    unsafe fn may_sign(cert: *const CERT_CONTEXT) -> bool {
        let mut usage = [0u8; 2];
        if CertGetIntendedKeyUsage(ENCODING, (*cert).pCertInfo, &mut usage).is_err() {
            return true;
        }
        // digitalSignature (0x80) or nonRepudiation (0x40)
        usage[0] == 0 || usage[0] & 0xC0 != 0
    }

    /// Certificates with a private key in the user's Windows certificate store.
    pub fn store_identities() -> Result<Vec<Identity>, String> {
        unsafe {
            let store = CertOpenSystemStoreW(None, windows::core::w!("MY")).map_err(|e| format!("The Windows certificate store could not be opened: {}", e.message()))?;
            let store = StoreHandle(store);
            let mut out = Vec::new();
            let mut cert: *mut CERT_CONTEXT = std::ptr::null_mut();
            loop {
                cert = CertEnumCertificatesInStore(store.0, if cert.is_null() { None } else { Some(cert as *const CERT_CONTEXT) });
                if cert.is_null() {
                    break;
                }
                if has_key(cert) && may_sign(cert) {
                    out.push(identity_of(cert));
                }
            }
            out.sort_by(|a, b| a.expired.cmp(&b.expired).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
            Ok(out)
        }
    }

    /// Opens a PKCS #12 file without installing anything; returns its store and the certificate that has the key.
    unsafe fn open_pfx(path: &str, password: &str) -> Result<(StoreHandle, *const CERT_CONTEXT), String> {
        let mut data = std::fs::read(path).map_err(|e| format!("The file could not be read: {e}"))?;
        let blob = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_mut_ptr() };
        let store = PFXImportCertStore(&blob, &HSTRING::from(password), CRYPT_KEY_FLAGS(PKCS12_NO_PERSIST_KEY.0 | PKCS12_ALWAYS_CNG_KSP.0)).map_err(|e| {
            if e.code().0 as u32 == 0x8007_0056 {
                "That password does not open the file.".to_string()
            } else {
                format!("The file could not be opened as a digital ID: {}", e.message())
            }
        })?;
        let store = StoreHandle(store);
        let mut cert: *mut CERT_CONTEXT = std::ptr::null_mut();
        loop {
            cert = CertEnumCertificatesInStore(store.0, if cert.is_null() { None } else { Some(cert as *const CERT_CONTEXT) });
            if cert.is_null() {
                return Err("The file holds no certificate with a private key.".into());
            }
            if has_key(cert) {
                // Keep it alive beyond the enumeration.
                let kept = CertDuplicateCertificateContext(Some(cert as *const CERT_CONTEXT));
                let _ = CertFreeCertificateContext(Some(cert as *const CERT_CONTEXT));
                return Ok((store, kept as *const CERT_CONTEXT));
            }
        }
    }

    pub fn file_identity(path: &str, password: &str) -> Result<Identity, String> {
        unsafe {
            let (_store, cert) = open_pfx(path, password)?;
            let cert = Cert(cert);
            Ok(identity_of(cert.0))
        }
    }

    unsafe fn open_identity(id: &IdentityRef) -> Result<(StoreHandle, Cert), String> {
        match id {
            IdentityRef::File { path, password } => {
                let (store, cert) = open_pfx(path, password)?;
                Ok((store, Cert(cert)))
            }
            IdentityRef::Store { thumbprint: want } => {
                let store = CertOpenSystemStoreW(None, windows::core::w!("MY")).map_err(|e| format!("The Windows certificate store could not be opened: {}", e.message()))?;
                let store = StoreHandle(store);
                let mut cert: *mut CERT_CONTEXT = std::ptr::null_mut();
                loop {
                    cert = CertEnumCertificatesInStore(store.0, if cert.is_null() { None } else { Some(cert as *const CERT_CONTEXT) });
                    if cert.is_null() {
                        return Err("That certificate is no longer in the Windows certificate store.".into());
                    }
                    if thumbprint(cert).eq_ignore_ascii_case(want) {
                        let kept = CertDuplicateCertificateContext(Some(cert as *const CERT_CONTEXT));
                        let _ = CertFreeCertificateContext(Some(cert as *const CERT_CONTEXT));
                        return Ok((store, Cert(kept as *const CERT_CONTEXT)));
                    }
                }
            }
        }
    }

    /// Opens an identity and hands its description and a signing function to `f`.
    pub fn sign_with<R>(id: &IdentityRef, f: impl FnOnce(&Identity, &dyn Fn(&[u8], &[u8]) -> Result<Vec<u8>, String>) -> Result<R, String>) -> Result<R, String> {
        unsafe {
            let (store, cert) = open_identity(id)?;
            let identity = identity_of(cert.0);
            if identity.expired {
                return Err("That certificate has expired (or is not valid yet).".into());
            }
            let cert_ptr = cert.0;
            let store_handle = store.0;
            let sign = move |a: &[u8], b: &[u8]| -> Result<Vec<u8>, String> { cms_sign(cert_ptr, store_handle, &[a, b]) };
            f(&identity, &sign)
        }
    }

    /// A detached CMS signature over `parts`, SHA-256, with the signing certificate bound in (PAdES baseline).
    unsafe fn cms_sign(cert: *const CERT_CONTEXT, store: HCERTSTORE, parts: &[&[u8]]) -> Result<Vec<u8>, String> {
        let cert_der = std::slice::from_raw_parts((*cert).pbCertEncoded, (*cert).cbCertEncoded as usize);
        let cert_hash = hash(BCRYPT_SHA256_ALGORITHM, cert_der).ok_or("The certificate could not be hashed.")?;
        // SigningCertificateV2 ::= SEQUENCE { certs SEQUENCE OF ESSCertIDv2 { certHash OCTET STRING } }
        let mut ess = vec![0x30, 0x26, 0x30, 0x24, 0x30, 0x22, 0x04, 0x20];
        ess.extend(&cert_hash);
        let mut ess_blob = CRYPT_INTEGER_BLOB { cbData: ess.len() as u32, pbData: ess.as_mut_ptr() };
        let mut attr = CRYPT_ATTRIBUTE { pszObjId: PSTR(OID_SIGNING_CERT_V2.as_ptr() as *mut u8), cValue: 1, rgValue: &mut ess_blob };

        // The certificates that lead to the issuer travel with the signature.
        let chain = chain_status(cert, Some(store), None);
        let mut certs: Vec<*mut CERT_CONTEXT> = match &chain {
            Some((_, list, _)) if !list.is_empty() => list.iter().map(|c| *c as *mut CERT_CONTEXT).collect(),
            _ => vec![cert as *mut CERT_CONTEXT],
        };
        let para = CRYPT_SIGN_MESSAGE_PARA {
            cbSize: std::mem::size_of::<CRYPT_SIGN_MESSAGE_PARA>() as u32,
            dwMsgEncodingType: ENCODING.0,
            pSigningCert: cert,
            HashAlgorithm: CRYPT_ALGORITHM_IDENTIFIER { pszObjId: PSTR(OID_SHA256.as_ptr() as *mut u8), Parameters: Default::default() },
            pvHashAuxInfo: std::ptr::null_mut(),
            cMsgCert: certs.len() as u32,
            rgpMsgCert: certs.as_mut_ptr(),
            cMsgCrl: 0,
            rgpMsgCrl: std::ptr::null_mut(),
            cAuthAttr: 1,
            rgAuthAttr: &mut attr,
            cUnauthAttr: 0,
            rgUnauthAttr: std::ptr::null_mut(),
            dwFlags: 0,
            dwInnerContentType: 0,
        };
        let pointers: Vec<*const u8> = parts.iter().map(|p| p.as_ptr()).collect();
        let lengths: Vec<u32> = parts.iter().map(|p| p.len() as u32).collect();
        if parts.iter().any(|p| p.len() > u32::MAX as usize) {
            return Err("The document is too large to sign in one piece.".into());
        }
        let mut size = 0u32;
        let result = (|| {
            CryptSignMessage(&para, true, parts.len() as u32, Some(pointers.as_ptr()), lengths.as_ptr(), None, &mut size)?;
            let mut out = vec![0u8; size as usize];
            CryptSignMessage(&para, true, parts.len() as u32, Some(pointers.as_ptr()), lengths.as_ptr(), Some(out.as_mut_ptr()), &mut size)?;
            out.truncate(size as usize);
            Ok::<Vec<u8>, windows::core::Error>(out)
        })();
        if let Some((_, _, c)) = chain {
            CertFreeCertificateChain(c);
        }
        result.map_err(|e| {
            let code = e.code().0 as u32;
            if code == 0x8009_0016 || code == 0x8009_000B {
                "The private key of that certificate is not available on this computer.".to_string()
            } else if code == 0x8010_006E || code == 0x8009_0036 {
                "Signing was cancelled.".to_string()
            } else {
                format!("Windows could not sign: {}", e.message())
            }
        })
    }

    /// Asks a timestamp authority (RFC 3161) to confirm when the signature
    /// in `cms` was made, and stores its answer in the signature as an
    /// unsigned attribute (PAdES B-T). Only a hash of the signature value
    /// leaves the computer.
    pub fn timestamp(cms: Vec<u8>, url: &str) -> Result<Vec<u8>, String> {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("The timestamp server address must start with http:// or https://.".into());
        }
        unsafe {
            let handle = CryptMsgOpenToDecode(ENCODING.0, CMSG_DETACHED_FLAG, 0, None, None, None);
            if handle.is_null() {
                return Err("Windows could not read the new signature.".into());
            }
            let msg = Msg(handle);
            CryptMsgUpdate(msg.0, Some(&cms), true).map_err(|e| format!("Windows could not read the new signature: {}", e.message()))?;
            let value = msg_param(&msg, CMSG_ENCRYPTED_DIGEST, 0).ok_or("The signature value could not be read.")?;
            let mut nonce = [0u8; 8];
            let _ = getrandom::fill(&mut nonce);
            nonce[7] &= 0x7f;
            let para = CRYPT_TIMESTAMP_PARA {
                pszTSAPolicyId: PCSTR::null(),
                fRequestCerts: true.into(),
                Nonce: CRYPT_INTEGER_BLOB { cbData: nonce.len() as u32, pbData: nonce.as_mut_ptr() },
                cExtension: 0,
                rgExtension: std::ptr::null_mut(),
            };
            let mut context: *mut CRYPT_TIMESTAMP_CONTEXT = std::ptr::null_mut();
            CryptRetrieveTimeStamp(&HSTRING::from(url), TIMESTAMP_VERIFY_CONTEXT_SIGNATURE, 20_000, PCSTR(OID_SHA256.as_ptr()), Some(&para), &value, &mut context, std::ptr::null_mut(), None)
                .map_err(|e| format!("The timestamp server did not give a timestamp ({}). Check the address in Settings, or clear it to sign without one.", e.message().trim()))?;
            if context.is_null() {
                return Err("The timestamp server gave an empty answer.".into());
            }
            let token = std::slice::from_raw_parts((*context).pbEncoded, (*context).cbEncoded as usize).to_vec();
            CryptMemFree(Some(context as *const c_void));

            // Attribute ::= SEQUENCE { id-aa-signatureTimeStampToken, SET { token } }
            let len = |n: usize| -> Vec<u8> {
                if n < 0x80 {
                    vec![n as u8]
                } else if n < 0x100 {
                    vec![0x81, n as u8]
                } else {
                    vec![0x82, (n >> 8) as u8, n as u8]
                }
            };
            let oid = [0x06, 0x0B, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x10, 0x02, 0x0E];
            let mut set = vec![0x31];
            set.extend(len(token.len()));
            set.extend(&token);
            let mut attr = vec![0x30];
            attr.extend(len(oid.len() + set.len()));
            attr.extend(oid);
            attr.extend(set);
            let add = CMSG_CTRL_ADD_SIGNER_UNAUTH_ATTR_PARA {
                cbSize: std::mem::size_of::<CMSG_CTRL_ADD_SIGNER_UNAUTH_ATTR_PARA>() as u32,
                dwSignerIndex: 0,
                blob: CRYPT_INTEGER_BLOB { cbData: attr.len() as u32, pbData: attr.as_mut_ptr() },
            };
            CryptMsgControl(msg.0, 0, CMSG_CTRL_ADD_SIGNER_UNAUTH_ATTR, Some(&add as *const _ as *const c_void))
                .map_err(|e| format!("The timestamp could not be stored in the signature: {}", e.message()))?;
            msg_param(&msg, CMSG_ENCODED_MESSAGE, 0).ok_or_else(|| "The timestamped signature could not be written.".to_string())
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn quoted(s: &str) -> String {
        format!("\"{}\"", s.trim().replace('"', "\"\""))
    }

    /// Makes a self-signed digital ID and saves it as a password-protected PKCS #12 file.
    pub fn create_identity(req: &NewIdentity) -> Result<Identity, String> {
        if req.name.trim().is_empty() {
            return Err("Enter the name the signature should show.".into());
        }
        if req.password.chars().count() < 4 {
            return Err("Protect the digital ID with a password of at least four characters.".into());
        }
        unsafe {
            let fail = |what: &str, e: windows::core::Error| format!("{what}: {}", e.message());
            let mut provider = NCRYPT_PROV_HANDLE::default();
            NCryptOpenStorageProvider(&mut provider, MS_KEY_STORAGE_PROVIDER, 0).map_err(|e| fail("The Windows key store could not be opened", e))?;
            let mut tag = [0u8; 8];
            let _ = getrandom::fill(&mut tag);
            let container = wide(&format!("InqStudio-{}", tag.iter().map(|b| format!("{b:02x}")).collect::<String>()));
            let mut key = NCRYPT_KEY_HANDLE::default();
            let made = NCryptCreatePersistedKey(provider, &mut key, NCRYPT_RSA_ALGORITHM, PCWSTR(container.as_ptr()), CERT_KEY_SPEC(0), NCRYPT_FLAGS(0));
            let _ = NCryptFreeObject(NCRYPT_HANDLE(provider.0));
            made.map_err(|e| fail("A key could not be created", e))?;
            // From here on the key must be deleted again, whatever happens.
            let result = (|| -> Result<Identity, String> {
                NCryptSetProperty(NCRYPT_HANDLE(key.0), NCRYPT_LENGTH_PROPERTY, &3072u32.to_le_bytes(), NCRYPT_FLAGS(0)).map_err(|e| fail("A key could not be created", e))?;
                let policy = NCRYPT_ALLOW_EXPORT_FLAG | NCRYPT_ALLOW_PLAINTEXT_EXPORT_FLAG;
                NCryptSetProperty(NCRYPT_HANDLE(key.0), NCRYPT_EXPORT_POLICY_PROPERTY, &policy.to_le_bytes(), NCRYPT_FLAGS(0)).map_err(|e| fail("A key could not be created", e))?;
                NCryptFinalizeKey(key, NCRYPT_FLAGS(0)).map_err(|e| fail("A key could not be created", e))?;

                let mut x500 = format!("CN={}", quoted(&req.name));
                if !req.email.trim().is_empty() {
                    x500.push_str(&format!(", E={}", quoted(&req.email)));
                }
                if !req.organisation.trim().is_empty() {
                    x500.push_str(&format!(", O={}", quoted(&req.organisation)));
                }
                if req.country.trim().len() == 2 {
                    x500.push_str(&format!(", C={}", req.country.trim().to_uppercase()));
                }
                let x500 = wide(&x500);
                let mut size = 0u32;
                CertStrToNameW(X509_ASN_ENCODING, PCWSTR(x500.as_ptr()), CERT_X500_NAME_STR, None, None, &mut size, None).map_err(|e| fail("The name could not be used", e))?;
                let mut name = vec![0u8; size as usize];
                CertStrToNameW(X509_ASN_ENCODING, PCWSTR(x500.as_ptr()), CERT_X500_NAME_STR, None, Some(name.as_mut_ptr()), &mut size, None).map_err(|e| fail("The name could not be used", e))?;
                let subject = CRYPT_INTEGER_BLOB { cbData: size, pbData: name.as_mut_ptr() };

                let mut provider_name = wide("Microsoft Software Key Storage Provider");
                let mut container = container.clone();
                let prov_info = CRYPT_KEY_PROV_INFO {
                    pwszContainerName: PWSTR(container.as_mut_ptr()),
                    pwszProvName: PWSTR(provider_name.as_mut_ptr()),
                    dwProvType: 0,
                    dwFlags: CRYPT_KEY_FLAGS(0),
                    cProvParam: 0,
                    rgProvParam: std::ptr::null_mut(),
                    dwKeySpec: 0,
                };
                let algorithm = CRYPT_ALGORITHM_IDENTIFIER { pszObjId: PSTR(OID_RSA_SHA256.as_ptr() as *mut u8), Parameters: Default::default() };
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                let stamp = |secs: i64| {
                    let (y, m, d) = civil(secs.div_euclid(86_400));
                    let s = secs.rem_euclid(86_400);
                    SYSTEMTIME { wYear: y as u16, wMonth: m as u16, wDayOfWeek: 0, wDay: d as u16, wHour: (s / 3600) as u16, wMinute: (s % 3600 / 60) as u16, wSecond: (s % 60) as u16, wMilliseconds: 0 }
                };
                let start = stamp(secs - 3600);
                let end = stamp(secs + req.years.clamp(1, 30) as i64 * 365 * 86_400);
                let made = CertCreateSelfSignCertificate(
                    Some(HCRYPTPROV_OR_NCRYPT_KEY_HANDLE(key.0)),
                    &subject,
                    CERT_CREATE_SELFSIGN_FLAGS(0),
                    Some(&prov_info),
                    Some(&algorithm),
                    Some(&start),
                    Some(&end),
                    None,
                );
                if made.is_null() {
                    return Err(format!("The certificate could not be created: {}", windows::core::Error::from_thread().message()));
                }
                let cert = Cert(made);
                let memory = CertOpenStore(CERT_STORE_PROV_MEMORY, CERT_QUERY_ENCODING_TYPE(0), None, CERT_OPEN_STORE_FLAGS(0), None).map_err(|e| fail("The digital ID could not be packed", e))?;
                let memory = StoreHandle(memory);
                CertAddCertificateContextToStore(Some(memory.0), cert.0, CERT_STORE_ADD_ALWAYS, None).map_err(|e| fail("The digital ID could not be packed", e))?;
                let flags = EXPORT_PRIVATE_KEYS | REPORT_NO_PRIVATE_KEY | REPORT_NOT_ABLE_TO_EXPORT_PRIVATE_KEY;
                let password = HSTRING::from(req.password.as_str());
                let mut blob = CRYPT_INTEGER_BLOB::default();
                PFXExportCertStoreEx(memory.0, &mut blob, &password, std::ptr::null(), flags).map_err(|e| fail("The digital ID could not be packed", e))?;
                let mut pfx = vec![0u8; blob.cbData as usize];
                blob.pbData = pfx.as_mut_ptr();
                PFXExportCertStoreEx(memory.0, &mut blob, &password, std::ptr::null(), flags).map_err(|e| fail("The digital ID could not be packed", e))?;
                pfx.truncate(blob.cbData as usize);
                std::fs::write(&req.path, &pfx).map_err(|e| format!("The digital ID could not be saved: {e}"))?;
                Ok(identity_of(cert.0))
            })();
            // The key lives on in the file only.
            let _ = NCryptDeleteKey(key, 0);
            result
        }
    }

    #[allow(dead_code)]
    fn _unused(_: PCSTR) {}
}

#[cfg(not(windows))]
mod verify {
    use super::*;

    const ONLY_WINDOWS: &str = "Signatures use the certificate services built into Windows.";

    pub fn check(_: &[u8], _: &[&[u8]], _: &str, info: &mut SigInfo) {
        info.problem = ONLY_WINDOWS.into();
    }

    pub fn store_identities() -> Result<Vec<Identity>, String> {
        Err(ONLY_WINDOWS.into())
    }

    pub fn timestamp(_: Vec<u8>, _: &str) -> Result<Vec<u8>, String> {
        Err(ONLY_WINDOWS.into())
    }

    pub fn file_identity(_: &str, _: &str) -> Result<Identity, String> {
        Err(ONLY_WINDOWS.into())
    }

    pub fn create_identity(_: &NewIdentity) -> Result<Identity, String> {
        Err(ONLY_WINDOWS.into())
    }

    pub fn sign_with<R>(_: &IdentityRef, _: impl FnOnce(&Identity, &dyn Fn(&[u8], &[u8]) -> Result<Vec<u8>, String>) -> Result<R, String>) -> Result<R, String> {
        Err(ONLY_WINDOWS.into())
    }
}
