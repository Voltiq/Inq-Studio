//! Images: list where a page draws them, insert new ones, move, resize,
//! replace and delete them, and extract them as files. Decoding also serves
//! redaction, which blacks out pixels under a redaction box.

use crate::edit::{add_resource, set_ops};
use crate::interp::{scan_page, ImagePlacement, Src};
use crate::util::*;
use lopdf::content::Operation;
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub index: usize,
    pub quad: [[f32; 2]; 4],
    pub bbox: [f32; 4],
    pub width: u32,
    pub height: u32,
    pub inline: bool,
    pub filter: String,
    pub color_space: String,
    pub bits: u32,
    pub has_alpha: bool,
    /// Effective resolution in pixels per inch.
    pub dpi: f32,
}

pub fn list(doc: &Document, page_id: ObjectId) -> Result<Vec<ImageInfo>, String> {
    let scan = scan_page(doc, page_id)?;
    Ok(scan
        .images
        .iter()
        .enumerate()
        .map(|(index, im)| {
            let d = im.id.and_then(|id| doc.get_object(id).ok()).and_then(|o| o.as_stream().ok()).map(|s| s.dict.clone());
            let filter = d
                .as_ref()
                .and_then(|d| d.get(b"Filter").ok())
                .map(|f| match f {
                    Object::Name(n) => String::from_utf8_lossy(n).to_string(),
                    Object::Array(a) => a.iter().filter_map(|x| x.as_name().ok()).map(|n| String::from_utf8_lossy(n).to_string()).collect::<Vec<_>>().join(" "),
                    _ => String::new(),
                })
                .unwrap_or_default();
            let color_space = d.as_ref().map(|d| cs_label(doc, d.get(b"ColorSpace").ok())).unwrap_or_default();
            let bits = d.as_ref().and_then(|d| d.get(b"BitsPerComponent").ok()).and_then(num).unwrap_or(8.0) as u32;
            let has_alpha = d.as_ref().map(|d| d.has(b"SMask")).unwrap_or(false);
            let bbox = im.bbox();
            let dpi = if bbox[2] > bbox[0] { im.width as f32 / ((bbox[2] - bbox[0]) / 72.0) } else { 0.0 };
            ImageInfo { index, quad: im.quad(), bbox, width: im.width, height: im.height, inline: im.id.is_none(), filter, color_space, bits, has_alpha, dpi }
        })
        .collect())
}

fn cs_label(doc: &Document, cs: Option<&Object>) -> String {
    match cs.map(|o| deref(doc, o)) {
        Some(Object::Name(n)) => String::from_utf8_lossy(n).to_string(),
        Some(Object::Array(a)) => a.first().and_then(|x| x.as_name().ok()).map(|n| String::from_utf8_lossy(n).to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Components and, for indexed images, the palette (in RGB).
fn color_model(doc: &Document, cs: Option<&Object>) -> Result<(usize, Option<Vec<[u8; 3]>>), String> {
    match cs.map(|o| deref(doc, o)) {
        None => Ok((1, None)),
        Some(Object::Name(n)) => match n.as_slice() {
            b"DeviceGray" | b"CalGray" | b"G" => Ok((1, None)),
            b"DeviceRGB" | b"CalRGB" | b"RGB" => Ok((3, None)),
            b"DeviceCMYK" | b"CMYK" => Ok((4, None)),
            other => Err(format!("Images in the {} colour space are not supported yet.", String::from_utf8_lossy(other))),
        },
        Some(Object::Array(a)) => {
            let kind = a.first().and_then(|x| x.as_name().ok()).unwrap_or(b"");
            match kind {
                b"ICCBased" => {
                    let n = a.get(1).and_then(|x| dict(doc, x)).and_then(|d| get_num(doc, d, b"N")).unwrap_or(3.0) as usize;
                    Ok((n, None))
                }
                b"CalRGB" => Ok((3, None)),
                b"CalGray" => Ok((1, None)),
                b"Indexed" | b"I" => {
                    let (base_n, _) = color_model(doc, a.get(1))?;
                    let lookup = match a.get(3).map(|x| deref(doc, x)) {
                        Some(Object::String(s, _)) => s.clone(),
                        Some(o @ Object::Stream(_)) => stream_bytes(doc, o).unwrap_or_default(),
                        _ => Vec::new(),
                    };
                    let pal = lookup
                        .chunks(base_n.max(1))
                        .map(|c| match base_n {
                            1 => [c[0]; 3],
                            4 if c.len() == 4 => cmyk(c[0], c[1], c[2], c[3]),
                            _ if c.len() >= 3 => [c[0], c[1], c[2]],
                            _ => [0; 3],
                        })
                        .collect();
                    Ok((1, Some(pal)))
                }
                other => Err(format!("Images in the {} colour space are not supported yet.", String::from_utf8_lossy(other))),
            }
        }
        _ => Ok((3, None)),
    }
}

fn cmyk(c: u8, m: u8, y: u8, k: u8) -> [u8; 3] {
    let f = |x: u8| ((255 - x as u16) * (255 - k as u16) / 255) as u8;
    [f(c), f(m), f(y)]
}

fn is_dct(d: &lopdf::Dictionary) -> bool {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => n == b"DCTDecode",
        Ok(Object::Array(a)) => a.len() == 1 && a[0].as_name().ok() == Some(b"DCTDecode"),
        _ => false,
    }
}

/// Decodes an image XObject to RGBA, applying its soft mask.
pub fn decode(doc: &Document, id: ObjectId) -> Result<image::RgbaImage, String> {
    let s = doc.get_object(id).and_then(Object::as_stream).map_err(err)?;
    let w = s.dict.get(b"Width").ok().and_then(num).unwrap_or(0.0) as u32;
    let h = s.dict.get(b"Height").ok().and_then(num).unwrap_or(0.0) as u32;
    if w == 0 || h == 0 || (w as u64) * (h as u64) > 120_000_000 {
        return Err("The image size is not usable.".into());
    }
    let mut img = if is_dct(&s.dict) {
        image::load_from_memory_with_format(&s.content, image::ImageFormat::Jpeg).map_err(|e| format!("The JPEG could not be decoded: {e}"))?.to_rgba8()
    } else {
        let data = if s.dict.get(b"Filter").is_ok() { s.decompressed_content().map_err(|e| format!("The image data could not be decompressed: {e}"))? } else { s.content.clone() };
        let bpc = s.dict.get(b"BitsPerComponent").ok().and_then(num).unwrap_or(8.0) as usize;
        let mask = s.dict.get(b"ImageMask").ok().and_then(|o| o.as_bool().ok()).unwrap_or(false);
        let (n, palette) = if mask { (1, None) } else { color_model(doc, s.dict.get(b"ColorSpace").ok())? };
        let row = (w as usize * n * bpc).div_ceil(8);
        if data.len() < row * h as usize {
            return Err("The image data is shorter than its size says.".into());
        }
        let mut out = image::RgbaImage::new(w, h);
        for y in 0..h as usize {
            let line = &data[y * row..(y + 1) * row];
            for x in 0..w as usize {
                let sample = |c: usize| -> u8 {
                    let i = x * n + c;
                    match bpc {
                        8 => line[i],
                        16 => line[i * 2],
                        1 | 2 | 4 => {
                            let bit = i * bpc;
                            let v = (line[bit / 8] >> (8 - bpc - bit % 8)) & ((1 << bpc) - 1);
                            if palette.is_some() { v } else { (v as u32 * 255 / ((1 << bpc) - 1)) as u8 }
                        }
                        _ => 0,
                    }
                };
                let px = if let Some(pal) = &palette {
                    let c = pal.get(sample(0) as usize).copied().unwrap_or([0; 3]);
                    [c[0], c[1], c[2], 255]
                } else if mask {
                    if sample(0) == 0 { [0, 0, 0, 255] } else { [0, 0, 0, 0] }
                } else {
                    match n {
                        1 => {
                            let v = sample(0);
                            [v, v, v, 255]
                        }
                        3 => [sample(0), sample(1), sample(2), 255],
                        4 => {
                            let c = cmyk(sample(0), sample(1), sample(2), sample(3));
                            [c[0], c[1], c[2], 255]
                        }
                        _ => [0, 0, 0, 255],
                    }
                };
                out.put_pixel(x as u32, y as u32, image::Rgba(px));
            }
        }
        out
    };
    if let Some(Object::Reference(mid)) = s.dict.get(b"SMask").ok() {
        if let Ok(alpha) = decode(doc, *mid) {
            let a = image::imageops::resize(&alpha, img.width(), img.height(), image::imageops::FilterType::Triangle);
            for (p, q) in img.pixels_mut().zip(a.pixels()) {
                p.0[3] = q.0[0];
            }
        }
    }
    Ok(img)
}

/// An image XObject from RGBA pixels (alpha becomes a soft mask when used).
pub fn object_from_rgba(doc: &mut Document, img: &image::RgbaImage) -> ObjectId {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let rgb: Vec<u8> = img.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2]]).collect();
    let mut d = dictionary! {
        "Type" => "XObject", "Subtype" => "Image", "Width" => w, "Height" => h,
        "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
    };
    if img.pixels().any(|p| p.0[3] < 255) {
        let alpha: Vec<u8> = img.pixels().map(|p| p.0[3]).collect();
        let mut m = Stream::new(
            dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => w, "Height" => h, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 },
            alpha,
        );
        let _ = m.compress();
        d.set("SMask", doc.add_object(m));
    }
    let mut s = Stream::new(d, rgb);
    let _ = s.compress();
    doc.add_object(s)
}

/// An image XObject from a file: JPEGs are embedded as they are, other
/// formats are decoded and stored losslessly.
pub fn object_from_file(doc: &mut Document, path: &Path) -> Result<(ObjectId, u32, u32), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("The image could not be read: {e}"))?;
    let format = image::guess_format(&bytes).map_err(|_| "That file is not a PNG or JPEG image.".to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("The image could not be decoded: {e}"))?;
    let (w, h) = (img.width(), img.height());
    if format == image::ImageFormat::Jpeg && matches!(img.color(), image::ColorType::Rgb8 | image::ColorType::L8) {
        let cs = if img.color() == image::ColorType::L8 { "DeviceGray" } else { "DeviceRGB" };
        let s = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
                "ColorSpace" => cs, "BitsPerComponent" => 8, "Filter" => "DCTDecode",
            },
            bytes,
        )
        .with_compression(false);
        return Ok((doc.add_object(s), w, h));
    }
    Ok((object_from_rgba(doc, &img.to_rgba8()), w, h))
}

// ---------------------------------------------------------------------------
// Editing placements
// ---------------------------------------------------------------------------

fn placement(doc: &Document, page_id: ObjectId, index: usize) -> Result<(crate::interp::Scan, ImagePlacement), String> {
    let scan = scan_page(doc, page_id)?;
    let im = scan.images.get(index).cloned().ok_or("That image is no longer on the page.")?;
    Ok((scan, im))
}

fn replace_op(doc: &mut Document, page_id: ObjectId, scan: &crate::interp::Scan, src: Src, op: usize, with: Vec<Operation>) -> Result<(), String> {
    let mut ops = scan.ops.get(&src).cloned().unwrap_or_default();
    if op >= ops.len() {
        return Err("The image could not be found in the page content.".into());
    }
    ops.splice(op..=op, with);
    set_ops(doc, page_id, src, ops)
}

/// `cm` that, applied before an image drawn with CTM `c`, maps its old box onto `new`.
fn retarget(c: &Matrix, old: [f32; 4], new: [f32; 4]) -> Result<Matrix, String> {
    let sx = (new[2] - new[0]) / (old[2] - old[0]).max(1e-3);
    let sy = (new[3] - new[1]) / (old[3] - old[1]).max(1e-3);
    let t = [sx, 0.0, 0.0, sy, new[0] - old[0] * sx, new[1] - old[1] * sy];
    let inv = invert(c).ok_or("The image is drawn with a degenerate transform.")?;
    Ok(mul(&mul(c, &t), &inv))
}

fn draw_ops(x: Matrix, name: Vec<u8>, inline: Option<Operation>) -> Vec<Operation> {
    vec![
        Operation::new("q", vec![]),
        Operation::new("cm", x.iter().map(|v| real(*v)).collect()),
        inline.unwrap_or_else(|| Operation::new("Do", vec![Object::Name(name)])),
        Operation::new("Q", vec![]),
    ]
}

/// Moves and/or resizes an image to a new bounding box in user space.
pub fn place(doc: &mut Document, page_id: ObjectId, index: usize, new_box: [f32; 4]) -> Result<(), String> {
    let (scan, im) = placement(doc, page_id, index)?;
    if new_box[2] - new_box[0] < 1.0 || new_box[3] - new_box[1] < 1.0 {
        return Err("The image would be too small.".into());
    }
    let x = retarget(&im.ctm, im.bbox(), new_box)?;
    let original = scan.ops[&im.src][im.op].clone();
    replace_op(doc, page_id, &scan, im.src, im.op, draw_ops(x, im.name.clone(), Some(original)))
}

pub fn delete(doc: &mut Document, page_id: ObjectId, index: usize) -> Result<(), String> {
    let (scan, im) = placement(doc, page_id, index)?;
    replace_op(doc, page_id, &scan, im.src, im.op, Vec::new())
}

/// Replaces an image with another file, fitted into the same box.
pub fn replace(doc: &mut Document, page_id: ObjectId, index: usize, path: &Path) -> Result<(), String> {
    let (scan, im) = placement(doc, page_id, index)?;
    let (id, w, h) = object_from_file(doc, path)?;
    let name = add_resource(doc, page_id, im.src, "XObject", id, "InqIm")?;
    let b = im.bbox();
    let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
    let s = (bw / w as f32).min(bh / h as f32);
    let (nw, nh) = (w as f32 * s, h as f32 * s);
    let fit = [b[0] + (bw - nw) / 2.0, b[1] + (bh - nh) / 2.0, b[0] + (bw + nw) / 2.0, b[1] + (bh + nh) / 2.0];
    // The new image's own unit square goes upright into the fitted box.
    let inv = invert(&im.ctm).ok_or("The image is drawn with a degenerate transform.")?;
    let target = [fit[2] - fit[0], 0.0, 0.0, fit[3] - fit[1], fit[0], fit[1]];
    let x = mul(&target, &inv);
    replace_op(doc, page_id, &scan, im.src, im.op, draw_ops(x, name, None))
}

/// Inserts an image file, with its top-left corner at `at` (user space) or
/// centred on the page, upright as the page is shown.
pub fn insert(doc: &mut Document, page_id: ObjectId, path: &Path, at: Option<[f32; 2]>) -> Result<(), String> {
    let (id, w, h) = object_from_file(doc, path)?;
    let name = add_resource(doc, page_id, Src::Page, "XObject", id, "InqIm")?;
    let media = inherited(doc, page_id, b"CropBox")
        .or_else(|| inherited(doc, page_id, b"MediaBox"))
        .and_then(|o| rect(doc, o))
        .unwrap_or([0.0, 0.0, 595.0, 842.0]);
    let rot = crate::edit::page_rotation(doc, page_id) as f32;
    let (sn, cs) = rot.to_radians().sin_cos();
    let page_w = if rot as i64 % 180 == 0 { media[2] - media[0] } else { media[3] - media[1] };
    // 96 pixels per inch, at most 60 % of the page width.
    let mut iw = w as f32 * 0.75;
    let mut ih = h as f32 * 0.75;
    let max = page_w * 0.6;
    if iw > max {
        ih *= max / iw;
        iw = max;
    }
    let up = (-sn, cs);
    let origin = match at {
        Some(p) => (p[0] - up.0 * ih, p[1] - up.1 * ih),
        None => {
            let c = ((media[0] + media[2]) / 2.0, (media[1] + media[3]) / 2.0);
            let dir = (cs, sn);
            (c.0 - dir.0 * iw / 2.0 - up.0 * ih / 2.0, c.1 - dir.1 * iw / 2.0 - up.1 * ih / 2.0)
        }
    };
    let m = [iw * cs, iw * sn, -ih * sn, ih * cs, origin.0, origin.1];
    crate::edit::add_page_ops(doc, page_id, draw_ops(m, name, None), false)
}

/// The image as a file: JPEGs as they are stored, everything else as PNG.
pub fn extract(doc: &Document, page_id: ObjectId, index: usize) -> Result<(Vec<u8>, &'static str), String> {
    let (_, im) = placement(doc, page_id, index)?;
    let id = im.id.ok_or("Inline images cannot be extracted yet.")?;
    let s = doc.get_object(id).and_then(Object::as_stream).map_err(err)?;
    if is_dct(&s.dict) && !s.dict.has(b"SMask") {
        return Ok((s.content.clone(), "jpg"));
    }
    let img = decode(doc, id)?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(err)?;
    Ok((out.into_inner(), "png"))
}
