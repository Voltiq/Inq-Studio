//! Annotations: markup (highlight, underline, strike-out), sticky notes, ink,
//! shapes (rectangle, ellipse, line, arrow, polygon), text boxes, stamps and
//! links.
//!
//! Every appearance is generated from the annotation's own data (its quad
//! points, ink list, line, vertices, rectangle, colours, text), so the same
//! code draws a new annotation and redraws an edited one, and it looks the
//! same in every PDF reader. Flattening turns appearances into page content.

use crate::encoding::win_ansi_byte;
use crate::fonts::{std14_width, Std14};
use crate::interp::Src;
use crate::util::*;
use lopdf::content::Operation;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotInfo {
    pub index: usize,
    pub subtype: String,
    pub rect: [f32; 4],
    pub contents: String,
    pub author: String,
    pub color: Option<[f32; 3]>,
    pub fill: Option<[f32; 3]>,
    pub opacity: f32,
    pub width: f32,
    /// For links: "https://…" or "page N".
    pub target: Option<String>,
    /// Form fields are managed as fields, not here.
    pub removable: bool,
    /// Its appearance can be regenerated after a change.
    pub editable: bool,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NewAnnot {
    /// highlight | underline | strikeout | note | ink | rect | ellipse | line | arrow | polygon | textbox | stamp | link
    pub kind: String,
    /// For markup: one quad per line, as 4 points (bottom-left, bottom-right, top-right, top-left).
    pub quads: Vec<[[f32; 2]; 4]>,
    pub rect: Option<[f32; 4]>,
    pub paths: Vec<Vec<[f32; 2]>>,
    /// Line / arrow end points, or polygon vertices.
    pub points: Vec<[f32; 2]>,
    pub color: [f32; 3],
    pub fill: Option<[f32; 3]>,
    pub contents: String,
    pub author: String,
    pub width: Option<f32>,
    pub opacity: Option<f32>,
    /// Text box font size.
    pub size: Option<f32>,
    /// Stamp label, e.g. "APPROVED".
    pub label: Option<String>,
    /// Link target: a web address, or a page index.
    pub uri: Option<String>,
    pub page: Option<usize>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AnnotChange {
    pub color: Option<[f32; 3]>,
    /// `Some(None)` removes the fill.
    pub fill: Option<Option<[f32; 3]>>,
    pub contents: Option<String>,
    pub opacity: Option<f32>,
    pub width: Option<f32>,
    pub dx: f32,
    pub dy: f32,
    pub uri: Option<String>,
    pub page: Option<usize>,
}

/// "D:20261110120000Z" for now.
pub fn pdf_date() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("D:{y:04}{m:02}{d:02}{:02}{:02}{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Today as YYYY-MM-DD, for headers and footers.
pub fn today() -> String {
    let d = pdf_date();
    format!("{}-{}-{}", &d[2..6], &d[6..8], &d[8..10])
}

pub fn annots_array(doc: &Document, page_id: ObjectId) -> Vec<Object> {
    let Ok(page) = doc.get_dictionary(page_id) else { return Vec::new() };
    match page.get(b"Annots").map(|o| deref(doc, o)) {
        Ok(Object::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

pub fn set_annots(doc: &mut Document, page_id: ObjectId, arr: Vec<Object>) -> Result<(), String> {
    let page = doc.get_dictionary(page_id).map_err(err)?;
    if let Ok(Object::Reference(id)) = page.get(b"Annots") {
        let id = *id;
        doc.objects.insert(id, Object::Array(arr));
    } else if arr.is_empty() {
        doc.get_dictionary_mut(page_id).map_err(err)?.remove(b"Annots");
    } else {
        doc.get_dictionary_mut(page_id).map_err(err)?.set("Annots", arr);
    }
    Ok(())
}

fn floats(doc: &Document, d: &Dictionary, key: &[u8]) -> Vec<f32> {
    get_array(doc, d, key).map(|a| a.iter().filter_map(|x| num(deref(doc, x))).collect()).unwrap_or_default()
}

fn color_of(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<[f32; 3]> {
    let v = floats(doc, d, key);
    match v.len() {
        3 => Some([v[0], v[1], v[2]]),
        1 => Some([v[0]; 3]),
        4 => Some([(1.0 - v[0]) * (1.0 - v[3]), (1.0 - v[1]) * (1.0 - v[3]), (1.0 - v[2]) * (1.0 - v[3])]),
        _ => None,
    }
}

/// Where a link goes, as shown to people.
fn link_target(doc: &Document, d: &Dictionary) -> Option<String> {
    let page_of = |dest: &Object| -> Option<String> {
        let arr = match deref(doc, dest) {
            Object::Array(a) => a.clone(),
            _ => return None,
        };
        let pid = arr.first()?.as_reference().ok()?;
        let idx = crate::pages::page_ids(doc).iter().position(|p| *p == pid)?;
        Some(format!("page {}", idx + 1))
    };
    if let Some(a) = d.get(b"A").ok().and_then(|o| dict(doc, o)) {
        match get_name(doc, a, b"S").as_deref() {
            Some("URI") => return get(doc, a, b"URI").and_then(|u| u.as_str().ok()).map(|u| String::from_utf8_lossy(u).to_string()),
            Some("GoTo") => return a.get(b"D").ok().and_then(page_of),
            _ => {}
        }
    }
    d.get(b"Dest").ok().and_then(page_of)
}

const REGENERATED: [&str; 12] =
    ["Highlight", "Underline", "StrikeOut", "Text", "Ink", "Square", "Circle", "Line", "Polygon", "FreeText", "Stamp", "Link"];

pub fn list(doc: &Document, page_id: ObjectId) -> Vec<AnnotInfo> {
    annots_array(doc, page_id)
        .iter()
        .enumerate()
        .filter_map(|(index, a)| {
            let d = dict(doc, a)?;
            let subtype = get_name(doc, d, b"Subtype").unwrap_or_default();
            if subtype == "Popup" {
                return None;
            }
            let rect = d.get(b"Rect").ok().and_then(|r| rect(doc, r)).unwrap_or([0.0; 4]);
            let text = |k: &[u8]| get(doc, d, k).and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default();
            let width = get(doc, d, b"BS")
                .and_then(|o| match o {
                    Object::Dictionary(bs) => get_num(doc, bs, b"W"),
                    _ => None,
                })
                .unwrap_or(1.0);
            Some(AnnotInfo {
                index,
                removable: subtype != "Widget",
                editable: REGENERATED.contains(&subtype.as_str()),
                target: if subtype == "Link" { link_target(doc, d) } else { None },
                subtype,
                rect,
                contents: text(b"Contents"),
                author: text(b"T"),
                color: color_of(doc, d, b"C"),
                fill: color_of(doc, d, b"IC"),
                opacity: get_num(doc, d, b"CA").unwrap_or(1.0),
                width,
            })
        })
        .collect()
}

pub fn delete(doc: &mut Document, page_id: ObjectId, index: usize) -> Result<(), String> {
    let mut arr = annots_array(doc, page_id);
    if index >= arr.len() {
        return Err("That annotation no longer exists.".into());
    }
    let removed = arr.remove(index);
    // Its popup goes with it.
    if let Ok(id) = removed.as_reference() {
        arr.retain(|a| dict(doc, a).and_then(|d| d.get(b"Parent").ok()).and_then(|p| p.as_reference().ok()) != Some(id));
    }
    set_annots(doc, page_id, arr)
}

// ---------------------------------------------------------------------------
// Appearances
// ---------------------------------------------------------------------------

fn f(v: f32) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    format!("{r}")
}

fn rgb(c: [f32; 3]) -> String {
    format!("{} {} {}", f(c[0]), f(c[1]), f(c[2]))
}

fn bounds(points: impl Iterator<Item = [f32; 2]>, pad: f32) -> [f32; 4] {
    let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for p in points {
        r[0] = r[0].min(p[0]);
        r[1] = r[1].min(p[1]);
        r[2] = r[2].max(p[0]);
        r[3] = r[3].max(p[1]);
    }
    [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad]
}

/// An ellipse inscribed in a box, as Bézier curves.
fn ellipse_path(x0: f32, y0: f32, x1: f32, y1: f32) -> String {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let k = 0.552_284_8;
    let (ox, oy) = (rx * k, ry * k);
    format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h ",
        f(cx + rx), f(cy),
        f(cx + rx), f(cy + oy), f(cx + ox), f(cy + ry), f(cx), f(cy + ry),
        f(cx - ox), f(cy + ry), f(cx - rx), f(cy + oy), f(cx - rx), f(cy),
        f(cx - rx), f(cy - oy), f(cx - ox), f(cy - ry), f(cx), f(cy - ry),
        f(cx + ox), f(cy - ry), f(cx + rx), f(cy - oy), f(cx + rx), f(cy),
    )
}

fn hex_win_ansi(text: &str) -> String {
    let bytes: Vec<u8> = text.chars().map(|c| win_ansi_byte(c).unwrap_or(b'?')).collect();
    format!("<{}>", bytes.iter().map(|b| format!("{b:02X}")).collect::<String>())
}

fn helv_width(text: &str, bold: bool) -> f32 {
    let face = if bold { Std14::HelveticaBold } else { Std14::Helvetica };
    text.chars().map(|c| std14_width(face, c)).sum::<f32>() / 1000.0
}

/// Breaks text into lines no wider than `width` at `size` (Helvetica metrics).
fn wrap_text(text: &str, size: f32, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && helv_width(&candidate, false) * size > width {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        out.push(line);
    }
    out
}

fn helvetica(bold: bool) -> Dictionary {
    dictionary! {
        "Type" => "Font", "Subtype" => "Type1",
        "BaseFont" => if bold { "Helvetica-Bold" } else { "Helvetica" },
        "Encoding" => "WinAnsiEncoding",
    }
}

/// Builds the normal appearance of an annotation from its dictionary.
/// Returns `None` for kinds that do not need or support one.
fn build_ap(doc: &mut Document, d: &Dictionary) -> Option<ObjectId> {
    let subtype = get_name(doc, d, b"Subtype")?;
    let r = d.get(b"Rect").ok().and_then(|o| rect(doc, o))?;
    let col = color_of(doc, d, b"C").unwrap_or([0.0; 3]);
    let fill = color_of(doc, d, b"IC");
    let opacity = get_num(doc, d, b"CA").unwrap_or(1.0);
    let w = d
        .get(b"BS")
        .ok()
        .and_then(|o| dict(doc, o))
        .and_then(|bs| get_num(doc, bs, b"W"))
        .unwrap_or(1.0);
    let c = rgb(col);
    let mut fonts = Dictionary::new();
    let mut multiply = false;
    let body = match subtype.as_str() {
        "Highlight" | "Underline" | "StrikeOut" => {
            let q = floats(doc, d, b"QuadPoints");
            let mut s = String::new();
            for quad in q.chunks(8).filter(|q| q.len() == 8) {
                // QuadPoints order: top-left, top-right, bottom-left, bottom-right.
                let (tl, tr, bl, br) = ([quad[0], quad[1]], [quad[2], quad[3]], [quad[4], quad[5]], [quad[6], quad[7]]);
                let h = ((tl[0] - bl[0]).powi(2) + (tl[1] - bl[1]).powi(2)).sqrt();
                if subtype == "Highlight" {
                    multiply = true;
                    s.push_str(&format!(
                        "{c} rg {} {} m {} {} l {} {} l {} {} l f\n",
                        f(bl[0]), f(bl[1]), f(br[0]), f(br[1]), f(tr[0]), f(tr[1]), f(tl[0]), f(tl[1])
                    ));
                } else {
                    let t = if subtype == "Underline" { 0.08 } else { 0.42 };
                    let lerp = |p: [f32; 2], q: [f32; 2]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                    let (a, b) = (lerp(bl, tl), lerp(br, tr));
                    s.push_str(&format!("{c} RG {} w {} {} m {} {} l S\n", f((h * 0.07).max(0.6)), f(a[0]), f(a[1]), f(b[0]), f(b[1])));
                }
            }
            s
        }
        "Text" => {
            let (x, y) = (r[0], r[1]);
            format!(
                "{c} rg 0.2 0.2 0.2 RG 0.6 w\n{} {} m {} {} l {} {} l {} {} l h B\n\
                 1 1 1 RG 1.2 w {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S\n",
                f(x + 1.5), f(y + 1.5), f(x + 18.5), f(y + 1.5), f(x + 18.5), f(y + 18.5), f(x + 1.5), f(y + 18.5),
                f(x + 5.0), f(y + 14.5), f(x + 15.0), f(y + 14.5),
                f(x + 5.0), f(y + 11.0), f(x + 15.0), f(y + 11.0),
                f(x + 5.0), f(y + 7.5), f(x + 12.0), f(y + 7.5),
            )
        }
        "Ink" => {
            let mut s = format!("{c} RG {} w 1 J 1 j\n", f(w));
            if let Some(list) = get_array(doc, d, b"InkList") {
                for path in list {
                    let v: Vec<f32> = match deref(doc, path) {
                        Object::Array(a) => a.iter().filter_map(num).collect(),
                        _ => continue,
                    };
                    if v.len() < 4 {
                        continue;
                    }
                    s.push_str(&format!("{} {} m ", f(v[0]), f(v[1])));
                    for p in v[2..].chunks(2).filter(|p| p.len() == 2) {
                        s.push_str(&format!("{} {} l ", f(p[0]), f(p[1])));
                    }
                    s.push_str("S\n");
                }
            }
            s
        }
        "Square" | "Circle" => {
            let h = w / 2.0;
            let (x0, y0, x1, y1) = (r[0] + h, r[1] + h, r[2] - h, r[3] - h);
            let path = if subtype == "Square" {
                format!("{} {} {} {} re ", f(x0), f(y0), f(x1 - x0), f(y1 - y0))
            } else {
                ellipse_path(x0, y0, x1, y1)
            };
            match fill {
                Some(fc) => format!("{} rg {c} RG {} w {path}B\n", rgb(fc), f(w)),
                None => format!("{c} RG {} w {path}S\n", f(w)),
            }
        }
        "Line" => {
            let l = floats(doc, d, b"L");
            if l.len() != 4 {
                return None;
            }
            let mut s = format!("{c} RG {c} rg {} w 1 J 1 j {} {} m {} {} l S\n", f(w), f(l[0]), f(l[1]), f(l[2]), f(l[3]));
            let arrow = get_array(doc, d, b"LE")
                .and_then(|a| a.get(1))
                .and_then(|o| o.as_name().ok())
                .map(|n| n != b"None")
                .unwrap_or(false);
            if arrow {
                let (dx, dy) = (l[2] - l[0], l[3] - l[1]);
                let len = (dx * dx + dy * dy).sqrt().max(0.01);
                let (ux, uy) = (dx / len, dy / len);
                let size = (w * 4.0).max(7.0);
                let back = (l[2] - ux * size, l[3] - uy * size);
                let (px, py) = (-uy * size * 0.5, ux * size * 0.5);
                s.push_str(&format!(
                    "{} {} m {} {} l {} {} l h f\n",
                    f(l[2]), f(l[3]), f(back.0 + px), f(back.1 + py), f(back.0 - px), f(back.1 - py)
                ));
            }
            s
        }
        "Polygon" => {
            let v = floats(doc, d, b"Vertices");
            if v.len() < 6 {
                return None;
            }
            let mut s = format!("{c} RG {} w 1 j ", f(w));
            if let Some(fc) = fill {
                s.push_str(&format!("{} rg ", rgb(fc)));
            }
            s.push_str(&format!("{} {} m ", f(v[0]), f(v[1])));
            for p in v[2..].chunks(2).filter(|p| p.len() == 2) {
                s.push_str(&format!("{} {} l ", f(p[0]), f(p[1])));
            }
            s.push_str(if fill.is_some() { "h B\n" } else { "h S\n" });
            s
        }
        "FreeText" => {
            let text = get(doc, d, b"Contents").and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default();
            let da = get(doc, d, b"DA").and_then(|o| o.as_str().ok()).map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or_default();
            let size = da
                .split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .find(|w| w[1] == "Tf")
                .and_then(|w| w[0].parse::<f32>().ok())
                .unwrap_or(12.0);
            fonts.set("Helv", helvetica(false));
            let pad = 4.0;
            let lines = wrap_text(&text, size, r[2] - r[0] - pad * 2.0);
            let mut s = String::new();
            if let Some(fc) = fill {
                s.push_str(&format!("{} rg {} {} {} {} re f\n", rgb(fc), f(r[0]), f(r[1]), f(r[2] - r[0]), f(r[3] - r[1])));
            }
            if w > 0.0 && d.has(b"BS") {
                s.push_str(&format!(
                    "{c} RG {} w {} {} {} {} re S\n",
                    f(w), f(r[0] + w / 2.0), f(r[1] + w / 2.0), f(r[2] - r[0] - w), f(r[3] - r[1] - w)
                ));
            }
            s.push_str(&format!("q {} {} {} {} re W n BT /Helv {} Tf {c} rg\n", f(r[0]), f(r[1]), f(r[2] - r[0]), f(r[3] - r[1]), f(size)));
            let mut y = r[3] - pad - size * 0.85;
            for l in lines {
                s.push_str(&format!("1 0 0 1 {} {} Tm {} Tj\n", f(r[0] + pad), f(y), hex_win_ansi(&l)));
                y -= size * 1.2;
            }
            s.push_str("ET Q\n");
            s
        }
        "Stamp" => {
            let label = get(doc, d, b"Contents")
                .and_then(|o| lopdf::decode_text_string(o).ok())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "APPROVED".into())
                .to_uppercase();
            fonts.set("HelvB", helvetica(true));
            let (bw, bh) = (r[2] - r[0], r[3] - r[1]);
            let size = ((bh * 0.5).min((bw - 16.0) / helv_width(&label, true).max(0.1))).max(4.0);
            let tw = helv_width(&label, true) * size;
            let lw = (bh * 0.06).max(1.5);
            format!(
                "{c} RG {c} rg {} w {} {} {} {} re S\n{} w {} {} {} {} re S\n\
                 BT /HelvB {} Tf {} {} Td {} Tj ET\n",
                f(lw), f(r[0] + lw / 2.0), f(r[1] + lw / 2.0), f(bw - lw), f(bh - lw),
                f(lw / 3.0), f(r[0] + lw * 2.0), f(r[1] + lw * 2.0), f(bw - lw * 4.0), f(bh - lw * 4.0),
                f(size), f(r[0] + (bw - tw) / 2.0), f(r[1] + (bh - size * 0.7) / 2.0), hex_win_ansi(&label),
            )
        }
        _ => return None,
    };
    let mut gs = dictionary! { "Type" => "ExtGState", "CA" => real(opacity), "ca" => real(opacity) };
    if multiply {
        gs.set("BM", "Multiply");
    }
    let mut res = dictionary! { "ExtGState" => dictionary! { "GS0" => gs } };
    if !fonts.is_empty() {
        res.set("Font", fonts);
    }
    let mut s = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![real(r[0]), real(r[1]), real(r[2]), real(r[3])],
            "Resources" => res,
        },
        format!("/GS0 gs\n{body}").into_bytes(),
    );
    let _ = s.compress();
    Some(doc.add_object(s))
}

fn label_of(kind: &str) -> &'static str {
    match kind {
        "highlight" => "Highlight",
        "underline" => "Underline",
        "strikeout" => "StrikeOut",
        "note" => "Text",
        "ink" => "Ink",
        "rect" => "Square",
        "ellipse" => "Circle",
        "line" | "arrow" => "Line",
        "polygon" => "Polygon",
        "textbox" => "FreeText",
        "stamp" => "Stamp",
        "link" => "Link",
        _ => "",
    }
}

fn dest_for(doc: &Document, page: usize) -> Result<Object, String> {
    let pid = crate::pages::page_id(doc, page)?;
    Ok(Object::Array(vec![Object::Reference(pid), "XYZ".into(), Object::Null, Object::Null, Object::Null]))
}

fn set_link_target(doc: &Document, d: &mut Dictionary, uri: &Option<String>, page: Option<usize>) -> Result<(), String> {
    if let Some(u) = uri.as_ref().filter(|u| !u.trim().is_empty()) {
        d.remove(b"Dest");
        d.set("A", dictionary! { "S" => "URI", "URI" => Object::string_literal(u.trim()) });
    } else if let Some(p) = page {
        d.remove(b"A");
        d.set("Dest", dest_for(doc, p)?);
    }
    Ok(())
}

pub fn add(doc: &mut Document, page_id: ObjectId, a: &NewAnnot) -> Result<(), String> {
    let sub = label_of(&a.kind);
    if sub.is_empty() {
        return Err(format!("Unknown annotation kind: {}", a.kind));
    }
    let width = a.width.unwrap_or(1.0).max(0.1);
    let mut d = Dictionary::new();
    d.set("Type", "Annot");
    d.set("Subtype", sub);
    let rect: [f32; 4] = match a.kind.as_str() {
        "highlight" | "underline" | "strikeout" => {
            if a.quads.is_empty() {
                return Err("Select some text to mark up first.".into());
            }
            let mut qp = Vec::new();
            for q in &a.quads {
                let [bl, br, tr, tl] = *q;
                for p in [tl, tr, bl, br] {
                    qp.push(real(p[0]));
                    qp.push(real(p[1]));
                }
            }
            d.set("QuadPoints", qp);
            bounds(a.quads.iter().flatten().copied(), 1.0)
        }
        "note" => {
            let at = a.rect.ok_or("Click where the note should go.")?;
            d.set("Name", "Comment");
            d.set("Open", false);
            [at[0], at[1] - 20.0, at[0] + 20.0, at[1]]
        }
        "ink" => {
            if a.paths.iter().all(|p| p.len() < 2) {
                return Err("Draw a stroke first.".into());
            }
            d.set(
                "InkList",
                a.paths.iter().map(|p| Object::Array(p.iter().flat_map(|pt| [real(pt[0]), real(pt[1])]).collect())).collect::<Vec<_>>(),
            );
            bounds(a.paths.iter().flatten().copied(), width)
        }
        "rect" | "ellipse" | "textbox" | "stamp" | "link" => {
            let r = a.rect.ok_or("Drag out an area first.")?;
            if a.kind == "textbox" {
                let size = a.size.unwrap_or(12.0);
                d.set("DA", Object::string_literal(format!("/Helv {} Tf {} rg", f(size), rgb(a.color))));
            }
            if a.kind == "stamp" {
                d.set("Name", "Approved");
            }
            if a.kind == "link" {
                d.set("Border", vec![0.into(), 0.into(), 0.into()]);
                set_link_target(doc, &mut d, &a.uri, a.page)?;
                if !d.has(b"A") && !d.has(b"Dest") {
                    return Err("Give the link a web address or a page.".into());
                }
            }
            r
        }
        "line" | "arrow" => {
            if a.points.len() < 2 {
                return Err("Drag to draw a line.".into());
            }
            let (p, q) = (a.points[0], a.points[a.points.len() - 1]);
            d.set("L", vec![real(p[0]), real(p[1]), real(q[0]), real(q[1])]);
            if a.kind == "arrow" {
                d.set("LE", vec![Object::Name(b"None".to_vec()), Object::Name(b"OpenArrow".to_vec())]);
            }
            bounds([p, q].into_iter(), (width * 4.0).max(7.0))
        }
        "polygon" => {
            if a.points.len() < 3 {
                return Err("A polygon needs at least three points.".into());
            }
            d.set("Vertices", a.points.iter().flat_map(|p| [real(p[0]), real(p[1])]).collect::<Vec<_>>());
            bounds(a.points.iter().copied(), width)
        }
        _ => unreachable!(),
    };
    let date = pdf_date();
    d.set("Rect", vec![real(rect[0]), real(rect[1]), real(rect[2]), real(rect[3])]);
    d.set("F", 4);
    d.set("P", Object::Reference(page_id));
    d.set("M", Object::string_literal(date.clone()));
    if a.kind != "link" {
        d.set("C", vec![real(a.color[0]), real(a.color[1]), real(a.color[2])]);
        d.set("CreationDate", Object::string_literal(date.clone()));
        d.set("NM", Object::string_literal(format!("inq-{}-{}", date, doc.max_id + 1)));
        if !matches!(a.kind.as_str(), "highlight" | "underline" | "strikeout" | "note" | "stamp" | "textbox") || a.width.is_some() {
            d.set("BS", dictionary! { "W" => real(width), "S" => "S" });
        }
        if a.kind == "textbox" && a.width.is_none() {
            d.remove(b"BS");
        }
    }
    if let Some(fc) = a.fill {
        d.set("IC", vec![real(fc[0]), real(fc[1]), real(fc[2])]);
    }
    if let Some(o) = a.opacity.filter(|o| *o < 1.0) {
        d.set("CA", real(o));
    }
    let contents = if a.kind == "stamp" { a.label.clone().unwrap_or_else(|| "Approved".into()) } else { a.contents.clone() };
    if !contents.is_empty() {
        d.set("Contents", lopdf::text_string(&contents));
    }
    if !a.author.is_empty() && a.kind != "link" {
        d.set("T", lopdf::text_string(&a.author));
    }
    if let Some(ap) = build_ap(doc, &d) {
        d.set("AP", dictionary! { "N" => ap });
    }
    let id = doc.add_object(d);
    let mut arr = annots_array(doc, page_id);
    arr.push(Object::Reference(id));
    set_annots(doc, page_id, arr)
}

fn shift(v: &mut [Object], dx: f32, dy: f32) {
    for (i, o) in v.iter_mut().enumerate() {
        if let Some(n) = num(o) {
            *o = real(n + if i % 2 == 0 { dx } else { dy });
        }
    }
}

/// Changes an annotation (colour, fill, text, opacity, width, position, link
/// target) and redraws its appearance.
pub fn update(doc: &mut Document, page_id: ObjectId, index: usize, ch: &AnnotChange) -> Result<(), String> {
    let mut arr = annots_array(doc, page_id);
    let entry = arr.get(index).ok_or("That annotation no longer exists.")?.clone();
    let mut d = dict(doc, &entry).ok_or("That annotation could not be read.")?.clone();
    let subtype = get_name(doc, &d, b"Subtype").unwrap_or_default();
    if subtype == "Widget" {
        return Err("Form fields are changed by filling them in.".into());
    }
    if let Some(c) = ch.color {
        d.set("C", vec![real(c[0]), real(c[1]), real(c[2])]);
        if subtype == "FreeText" {
            let da = get(doc, &d, b"DA").and_then(|o| o.as_str().ok()).map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or_default();
            let size = da.split_whitespace().collect::<Vec<_>>().windows(2).find(|w| w[1] == "Tf").and_then(|w| w[0].parse::<f32>().ok()).unwrap_or(12.0);
            d.set("DA", Object::string_literal(format!("/Helv {} Tf {} rg", f(size), rgb(c))));
        }
    }
    match ch.fill {
        Some(Some(fc)) => d.set("IC", vec![real(fc[0]), real(fc[1]), real(fc[2])]),
        Some(None) => {
            d.remove(b"IC");
        }
        None => {}
    }
    if let Some(t) = &ch.contents {
        if t.is_empty() {
            d.remove(b"Contents");
        } else {
            d.set("Contents", lopdf::text_string(t));
        }
    }
    if let Some(o) = ch.opacity {
        d.set("CA", real(o.clamp(0.05, 1.0)));
    }
    if let Some(w) = ch.width {
        d.set("BS", dictionary! { "W" => real(w.max(0.1)), "S" => "S" });
    }
    if ch.dx != 0.0 || ch.dy != 0.0 {
        for key in [&b"Rect"[..], b"QuadPoints", b"L", b"Vertices"] {
            if let Some(Object::Array(mut a)) = d.get(key).ok().map(|o| deref(doc, o).clone()) {
                shift(&mut a, ch.dx, ch.dy);
                d.set(key.to_vec(), a);
            }
        }
        if let Some(Object::Array(paths)) = d.get(b"InkList").ok().map(|o| deref(doc, o).clone()) {
            let moved: Vec<Object> = paths
                .into_iter()
                .map(|p| match p {
                    Object::Array(mut a) => {
                        shift(&mut a, ch.dx, ch.dy);
                        Object::Array(a)
                    }
                    o => o,
                })
                .collect();
            d.set("InkList", moved);
        }
    }
    if subtype == "Link" {
        set_link_target(doc, &mut d, &ch.uri, ch.page)?;
    }
    d.set("M", Object::string_literal(pdf_date()));
    if REGENERATED.contains(&subtype.as_str()) && subtype != "Link" {
        if let Some(ap) = build_ap(doc, &d) {
            d.set("AP", dictionary! { "N" => ap });
        }
    } else if ch.dx != 0.0 || ch.dy != 0.0 {
        // An appearance we did not make still follows the rectangle (it is drawn into Rect).
    }
    match entry {
        Object::Reference(id) => {
            doc.objects.insert(id, Object::Dictionary(d));
        }
        _ => {
            arr[index] = Object::Dictionary(d);
            set_annots(doc, page_id, arr)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Flattening
// ---------------------------------------------------------------------------

/// The appearance stream an annotation shows now.
fn normal_appearance(doc: &Document, d: &Dictionary) -> Option<ObjectId> {
    let ap = d.get(b"AP").ok().and_then(|o| dict(doc, o))?;
    match ap.get(b"N").ok()? {
        Object::Reference(id) => match doc.get_object(*id).ok()? {
            Object::Stream(_) => Some(*id),
            Object::Dictionary(states) => {
                let state = get_name(doc, d, b"AS")?;
                states.get(state.as_bytes()).ok()?.as_reference().ok()
            }
            _ => None,
        },
        Object::Dictionary(states) => {
            let state = get_name(doc, d, b"AS")?;
            states.get(state.as_bytes()).ok()?.as_reference().ok()
        }
        _ => None,
    }
}

/// Draws annotations into the page content and removes them: form fields
/// (`widgets`), everything else except links (`others`), or both. Returns how
/// many were flattened.
pub fn flatten(doc: &mut Document, page_id: ObjectId, widgets: bool, others: bool) -> Result<usize, String> {
    let arr = annots_array(doc, page_id);
    let mut keep = Vec::new();
    let mut ops: Vec<Operation> = Vec::new();
    let mut count = 0;
    for a in arr {
        let Some(d) = dict(doc, &a).cloned() else { continue };
        let subtype = get_name(doc, &d, b"Subtype").unwrap_or_default();
        let is_widget = subtype == "Widget";
        let wanted = (is_widget && widgets) || (!is_widget && others && subtype != "Link");
        if !wanted {
            // Popups go with the annotations they belong to.
            let parent_gone = subtype == "Popup" && others;
            if !parent_gone {
                keep.push(a);
            }
            continue;
        }
        count += 1;
        let flags = get_num(doc, &d, b"F").unwrap_or(0.0) as i64;
        if flags & 2 != 0 || flags & 32 != 0 {
            continue; // hidden or not for viewing: nothing to draw
        }
        let Some(ap) = normal_appearance(doc, &d) else { continue };
        let Some(r) = d.get(b"Rect").ok().and_then(|o| rect(doc, o)) else { continue };
        let Ok(Object::Stream(form)) = doc.get_object(ap) else { continue };
        let bbox = get_array(doc, &form.dict, b"BBox")
            .map(|a| a.iter().filter_map(|x| num(deref(doc, x))).collect::<Vec<_>>())
            .filter(|v| v.len() == 4)
            .unwrap_or_else(|| vec![0.0, 0.0, r[2] - r[0], r[3] - r[1]]);
        let m = get_array(doc, &form.dict, b"Matrix")
            .map(|a| a.iter().filter_map(|x| num(deref(doc, x))).collect::<Vec<_>>())
            .filter(|v| v.len() == 6)
            .map(|v| [v[0], v[1], v[2], v[3], v[4], v[5]])
            .unwrap_or(IDENTITY);
        // The bounding box as transformed by the form matrix, mapped onto Rect.
        let corners = [(bbox[0], bbox[1]), (bbox[2], bbox[1]), (bbox[2], bbox[3]), (bbox[0], bbox[3])].map(|(x, y)| apply(&m, x, y));
        let bx0 = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
        let by0 = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
        let bx1 = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
        let by1 = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);
        let (sx, sy) = ((r[2] - r[0]) / (bx1 - bx0).max(1e-3), (r[3] - r[1]) / (by1 - by0).max(1e-3));
        let a_m = [sx, 0.0, 0.0, sy, r[0] - bx0 * sx, r[1] - by0 * sy];
        if let Ok(Object::Stream(s)) = doc.get_object_mut(ap) {
            if !s.dict.has(b"Subtype") {
                s.dict.set("Subtype", "Form");
                s.dict.set("Type", "XObject");
            }
        }
        let name = crate::edit::add_resource(doc, page_id, Src::Page, "XObject", ap, "InqFlat")?;
        ops.push(Operation::new("q", vec![]));
        ops.push(Operation::new("cm", a_m.iter().map(|v| real(*v)).collect()));
        ops.push(Operation::new("Do", vec![Object::Name(name)]));
        ops.push(Operation::new("Q", vec![]));
    }
    set_annots(doc, page_id, keep)?;
    crate::edit::add_page_ops(doc, page_id, ops, false)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn date_shape() {
        let d = super::pdf_date();
        assert!(d.starts_with("D:20") && d.ends_with('Z') && d.len() == 17);
        assert_eq!(super::today().len(), 10);
    }
}
