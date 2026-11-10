//! Content stream interpretation.
//!
//! Walks a page's operators keeping the graphics and text state the way a
//! renderer would (CTM, text and line matrices, font, spacing, fill colour),
//! and records every text-showing operator as a [`Run`] with its exact
//! position in user space. Runs that continue each other on one baseline are
//! joined into [`TextLine`]s, which is the unit people edit.

use crate::fonts::{FontKind, PdfFont};
use crate::util::*;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// The fill colour as set in the content, so it can be restored exactly.
#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Gray(f32),
    Rgb(f32, f32, f32),
    Cmyk(f32, f32, f32, f32),
    Other { space: Vec<u8>, comps: Vec<Object> },
}

impl Fill {
    pub fn rgb(&self) -> [f32; 3] {
        match self {
            Fill::Gray(g) => [*g, *g, *g],
            Fill::Rgb(r, g, b) => [*r, *g, *b],
            Fill::Cmyk(c, m, y, k) => [(1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k)],
            Fill::Other { comps, .. } => {
                let v: Vec<f32> = comps.iter().filter_map(num).collect();
                match v.len() {
                    1 => [v[0]; 3],
                    3 => [v[0], v[1], v[2]],
                    4 => Fill::Cmyk(v[0], v[1], v[2], v[3]).rgb(),
                    _ => [0.0; 3],
                }
            }
        }
    }

    /// Operators that set this colour again.
    pub fn ops(&self) -> Vec<Operation> {
        match self {
            Fill::Gray(g) => vec![Operation::new("g", vec![real(*g)])],
            Fill::Rgb(r, g, b) => vec![Operation::new("rg", vec![real(*r), real(*g), real(*b)])],
            Fill::Cmyk(c, m, y, k) => vec![Operation::new("k", vec![real(*c), real(*m), real(*y), real(*k)])],
            Fill::Other { space, comps } => vec![
                Operation::new("cs", vec![Object::Name(space.clone())]),
                Operation::new("scn", comps.clone()),
            ],
        }
    }
}

#[derive(Clone)]
struct GState {
    ctm: Matrix,
    fill: Fill,
    fill_space: Vec<u8>,
    font: Option<(Vec<u8>, Arc<PdfFont>)>,
    size: f32,
    tc: f32,
    tw: f32,
    th: f32,
    tl: f32,
    rise: f32,
    mode: i64,
}

impl Default for GState {
    fn default() -> Self {
        GState {
            ctm: IDENTITY,
            fill: Fill::Gray(0.0),
            fill_space: b"DeviceGray".to_vec(),
            font: None,
            size: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            rise: 0.0,
            mode: 0,
        }
    }
}

/// One text-showing operator.
#[derive(Clone)]
pub struct Run {
    pub op: usize,
    pub font: Arc<PdfFont>,
    pub res_name: Vec<u8>,
    pub size: f32,
    pub text: String,
    /// Baseline start and end in user space.
    pub start: (f32, f32),
    pub end: (f32, f32),
    /// Unit vectors along and up the baseline in user space.
    pub dir: (f32, f32),
    pub up: (f32, f32),
    /// Font size in user-space units.
    pub em: f32,
    pub fill: Fill,
    pub mode: i64,
    /// Text matrix where the glyphs start, and the line matrix then.
    pub tm: Matrix,
    pub tlm: Matrix,
    pub ctm: Matrix,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextLine {
    pub id: usize,
    pub ops: Vec<usize>,
    pub text: String,
    pub font_key: String,
    pub font_name: String,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub size: f32,
    /// Size as drawn, in points.
    pub em: f32,
    /// Corners in user space: bottom-left, bottom-right, top-right, top-left.
    pub quad: [[f32; 2]; 4],
    pub origin: [f32; 2],
    /// Baseline angle in degrees, counter-clockwise.
    pub angle: f32,
    pub color: [f32; 3],
    pub invisible: bool,
    pub editable: bool,
    pub note: Option<String>,
}

pub struct Scan {
    pub ops: Vec<Operation>,
    pub runs: Vec<Run>,
    pub lines: Vec<TextLine>,
    /// Whether content can be appended without wrapping the page in q/Q.
    pub clean_end: bool,
}

/// The page's resources, following inheritance.
pub fn page_resources(doc: &Document, page_id: ObjectId) -> Dictionary {
    match inherited(doc, page_id, b"Resources") {
        Some(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    }
}

/// Fonts named in a resource dictionary.
pub fn resource_fonts(doc: &Document, res: &Dictionary, scope: &str) -> HashMap<Vec<u8>, Arc<PdfFont>> {
    let mut out = HashMap::new();
    if let Some(Object::Dictionary(fonts)) = get(doc, res, b"Font") {
        for (name, obj) in fonts.iter() {
            let key = match obj.as_reference() {
                Ok((n, g)) => format!("{n} {g}"),
                Err(_) => format!("{scope}/{}", String::from_utf8_lossy(name)),
            };
            out.insert(name.clone(), Arc::new(PdfFont::load(doc, obj, key)));
        }
    }
    out
}

pub fn page_content_ops(doc: &Document, page_id: ObjectId) -> Result<Vec<Operation>, String> {
    let bytes = doc.get_page_content(page_id);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    Content::decode(&bytes).map(|c| c.operations).map_err(|e| format!("The page content could not be read: {e}"))
}

pub fn scan_page(doc: &Document, page_id: ObjectId) -> Result<Scan, String> {
    let ops = page_content_ops(doc, page_id)?;
    let res = page_resources(doc, page_id);
    let fonts = resource_fonts(doc, &res, &format!("p{}", page_id.0));
    let (runs, clean_end) = interpret(&ops, &fonts);
    let lines = group(&runs);
    Ok(Scan { ops, runs, lines, clean_end })
}

fn fill_from(space: &[u8], comps: &[Object]) -> Fill {
    let v: Vec<f32> = comps.iter().filter_map(num).collect();
    match (space, v.len()) {
        (b"DeviceGray", 1) => Fill::Gray(v[0]),
        (b"DeviceRGB", 3) => Fill::Rgb(v[0], v[1], v[2]),
        (b"DeviceCMYK", 4) => Fill::Cmyk(v[0], v[1], v[2], v[3]),
        _ => Fill::Other { space: space.to_vec(), comps: comps.to_vec() },
    }
}

fn interpret(ops: &[Operation], fonts: &HashMap<Vec<u8>, Arc<PdfFont>>) -> (Vec<Run>, bool) {
    let mut gs = GState::default();
    let mut stack: Vec<GState> = Vec::new();
    let mut tm = IDENTITY;
    let mut tlm = IDENTITY;
    let mut runs = Vec::new();
    let n = |o: &Operation, i: usize| o.operands.get(i).and_then(num).unwrap_or(0.0);

    for (idx, op) in ops.iter().enumerate() {
        match op.operator.as_str() {
            "q" => stack.push(gs.clone()),
            "Q" => {
                if let Some(s) = stack.pop() {
                    gs = s;
                }
            }
            "cm" if op.operands.len() == 6 => {
                let m = [n(op, 0), n(op, 1), n(op, 2), n(op, 3), n(op, 4), n(op, 5)];
                gs.ctm = mul(&m, &gs.ctm);
            }
            "BT" => {
                tm = IDENTITY;
                tlm = IDENTITY;
            }
            "Tf" => {
                if let Some(Object::Name(name)) = op.operands.first() {
                    gs.font = fonts.get(name).map(|f| (name.clone(), f.clone()));
                    if gs.font.is_none() {
                        gs.font = Some((name.clone(), Arc::new(PdfFont::load(&Document::new(), &Object::Null, "missing".into()))));
                    }
                }
                gs.size = n(op, 1);
            }
            "Td" => {
                tlm = mul(&translate(n(op, 0), n(op, 1)), &tlm);
                tm = tlm;
            }
            "TD" => {
                gs.tl = -n(op, 1);
                tlm = mul(&translate(n(op, 0), n(op, 1)), &tlm);
                tm = tlm;
            }
            "Tm" if op.operands.len() == 6 => {
                tlm = [n(op, 0), n(op, 1), n(op, 2), n(op, 3), n(op, 4), n(op, 5)];
                tm = tlm;
            }
            "T*" => {
                tlm = mul(&translate(0.0, -gs.tl), &tlm);
                tm = tlm;
            }
            "Tc" => gs.tc = n(op, 0),
            "Tw" => gs.tw = n(op, 0),
            "Tz" => gs.th = n(op, 0) / 100.0,
            "TL" => gs.tl = n(op, 0),
            "Ts" => gs.rise = n(op, 0),
            "Tr" => gs.mode = n(op, 0) as i64,
            "g" => {
                gs.fill = Fill::Gray(n(op, 0));
                gs.fill_space = b"DeviceGray".to_vec();
            }
            "rg" => {
                gs.fill = Fill::Rgb(n(op, 0), n(op, 1), n(op, 2));
                gs.fill_space = b"DeviceRGB".to_vec();
            }
            "k" => {
                gs.fill = Fill::Cmyk(n(op, 0), n(op, 1), n(op, 2), n(op, 3));
                gs.fill_space = b"DeviceCMYK".to_vec();
            }
            "cs" => {
                if let Some(Object::Name(cs)) = op.operands.first() {
                    gs.fill_space = cs.clone();
                    gs.fill = match cs.as_slice() {
                        b"DeviceRGB" => Fill::Rgb(0.0, 0.0, 0.0),
                        b"DeviceCMYK" => Fill::Cmyk(0.0, 0.0, 0.0, 1.0),
                        b"DeviceGray" => Fill::Gray(0.0),
                        _ => Fill::Other { space: cs.clone(), comps: vec![Object::Integer(0)] },
                    };
                }
            }
            "sc" | "scn" => gs.fill = fill_from(&gs.fill_space, &op.operands),
            "Tj" | "TJ" | "'" | "\"" => {
                if op.operator == "'" || op.operator == "\"" {
                    if op.operator == "\"" {
                        gs.tw = n(op, 0);
                        gs.tc = n(op, 1);
                    }
                    tlm = mul(&translate(0.0, -gs.tl), &tlm);
                    tm = tlm;
                }
                let Some((res_name, font)) = gs.font.clone() else { continue };
                let items: Vec<&Object> = match op.operator.as_str() {
                    "TJ" => match op.operands.first() {
                        Some(Object::Array(a)) => a.iter().collect(),
                        _ => Vec::new(),
                    },
                    "\"" => op.operands.get(2).into_iter().collect(),
                    _ => op.operands.first().into_iter().collect(),
                };
                let start_tm = tm;
                let mut text = String::new();
                for item in items {
                    match item {
                        Object::String(bytes, _) => {
                            for g in font.decode(bytes) {
                                let w0 = g.width * font.width_scale;
                                let space = if font.code_len == 1 && g.code == 32 { gs.tw } else { 0.0 };
                                let tx = (w0 * gs.size + gs.tc + space) * gs.th;
                                tm = mul(&translate(tx, 0.0), &tm);
                                text.push_str(&g.text);
                            }
                        }
                        o => {
                            if let Some(adj) = num(o) {
                                let tx = -adj / 1000.0 * gs.size * gs.th;
                                tm = mul(&translate(tx, 0.0), &tm);
                                if adj < -180.0 && !text.is_empty() && !text.ends_with(' ') {
                                    text.push(' ');
                                }
                            }
                        }
                    }
                }
                let trm0 = mul(&start_tm, &gs.ctm);
                let trm1 = mul(&tm, &gs.ctm);
                let start = apply(&trm0, 0.0, gs.rise);
                let end = apply(&trm1, 0.0, gs.rise);
                let d = apply_vec(&trm0, 1.0, 0.0);
                let u = apply_vec(&trm0, 0.0, 1.0);
                let (dl, ul) = (len(d).max(1e-6), len(u).max(1e-6));
                runs.push(Run {
                    op: idx,
                    font,
                    res_name,
                    size: gs.size,
                    text,
                    start,
                    end,
                    dir: (d.0 / dl, d.1 / dl),
                    up: (u.0 / ul, u.1 / ul),
                    em: (gs.size * ul).abs(),
                    fill: gs.fill.clone(),
                    mode: gs.mode,
                    tm: start_tm,
                    tlm,
                    ctm: gs.ctm,
                });
            }
            _ => {}
        }
    }
    let clean = stack.is_empty() && gs.ctm == IDENTITY;
    (runs, clean)
}

fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

/// Joins runs that continue each other on one baseline into lines.
fn group(runs: &[Run]) -> Vec<TextLine> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, r) in runs.iter().enumerate() {
        if r.text.trim().is_empty() && r.text.is_empty() {
            continue;
        }
        if let Some(g) = groups.last_mut() {
            let prev = &runs[*g.last().unwrap()];
            let same_style = prev.font.key == r.font.key
                && (prev.size - r.size).abs() < 0.01
                && (prev.em - r.em).abs() < 0.05
                && prev.fill == r.fill
                && prev.mode == r.mode;
            let parallel = (prev.dir.0 * r.dir.1 - prev.dir.1 * r.dir.0).abs() < 0.02 && dot(prev.dir, r.dir) > 0.0;
            let v = (r.start.0 - prev.end.0, r.start.1 - prev.end.1);
            let along = dot(v, prev.dir);
            let perp = dot(v, prev.up);
            let em = prev.em.max(0.5);
            if same_style && parallel && perp.abs() < 0.2 * em && along > -0.6 * em && along < 2.5 * em {
                g.push(i);
                continue;
            }
        }
        groups.push(vec![i]);
    }

    groups
        .into_iter()
        .filter_map(|g| {
            let first = &runs[g[0]];
            let last = &runs[*g.last().unwrap()];
            let mut text = String::new();
            for (k, &ri) in g.iter().enumerate() {
                let r = &runs[ri];
                if k > 0 {
                    let prev = &runs[g[k - 1]];
                    let along = dot((r.start.0 - prev.end.0, r.start.1 - prev.end.1), prev.dir);
                    if along > 0.18 * prev.em && !text.ends_with(' ') && !r.text.starts_with(' ') {
                        text.push(' ');
                    }
                }
                text.push_str(&r.text);
            }
            if text.trim().is_empty() {
                return None;
            }
            let f = &first.font;
            let asc = f.ascent / 1000.0 * first.em;
            let desc = f.descent / 1000.0 * first.em;
            let (s, e, u) = (first.start, last.end, first.up);
            let quad = [
                [s.0 + u.0 * desc, s.1 + u.1 * desc],
                [e.0 + u.0 * desc, e.1 + u.1 * desc],
                [e.0 + u.0 * asc, e.1 + u.1 * asc],
                [s.0 + u.0 * asc, s.1 + u.1 * asc],
            ];
            let invisible = first.mode == 3 || first.mode == 7;
            let (editable, note) = if f.key == "missing" {
                (false, Some("The font this text uses is missing from the page resources.".to_string()))
            } else if f.kind == FontKind::Type3 {
                (false, Some("Type 3 fonts are drawn with PDF operators and cannot be edited yet.".to_string()))
            } else if invisible {
                (true, Some("Invisible text: usually the OCR layer of a scanned page.".to_string()))
            } else {
                (true, None)
            };
            Some(TextLine {
                id: 0,
                ops: g.iter().map(|&ri| runs[ri].op).collect(),
                text: text.trim_end().to_string(),
                font_key: f.key.clone(),
                font_name: f.ps_name.clone(),
                family: f.family.clone(),
                bold: f.bold,
                italic: f.italic,
                size: first.size,
                em: first.em,
                quad,
                origin: [s.0, s.1],
                angle: first.dir.1.atan2(first.dir.0).to_degrees(),
                color: first.fill.rgb(),
                invisible,
                editable,
                note,
            })
        })
        .enumerate()
        .map(|(i, mut l)| {
            l.id = i;
            l
        })
        .collect()
}

/// The run behind each op index.
pub fn run_at(runs: &[Run], op: usize) -> Option<&Run> {
    runs.iter().find(|r| r.op == op)
}
