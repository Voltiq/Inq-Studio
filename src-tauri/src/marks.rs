//! Page marks: watermarks, headers and footers, Bates numbers, and page labels.
//!
//! Marks are set upright as the page is shown (rotated pages included), in any
//! font the app offers, and become ordinary page content.

use crate::edit::{page_rotation, place_text, FontChoice};
use crate::util::*;
use lopdf::{dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    pub font: FontChoice,
    pub size: f32,
    pub color: [f32; 3],
    #[serde(default = "one")]
    pub opacity: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Mark {
    /// Text across the middle of the page.
    Watermark {
        text: String,
        style: TextStyle,
        /// Degrees, counter-clockwise as the page is shown.
        angle: f32,
        behind: bool,
    },
    /// Up to three texts in the top and bottom margins. Tokens: {page},
    /// {pages}, {date}, {file}.
    HeaderFooter {
        header: [String; 3],
        footer: [String; 3],
        style: TextStyle,
        margin: f32,
        /// The number {page} shows on the first marked page.
        first_number: i64,
    },
    /// Sequential numbers, e.g. "ACME-000123".
    Bates {
        prefix: String,
        suffix: String,
        start: u64,
        digits: usize,
        /// top-left | top-center | top-right | bottom-left | bottom-center | bottom-right
        position: String,
        style: TextStyle,
        margin: f32,
    },
}

/// The visible page box and its size as shown (after rotation).
fn display_box(doc: &Document, page_id: ObjectId) -> ([f32; 4], i64, f32, f32) {
    let b = inherited(doc, page_id, b"CropBox")
        .or_else(|| inherited(doc, page_id, b"MediaBox"))
        .and_then(|o| rect(doc, o))
        .unwrap_or([0.0, 0.0, 595.0, 842.0]);
    let rot = page_rotation(doc, page_id);
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    if rot % 180 == 0 { (b, rot, w, h) } else { (b, rot, h, w) }
}

/// A point as shown (from the bottom-left of the displayed page) in user space.
fn to_user(b: [f32; 4], rot: i64, x: f32, y: f32) -> (f32, f32) {
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    let (u, v) = match rot {
        90 => (w - y, x),
        180 => (w - x, h - y),
        270 => (y, h - x),
        _ => (x, y),
    };
    (b[0] + u, b[1] + v)
}

fn expand(t: &str, page: i64, pages: usize, file: &str) -> String {
    t.replace("{page}", &page.to_string())
        .replace("{pages}", &pages.to_string())
        .replace("{date}", &crate::annots::today())
        .replace("{file}", file)
}

/// Applies a mark to the given pages. Returns font notices.
pub fn apply(doc: &mut Document, pages: &[usize], mark: &Mark, file: &str) -> Result<Vec<String>, String> {
    let ids = crate::pages::page_ids(doc);
    let total = ids.len();
    let mut notices = Vec::new();
    for (n, &pi) in pages.iter().enumerate() {
        let pid = *ids.get(pi).ok_or("No such page")?;
        let (b, rot, dw, dh) = display_box(doc, pid);
        let angle_base = rot as f32;
        let mut put = |doc: &mut Document, text: &str, style: &TextStyle, x: f32, y: f32, align: f32, angle: f32, behind: bool| -> Result<(), String> {
            let p = to_user(b, rot, x, y);
            let mut more = place_text(doc, pid, text, &style.font, style.size, style.color, p, angle_base + angle, align, style.opacity, behind)?;
            notices.append(&mut more);
            Ok(())
        };
        match mark {
            Mark::Watermark { text, style, angle, behind } => {
                // Centred on the page: shift down by about half the cap height across the baseline.
                let a = angle.to_radians();
                let drop = style.size * 0.35;
                let (x, y) = (dw / 2.0 + a.sin() * drop, dh / 2.0 - a.cos() * drop);
                put(doc, text, style, x, y, 0.5, *angle, *behind)?;
            }
            Mark::HeaderFooter { header, footer, style, margin, first_number } => {
                let number = first_number + n as i64;
                for (row, texts) in [(dh - margin - style.size * 0.8, header), (*margin, footer)] {
                    for (k, t) in texts.iter().enumerate() {
                        let t = expand(t, number, total, file);
                        if t.trim().is_empty() {
                            continue;
                        }
                        let (x, align) = match k {
                            0 => (*margin, 0.0),
                            1 => (dw / 2.0, 0.5),
                            _ => (dw - margin, 1.0),
                        };
                        put(doc, &t, style, x, row, align, 0.0, false)?;
                    }
                }
            }
            Mark::Bates { prefix, suffix, start, digits, position, style, margin } => {
                let t = format!("{prefix}{:0width$}{suffix}", start + n as u64, width = *digits);
                let top = position.starts_with("top");
                let y = if top { dh - margin - style.size * 0.8 } else { *margin };
                let (x, align) = if position.ends_with("left") {
                    (*margin, 0.0)
                } else if position.ends_with("center") {
                    (dw / 2.0, 0.5)
                } else {
                    (dw - margin, 1.0)
                };
                put(doc, &t, style, x, y, align, 0.0, false)?;
            }
        }
    }
    notices.dedup();
    Ok(notices)
}

// ---------------------------------------------------------------------------
// Page labels
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabelRange {
    /// First page index of the range.
    pub start: usize,
    /// D (1, 2, 3), r (i, ii), R (I, II), a (a, b), A (A, B), or "" (prefix only).
    pub style: String,
    pub prefix: String,
    /// The number the range starts counting at.
    pub first: i64,
}

pub fn labels(doc: &Document) -> Vec<LabelRange> {
    let Some(tree) = doc.catalog().ok().and_then(|c| c.get(b"PageLabels").ok()).and_then(|o| dict(doc, o)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_nums(doc, tree, &mut out, 0);
    out.sort_by_key(|r| r.start);
    out
}

fn collect_nums(doc: &Document, node: &lopdf::Dictionary, out: &mut Vec<LabelRange>, depth: usize) {
    if depth > 16 {
        return;
    }
    if let Some(nums) = get_array(doc, node, b"Nums") {
        for pair in nums.chunks(2) {
            let (Some(k), Some(v)) = (pair.first().and_then(|o| num(deref(doc, o))), pair.get(1).and_then(|o| dict(doc, o))) else { continue };
            out.push(LabelRange {
                start: k as usize,
                style: get_name(doc, v, b"S").unwrap_or_default(),
                prefix: get(doc, v, b"P").and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default(),
                first: get_num(doc, v, b"St").unwrap_or(1.0) as i64,
            });
        }
    }
    if let Some(kids) = get_array(doc, node, b"Kids") {
        for k in kids {
            if let Some(d) = dict(doc, k) {
                collect_nums(doc, d, out, depth + 1);
            }
        }
    }
}

pub fn set_labels(doc: &mut Document, ranges: &[LabelRange]) -> Result<(), String> {
    let mut ranges = ranges.to_vec();
    ranges.sort_by_key(|r| r.start);
    ranges.dedup_by_key(|r| r.start);
    let cat = doc.catalog_mut().map_err(err)?;
    if ranges.is_empty() {
        cat.remove(b"PageLabels");
        return Ok(());
    }
    let mut nums = Vec::new();
    for r in &ranges {
        let mut d = lopdf::Dictionary::new();
        if !r.style.is_empty() {
            d.set("S", Object::Name(r.style.clone().into_bytes()));
        }
        if !r.prefix.is_empty() {
            d.set("P", lopdf::text_string(&r.prefix));
        }
        if r.first != 1 {
            d.set("St", r.first);
        }
        nums.push(Object::Integer(r.start as i64));
        nums.push(Object::Dictionary(d));
    }
    cat.set("PageLabels", dictionary! { "Nums" => nums });
    Ok(())
}

fn roman(mut n: i64) -> String {
    let table = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn letters(n: i64) -> String {
    // a..z, then aa..zz, as PDF readers count.
    let n = n.max(1) - 1;
    let c = (b'a' + (n % 26) as u8) as char;
    std::iter::repeat_n(c, (n / 26 + 1) as usize).collect()
}

/// The label each page shows, as readers compute it.
pub fn label_strings(ranges: &[LabelRange], pages: usize) -> Vec<String> {
    (0..pages)
        .map(|i| {
            let Some(r) = ranges.iter().rev().find(|r| r.start <= i) else { return (i + 1).to_string() };
            let n = r.first + (i - r.start) as i64;
            let num = match r.style.as_str() {
                "D" => n.to_string(),
                "r" => roman(n),
                "R" => roman(n).to_uppercase(),
                "a" => letters(n),
                "A" => letters(n).to_uppercase(),
                _ => String::new(),
            };
            format!("{}{num}", r.prefix)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_numbering() {
        let r = vec![
            LabelRange { start: 0, style: "r".into(), prefix: String::new(), first: 1 },
            LabelRange { start: 3, style: "D".into(), prefix: "A-".into(), first: 1 },
            LabelRange { start: 5, style: "a".into(), prefix: String::new(), first: 27 },
        ];
        assert_eq!(label_strings(&r, 6), vec!["i", "ii", "iii", "A-1", "A-2", "aa"]);
        assert_eq!(roman(1994), "mcmxciv");
    }

    #[test]
    fn rotated_mapping() {
        let b = [0.0, 0.0, 600.0, 800.0];
        // Shown bottom-left on a page turned 90° clockwise is the user's bottom-right.
        assert_eq!(to_user(b, 90, 0.0, 0.0), (600.0, 0.0));
        assert_eq!(to_user(b, 0, 10.0, 20.0), (10.0, 20.0));
        assert_eq!(to_user(b, 180, 0.0, 0.0), (600.0, 800.0));
        assert_eq!(to_user(b, 270, 0.0, 0.0), (0.0, 800.0));
    }
}
