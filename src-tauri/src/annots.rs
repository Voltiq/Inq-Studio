//! Annotations: highlight, underline, strike-out, sticky notes, freehand ink
//! and rectangles. Each gets its own appearance stream, so it looks the same
//! in every PDF reader, not just in Inq Studio.

use crate::util::*;
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
    /// Link and widget (form field) annotations are listed but not deleted from here.
    pub removable: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewAnnot {
    /// highlight | underline | strikeout | note | ink | rect
    pub kind: String,
    /// For markup: one quad per line, as 4 points (bottom-left, bottom-right, top-right, top-left).
    #[serde(default)]
    pub quads: Vec<[[f32; 2]; 4]>,
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
    #[serde(default)]
    pub paths: Vec<Vec<[f32; 2]>>,
    pub color: [f32; 3],
    #[serde(default)]
    pub contents: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "one")]
    pub width: f32,
    #[serde(default = "one")]
    pub opacity: f32,
}

fn one() -> f32 {
    1.0
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

fn annots_array(doc: &Document, page_id: ObjectId) -> Vec<Object> {
    let Ok(page) = doc.get_dictionary(page_id) else { return Vec::new() };
    match page.get(b"Annots").map(|o| deref(doc, o)) {
        Ok(Object::Array(a)) => a.clone(),
        _ => Vec::new(),
    }
}

fn set_annots(doc: &mut Document, page_id: ObjectId, arr: Vec<Object>) -> Result<(), String> {
    let page = doc.get_dictionary(page_id).map_err(err)?;
    if let Ok(Object::Reference(id)) = page.get(b"Annots") {
        let id = *id;
        doc.objects.insert(id, Object::Array(arr));
    } else {
        doc.get_dictionary_mut(page_id).map_err(err)?.set("Annots", arr);
    }
    Ok(())
}

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
            let color = get_array(doc, d, b"C").and_then(|c| {
                let v: Vec<f32> = c.iter().filter_map(num).collect();
                match v.len() {
                    3 => Some([v[0], v[1], v[2]]),
                    1 => Some([v[0]; 3]),
                    _ => None,
                }
            });
            Some(AnnotInfo {
                index,
                removable: !matches!(subtype.as_str(), "Link" | "Widget"),
                subtype,
                rect,
                contents: text(b"Contents"),
                author: text(b"T"),
                color,
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
        arr.retain(|a| {
            dict(doc, a).and_then(|d| d.get(b"Parent").ok()).and_then(|p| p.as_reference().ok()) != Some(id)
        });
    }
    set_annots(doc, page_id, arr)
}

fn appearance(doc: &mut Document, bbox: [f32; 4], content: String, multiply: bool, opacity: f32) -> ObjectId {
    let mut gs = dictionary! { "Type" => "ExtGState", "CA" => real(opacity), "ca" => real(opacity) };
    if multiply {
        gs.set("BM", "Multiply");
    }
    let mut s = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![real(bbox[0]), real(bbox[1]), real(bbox[2]), real(bbox[3])],
            "Resources" => dictionary! { "ExtGState" => dictionary! { "GS0" => gs } },
        },
        content.into_bytes(),
    );
    let _ = s.compress();
    doc.add_object(s)
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

fn f(v: f32) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    format!("{r}")
}

pub fn add(doc: &mut Document, page_id: ObjectId, a: &NewAnnot) -> Result<(), String> {
    let [r, g, b] = a.color;
    let col = format!("{} {} {}", f(r), f(g), f(b));
    let mut d = Dictionary::new();
    let ap: ObjectId;
    let rect: [f32; 4];
    match a.kind.as_str() {
        "highlight" | "underline" | "strikeout" => {
            if a.quads.is_empty() {
                return Err("Select some text to mark up first.".into());
            }
            rect = bounds(a.quads.iter().flatten().copied(), 1.0);
            let mut qp = Vec::new();
            let mut content = String::new();
            for q in &a.quads {
                let [bl, br, tr, tl] = *q;
                // QuadPoints order readers expect: top-left, top-right, bottom-left, bottom-right.
                for p in [tl, tr, bl, br] {
                    qp.push(real(p[0]));
                    qp.push(real(p[1]));
                }
                let h = ((tl[0] - bl[0]).powi(2) + (tl[1] - bl[1]).powi(2)).sqrt();
                match a.kind.as_str() {
                    "highlight" => content.push_str(&format!(
                        "{col} rg {} {} m {} {} l {} {} l {} {} l f\n",
                        f(bl[0]), f(bl[1]), f(br[0]), f(br[1]), f(tr[0]), f(tr[1]), f(tl[0]), f(tl[1])
                    )),
                    kind => {
                        let t = if kind == "underline" { 0.08 } else { 0.42 };
                        let lerp = |p: [f32; 2], q: [f32; 2]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                        let (s, e) = (lerp(bl, tl), lerp(br, tr));
                        content.push_str(&format!(
                            "{col} RG {} w {} {} m {} {} l S\n",
                            f((h * 0.07).max(0.6)), f(s[0]), f(s[1]), f(e[0]), f(e[1])
                        ));
                    }
                }
            }
            let sub = match a.kind.as_str() {
                "highlight" => "Highlight",
                "underline" => "Underline",
                _ => "StrikeOut",
            };
            d.set("Subtype", sub);
            d.set("QuadPoints", qp);
            ap = appearance(doc, rect, format!("/GS0 gs\n{content}"), a.kind == "highlight", a.opacity);
        }
        "note" => {
            let at = a.rect.ok_or("Click where the note should go.")?;
            rect = [at[0], at[1] - 20.0, at[0] + 20.0, at[1]];
            d.set("Subtype", "Text");
            d.set("Name", "Comment");
            d.set("Open", false);
            let (x, y) = (rect[0], rect[1]);
            let content = format!(
                "/GS0 gs {col} rg 0.2 0.2 0.2 RG 0.6 w\n{} {} m {} {} l {} {} l {} {} l {} {} l {} {} l h B\n\
                 1 1 1 RG 1.2 w {} {} m {} {} l S {} {} m {} {} l S {} {} m {} {} l S\n",
                f(x + 1.5), f(y + 5.0), f(x + 18.5), f(y + 5.0), f(x + 18.5), f(y + 18.5),
                f(x + 1.5), f(y + 18.5), f(x + 1.5), f(y + 5.0), f(x + 1.5), f(y + 5.0),
                f(x + 5.0), f(y + 14.5), f(x + 15.0), f(y + 14.5),
                f(x + 5.0), f(y + 11.5), f(x + 15.0), f(y + 11.5),
                f(x + 5.0), f(y + 8.5), f(x + 12.0), f(y + 8.5),
            );
            ap = appearance(doc, rect, content, false, 1.0);
        }
        "ink" => {
            if a.paths.iter().all(|p| p.len() < 2) {
                return Err("Draw a stroke first.".into());
            }
            rect = bounds(a.paths.iter().flatten().copied(), a.width);
            d.set("Subtype", "Ink");
            d.set(
                "InkList",
                a.paths
                    .iter()
                    .map(|p| Object::Array(p.iter().flat_map(|pt| [real(pt[0]), real(pt[1])]).collect()))
                    .collect::<Vec<_>>(),
            );
            d.set("BS", dictionary! { "W" => real(a.width), "S" => "S" });
            let mut content = format!("/GS0 gs {col} RG {} w 1 J 1 j\n", f(a.width));
            for p in a.paths.iter().filter(|p| p.len() >= 2) {
                content.push_str(&format!("{} {} m ", f(p[0][0]), f(p[0][1])));
                for pt in &p[1..] {
                    content.push_str(&format!("{} {} l ", f(pt[0]), f(pt[1])));
                }
                content.push_str("S\n");
            }
            ap = appearance(doc, rect, content, false, a.opacity);
        }
        "rect" => {
            let rr = a.rect.ok_or("Drag out a rectangle first.")?;
            rect = [rr[0] - a.width, rr[1] - a.width, rr[2] + a.width, rr[3] + a.width];
            d.set("Subtype", "Square");
            d.set("BS", dictionary! { "W" => real(a.width), "S" => "S" });
            let content = format!(
                "/GS0 gs {col} RG {} w {} {} {} {} re S\n",
                f(a.width), f(rr[0]), f(rr[1]), f(rr[2] - rr[0]), f(rr[3] - rr[1])
            );
            ap = appearance(doc, rect, content, false, a.opacity);
        }
        other => return Err(format!("Unknown annotation kind: {other}")),
    }
    let date = pdf_date();
    d.set("Type", "Annot");
    d.set("Rect", vec![real(rect[0]), real(rect[1]), real(rect[2]), real(rect[3])]);
    d.set("C", vec![real(r), real(g), real(b)]);
    d.set("F", 4);
    d.set("P", Object::Reference(page_id));
    d.set("M", Object::string_literal(date.clone()));
    d.set("CreationDate", Object::string_literal(date.clone()));
    d.set("NM", Object::string_literal(format!("inq-{}-{}", date, doc.max_id + 1)));
    if a.opacity < 1.0 {
        d.set("CA", real(a.opacity));
    }
    if !a.contents.is_empty() {
        d.set("Contents", lopdf::text_string(&a.contents));
    }
    if !a.author.is_empty() {
        d.set("T", lopdf::text_string(&a.author));
    }
    d.set("AP", dictionary! { "N" => ap });
    let id = doc.add_object(d);
    let mut arr = annots_array(doc, page_id);
    arr.push(Object::Reference(id));
    set_annots(doc, page_id, arr)
}

#[cfg(test)]
mod tests {
    #[test]
    fn date_shape() {
        let d = super::pdf_date();
        assert!(d.starts_with("D:20") && d.ends_with('Z') && d.len() == 17);
    }
}
