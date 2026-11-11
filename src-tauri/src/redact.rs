//! True redaction.
//!
//! Covering text with a black box hides it on screen but leaves it in the
//! file, where anyone can copy it out. Redaction here removes what lies under
//! each box from the content itself:
//!
//! * glyphs are taken out of their text operators and replaced by an equal
//!   gap, so the text around them does not move;
//! * images entirely inside a box are removed; partly covered images get the
//!   covered pixels painted black in a new copy of the image;
//! * vector paths entirely inside a box are removed;
//! * annotations touching a box are deleted.
//!
//! Then the box itself is painted, if a colour is given.

use crate::edit::{add_resource, add_page_ops, set_ops};
use crate::interp::{scan_page, RItem, Src};
use crate::util::*;
use lopdf::content::Operation;
use lopdf::{Document, Object, ObjectId, StringFormat};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Redacted {
    pub glyphs: usize,
    pub images_removed: usize,
    pub images_changed: usize,
    pub paths: usize,
    pub annotations: usize,
}

impl Redacted {
    pub fn add(&mut self, o: &Redacted) {
        self.glyphs += o.glyphs;
        self.images_removed += o.images_removed;
        self.images_changed += o.images_changed;
        self.paths += o.paths;
        self.annotations += o.annotations;
    }
}

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

fn inside(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] >= b[0] - 0.5 && a[1] >= b[1] - 0.5 && a[2] <= b[2] + 0.5 && a[3] <= b[3] + 0.5
}

fn norm_rect(r: [f32; 4]) -> [f32; 4] {
    [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])]
}

pub fn redact(doc: &mut Document, page_id: ObjectId, rects: &[[f32; 4]], fill: Option<[f32; 3]>) -> Result<Redacted, String> {
    let rects: Vec<[f32; 4]> = rects.iter().map(|r| norm_rect(*r)).filter(|r| r[2] - r[0] > 0.1 && r[3] - r[1] > 0.1).collect();
    if rects.is_empty() {
        return Err("Mark an area to redact first.".into());
    }
    let scan = scan_page(doc, page_id)?;
    let mut out = Redacted::default();
    let mut replace: HashMap<(Src, usize), Vec<Operation>> = HashMap::new();

    // Text, glyph by glyph.
    for run in &scan.runs {
        let f = &run.font;
        let (asc, desc) = (f.ascent / 1000.0 * run.em, f.descent / 1000.0 * run.em);
        let gbox = |from: f32, to: f32| -> [f32; 4] {
            // The middle 60 % of the glyph's advance, so neighbours just touching a box survive.
            let (a, b) = (from + (to - from) * 0.2, to - (to - from) * 0.2);
            let pts = [(a, desc), (b, desc), (b, asc), (a, asc)].map(|(s, v)| {
                (run.start.0 + run.dir.0 * s + run.up.0 * v, run.start.1 + run.dir.1 * s + run.up.1 * v)
            });
            [
                pts.iter().map(|p| p.0).fold(f32::MAX, f32::min),
                pts.iter().map(|p| p.1).fold(f32::MAX, f32::min),
                pts.iter().map(|p| p.0).fold(f32::MIN, f32::max),
                pts.iter().map(|p| p.1).fold(f32::MIN, f32::max),
            ]
        };
        let hit = |from: f32, to: f32| rects.iter().any(|r| overlaps(gbox(from, to), *r));
        let removed = run.items.iter().filter(|i| matches!(i, RItem::Glyph { from, to, .. } if hit(*from, *to))).count();
        if removed == 0 {
            continue;
        }
        out.glyphs += removed;
        let format = if f.code_len == 2 { StringFormat::Hexadecimal } else { StringFormat::Literal };
        let scale = (run.size * run.th).abs().max(1e-6);
        let mut arr: Vec<Object> = Vec::new();
        let mut cur: Vec<u8> = Vec::new();
        let mut gap = 0.0f32;
        let flush = |arr: &mut Vec<Object>, cur: &mut Vec<u8>, gap: &mut f32| {
            if !cur.is_empty() {
                arr.push(Object::String(std::mem::take(cur), format));
            }
            if gap.abs() > 1e-4 {
                arr.push(real(*gap));
                *gap = 0.0;
            }
        };
        for it in &run.items {
            match it {
                RItem::Glyph { bytes, from, to, tx } => {
                    if hit(*from, *to) {
                        if !cur.is_empty() {
                            arr.push(Object::String(std::mem::take(&mut cur), format));
                        }
                        gap += -tx * 1000.0 / scale;
                    } else {
                        if gap.abs() > 1e-4 {
                            arr.push(real(gap));
                            gap = 0.0;
                        }
                        cur.extend_from_slice(bytes);
                    }
                }
                RItem::Adjust(a) => {
                    if !cur.is_empty() {
                        arr.push(Object::String(std::mem::take(&mut cur), format));
                    }
                    gap += a;
                }
            }
        }
        flush(&mut arr, &mut cur, &mut gap);
        let op = &scan.ops[&run.src][run.op];
        let mut ops = Vec::new();
        match op.operator.as_str() {
            "'" => ops.push(Operation::new("T*", vec![])),
            "\"" => {
                ops.push(Operation::new("Tw", vec![op.operands.first().cloned().unwrap_or(Object::Integer(0))]));
                ops.push(Operation::new("Tc", vec![op.operands.get(1).cloned().unwrap_or(Object::Integer(0))]));
                ops.push(Operation::new("T*", vec![]));
            }
            _ => {}
        }
        ops.push(Operation::new("TJ", vec![Object::Array(arr)]));
        replace.insert((run.src, run.op), ops);
    }

    // Images.
    for im in &scan.images {
        let b = im.bbox();
        if !rects.iter().any(|r| overlaps(b, *r)) {
            continue;
        }
        if rects.iter().any(|r| inside(b, *r)) || im.id.is_none() {
            replace.insert((im.src, im.op), Vec::new());
            out.images_removed += 1;
            continue;
        }
        let id = im.id.unwrap();
        match crate::images::decode(doc, id) {
            Ok(mut img) => {
                let (w, h) = (img.width(), img.height());
                for py in 0..h {
                    for px in 0..w {
                        // Pixel centre in image space (row 0 is the top), then on the page.
                        let u = (px as f32 + 0.5) / w as f32;
                        let v = 1.0 - (py as f32 + 0.5) / h as f32;
                        let (x, y) = apply(&im.ctm, u, v);
                        if rects.iter().any(|r| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]) {
                            img.put_pixel(px, py, image::Rgba([0, 0, 0, 255]));
                        }
                    }
                }
                let new_id = crate::images::object_from_rgba(doc, &img);
                let name = add_resource(doc, page_id, im.src, "XObject", new_id, "InqRx")?;
                replace.insert((im.src, im.op), vec![Operation::new("Do", vec![Object::Name(name)])]);
                out.images_changed += 1;
            }
            Err(_) => {
                // Cannot be edited safely: remove it entirely.
                replace.insert((im.src, im.op), Vec::new());
                out.images_removed += 1;
            }
        }
    }

    // Vector paths entirely under a box.
    for p in &scan.paths {
        if p.bbox[0] > p.bbox[2] {
            continue;
        }
        if rects.iter().any(|r| inside(p.bbox, *r)) {
            for i in p.first..=p.last {
                replace.insert((p.src, i), Vec::new());
            }
            out.paths += 1;
        }
    }

    // Write the streams back.
    let mut srcs: Vec<Src> = replace.keys().map(|k| k.0).collect();
    srcs.sort_by_key(|s| match s {
        Src::Page => (0, 0, 0),
        Src::Form((n, g)) => (1, *n, *g),
    });
    srcs.dedup();
    for src in srcs {
        let ops = scan.ops.get(&src).cloned().unwrap_or_default();
        let mut new_ops = Vec::with_capacity(ops.len());
        for (i, op) in ops.into_iter().enumerate() {
            match replace.get(&(src, i)) {
                Some(r) => new_ops.extend(r.iter().cloned()),
                None => new_ops.push(op),
            }
        }
        set_ops(doc, page_id, src, new_ops)?;
    }

    // Annotations under a box.
    let arr = crate::annots::annots_array(doc, page_id);
    let before = arr.len();
    let keep: Vec<Object> = arr
        .into_iter()
        .filter(|a| {
            let r = dict(doc, a).and_then(|d| d.get(b"Rect").ok()).and_then(|o| rect(doc, o));
            !matches!(r, Some(r) if rects.iter().any(|x| overlaps(r, *x)))
        })
        .collect();
    out.annotations = before - keep.len();
    crate::annots::set_annots(doc, page_id, keep)?;

    // The boxes themselves.
    if let Some([r, g, b]) = fill {
        let mut ops = vec![Operation::new("q", vec![]), Operation::new("rg", vec![real(r), real(g), real(b)])];
        for x in &rects {
            ops.push(Operation::new("re", vec![real(x[0]), real(x[1]), real(x[2] - x[0]), real(x[3] - x[1])]));
            ops.push(Operation::new("f", vec![]));
        }
        ops.push(Operation::new("Q", vec![]));
        add_page_ops(doc, page_id, ops, false)?;
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub page: usize,
    /// One quad per match (user space: bottom-left, bottom-right, top-right, top-left).
    pub quads: Vec<[[f32; 2]; 4]>,
}

/// Every occurrence of `query` (case-insensitive, within a line), as quads.
pub fn find(doc: &Document, query: &str) -> Result<Vec<Found>, String> {
    let q: Vec<char> = query.to_lowercase().chars().collect();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (pi, pid) in crate::pages::page_ids(doc).into_iter().enumerate() {
        let scan = scan_page(doc, pid)?;
        let mut quads = Vec::new();
        for l in &scan.lines {
            let t: Vec<char> = l.text.to_lowercase().chars().collect();
            if t.len() != l.chars.len() - 1 || t.len() < q.len() {
                continue;
            }
            let total = *l.chars.last().unwrap_or(&1.0);
            let mut i = 0;
            while i + q.len() <= t.len() {
                if t[i..i + q.len()] == q[..] {
                    let (a, b) = (l.chars[i] / total.max(1e-3), l.chars[i + q.len()] / total.max(1e-3));
                    let [bl, br, tr, tl] = l.quad;
                    let lerp = |p: [f32; 2], r: [f32; 2], t: f32| [p[0] + (r[0] - p[0]) * t, p[1] + (r[1] - p[1]) * t];
                    quads.push([lerp(bl, br, a), lerp(bl, br, b), lerp(tl, tr, b), lerp(tl, tr, a)]);
                    i += q.len();
                } else {
                    i += 1;
                }
            }
        }
        if !quads.is_empty() {
            out.push(Found { page: pi, quads });
        }
    }
    Ok(out)
}
