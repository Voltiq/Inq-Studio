//! Interactive forms (AcroForm): reading fields, filling them in, and drawing
//! their appearances so the filled-in values show in every reader, not only
//! in ones that rebuild appearances themselves.

use crate::fonts::{Piece, PdfFont};
use crate::util::*;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WidgetInfo {
    pub page: usize,
    pub rect: [f32; 4],
    /// For check boxes and radio buttons: the value this widget stands for.
    pub on_state: Option<String>,
    pub checked: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    /// Fully qualified name ("address.city").
    pub name: String,
    /// text | checkbox | radio | combo | list | button | signature
    pub kind: String,
    pub value: String,
    pub options: Vec<String>,
    pub multiline: bool,
    pub readonly: bool,
    pub required: bool,
    pub max_len: Option<usize>,
    pub widgets: Vec<WidgetInfo>,
    #[serde(skip)]
    pub id: Option<ObjectId>,
    #[serde(skip)]
    pub widget_ids: Vec<ObjectId>,
    #[serde(skip)]
    pub da: String,
    #[serde(skip)]
    pub quadding: i64,
}

#[derive(Clone, Default)]
struct Inherited {
    name: String,
    ft: Option<String>,
    ff: i64,
    v: Option<Object>,
    da: Option<String>,
    q: Option<i64>,
    opt: Option<Vec<Object>>,
    max_len: Option<usize>,
}

fn text_of(doc: &Document, o: &Object) -> String {
    match deref(doc, o) {
        Object::Name(n) => String::from_utf8_lossy(n).to_string(),
        Object::Array(a) => a.iter().map(|x| text_of(doc, x)).collect::<Vec<_>>().join(", "),
        o => lopdf::decode_text_string(o).unwrap_or_default(),
    }
}

fn acroform(doc: &Document) -> Option<&Dictionary> {
    doc.catalog().ok()?.get(b"AcroForm").ok().and_then(|o| dict(doc, o))
}

/// Widget states other than Off: the values a check box or radio button can take.
fn on_states(doc: &Document, w: &Dictionary) -> Vec<String> {
    let Some(ap) = w.get(b"AP").ok().and_then(|o| dict(doc, o)) else { return Vec::new() };
    let Some(n) = ap.get(b"N").ok().and_then(|o| dict(doc, o)) else { return Vec::new() };
    n.iter().map(|(k, _)| String::from_utf8_lossy(k).to_string()).filter(|k| k != "Off").collect()
}

/// Every terminal field of the document.
pub fn fields(doc: &Document) -> Vec<FieldInfo> {
    let Some(form) = acroform(doc) else { return Vec::new() };
    let mut page_of: HashMap<ObjectId, (usize, ObjectId)> = HashMap::new();
    for (i, pid) in crate::pages::page_ids(doc).into_iter().enumerate() {
        for a in crate::annots::annots_array(doc, pid) {
            if let Ok(id) = a.as_reference() {
                page_of.insert(id, (i, pid));
            }
        }
    }
    let mut out = Vec::new();
    let roots = get_array(doc, form, b"Fields").cloned().unwrap_or_default();
    let da = get(doc, form, b"DA").and_then(|o| o.as_str().ok()).map(|b| String::from_utf8_lossy(b).to_string());
    let base = Inherited { da, ..Default::default() };
    for r in roots {
        walk(doc, &r, &base, &page_of, &mut out, 0);
    }
    out
}

fn walk(doc: &Document, node: &Object, parent: &Inherited, page_of: &HashMap<ObjectId, (usize, ObjectId)>, out: &mut Vec<FieldInfo>, depth: usize) {
    if depth > 32 {
        return;
    }
    let id = node.as_reference().ok();
    let Some(d) = dict(doc, node) else { return };
    let mut inh = parent.clone();
    if let Some(t) = get(doc, d, b"T").map(|o| text_of(doc, o)) {
        inh.name = if inh.name.is_empty() { t } else { format!("{}.{t}", inh.name) };
    }
    if let Some(ft) = get_name(doc, d, b"FT") {
        inh.ft = Some(ft);
    }
    if let Some(ff) = get_num(doc, d, b"Ff") {
        inh.ff = ff as i64;
    }
    if let Ok(v) = d.get(b"V") {
        inh.v = Some(v.clone());
    }
    if let Some(da) = get(doc, d, b"DA").and_then(|o| o.as_str().ok()) {
        inh.da = Some(String::from_utf8_lossy(da).to_string());
    }
    if let Some(q) = get_num(doc, d, b"Q") {
        inh.q = Some(q as i64);
    }
    if let Some(opt) = get_array(doc, d, b"Opt") {
        inh.opt = Some(opt.clone());
    }
    if let Some(m) = get_num(doc, d, b"MaxLen") {
        inh.max_len = Some(m as usize);
    }
    let kids = get_array(doc, d, b"Kids").cloned().unwrap_or_default();
    let field_kids: Vec<&Object> = kids.iter().filter(|k| dict(doc, k).map(|kd| kd.has(b"T")).unwrap_or(false)).collect();
    if !field_kids.is_empty() {
        for k in field_kids {
            walk(doc, k, &inh, page_of, out, depth + 1);
        }
        return;
    }
    // A terminal field: its widgets are itself, or its kids.
    let mut widget_ids = Vec::new();
    if get_name(doc, d, b"Subtype").as_deref() == Some("Widget") {
        if let Some(id) = id {
            widget_ids.push(id);
        }
    }
    for k in &kids {
        if let Ok(kid) = k.as_reference() {
            widget_ids.push(kid);
        }
    }
    let ft = inh.ft.clone().unwrap_or_default();
    let kind = match ft.as_str() {
        "Tx" => "text",
        "Btn" if inh.ff & (1 << 16) != 0 => "button",
        "Btn" if inh.ff & (1 << 15) != 0 => "radio",
        "Btn" => "checkbox",
        "Ch" if inh.ff & (1 << 17) != 0 => "combo",
        "Ch" => "list",
        "Sig" => "signature",
        _ => "unknown",
    };
    let value = inh.v.as_ref().map(|v| text_of(doc, v)).unwrap_or_default();
    let options = inh
        .opt
        .as_ref()
        .map(|o| {
            o.iter()
                .map(|x| match deref(doc, x) {
                    Object::Array(pair) if pair.len() == 2 => text_of(doc, &pair[1]),
                    other => text_of(doc, other),
                })
                .collect()
        })
        .unwrap_or_default();
    let widgets = widget_ids
        .iter()
        .filter_map(|wid| {
            let wd = doc.get_dictionary(*wid).ok()?;
            let (page, _) = page_of.get(wid).copied().or_else(|| {
                let p = wd.get(b"P").ok()?.as_reference().ok()?;
                crate::pages::page_ids(doc).iter().position(|x| *x == p).map(|i| (i, p))
            })?;
            let rect = wd.get(b"Rect").ok().and_then(|o| rect(doc, o))?;
            let states = on_states(doc, wd);
            let state = get_name(doc, wd, b"AS");
            let on = states.first().cloned();
            let checked = matches!((&state, &on), (Some(s), Some(o)) if s == o);
            Some(WidgetInfo { page, rect, on_state: if kind == "checkbox" || kind == "radio" { on } else { None }, checked })
        })
        .collect();
    out.push(FieldInfo {
        name: inh.name.clone(),
        kind: kind.into(),
        value,
        options,
        multiline: inh.ff & (1 << 12) != 0,
        readonly: inh.ff & 1 != 0,
        required: inh.ff & 2 != 0,
        max_len: inh.max_len,
        widgets,
        id,
        widget_ids,
        da: inh.da.clone().unwrap_or_else(|| "/Helv 0 Tf 0 g".into()),
        quadding: inh.q.unwrap_or(0),
    });
}

/// Font name, size and colour operators from a default appearance string.
fn parse_da(da: &str) -> (String, f32, String) {
    let toks: Vec<&str> = da.split_whitespace().collect();
    let mut font = "Helv".to_string();
    let mut size = 0.0;
    let mut color = "0 g".to_string();
    for (i, t) in toks.iter().enumerate() {
        if *t == "Tf" && i >= 2 {
            font = toks[i - 2].trim_start_matches('/').to_string();
            size = toks[i - 1].parse().unwrap_or(0.0);
        }
        if (*t == "g" || *t == "rg" || *t == "k") && i >= 1 {
            let n = match *t {
                "g" => 1,
                "rg" => 3,
                _ => 4,
            };
            if i >= n {
                color = format!("{} {t}", toks[i - n..i].join(" "));
            }
        }
    }
    (font, size, color)
}

/// The font object behind a DA font name, creating Helvetica in the form's
/// resources when the document has none.
fn da_font(doc: &mut Document, name: &str) -> Result<ObjectId, String> {
    let existing = acroform(doc)
        .and_then(|f| get(doc, f, b"DR"))
        .and_then(|dr| match dr {
            Object::Dictionary(d) => get(doc, d, b"Font").cloned(),
            _ => None,
        })
        .and_then(|fonts| match fonts {
            Object::Dictionary(d) => d.get(name.as_bytes()).ok().and_then(|o| o.as_reference().ok()),
            _ => None,
        });
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = crate::sysfonts::standard_font(doc, "Helvetica");
    // Record it in /DR so other tools find it too.
    let cat = doc.catalog().map_err(err)?.get(b"AcroForm").ok().cloned();
    if let Some(Object::Reference(fid)) = cat {
        let f = doc.get_dictionary_mut(fid).map_err(err)?;
        let mut dr = f.get(b"DR").and_then(Object::as_dict).cloned().unwrap_or_default();
        let mut fonts = dr.get(b"Font").and_then(Object::as_dict).cloned().unwrap_or_default();
        fonts.set(name, Object::Reference(id));
        dr.set("Font", fonts);
        f.set("DR", dr);
    }
    Ok(id)
}

fn hex(bytes: &[u8]) -> String {
    format!("<{}>", bytes.iter().map(|b| format!("{b:02X}")).collect::<String>())
}

/// Encodes a line of text for an appearance: (TJ operand, width at size 1).
fn encode_line(doc: &Document, font: &PdfFont, text: &str) -> Option<(String, f32)> {
    let enc = font.encode_with(doc, text, false);
    if !enc.missing.is_empty() {
        return None;
    }
    let mut parts = String::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut width = 0.0;
    for p in &enc.pieces {
        match p {
            Piece::Code(c) => {
                if font.code_len == 2 {
                    cur.push((c >> 8) as u8);
                }
                cur.push(*c as u8);
                width += font.width(*c) / 1000.0;
            }
            Piece::Gap(g) => {
                if !cur.is_empty() {
                    parts.push_str(&hex(&std::mem::take(&mut cur)));
                }
                parts.push_str(&format!(" {} ", -g.round()));
                width += g / 1000.0;
            }
        }
    }
    if !cur.is_empty() {
        parts.push_str(&hex(&cur));
    }
    Some((format!("[{parts}]"), width))
}

/// Draws a text or choice field's value into a widget appearance.
fn text_appearance(doc: &mut Document, widget: ObjectId, field: &FieldInfo, value: &str) -> Result<(), String> {
    let wd = doc.get_dictionary(widget).map_err(err)?.clone();
    let r = wd.get(b"Rect").ok().and_then(|o| rect(doc, o)).ok_or("The field has no rectangle.")?;
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let da = get(doc, &wd, b"DA").and_then(|o| o.as_str().ok()).map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or(field.da.clone());
    let (font_name, da_size, color) = parse_da(&da);
    let mut font_id = da_font(doc, &font_name)?;
    let mut font = PdfFont::load(doc, &Object::Reference(font_id), format!("{} {}", font_id.0, font_id.1));
    // Characters the form's font cannot draw come from Arial on this computer.
    let probe = if field.kind == "list" { field.options.join(" ") } else { value.to_string() };
    if !font.encode_with(doc, &probe, false).missing.is_empty() {
        if let Some(face) = crate::sysfonts::find_family("Arial", None, false, false) {
            if let Ok(emb) = crate::sysfonts::embed(doc, &face, &probe) {
                font_id = emb.font_id;
                font = PdfFont::load(doc, &Object::Reference(font_id), format!("{} {}", font_id.0, font_id.1));
            }
        }
    }
    let pad = 2.0;
    let lines: Vec<String> = if field.kind == "list" {
        field.options.clone()
    } else if field.multiline {
        let size_guess = if da_size > 0.0 { da_size } else { 10.0 };
        let mut out = Vec::new();
        for para in value.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                let wd = encode_line(doc, &font, &cand).map(|x| x.1).unwrap_or(0.0) * size_guess;
                if !line.is_empty() && wd > w - pad * 2.0 {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = cand;
                }
            }
            out.push(line);
        }
        out
    } else {
        vec![value.replace('\n', " ")]
    };
    let widths: Vec<f32> = lines.iter().map(|l| encode_line(doc, &font, l).map(|x| x.1).unwrap_or(0.0)).collect();
    let size = if da_size > 0.0 {
        da_size
    } else if field.multiline || field.kind == "list" {
        10.0
    } else {
        let by_height = (h - pad * 2.0) * 0.72;
        let by_width = widths.first().map(|wd| if *wd > 0.0 { (w - pad * 2.0) / wd } else { f32::MAX }).unwrap_or(f32::MAX);
        by_height.min(by_width).clamp(4.0, 12.0)
    };
    let mut body = String::from("/Tx BMC q ");
    body.push_str(&format!("1 1 {} {} re W n ", w - 2.0, h - 2.0));
    let leading = size * 1.15;
    let selected: Option<usize> = if field.kind == "list" { field.options.iter().position(|o| o == value) } else { None };
    if let Some(i) = selected {
        let y = h - pad - leading * (i as f32 + 1.0);
        body.push_str(&format!("0.6 0.75 0.95 rg 1 {} {} {} re f ", y, w - 2.0, leading));
    }
    body.push_str(&format!("BT /F {size} Tf {color} "));
    for (i, (l, lw)) in lines.iter().zip(&widths).enumerate() {
        let Some((operand, _)) = encode_line(doc, &font, l) else { continue };
        let tw = lw * size;
        let x = match field.quadding {
            1 => (w - tw) / 2.0,
            2 => w - pad - tw,
            _ => pad,
        };
        let y = if field.multiline || field.kind == "list" {
            h - pad - size * 0.9 - leading * i as f32
        } else {
            (h - size * 0.72) / 2.0
        };
        body.push_str(&format!("1 0 0 1 {x} {y} Tm {operand} TJ "));
    }
    body.push_str("ET Q EMC");
    let mut s = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), real(w), real(h)],
            "Resources" => dictionary! { "Font" => dictionary! { "F" => font_id } },
        },
        body.into_bytes(),
    );
    let _ = s.compress();
    let ap = doc.add_object(s);
    doc.get_dictionary_mut(widget).map_err(err)?.set("AP", dictionary! { "N" => ap });
    Ok(())
}

/// A check mark appearance for check boxes that have none.
fn ensure_check_states(doc: &mut Document, widget: ObjectId) -> Result<String, String> {
    let wd = doc.get_dictionary(widget).map_err(err)?.clone();
    if let Some(s) = on_states(doc, &wd).into_iter().next() {
        return Ok(s);
    }
    let r = wd.get(b"Rect").ok().and_then(|o| rect(doc, o)).ok_or("The field has no rectangle.")?;
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let size = h.min(w) * 0.8;
    let on = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), real(w), real(h)],
            "Resources" => dictionary! { "Font" => dictionary! { "ZaDb" => dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "ZapfDingbats" } } },
        },
        format!("q BT /ZaDb {size} Tf 0 g {} {} Td (4) Tj ET Q", (w - size * 0.78) / 2.0, (h - size * 0.7) / 2.0).into_bytes(),
    );
    let off = Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), real(w), real(h)] }, Vec::new());
    let (on_id, off_id) = (doc.add_object(on), doc.add_object(off));
    doc.get_dictionary_mut(widget).map_err(err)?.set("AP", dictionary! { "N" => dictionary! { "Yes" => on_id, "Off" => off_id } });
    Ok("Yes".into())
}

/// Fills in a field and redraws its widgets.
pub fn set_field(doc: &mut Document, name: &str, value: &str) -> Result<(), String> {
    let field = fields(doc).into_iter().find(|f| f.name == name).ok_or_else(|| format!("There is no field called {name}."))?;
    if field.readonly {
        return Err(format!("{name} is read-only."));
    }
    let fid = field.id.ok_or("This field cannot be changed.")?;
    let value = match field.max_len {
        Some(m) if field.kind == "text" => value.chars().take(m).collect::<String>(),
        _ => value.to_string(),
    };
    match field.kind.as_str() {
        "text" | "combo" | "list" => {
            doc.get_dictionary_mut(fid).map_err(err)?.set("V", lopdf::text_string(&value));
            for w in &field.widget_ids {
                text_appearance(doc, *w, &field, &value)?;
            }
        }
        "checkbox" => {
            let on = !(value.is_empty() || value == "Off" || value == "false");
            let mut state = "Off".to_string();
            for w in &field.widget_ids {
                let s = if on { ensure_check_states(doc, *w)? } else { "Off".into() };
                doc.get_dictionary_mut(*w).map_err(err)?.set("AS", Object::Name(s.clone().into_bytes()));
                if on {
                    state = s;
                }
            }
            doc.get_dictionary_mut(fid).map_err(err)?.set("V", Object::Name(state.into_bytes()));
        }
        "radio" => {
            for w in &field.widget_ids {
                let wd = doc.get_dictionary(*w).map_err(err)?.clone();
                let has = on_states(doc, &wd).iter().any(|s| s == &value);
                let s = if has { value.clone() } else { "Off".into() };
                doc.get_dictionary_mut(*w).map_err(err)?.set("AS", Object::Name(s.into_bytes()));
            }
            doc.get_dictionary_mut(fid).map_err(err)?.set("V", Object::Name(value.clone().into_bytes()));
        }
        "signature" => return Err("Signature fields are signed, not filled in.".into()),
        _ => return Err("This kind of field cannot be filled in yet.".into()),
    }
    // Appearances are up to date now; readers need not rebuild them.
    if let Ok(Object::Reference(af)) = doc.catalog().map_err(err)?.get(b"AcroForm").cloned() {
        if let Ok(d) = doc.get_dictionary_mut(af) {
            d.remove(b"NeedAppearances");
        }
    } else if let Ok(cat) = doc.catalog_mut() {
        if let Ok(Object::Dictionary(d)) = cat.get_mut(b"AcroForm") {
            d.remove(b"NeedAppearances");
        }
    }
    Ok(())
}

/// Flattens annotations and/or form fields on every page. Returns how many were flattened.
pub fn flatten_document(doc: &mut Document, widgets: bool, others: bool) -> Result<usize, String> {
    let mut n = 0;
    for pid in crate::pages::page_ids(doc) {
        n += crate::annots::flatten(doc, pid, widgets, others)?;
    }
    if widgets {
        if let Ok(cat) = doc.catalog_mut() {
            cat.remove(b"AcroForm");
        }
    }
    Ok(n)
}
