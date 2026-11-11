//! PDF/A: checking a document against the archival rules, and converting it.
//!
//! PDF/A (ISO 19005) asks that a file carries everything needed to show it
//! the same way decades from now: its fonts, a description of its colours,
//! its metadata, and nothing that depends on the outside world or on
//! software behaviour (scripts, encryption, external content).
//!
//! The checks here cover the rules that documents fail in practice. They do
//! not replace a certified validator: glyph-level font checks and colour
//! management details are out of scope, and the report says so.
//!
//! Conversion fixes what can be fixed without changing how pages look:
//! metadata, the output intent (sRGB), file identifier, scripts and actions,
//! annotation flags, forbidden filters and entries, and fonts that are not
//! embedded but installed on this computer.

use crate::encoding::BaseEncoding;
use crate::fonts::{FontKind, PdfFont};
use crate::interp::scan_page;
use crate::pages;
use crate::sysfonts;
use crate::util::*;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Level {
    /// 1, 2 or 3
    pub part: u8,
}

impl Level {
    pub fn label(&self) -> String {
        format!("PDF/A-{}b", self.part)
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    /// The clause of ISO 19005 the rule comes from.
    pub clause: String,
    pub message: String,
    /// Where: font names, page numbers and the like.
    pub detail: String,
    pub count: usize,
    /// Converting takes care of it.
    pub fixable: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub target: String,
    /// What the document itself claims, e.g. "PDF/A-1b".
    pub claimed: Option<String>,
    pub issues: Vec<Issue>,
    /// Number of rules checked.
    pub checked: usize,
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

fn metadata_id(doc: &Document) -> Option<ObjectId> {
    doc.catalog().ok().and_then(|c| ref_id(c, b"Metadata"))
}

fn metadata_text(doc: &Document) -> Option<String> {
    let id = metadata_id(doc)?;
    stream_bytes(doc, &Object::Reference(id)).map(|b| String::from_utf8_lossy(&b).to_string())
}

fn xmp_value(xmp: &str, name: &str) -> Option<String> {
    // <pdfaid:part>1</pdfaid:part> or pdfaid:part="1"
    if let Some(at) = xmp.find(&format!("<{name}>")) {
        let rest = &xmp[at + name.len() + 2..];
        return rest.find('<').map(|end| rest[..end].trim().to_string());
    }
    for quote in ['"', '\''] {
        if let Some(at) = xmp.find(&format!("{name}={quote}")) {
            let rest = &xmp[at + name.len() + 2..];
            return rest.find(quote).map(|end| rest[..end].trim().to_string());
        }
    }
    None
}

/// What the document says it conforms to ("PDF/A-2b").
pub fn claimed(doc: &Document) -> Option<String> {
    let xmp = metadata_text(doc)?;
    let part = xmp_value(&xmp, "pdfaid:part")?;
    let conf = xmp_value(&xmp, "pdfaid:conformance").unwrap_or_default();
    Some(format!("PDF/A-{}{}", part, conf.to_lowercase()))
}

/// PDF/A-1 is PDF 1.4: no object streams, no cross-reference streams.
pub fn allows_object_streams(doc: &Document) -> bool {
    !claimed(doc).map(|c| c.starts_with("PDF/A-1")).unwrap_or(false)
}

/// The catalog's metadata must stay readable without decoding.
pub fn keep_metadata_plain(doc: &mut Document) {
    let Some(id) = metadata_id(doc) else { return };
    if let Ok(Object::Stream(s)) = doc.get_object_mut(id) {
        if s.dict.has(b"Filter") {
            let _ = s.decompress();
        }
        s.allows_compression = false;
    }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// "D:20260102030405+01'00'" → "2026-01-02T03:04:05+01:00"
fn iso_date(pdf: &str) -> Option<String> {
    let s = pdf.trim().trim_start_matches("D:");
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return None;
    }
    let part = |from: usize, len: usize, default: &str| digits.get(from..from + len).unwrap_or(default).to_string();
    let mut out = format!("{}-{}-{}T{}:{}:{}", part(0, 4, "0000"), part(4, 2, "01"), part(6, 2, "01"), part(8, 2, "00"), part(10, 2, "00"), part(12, 2, "00"));
    let zone = &s[digits.len()..];
    let mut z = zone.chars();
    match z.next() {
        Some(sign @ ('+' | '-')) => {
            let nums: String = zone[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            let hh = nums.get(0..2).unwrap_or("00");
            let mm = nums.get(2..4).unwrap_or("00");
            out.push_str(&format!("{sign}{hh}:{mm}"));
        }
        _ => out.push('Z'),
    }
    Some(out)
}

fn xmp_packet(meta: &crate::store::Meta, level: Level) -> Vec<u8> {
    let mut props = String::new();
    if !meta.title.is_empty() {
        props.push_str(&format!("   <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>\n", xml(&meta.title)));
    }
    if !meta.author.is_empty() {
        props.push_str(&format!("   <dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>\n", xml(&meta.author)));
    }
    if !meta.subject.is_empty() {
        props.push_str(&format!("   <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:description>\n", xml(&meta.subject)));
    }
    if !meta.keywords.is_empty() {
        props.push_str(&format!("   <pdf:Keywords>{}</pdf:Keywords>\n", xml(&meta.keywords)));
    }
    if !meta.producer.is_empty() {
        props.push_str(&format!("   <pdf:Producer>{}</pdf:Producer>\n", xml(&meta.producer)));
    }
    if !meta.creator.is_empty() {
        props.push_str(&format!("   <xmp:CreatorTool>{}</xmp:CreatorTool>\n", xml(&meta.creator)));
    }
    if let Some(d) = iso_date(&meta.created) {
        props.push_str(&format!("   <xmp:CreateDate>{d}</xmp:CreateDate>\n"));
    }
    if let Some(d) = iso_date(&meta.modified) {
        props.push_str(&format!("   <xmp:ModifyDate>{d}</xmp:ModifyDate>\n   <xmp:MetadataDate>{d}</xmp:MetadataDate>\n"));
    }
    props.push_str(&format!("   <pdfaid:part>{}</pdfaid:part>\n   <pdfaid:conformance>B</pdfaid:conformance>\n", level.part));
    let mut out = String::new();
    out.push_str("<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    out.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    out.push_str("  <rdf:Description rdf:about=\"\"\n    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n    xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"\n    xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n");
    out.push_str(&props);
    out.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n");
    // Room for other tools to edit the packet in place.
    for _ in 0..12 {
        out.push_str(&" ".repeat(99));
        out.push('\n');
    }
    out.push_str("<?xpacket end=\"w\"?>");
    out.into_bytes()
}

fn write_metadata(doc: &mut Document, level: Level) {
    let meta = crate::store::read_meta(doc);
    let stream = Stream::new(dictionary! { "Type" => "Metadata", "Subtype" => "XML" }, xmp_packet(&meta, level)).with_compression(false);
    let id = match metadata_id(doc) {
        Some(id) => {
            doc.objects.insert(id, Object::Stream(stream));
            id
        }
        None => doc.add_object(stream),
    };
    if let Ok(cat) = doc.catalog_mut() {
        cat.set("Metadata", Object::Reference(id));
    }
}

/// Rewrites the XMP packet from the document information, keeping the
/// conformance the document claims. Called before every save of a PDF/A file,
/// because the two must agree.
pub fn sync_metadata(doc: &mut Document) {
    let part = claimed(doc).and_then(|c| c.trim_start_matches("PDF/A-").chars().next()).and_then(|c| c.to_digit(10)).unwrap_or(2) as u8;
    write_metadata(doc, Level { part });
}

// ---------------------------------------------------------------------------
// sRGB output intent
// ---------------------------------------------------------------------------

fn s15(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

/// A small sRGB display profile (ICC version 2.1): primaries and white point
/// adapted to D50, and the sRGB transfer curve sampled at 1024 points.
pub fn srgb_profile() -> Vec<u8> {
    let xyz = |x: f64, y: f64, z: f64| -> Vec<u8> {
        let mut v = b"XYZ \0\0\0\0".to_vec();
        v.extend(s15(x));
        v.extend(s15(y));
        v.extend(s15(z));
        v
    };
    let text = |sig: &[u8; 4], s: &str| -> Vec<u8> {
        let mut v = sig.to_vec();
        v.extend([0u8; 4]);
        if sig == b"desc" {
            v.extend(((s.len() + 1) as u32).to_be_bytes());
            v.extend(s.as_bytes());
            v.push(0);
            v.extend([0u8; 4 + 4 + 2 + 1 + 67]);
        } else {
            v.extend(s.as_bytes());
            v.push(0);
        }
        v
    };
    let mut curve = b"curv\0\0\0\0".to_vec();
    curve.extend(1024u32.to_be_bytes());
    for i in 0..1024 {
        let x = i as f64 / 1023.0;
        let y = if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
        curve.extend(((y * 65535.0).round() as u16).to_be_bytes());
    }
    let blocks: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", text(b"desc", "sRGB IEC61966-2.1")),
        (b"cprt", text(b"text", "No copyright, use freely")),
        (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
        (b"rXYZ", xyz(0.43607, 0.22249, 0.01392)),
        (b"gXYZ", xyz(0.38515, 0.71687, 0.09708)),
        (b"bXYZ", xyz(0.14307, 0.06061, 0.71410)),
        (b"rTRC", curve),
    ];
    // rTRC, gTRC and bTRC share one curve.
    let tags: [(&[u8; 4], usize); 9] =
        [(b"desc", 0), (b"cprt", 1), (b"wtpt", 2), (b"rXYZ", 3), (b"gXYZ", 4), (b"bXYZ", 5), (b"rTRC", 6), (b"gTRC", 6), (b"bTRC", 6)];
    let mut offsets = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    let base = 128 + 4 + tags.len() * 12;
    for (_, data) in &blocks {
        offsets.push((base + body.len(), data.len()));
        body.extend(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
    }
    let size = base + body.len();
    let mut p = Vec::with_capacity(size);
    p.extend((size as u32).to_be_bytes());
    p.extend([0u8; 4]); // preferred CMM
    p.extend([2, 0x10, 0, 0]); // version 2.1
    p.extend(b"mntrRGB XYZ ");
    for v in [2026u16, 1, 1, 0, 0, 0] {
        p.extend(v.to_be_bytes());
    }
    p.extend(b"acspMSFT");
    p.extend([0u8; 4]); // flags
    p.extend([0u8; 8]); // manufacturer, model
    p.extend([0u8; 8]); // attributes
    p.extend([0u8; 4]); // rendering intent: perceptual
    p.extend(s15(0.9642));
    p.extend(s15(1.0));
    p.extend(s15(0.8249));
    p.extend([0u8; 4]); // creator
    p.extend([0u8; 16]); // profile id
    p.extend([0u8; 28]);
    debug_assert_eq!(p.len(), 128);
    p.extend((tags.len() as u32).to_be_bytes());
    for (sig, block) in tags {
        p.extend(sig);
        p.extend((offsets[block].0 as u32).to_be_bytes());
        p.extend((offsets[block].1 as u32).to_be_bytes());
    }
    p.extend(body);
    p
}

fn output_intents(doc: &Document) -> Vec<&Dictionary> {
    doc.catalog()
        .ok()
        .and_then(|c| get_array(doc, c, b"OutputIntents"))
        .map(|a| a.iter().filter_map(|o| dict(doc, o)).collect())
        .unwrap_or_default()
}

/// Number of colour components of the PDF/A output intent's profile, if there is one.
fn intent_components(doc: &Document) -> Option<i64> {
    output_intents(doc)
        .into_iter()
        .filter(|d| get_name(doc, d, b"S").as_deref() == Some("GTS_PDFA1"))
        .find_map(|d| d.get(b"DestOutputProfile").ok().and_then(|o| dict(doc, o)).and_then(|p| get_num(doc, p, b"N")).map(|n| n as i64))
}

fn add_srgb_intent(doc: &mut Document) {
    let mut profile = Stream::new(dictionary! { "N" => 3, "Alternate" => "DeviceRGB" }, srgb_profile());
    let _ = profile.compress();
    let pid = doc.add_object(profile);
    let intent = doc.add_object(dictionary! {
        "Type" => "OutputIntent",
        "S" => "GTS_PDFA1",
        "OutputConditionIdentifier" => Object::string_literal("sRGB IEC61966-2.1"),
        "Info" => Object::string_literal("sRGB IEC61966-2.1"),
        "DestOutputProfile" => pid,
    });
    if let Ok(cat) = doc.catalog_mut() {
        cat.set("OutputIntents", vec![Object::Reference(intent)]);
    }
}

// ---------------------------------------------------------------------------
// Walking the document
// ---------------------------------------------------------------------------

fn each_dict<'a>(doc: &'a Document, mut f: impl FnMut(ObjectId, &'a Dictionary)) {
    fn walk<'a>(id: ObjectId, o: &'a Object, f: &mut dyn FnMut(ObjectId, &'a Dictionary), depth: usize) {
        if depth > 12 {
            return;
        }
        match o {
            Object::Dictionary(d) => {
                f(id, d);
                for (_, v) in d.iter() {
                    walk(id, v, f, depth + 1);
                }
            }
            Object::Stream(s) => {
                f(id, &s.dict);
                for (_, v) in s.dict.iter() {
                    walk(id, v, f, depth + 1);
                }
            }
            Object::Array(a) => a.iter().for_each(|x| walk(id, x, f, depth + 1)),
            _ => {}
        }
    }
    for (id, o) in &doc.objects {
        walk(*id, o, &mut f, 0);
    }
}

fn each_dict_mut(doc: &mut Document, mut f: impl FnMut(&mut Dictionary)) {
    fn walk(o: &mut Object, f: &mut dyn FnMut(&mut Dictionary), depth: usize) {
        if depth > 12 {
            return;
        }
        match o {
            Object::Dictionary(d) => {
                f(d);
                for (_, v) in d.iter_mut() {
                    walk(v, f, depth + 1);
                }
            }
            Object::Stream(s) => {
                f(&mut s.dict);
                for (_, v) in s.dict.iter_mut() {
                    walk(v, f, depth + 1);
                }
            }
            Object::Array(a) => a.iter_mut().for_each(|x| walk(x, f, depth + 1)),
            _ => {}
        }
    }
    for o in doc.objects.values_mut() {
        walk(o, &mut f, 0);
    }
}

const BAD_ACTIONS: [&str; 9] = ["JavaScript", "Launch", "Sound", "Movie", "ResetForm", "ImportData", "Hide", "SetOCGState", "Rendition"];

fn is_bad_action(doc: &Document, d: &Dictionary) -> bool {
    match get_name(doc, d, b"S") {
        Some(s) if BAD_ACTIONS.contains(&s.as_str()) => true,
        Some(s) if s == "Named" => !matches!(get_name(doc, d, b"N").as_deref(), Some("NextPage" | "PrevPage" | "FirstPage" | "LastPage")),
        _ => false,
    }
}

fn filters(d: &Dictionary) -> Vec<Vec<u8>> {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => vec![n.clone()],
        Ok(Object::Array(a)) => a.iter().filter_map(|x| x.as_name().ok().map(|n| n.to_vec())).collect(),
        _ => Vec::new(),
    }
}

/// Fonts that are drawn visibly somewhere but have no embedded program.
fn unembedded_fonts(doc: &Document) -> BTreeMap<String, (PdfFont, Vec<usize>)> {
    let mut out: BTreeMap<String, (PdfFont, Vec<usize>)> = BTreeMap::new();
    let mut note = |f: &PdfFont, page: usize| {
        if f.program.is_some() || f.kind == FontKind::Type3 || f.key == "missing" {
            return;
        }
        let e = out.entry(f.key.clone()).or_insert_with(|| (f.clone(), Vec::new()));
        if !e.1.contains(&page) {
            e.1.push(page);
        }
    };
    for (i, pid) in pages::page_ids(doc).into_iter().enumerate() {
        if let Ok(scan) = scan_page(doc, pid) {
            // Invisible text (an OCR layer) is never rendered and needs no font program.
            for r in scan.runs.iter().filter(|r| r.mode != 3) {
                note(&r.font, i);
            }
        }
        // Appearance streams of annotations and form fields draw text too.
        for a in crate::annots::annots_array(doc, pid) {
            let Some(ad) = dict(doc, &a) else { continue };
            let Some(ap) = get(doc, ad, b"AP").and_then(|o| dict(doc, o)) else { continue };
            let Some(n) = ap.get(b"N").ok() else { continue };
            let streams: Vec<&Object> = match deref(doc, n) {
                Object::Stream(_) => vec![n],
                Object::Dictionary(states) => states.iter().map(|(_, v)| v).collect(),
                _ => Vec::new(),
            };
            for s in streams {
                let Some(sd) = dict(doc, s) else { continue };
                let Some(res) = get(doc, sd, b"Resources").and_then(|o| dict(doc, o)) else { continue };
                let has_text = stream_bytes(doc, s).map(|b| b.windows(2).any(|w| w == b"Tj" || w == b"TJ")).unwrap_or(false);
                if !has_text {
                    continue;
                }
                for f in crate::interp::resource_fonts(doc, res, "ap").values() {
                    note(f, i);
                }
            }
        }
    }
    out
}

#[derive(Default)]
struct Colours {
    rgb: bool,
    gray: bool,
    cmyk: bool,
}

fn device_colours(doc: &Document) -> Colours {
    let mut c = Colours::default();
    for pid in pages::page_ids(doc) {
        let Ok(scan) = scan_page(doc, pid) else { continue };
        // Whatever is painted before a colour is chosen is painted in device grey.
        if !scan.runs.is_empty() || !scan.paths.is_empty() {
            c.gray = true;
        }
        for ops in scan.ops.values() {
            for op in ops {
                match op.operator.as_str() {
                    "rg" | "RG" => c.rgb = true,
                    "g" | "G" => c.gray = true,
                    "k" | "K" => c.cmyk = true,
                    "cs" | "CS" => match op.operands.first().and_then(|o| o.as_name().ok()) {
                        Some(b"DeviceRGB") => c.rgb = true,
                        Some(b"DeviceGray") => c.gray = true,
                        Some(b"DeviceCMYK") => c.cmyk = true,
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        for im in &scan.images {
            let Some(d) = im.id.and_then(|id| doc.get_object(id).ok()).and_then(|o| o.as_stream().ok()).map(|s| &s.dict) else { continue };
            let name = match d.get(b"ColorSpace").ok().map(|o| deref(doc, o)) {
                Some(Object::Name(n)) => Some(n.as_slice()),
                Some(Object::Array(a)) if a.first().and_then(|x| x.as_name().ok()) == Some(b"Indexed") => a.get(1).map(|x| deref(doc, x)).and_then(|x| x.as_name().ok()),
                _ => None,
            };
            match name {
                Some(b"DeviceRGB") => c.rgb = true,
                Some(b"DeviceGray") => c.gray = true,
                Some(b"DeviceCMYK") => c.cmyk = true,
                _ => {}
            }
        }
    }
    c
}

fn page_list(pages: &[usize]) -> String {
    let mut p: Vec<String> = pages.iter().take(12).map(|i| (i + 1).to_string()).collect();
    if pages.len() > 12 {
        p.push("…".into());
    }
    format!("page{} {}", if pages.len() == 1 { "" } else { "s" }, p.join(", "))
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

pub fn validate(doc: &Document, level: Level, protected: bool) -> Report {
    let mut issues: Vec<Issue> = Vec::new();
    let mut checked = 0;
    let a1 = level.part == 1;
    let mut rule = |issues: &mut Vec<Issue>, bad: usize, id: &str, clause: (&str, &str), message: &str, detail: String, fixable: bool| {
        checked += 1;
        if bad > 0 {
            issues.push(Issue { id: id.into(), clause: if a1 { clause.0 } else { clause.1 }.into(), message: message.into(), detail, count: bad, fixable });
        }
    };
    let cat = doc.catalog().ok();

    rule(&mut issues, protected as usize, "encrypted", ("6.1.3", "6.1.3"), "The file is password protected. An archival file must open without a key.", String::new(), true);

    let has_id = matches!(doc.trailer.get(b"ID"), Ok(Object::Array(a)) if a.len() == 2);
    rule(&mut issues, (!has_id) as usize, "file-id", ("6.1.3", "6.1.3"), "The file has no identifier.", String::new(), true);

    let want_version = if a1 { "1.4" } else { "1.7" };
    rule(
        &mut issues,
        (doc.version.as_str() > want_version) as usize,
        "version",
        ("6.1.2", "6.1.2"),
        &format!("The file says it is PDF {}; {} is based on PDF {want_version}.", doc.version, level.label()),
        String::new(),
        true,
    );

    let xmp = metadata_text(doc);
    let claim = claimed(doc);
    let meta_plain = metadata_id(doc).and_then(|id| doc.get_object(id).ok()).and_then(|o| o.as_stream().ok()).map(|s| !s.dict.has(b"Filter")).unwrap_or(true);
    rule(&mut issues, xmp.is_none() as usize, "xmp-missing", ("6.7.2", "6.6.2.1"), "The document has no XMP metadata.", String::new(), true);
    if xmp.is_some() {
        rule(&mut issues, (!meta_plain) as usize, "xmp-filter", ("6.7.2", "6.6.2.1"), "The XMP metadata is compressed; it must be readable as plain text.", String::new(), true);
        rule(
            &mut issues,
            (claim.as_deref() != Some(level.label().as_str())) as usize,
            "xmp-claim",
            ("6.7.11", "6.6.4"),
            &match &claim {
                Some(c) => format!("The metadata says {c}, not {}.", level.label()),
                None => format!("The metadata does not say the document is {}.", level.label()),
            },
            String::new(),
            true,
        );
    }

    // Fonts
    let missing = unembedded_fonts(doc);
    let embeddable = missing.values().filter(|(f, _)| system_match(doc, f).is_some()).count();
    let names: Vec<String> = missing.values().take(8).map(|(f, p)| format!("{} ({})", f.ps_name, page_list(p))).collect();
    rule(
        &mut issues,
        missing.len(),
        "font-embed",
        ("6.3.4", "6.2.11.4.1"),
        "Fonts are not embedded, so the text may look different on another computer.",
        format!(
            "{}{}",
            names.join("; "),
            if embeddable == missing.len() {
                String::new()
            } else {
                format!(". {} of {} can be embedded from the fonts installed here.", embeddable, missing.len())
            }
        ),
        embeddable == missing.len(),
    );

    // Colour
    let colours = device_colours(doc);
    let n = intent_components(doc);
    let uses_device = colours.rgb || colours.gray || colours.cmyk;
    rule(
        &mut issues,
        (uses_device && n.is_none()) as usize,
        "output-intent",
        ("6.2.2", "6.2.3"),
        "Colours are given without saying what they mean (device colours and no output intent).",
        if colours.cmyk { "CMYK colours need a CMYK output profile, which Inq Studio does not supply.".to_string() } else { String::new() },
        !colours.cmyk,
    );
    let mismatch = match n {
        Some(3) => colours.cmyk,
        Some(4) => colours.rgb,
        Some(1) => colours.rgb || colours.cmyk,
        _ => false,
    } || (n.is_none() && colours.cmyk && colours.rgb);
    rule(
        &mut issues,
        mismatch as usize,
        "device-colour",
        ("6.2.3.3", "6.2.4.3"),
        "Both RGB and CMYK device colours are used; one output intent can describe only one of them.",
        String::new(),
        false,
    );

    // Scripts and actions
    let mut actions = 0;
    let mut aa = 0;
    let mut external = 0;
    let mut lzw = 0;
    let mut jpx = 0;
    let mut interpolate = 0;
    let mut alternates = 0;
    let mut postscript = 0;
    let mut transfer = 0;
    let mut transparency = 0;
    let mut cidset = 0;
    each_dict(doc, |_, d| {
        if is_bad_action(doc, d) {
            actions += 1;
        }
        if d.has(b"AA") {
            aa += 1;
        }
        if d.has(b"F") && d.has(b"Length") && (d.has(b"FFilter") || matches!(d.get(b"F"), Ok(Object::String(..) | Object::Dictionary(_)))) {
            external += 1;
        }
        let fs = filters(d);
        if fs.iter().any(|f| f == b"LZWDecode") {
            lzw += 1;
        }
        if fs.iter().any(|f| f == b"JPXDecode") {
            jpx += 1;
        }
        let subtype = d.get(b"Subtype").ok().and_then(|o| o.as_name().ok());
        if subtype == Some(b"Image") {
            if d.get(b"Interpolate").ok().and_then(|o| o.as_bool().ok()) == Some(true) {
                interpolate += 1;
            }
            if d.has(b"Alternates") || d.has(b"OPI") {
                alternates += 1;
            }
            if d.has(b"SMask") && !matches!(d.get(b"SMask"), Ok(Object::Name(_))) {
                transparency += 1;
            }
        }
        if subtype == Some(b"PS") || (subtype == Some(b"Form") && (d.has(b"Ref") || d.has(b"PS") || d.has(b"OPI"))) {
            postscript += 1;
        }
        if d.has(b"TR") || matches!(d.get(b"TR2"), Ok(o) if o.as_name().ok() != Some(b"Default")) {
            transfer += 1;
        }
        let type_name = d.get(b"Type").ok().and_then(|o| o.as_name().ok());
        if type_name == Some(b"ExtGState") || d.has(b"CA") || d.has(b"ca") || d.has(b"BM") {
            let alpha = |k: &[u8]| d.get(k).ok().and_then(num).map(|v| v < 0.999).unwrap_or(false);
            let blend = match d.get(b"BM") {
                Ok(Object::Name(n)) => n != b"Normal" && n != b"Compatible",
                _ => false,
            };
            let mask = matches!(d.get(b"SMask"), Ok(o) if o.as_name().ok() != Some(b"None")) && type_name == Some(b"ExtGState");
            if alpha(b"CA") || alpha(b"ca") || blend || mask {
                transparency += 1;
            }
        }
        if type_name == Some(b"FontDescriptor") && (d.has(b"CIDSet") || d.has(b"CharSet")) {
            cidset += 1;
        }
    });
    let js_names = cat.and_then(|c| get(doc, c, b"Names")).and_then(|o| dict(doc, o)).map(|n| n.has(b"JavaScript")).unwrap_or(false);
    rule(
        &mut issues,
        actions + js_names as usize,
        "actions",
        ("6.6.1", "6.5.1"),
        "The document contains scripts or actions that depend on the reader (JavaScript, launching files, sound, resetting forms).",
        String::new(),
        true,
    );
    rule(&mut issues, aa, "additional-actions", ("6.6.2", "6.5.2"), "Objects carry additional actions that run on events.", String::new(), true);
    rule(&mut issues, external, "external-streams", ("6.1.7", "6.1.7.1"), "Stream data is kept in external files.", String::new(), false);
    rule(&mut issues, lzw, "lzw", ("6.1.10", "6.1.7.2"), "Streams use LZW compression, which archives must not.", String::new(), true);
    if a1 {
        rule(&mut issues, jpx, "jpx", ("6.1.10", "6.1.7.2"), "JPEG 2000 images are not part of PDF 1.4.", String::new(), false);
        rule(
            &mut issues,
            transparency,
            "transparency",
            ("6.4", "6.4"),
            "The document uses transparency, which PDF/A-1 does not allow. PDF/A-2 does.",
            String::new(),
            false,
        );
        let layers = cat.map(|c| c.has(b"OCProperties")).unwrap_or(false);
        rule(&mut issues, layers as usize, "layers", ("6.1.13", "6.1.13"), "The document has layers (optional content), which PDF/A-1 does not allow.", String::new(), false);
    }
    rule(&mut issues, interpolate, "interpolate", ("6.2.4", "6.2.8"), "Images ask the reader to smooth them (Interpolate).", String::new(), true);
    rule(&mut issues, alternates, "alternates", ("6.2.4", "6.2.8"), "Images have alternate versions or OPI information.", String::new(), true);
    rule(&mut issues, postscript, "postscript", ("6.2.5", "6.2.9"), "The document embeds PostScript or reference XObjects.", String::new(), false);
    rule(&mut issues, transfer, "transfer", ("6.2.8", "6.2.5"), "Graphics states use transfer functions.", String::new(), true);
    if !a1 {
        rule(
            &mut issues,
            cidset,
            "cidset",
            ("6.3.5", "6.2.11.4.2"),
            "Font descriptors list their glyphs (CIDSet / CharSet); the lists are optional and must be exact when present.",
            String::new(),
            true,
        );
    }

    // Embedded files
    let embedded = cat.and_then(|c| get(doc, c, b"Names")).and_then(|o| dict(doc, o)).map(|n| n.has(b"EmbeddedFiles")).unwrap_or(false);
    if level.part < 3 {
        rule(
            &mut issues,
            embedded as usize,
            "embedded-files",
            ("6.1.11", "6.8"),
            if a1 { "The document has file attachments, which PDF/A-1 does not allow." } else { "The document has file attachments; PDF/A-2 allows only PDF/A files as attachments (PDF/A-3 allows any)." },
            String::new(),
            false,
        );
    }

    // Forms
    let form = cat.and_then(|c| get(doc, c, b"AcroForm")).and_then(|o| dict(doc, o));
    rule(&mut issues, form.map(|f| f.has(b"XFA")).unwrap_or(false) as usize, "xfa", ("6.9", "6.4.2"), "The form has an XFA layer.", String::new(), true);
    let need_ap = form.and_then(|f| f.get(b"NeedAppearances").ok()).and_then(|o| o.as_bool().ok()).unwrap_or(false);
    rule(&mut issues, need_ap as usize, "need-appearances", ("6.9", "6.4.1"), "The form asks the reader to draw the fields (NeedAppearances).", String::new(), true);

    // Annotations
    let allowed: &[&str] = if a1 {
        &["Text", "Link", "FreeText", "Line", "Square", "Circle", "Highlight", "Underline", "Squiggly", "StrikeOut", "Stamp", "Ink", "Popup", "Widget", "PrinterMark", "TrapNet"]
    } else {
        &[
            "Text", "Link", "FreeText", "Line", "Square", "Circle", "Polygon", "PolyLine", "Highlight", "Underline", "Squiggly", "StrikeOut", "Stamp", "Caret", "Ink", "Popup",
            "FileAttachment", "Widget", "PrinterMark", "TrapNet", "Watermark", "Redact",
        ]
    };
    let (mut bad_type, mut bad_flags, mut no_ap) = (Vec::new(), Vec::new(), Vec::new());
    for (i, pid) in pages::page_ids(doc).into_iter().enumerate() {
        for a in crate::annots::annots_array(doc, pid) {
            let Some(d) = dict(doc, &a) else { continue };
            let subtype = get_name(doc, d, b"Subtype").unwrap_or_default();
            if !allowed.contains(&subtype.as_str()) {
                bad_type.push(i);
            }
            let flags = get_num(doc, d, b"F").unwrap_or(0.0) as i64;
            let need_print = subtype != "Popup";
            if (need_print && flags & 4 == 0) || flags & (1 | 2 | 32) != 0 {
                bad_flags.push(i);
            }
            let rect_empty = d.get(b"Rect").ok().and_then(|r| rect(doc, r)).map(|r| r[2] - r[0] < 0.01 && r[3] - r[1] < 0.01).unwrap_or(false);
            let has_ap = get(doc, d, b"AP").and_then(|o| dict(doc, o)).map(|ap| ap.has(b"N")).unwrap_or(false);
            if !has_ap && subtype != "Popup" && subtype != "Link" && !rect_empty {
                no_ap.push(i);
            }
        }
    }
    let dedup = |v: &mut Vec<usize>| {
        v.sort();
        v.dedup();
    };
    let (nt, nf, na) = (bad_type.len(), bad_flags.len(), no_ap.len());
    dedup(&mut bad_type);
    dedup(&mut bad_flags);
    dedup(&mut no_ap);
    rule(&mut issues, nt, "annot-type", ("6.5.2", "6.3.1"), "Annotations of a kind archives do not allow (sound, movie, 3D, screen …).", if nt > 0 { page_list(&bad_type) } else { String::new() }, false);
    rule(
        &mut issues,
        nf,
        "annot-flags",
        ("6.5.3", "6.3.2"),
        "Annotations are hidden or not set to print, so a printout would differ from the screen.",
        if nf > 0 { page_list(&bad_flags) } else { String::new() },
        true,
    );
    rule(
        &mut issues,
        na,
        "annot-appearance",
        ("6.5.3", "6.3.3"),
        "Annotations have no stored appearance; each reader would draw them its own way.",
        if na > 0 { page_list(&no_ap) } else { String::new() },
        false,
    );

    Report { target: level.label(), claimed: claim, passed: issues.is_empty(), issues, checked }
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

fn std14_family(ps: &str) -> Option<&'static str> {
    let l = ps.to_lowercase();
    if l.starts_with("helvetica") || l.starts_with("arial") {
        Some("Arial")
    } else if l.starts_with("times") {
        Some("Times New Roman")
    } else if l.starts_with("courier") {
        Some("Courier New")
    } else {
        None
    }
}

/// The encoding of a simple font, when it is one a TrueType program can serve.
fn simple_encoding(doc: &Document, font: &Dictionary) -> Option<BaseEncoding> {
    match get(doc, font, b"Encoding")? {
        Object::Name(n) if n == b"WinAnsiEncoding" || n == b"MacRomanEncoding" => BaseEncoding::from_name(n),
        Object::Dictionary(d) if !d.has(b"Differences") => {
            let base = get_name(doc, d, b"BaseEncoding")?;
            if base == "WinAnsiEncoding" || base == "MacRomanEncoding" {
                BaseEncoding::from_name(base.as_bytes())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// An installed TrueType font that can stand in for a font that is not embedded.
fn system_match(doc: &Document, f: &PdfFont) -> Option<sysfonts::LoadedFont> {
    let id = f.obj?;
    let d = doc.get_dictionary(id).ok()?;
    if !matches!(f.kind, FontKind::Type1 | FontKind::TrueType) {
        return None;
    }
    simple_encoding(doc, d)?;
    let found = sysfonts::find(&f.ps_name, f.bold, f.italic)
        .or_else(|| std14_family(&f.ps_name).and_then(|fam| sysfonts::find_family(fam, None, f.bold, f.italic)))?;
    // A single TrueType face: collections and CFF outlines need other font file kinds.
    if found.data.starts_with(b"ttcf") {
        return None;
    }
    let face = ttf_parser::Face::parse(&found.data, found.index).ok()?;
    face.tables().glyf?;
    Some(found)
}

/// Embeds an installed font into a simple font that came without its program.
fn embed_font(doc: &mut Document, f: &PdfFont) -> Result<bool, String> {
    let Some(font_id) = f.obj else { return Ok(false) };
    let Some(sys) = system_match(doc, f) else { return Ok(false) };
    let enc = {
        let d = doc.get_dictionary(font_id).map_err(err)?;
        simple_encoding(doc, d).ok_or("encoding")?
    };
    let face = ttf_parser::Face::parse(&sys.data, sys.index).map_err(err)?;
    let scale = 1000.0 / face.units_per_em().max(1) as f32;
    let table = enc.table();
    let widths: Vec<Object> = (32..=255usize)
        .map(|code| {
            let w = table[code].and_then(|c| face.glyph_index(c)).and_then(|g| face.glyph_hor_advance(g)).map(|a| a as f32 * scale).unwrap_or(0.0);
            real(w.round())
        })
        .collect();
    let bbox = face.global_bounding_box();
    let mut flags = 32i64;
    if face.is_monospaced() {
        flags |= 1;
    }
    if face.is_italic() {
        flags |= 64;
    }
    let mut file = Stream::new(dictionary! { "Length1" => sys.data.len() as i64 }, sys.data.as_ref().clone());
    let _ = file.compress();
    let file_id = doc.add_object(file);
    let base_font = doc.get_dictionary(font_id).map_err(err)?.get(b"BaseFont").ok().cloned().unwrap_or(Object::Name(f.ps_name.as_bytes().to_vec()));
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => base_font,
        "Flags" => flags,
        "FontBBox" => vec![real(bbox.x_min as f32 * scale), real(bbox.y_min as f32 * scale), real(bbox.x_max as f32 * scale), real(bbox.y_max as f32 * scale)],
        "ItalicAngle" => real(face.italic_angle()),
        "Ascent" => real(face.ascender() as f32 * scale),
        "Descent" => real(face.descender() as f32 * scale),
        "CapHeight" => real(face.capital_height().unwrap_or(face.ascender()) as f32 * scale),
        "StemV" => 80,
        "FontFile2" => file_id,
    });
    let d = doc.get_dictionary_mut(font_id).map_err(err)?;
    d.set("Subtype", "TrueType");
    d.set("FirstChar", 32);
    d.set("LastChar", 255);
    d.set("Widths", widths);
    d.set("FontDescriptor", descriptor);
    Ok(true)
}

/// Makes the document conform as far as it can be done without changing the
/// look of the pages. Returns what was done.
pub fn convert(doc: &mut Document, level: Level) -> Result<Vec<String>, String> {
    let mut done: Vec<String> = Vec::new();
    let a1 = level.part == 1;

    // Fonts first: they decide whether the result is usable.
    let missing = unembedded_fonts(doc);
    let mut embedded = Vec::new();
    for (f, _) in missing.values() {
        if embed_font(doc, f).unwrap_or(false) {
            embedded.push(f.ps_name.clone());
        }
    }
    if !embedded.is_empty() {
        done.push(format!("Embedded {} from the fonts installed on this computer.", embedded.join(", ")));
    }

    // Scripts, actions and the entries only a particular reader understands.
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(err)?;
    let mut removed = 0;
    let names_ref = doc.get_dictionary(root).ok().and_then(|c| c.get(b"Names").ok().cloned());
    match names_ref {
        Some(Object::Reference(id)) => {
            if let Ok(n) = doc.get_dictionary_mut(id) {
                if n.remove(b"JavaScript").is_some() {
                    removed += 1;
                }
            }
        }
        Some(Object::Dictionary(_)) => {
            if let Ok(c) = doc.get_dictionary_mut(root) {
                if let Ok(Object::Dictionary(n)) = c.get_mut(b"Names") {
                    if n.remove(b"JavaScript").is_some() {
                        removed += 1;
                    }
                }
            }
        }
        _ => {}
    }
    // Actions are found through the keys that hold them, so the holder can drop them.
    let bad: HashSet<ObjectId> = doc
        .objects
        .iter()
        .filter(|(_, o)| matches!(o, Object::Dictionary(d) if is_bad_action(doc, d)))
        .map(|(id, _)| *id)
        .collect();
    let (mut interpolate, mut transfer, mut cidset, mut alternates) = (0, 0, 0, 0);
    let is_bad_inline = |v: &Object| match v {
        Object::Dictionary(a) => {
            let s = a.get(b"S").ok().and_then(|o| o.as_name().ok()).map(|n| String::from_utf8_lossy(n).to_string()).unwrap_or_default();
            BAD_ACTIONS.contains(&s.as_str())
        }
        Object::Reference(id) => bad.contains(id),
        _ => false,
    };
    each_dict_mut(doc, |d| {
        if d.remove(b"AA").is_some() {
            removed += 1;
        }
        for key in [&b"A"[..], b"OpenAction", b"Next"] {
            if d.get(key).map(is_bad_inline).unwrap_or(false) {
                d.remove(key);
                removed += 1;
            }
        }
        let subtype = d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()).map(|n| n.to_vec());
        if subtype.as_deref() == Some(b"Image") {
            if d.get(b"Interpolate").ok().and_then(|o| o.as_bool().ok()) == Some(true) {
                d.set("Interpolate", false);
                interpolate += 1;
            }
            for key in [&b"Alternates"[..], b"OPI"] {
                if d.remove(key).is_some() {
                    alternates += 1;
                }
            }
        }
        if d.remove(b"TR").is_some() {
            transfer += 1;
        }
        if matches!(d.get(b"TR2"), Ok(o) if o.as_name().ok() != Some(b"Default")) {
            d.remove(b"TR2");
            transfer += 1;
        }
        if !a1 && d.get(b"Type").ok().and_then(|o| o.as_name().ok()) == Some(b"FontDescriptor") {
            for key in [&b"CIDSet"[..], b"CharSet"] {
                if d.remove(key).is_some() {
                    cidset += 1;
                }
            }
        }
    });
    if removed > 0 {
        done.push(format!("Removed {removed} script{} and reader-dependent action{}.", if removed == 1 { "" } else { "s" }, if removed == 1 { "" } else { "s" }));
    }
    if interpolate + alternates > 0 {
        done.push("Turned off image smoothing hints and removed alternate images.".into());
    }
    if transfer > 0 {
        done.push("Removed transfer functions from graphics states.".into());
    }
    if cidset > 0 {
        done.push("Removed optional glyph lists from font descriptors.".into());
    }

    // Forms
    let form_id = doc.get_dictionary(root).ok().and_then(|c| c.get(b"AcroForm").ok().cloned());
    let mut form_fix = |f: &mut Dictionary| {
        let mut changed = false;
        changed |= f.remove(b"XFA").is_some();
        if f.get(b"NeedAppearances").ok().and_then(|o| o.as_bool().ok()) == Some(true) {
            f.remove(b"NeedAppearances");
            changed = true;
        }
        changed
    };
    let form_changed = match form_id {
        Some(Object::Reference(id)) => doc.get_dictionary_mut(id).map(&mut form_fix).unwrap_or(false),
        Some(Object::Dictionary(_)) => match doc.get_dictionary_mut(root).ok().and_then(|c| c.get_mut(b"AcroForm").ok()) {
            Some(Object::Dictionary(f)) => form_fix(f),
            _ => false,
        },
        _ => false,
    };
    if form_changed {
        done.push("Removed the XFA layer and the request to redraw form fields.".into());
    }

    // Annotation flags
    let mut flagged = 0;
    for pid in pages::page_ids(doc) {
        for a in crate::annots::annots_array(doc, pid) {
            let target = match &a {
                Object::Reference(id) => Some(*id),
                _ => None,
            };
            let Some(id) = target else { continue };
            let Ok(d) = doc.get_dictionary_mut(id) else { continue };
            let subtype = d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()).map(|n| n.to_vec()).unwrap_or_default();
            if subtype == b"Popup" {
                continue;
            }
            let flags = d.get(b"F").ok().and_then(num).unwrap_or(0.0) as i64;
            let mut new = (flags | 4) & !(1 | 2 | 32);
            if subtype == b"Text" {
                new |= 8 | 16;
            }
            if new != flags {
                d.set("F", new);
                flagged += 1;
            }
        }
    }
    if flagged > 0 {
        done.push(format!("Set {flagged} annotation{} to print and show as on screen.", if flagged == 1 { "" } else { "s" }));
    }

    // LZW streams are stored again with Flate.
    let mut relzw = 0;
    for o in doc.objects.values_mut() {
        if let Object::Stream(s) = o {
            if filters(&s.dict).iter().any(|f| f == b"LZWDecode") && s.decompress().is_ok() {
                let _ = s.compress();
                relzw += 1;
            }
        }
    }
    if relzw > 0 {
        done.push(format!("Recompressed {relzw} LZW stream{}.", if relzw == 1 { "" } else { "s" }));
    }

    // Colour: say what device colours mean.
    if intent_components(doc).is_none() {
        // Without a CMYK profile on hand, an intent for CMYK content cannot be written honestly.
        if !device_colours(doc).cmyk {
            add_srgb_intent(doc);
            done.push("Added an sRGB output intent, so device colours have a defined meaning.".into());
        }
    }

    // Identity and metadata last, so they describe the finished document.
    if !matches!(doc.trailer.get(b"ID"), Ok(Object::Array(a)) if a.len() == 2) {
        let mut id = [0u8; 16];
        let _ = getrandom::fill(&mut id);
        let s = Object::String(id.to_vec(), StringFormat::Hexadecimal);
        doc.trailer.set("ID", Object::Array(vec![s.clone(), s]));
        done.push("Gave the file an identifier.".into());
    }
    let want = if a1 { "1.4" } else { "1.7" };
    if doc.version.as_str() > want {
        doc.version = want.to_string();
    }
    let info = crate::store::info_dict_mut(doc);
    if !info.has(b"Producer") {
        info.set("Producer", Object::string_literal("Inq Studio"));
    }
    write_metadata(doc, level);
    done.push(format!("Wrote XMP metadata that identifies the document as {}.", level.label()));
    Ok(done)
}
