//! Content stream interpretation.
//!
//! Walks a page's operators keeping the graphics and text state the way a
//! renderer would (CTM, text and line matrices, font, spacing, fill colour),
//! following Form XObjects into their own streams, and records every
//! text-showing operator as a [`Run`] with its exact position in user space
//! and the position of every character.
//!
//! Runs that continue each other on one baseline become [`TextLine`]s, even
//! when they switch font, size or colour part-way (a bold word in a sentence).
//! Lines that stack with even spacing and aligned edges become [`Block`]s:
//! the paragraphs that are edited and reflowed as a whole.

use crate::fonts::{FontKind, PdfFont};
use crate::util::*;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Which content stream an operator lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Src {
    Page,
    Form(ObjectId),
}

impl Src {
    pub fn label(&self) -> String {
        match self {
            Src::Page => "page".into(),
            Src::Form((n, g)) => format!("form {n} {g}"),
        }
    }
}

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

    /// The same colour without references to named colour spaces, for
    /// content written outside the stream that defined them.
    pub fn portable(&self) -> Fill {
        match self {
            Fill::Other { .. } => {
                let [r, g, b] = self.rgb();
                Fill::Rgb(r, g, b)
            }
            f => f.clone(),
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
    pub src: Src,
    pub op: usize,
    pub font: Arc<PdfFont>,
    /// Font resource name, in the resources of `src`.
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
    pub tc: f32,
    pub th: f32,
    /// Character boundaries along the baseline from `start` (one more than characters).
    pub chars: Vec<f32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    /// Character range in the line's text.
    pub start: usize,
    pub end: usize,
    pub font_key: String,
    pub font_name: String,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub size: f32,
    pub em: f32,
    pub color: [f32; 3],
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextLine {
    pub id: usize,
    pub src: String,
    pub ops: Vec<usize>,
    pub text: String,
    /// Runs of one style within the line.
    pub spans: Vec<Span>,
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
    /// Character boundaries along the baseline from `origin`, in points.
    pub chars: Vec<f32>,
    /// The paragraph this line belongs to.
    pub block: usize,
    /// Indexes into the scan's runs.
    #[serde(skip)]
    pub runs: Vec<usize>,
    /// For every character of `text`, the run that drew it.
    #[serde(skip)]
    pub char_runs: Vec<usize>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: usize,
    pub lines: Vec<usize>,
    /// The paragraph's text, lines joined and hyphenation removed.
    pub text: String,
    /// left | right | center | justify
    pub align: String,
    /// Column width in points (the longest line, measured from the left edge).
    pub width: f32,
    /// Baseline-to-baseline distance in points.
    pub leading: f32,
    /// Corners around the whole paragraph, like a line's quad.
    pub quad: [[f32; 2]; 4],
    /// x of the first line, then of the others, along the baseline from the first line's origin.
    #[serde(skip)]
    pub first_x: f32,
    #[serde(skip)]
    pub left: f32,
    /// For every character of `text`: (line, character in that line); `None` for joining spaces.
    #[serde(skip)]
    pub char_src: Vec<Option<(usize, usize)>>,
}

pub struct Scan {
    pub ops: HashMap<Src, Vec<Operation>>,
    pub runs: Vec<Run>,
    pub lines: Vec<TextLine>,
    pub blocks: Vec<Block>,
    /// Whether content can be appended to the page without wrapping it in q/Q.
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

/// Fonts used inside a page's Form XObjects (for the font inventory).
pub fn form_fonts(doc: &Document, page_id: ObjectId) -> Vec<Arc<PdfFont>> {
    scan_page(doc, page_id)
        .map(|s| s.runs.iter().filter(|r| r.src != Src::Page).map(|r| r.font.clone()).collect())
        .unwrap_or_default()
}

pub fn page_content_ops(doc: &Document, page_id: ObjectId) -> Result<Vec<Operation>, String> {
    let bytes = doc.get_page_content(page_id);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    Content::decode(&bytes).map(|c| c.operations).map_err(|e| format!("The page content could not be read: {e}"))
}

pub fn form_ops(doc: &Document, id: ObjectId) -> Option<Vec<Operation>> {
    let bytes = stream_bytes(doc, &Object::Reference(id))?;
    Content::decode(&bytes).ok().map(|c| c.operations)
}

struct Ctx<'a> {
    doc: &'a Document,
    runs: Vec<Run>,
    ops: HashMap<Src, Vec<Operation>>,
    form_uses: HashMap<ObjectId, usize>,
    font_cache: HashMap<ObjectId, Arc<PdfFont>>,
    stack: Vec<ObjectId>,
}

impl Ctx<'_> {
    fn fonts(&mut self, res: &Dictionary, scope: &str) -> HashMap<Vec<u8>, Arc<PdfFont>> {
        let mut out = HashMap::new();
        if let Some(Object::Dictionary(fonts)) = get(self.doc, res, b"Font") {
            for (name, obj) in fonts.iter() {
                let f = match obj.as_reference() {
                    Ok(id) => self
                        .font_cache
                        .entry(id)
                        .or_insert_with(|| Arc::new(PdfFont::load(self.doc, obj, format!("{} {}", id.0, id.1))))
                        .clone(),
                    Err(_) => Arc::new(PdfFont::load(self.doc, obj, format!("{scope}/{}", String::from_utf8_lossy(name)))),
                };
                out.insert(name.clone(), f);
            }
        }
        out
    }
}

pub fn scan_page(doc: &Document, page_id: ObjectId) -> Result<Scan, String> {
    let ops = page_content_ops(doc, page_id)?;
    let res = page_resources(doc, page_id);
    let mut ctx = Ctx {
        doc,
        runs: Vec::new(),
        ops: HashMap::new(),
        form_uses: HashMap::new(),
        font_cache: HashMap::new(),
        stack: Vec::new(),
    };
    let fonts = ctx.fonts(&res, &format!("p{}", page_id.0));
    let end = interpret(&mut ctx, &ops, Src::Page, &res, &fonts, GState::default());
    ctx.ops.insert(Src::Page, ops);
    let clean_end = end.0 && end.1 == IDENTITY;
    let mut lines = group(&ctx.runs, &ctx.form_uses);
    let blocks = paragraphs(&mut lines);
    Ok(Scan { ops: ctx.ops, runs: ctx.runs, lines, blocks, clean_end })
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

/// Returns (graphics stack balanced, CTM at the end).
fn interpret(
    ctx: &mut Ctx,
    ops: &[Operation],
    src: Src,
    res: &Dictionary,
    fonts: &HashMap<Vec<u8>, Arc<PdfFont>>,
    start: GState,
) -> (bool, Matrix) {
    let mut gs = start;
    let mut stack: Vec<GState> = Vec::new();
    let mut tm = IDENTITY;
    let mut tlm = IDENTITY;
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
                    gs.font = Some((
                        name.clone(),
                        fonts.get(name).cloned().unwrap_or_else(|| {
                            Arc::new(PdfFont::load(&Document::new(), &Object::Null, "missing".into()))
                        }),
                    ));
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
            "Do" => {
                let Some(Object::Name(name)) = op.operands.first() else { continue };
                form(ctx, res, name, &gs);
            }
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
                let trm0 = mul(&start_tm, &gs.ctm);
                let start = apply(&trm0, 0.0, gs.rise);
                let d = apply_vec(&trm0, 1.0, 0.0);
                let u = apply_vec(&trm0, 0.0, 1.0);
                let (dl, ul) = (len(d).max(1e-6), len(u).max(1e-6));
                let dir = (d.0 / dl, d.1 / dl);
                let along = |m: &Matrix| -> f32 {
                    let p = apply(&mul(m, &gs.ctm), 0.0, gs.rise);
                    (p.0 - start.0) * dir.0 + (p.1 - start.1) * dir.1
                };
                let mut text = String::new();
                let mut chars: Vec<f32> = vec![0.0];
                for item in items {
                    match item {
                        Object::String(bytes, _) => {
                            for g in font.decode(bytes) {
                                let w0 = g.width * font.width_scale;
                                let space = if font.code_len == 1 && g.code == 32 { gs.tw } else { 0.0 };
                                let tx = (w0 * gs.size + gs.tc + space) * gs.th;
                                let before = *chars.last().unwrap();
                                tm = mul(&translate(tx, 0.0), &tm);
                                let after = along(&tm);
                                let k = g.text.chars().count().max(1);
                                for i in 0..g.text.chars().count() {
                                    chars.push(before + (after - before) * (i + 1) as f32 / k as f32);
                                }
                                text.push_str(&g.text);
                            }
                        }
                        o => {
                            if let Some(adj) = num(o) {
                                let tx = -adj / 1000.0 * gs.size * gs.th;
                                tm = mul(&translate(tx, 0.0), &tm);
                                let after = along(&tm);
                                if adj < -180.0 && !text.is_empty() && !text.ends_with(' ') {
                                    text.push(' ');
                                    chars.push(after);
                                } else if let Some(last) = chars.last_mut() {
                                    // The gap belongs to the space before it, if any; otherwise
                                    // it simply moves the next character.
                                    if text.ends_with(' ') {
                                        *last = after;
                                    }
                                }
                            }
                        }
                    }
                }
                let end = apply(&mul(&tm, &gs.ctm), 0.0, gs.rise);
                ctx.runs.push(Run {
                    src,
                    op: idx,
                    font,
                    res_name,
                    size: gs.size,
                    text,
                    start,
                    end,
                    dir,
                    up: (u.0 / ul, u.1 / ul),
                    em: (gs.size * ul).abs(),
                    fill: gs.fill.clone(),
                    mode: gs.mode,
                    tm: start_tm,
                    tlm,
                    ctm: gs.ctm,
                    tc: gs.tc,
                    th: gs.th,
                    chars,
                });
            }
            _ => {}
        }
    }
    (stack.is_empty(), gs.ctm)
}

/// Follows `Do` into a Form XObject.
fn form(ctx: &mut Ctx, res: &Dictionary, name: &[u8], gs: &GState) {
    let doc = ctx.doc;
    let Some(Object::Dictionary(xobjects)) = get(doc, res, b"XObject") else { return };
    let Ok(Object::Reference(id)) = xobjects.get(name) else { return };
    let id = *id;
    let Ok(Object::Stream(stream)) = doc.get_object(id) else { return };
    if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
        return;
    }
    if ctx.stack.contains(&id) || ctx.stack.len() >= 6 {
        return;
    }
    *ctx.form_uses.entry(id).or_insert(0) += 1;
    let ops = match ctx.ops.get(&Src::Form(id)) {
        Some(o) => o.clone(),
        None => match form_ops(doc, id) {
            Some(o) => o,
            None => return,
        },
    };
    let matrix = get_array(doc, &stream.dict, b"Matrix")
        .map(|a| a.iter().filter_map(|x| num(deref(doc, x))).collect::<Vec<_>>())
        .filter(|v| v.len() == 6)
        .map(|v| [v[0], v[1], v[2], v[3], v[4], v[5]])
        .unwrap_or(IDENTITY);
    let form_res = match get(doc, &stream.dict, b"Resources") {
        Some(Object::Dictionary(d)) => d.clone(),
        _ => res.clone(),
    };
    let fonts = ctx.fonts(&form_res, &format!("f{}", id.0));
    let mut child = gs.clone();
    child.ctm = mul(&matrix, &gs.ctm);
    ctx.stack.push(id);
    interpret(ctx, &ops, Src::Form(id), &form_res, &fonts, child);
    ctx.stack.pop();
    ctx.ops.entry(Src::Form(id)).or_insert(ops);
}

fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

/// Joins runs that continue each other on one baseline into lines.
fn group(runs: &[Run], form_uses: &HashMap<ObjectId, usize>) -> Vec<TextLine> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, r) in runs.iter().enumerate() {
        if r.text.is_empty() {
            continue;
        }
        if let Some(g) = groups.last_mut() {
            let prev = &runs[*g.last().unwrap()];
            let visible = |m: i64| m != 3 && m != 7;
            let parallel = (prev.dir.0 * r.dir.1 - prev.dir.1 * r.dir.0).abs() < 0.02 && dot(prev.dir, r.dir) > 0.0;
            let v = (r.start.0 - prev.end.0, r.start.1 - prev.end.1);
            let along = dot(v, prev.dir);
            let perp = dot(v, prev.up);
            let em = prev.em.max(r.em).max(0.5);
            if prev.src == r.src
                && visible(prev.mode) == visible(r.mode)
                && parallel
                && perp.abs() < 0.25 * em
                && along > -0.6 * em
                && along < 2.5 * em
            {
                g.push(i);
                continue;
            }
        }
        groups.push(vec![i]);
    }

    let mut lines = Vec::new();
    for g in groups {
        let first = &runs[g[0]];
        let last = &runs[*g.last().unwrap()];
        let mut text = String::new();
        let mut chars: Vec<f32> = vec![0.0];
        let mut char_runs: Vec<usize> = Vec::new();
        let mut spans: Vec<Span> = Vec::new();
        for (k, &ri) in g.iter().enumerate() {
            let r = &runs[ri];
            let offset = dot((r.start.0 - first.start.0, r.start.1 - first.start.1), first.dir);
            if k > 0 {
                let prev = &runs[g[k - 1]];
                let prev_end = dot((prev.end.0 - first.start.0, prev.end.1 - first.start.1), first.dir);
                if offset - prev_end > 0.18 * prev.em && !text.ends_with(' ') && !r.text.starts_with(' ') {
                    text.push(' ');
                    chars.push(offset);
                    char_runs.push(g[k - 1]);
                }
            }
            let span_start = text.chars().count();
            text.push_str(&r.text);
            for c in r.chars.iter().skip(1) {
                chars.push(offset + c);
            }
            let n = r.text.chars().count();
            char_runs.extend(std::iter::repeat(ri).take(n));
            // Keep `chars` aligned with the text even if a run's boundaries came out short.
            while chars.len() < text.chars().count() + 1 {
                chars.push(*chars.last().unwrap());
            }
            chars.truncate(text.chars().count() + 1);
            let same_style = |s: &Span| s.font_key == r.font.key && (s.size - r.size).abs() < 0.01 && s.color == r.fill.rgb();
            match spans.last_mut() {
                Some(s) if same_style(s) => s.end = text.chars().count(),
                _ => spans.push(Span {
                    start: span_start,
                    end: text.chars().count(),
                    font_key: r.font.key.clone(),
                    font_name: r.font.ps_name.clone(),
                    family: r.font.family.clone(),
                    bold: r.font.bold,
                    italic: r.font.italic,
                    size: r.size,
                    em: r.em,
                    color: r.fill.rgb(),
                }),
            }
        }
        // Trailing spaces don't belong to the visible line.
        while text.ends_with(' ') {
            text.pop();
            chars.pop();
            char_runs.pop();
        }
        if text.trim().is_empty() {
            continue;
        }
        let n = text.chars().count();
        for s in spans.iter_mut() {
            s.end = s.end.min(n);
        }
        spans.retain(|s| s.start < s.end);

        let asc = g.iter().map(|&ri| runs[ri].font.ascent / 1000.0 * runs[ri].em).fold(0.0f32, f32::max);
        let desc = g.iter().map(|&ri| runs[ri].font.descent / 1000.0 * runs[ri].em).fold(0.0f32, f32::min);
        let s = first.start;
        let width = *chars.last().unwrap_or(&0.0);
        let e = (s.0 + first.dir.0 * width, s.1 + first.dir.1 * width);
        let u = first.up;
        let quad = [
            [s.0 + u.0 * desc, s.1 + u.1 * desc],
            [e.0 + u.0 * desc, e.1 + u.1 * desc],
            [e.0 + u.0 * asc, e.1 + u.1 * asc],
            [s.0 + u.0 * asc, s.1 + u.1 * asc],
        ];
        let _ = last;
        let invisible = first.mode == 3 || first.mode == 7;
        let blocked = g.iter().map(|&ri| &runs[ri].font).find(|f| f.key == "missing" || f.kind == FontKind::Type3);
        let (editable, note) = if let Some(f) = blocked {
            if f.key == "missing" {
                (false, Some("The font this text uses is missing from the page resources.".to_string()))
            } else {
                (false, Some("Type 3 fonts are drawn with PDF operators and cannot be edited yet.".to_string()))
            }
        } else if invisible {
            (true, Some("Invisible text: usually the OCR layer of a scanned page.".to_string()))
        } else if let Src::Form(id) = first.src {
            let uses = form_uses.get(&id).copied().unwrap_or(1);
            if uses > 1 {
                (true, Some(format!("Part of a reusable element drawn {uses} times on this page; edits change every copy.")))
            } else {
                (true, None)
            }
        } else {
            (true, None)
        };
        let f = &first.font;
        lines.push(TextLine {
            id: lines.len(),
            src: first.src.label(),
            ops: g.iter().map(|&ri| runs[ri].op).collect(),
            text,
            spans,
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
            chars,
            block: 0,
            runs: g.clone(),
            char_runs,
        });
    }
    lines
}

fn spread(v: &[f32]) -> f32 {
    if v.len() < 2 {
        return 0.0;
    }
    let lo = v.iter().cloned().fold(f32::MAX, f32::min);
    let hi = v.iter().cloned().fold(f32::MIN, f32::max);
    hi - lo
}

/// Groups lines into paragraphs and records each line's paragraph.
fn paragraphs(lines: &mut [TextLine]) -> Vec<Block> {
    let mut blocks: Vec<Vec<usize>> = Vec::new();
    let frame = |l: &TextLine| -> ((f32, f32), (f32, f32)) {
        let a = l.angle.to_radians();
        ((a.cos(), a.sin()), (-a.sin(), a.cos()))
    };
    for i in 0..lines.len() {
        let l2 = &lines[i];
        let mut joined = false;
        if let Some(b) = blocks.last() {
            let f = &lines[b[0]];
            let l1 = &lines[*b.last().unwrap()];
            let (d, u) = frame(f);
            let pos = |l: &TextLine| -> (f32, f32, f32) {
                let v = (l.origin[0] - f.origin[0], l.origin[1] - f.origin[1]);
                let x0 = dot(v, d);
                (x0, x0 + l.chars.last().copied().unwrap_or(0.0), dot(v, u))
            };
            let (a0, a1, ay) = pos(l1);
            let (b0, b1, by) = pos(l2);
            let em = f.em.max(0.5);
            let dy = ay - by;
            let lefts: Vec<f32> = b.iter().skip(1).map(|&k| pos(&lines[k]).0).collect();
            let left_ref = lefts.first().copied().unwrap_or(pos(f).0);
            let leading = if b.len() >= 2 { Some(pos(&lines[b[0]]).2 - pos(&lines[b[1]]).2) } else { None };
            let ok = l1.src == l2.src
                && l1.editable
                && l2.editable
                && l1.invisible == l2.invisible
                && (l1.angle - l2.angle).abs() < 1.0
                && (l2.em / em) > 0.8
                && (l2.em / em) < 1.25
                && dy > 0.85 * em
                && dy < 2.2 * em
                && leading.map(|ld| (dy - ld).abs() < 0.2 * em).unwrap_or(true)
                && b0 < a1 + 0.5 * em
                && b1 > a0 - 0.5 * em
                && ((b0 - left_ref).abs() < 2.5 * em
                    || ((b0 + b1) / 2.0 - (a0 + a1) / 2.0).abs() < 1.5 * em
                    || (b1 - a1).abs() < 1.0 * em);
            if ok {
                joined = true;
            }
        }
        if joined {
            blocks.last_mut().unwrap().push(i);
        } else {
            blocks.push(vec![i]);
        }
    }

    blocks
        .into_iter()
        .enumerate()
        .map(|(id, ids)| {
            for &i in &ids {
                lines[i].block = id;
            }
            let f = &lines[ids[0]];
            let (d, u) = frame(f);
            let pos: Vec<(f32, f32, f32)> = ids
                .iter()
                .map(|&k| {
                    let l = &lines[k];
                    let v = (l.origin[0] - f.origin[0], l.origin[1] - f.origin[1]);
                    let x0 = dot(v, d);
                    (x0, x0 + l.chars.last().copied().unwrap_or(0.0), dot(v, u))
                })
                .collect();
            let first_x = pos[0].0;
            let left = if pos.len() > 1 { pos[1..].iter().map(|p| p.0).fold(f32::MAX, f32::min) } else { first_x };
            let right = pos.iter().map(|p| p.1).fold(f32::MIN, f32::max);
            let leading = if pos.len() > 1 { (pos[0].2 - pos[pos.len() - 1].2) / (pos.len() - 1) as f32 } else { f.em * 1.2 };
            let n = pos.len();
            let lefts: Vec<f32> = pos.iter().skip(1).map(|p| p.0).collect();
            let rights: Vec<f32> = pos.iter().take(n.saturating_sub(1)).map(|p| p.1).collect();
            let centers: Vec<f32> = pos.iter().map(|p| (p.0 + p.1) / 2.0).collect();
            let all_rights: Vec<f32> = pos.iter().map(|p| p.1).collect();
            let align = if n >= 3 && spread(&lefts) < 1.0 && spread(&rights) < 1.0 {
                "justify"
            } else if n >= 2 && spread(&lefts) < 1.0 {
                "left"
            } else if n >= 2 && spread(&centers) < 1.0 && spread(&all_rights) > 2.0 {
                "center"
            } else if n >= 2 && spread(&all_rights) < 1.0 {
                "right"
            } else {
                "left"
            };

            // Text: lines joined by spaces, words broken with a hyphen at a line end rejoined.
            let mut text = String::new();
            let mut char_src: Vec<Option<(usize, usize)>> = Vec::new();
            for (k, &li) in ids.iter().enumerate() {
                let l = &lines[li];
                let lc: Vec<char> = l.text.chars().collect();
                let next_lower = ids
                    .get(k + 1)
                    .and_then(|&n| lines[n].text.chars().next())
                    .map(|c| c.is_lowercase())
                    .unwrap_or(false);
                let hyphenated = lc.len() >= 2 && lc[lc.len() - 1] == '-' && lc[lc.len() - 2].is_alphabetic() && next_lower;
                let take = if hyphenated { lc.len() - 1 } else { lc.len() };
                for (ci, c) in lc.iter().take(take).enumerate() {
                    text.push(*c);
                    char_src.push(Some((li, ci)));
                }
                if k + 1 < ids.len() && !hyphenated && !text.ends_with(' ') {
                    text.push(' ');
                    char_src.push(None);
                }
            }

            // Bounding quad in the first line's frame.
            let asc = ids.iter().map(|&i| lines[i].quad[3]).map(|p| dot((p[0] - f.origin[0], p[1] - f.origin[1]), u)).fold(f32::MIN, f32::max);
            let bottom = lines[*ids.last().unwrap()].quad[0];
            let desc = dot((bottom[0] - f.origin[0], bottom[1] - f.origin[1]), u);
            let x0 = first_x.min(left);
            let pt = |x: f32, y: f32| [f.origin[0] + d.0 * x + u.0 * y, f.origin[1] + d.1 * x + u.1 * y];
            Block {
                id,
                lines: ids.clone(),
                text,
                align: align.into(),
                width: right - left.min(first_x),
                leading,
                quad: [pt(x0, desc), pt(right, desc), pt(right, asc), pt(x0, asc)],
                first_x,
                left,
                char_src,
            }
        })
        .collect()
}
