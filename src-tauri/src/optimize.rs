//! Making files smaller.
//!
//! * Images drawn at a higher resolution than asked for are resampled to
//!   that resolution; photographs are stored as JPEG, everything else
//!   losslessly. An image is only replaced when the result is smaller.
//! * Identical streams and font objects are merged, so a font or image
//!   embedded once per page is kept once.
//! * Page thumbnails and applications' private data can be dropped.
//!
//! Object streams (the compact file structure of PDF 1.5) are a property of
//! how the file is written, not of the document; the store applies them on
//! save.

use crate::images;
use crate::interp::scan_page;
use crate::pages;
use crate::util::*;
use lopdf::{dictionary, Document, Object, ObjectId, Stream, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    /// Resample images drawn above this many pixels per inch; 0 leaves images alone.
    pub image_dpi: f32,
    /// JPEG quality for photographs, 1–100.
    pub jpeg_quality: u8,
    pub merge_duplicates: bool,
    /// Drop page thumbnails and private application data.
    pub discard_extras: bool,
    /// Write object streams and a cross-reference stream when saving.
    pub compact: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { image_dpi: 150.0, jpeg_quality: 75, merge_duplicates: true, discard_extras: true, compact: true }
    }
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub images_resampled: usize,
    pub images_kept: usize,
    pub image_bytes_before: usize,
    pub image_bytes_after: usize,
    pub duplicates_merged: usize,
    pub fonts_merged: usize,
    pub extras_removed: usize,
    /// File size before and after, as a full save would write it.
    pub size_before: usize,
    pub size_after: usize,
}

/// The size a full save of `doc` has.
pub fn saved_size(doc: &Document, compact: bool) -> usize {
    let mut d = doc.clone();
    d.prune_objects();
    d.compress();
    let mut buf = Vec::new();
    let ok = if compact {
        d.save_modern(&mut buf).is_ok()
    } else {
        d.save_to(&mut buf).is_ok()
    };
    if ok {
        buf.len()
    } else {
        0
    }
}

fn stream_size(doc: &Document, id: ObjectId) -> usize {
    let Ok(Object::Stream(s)) = doc.get_object(id) else { return 0 };
    let mask = match s.dict.get(b"SMask") {
        Ok(Object::Reference(m)) => doc.get_object(*m).ok().and_then(|o| o.as_stream().ok()).map(|m| m.content.len()).unwrap_or(0),
        _ => 0,
    };
    s.content.len() + mask
}

fn is_gray(img: &image::RgbaImage) -> bool {
    img.pixels().all(|p| p.0[0] == p.0[1] && p.0[1] == p.0[2])
}

/// How many distinct colours an image has, up to `limit`.
fn few_colours(img: &image::RgbaImage, limit: usize) -> bool {
    let mut seen = std::collections::HashSet::new();
    for p in img.pixels() {
        seen.insert([p.0[0], p.0[1], p.0[2]]);
        if seen.len() > limit {
            return false;
        }
    }
    true
}

/// Replaces image `id` by a copy resampled by `factor` (< 1) when that is smaller.
fn resample(doc: &mut Document, id: ObjectId, factor: f32, quality: u8) -> Result<bool, String> {
    let (was_jpeg, bpc, is_mask, name_keys) = {
        let s = doc.get_object(id).and_then(Object::as_stream).map_err(err)?;
        let filter = match s.dict.get(b"Filter") {
            Ok(Object::Name(n)) => n.clone(),
            Ok(Object::Array(a)) if a.len() == 1 => a[0].as_name().map(|n| n.to_vec()).unwrap_or_default(),
            Ok(Object::Array(_)) => b"multiple".to_vec(),
            _ => Vec::new(),
        };
        // Formats this build cannot decode stay as they are.
        if matches!(filter.as_slice(), b"JPXDecode" | b"JBIG2Decode" | b"CCITTFaxDecode" | b"multiple") {
            return Ok(false);
        }
        let bpc = s.dict.get(b"BitsPerComponent").ok().and_then(num).unwrap_or(8.0) as u32;
        let is_mask = s.dict.get(b"ImageMask").ok().and_then(|o| o.as_bool().ok()).unwrap_or(false);
        // Entries that belong to how the image is used, not to its pixels.
        let keep: Vec<(Vec<u8>, Object)> = [&b"Name"[..], b"StructParent", b"OC", b"Metadata", b"Intent"]
            .iter()
            .filter_map(|k| s.dict.get(k).ok().map(|v| (k.to_vec(), v.clone())))
            .collect();
        (filter == b"DCTDecode", bpc, is_mask, keep)
    };
    // Stencil masks carry no colour of their own, and one-bit scans would grow as greyscale.
    if is_mask || bpc == 1 {
        return Ok(false);
    }
    let before = stream_size(doc, id);
    let img = images::decode(doc, id)?;
    let (w, h) = (((img.width() as f32 * factor).round() as u32).max(1), ((img.height() as f32 * factor).round() as u32).max(1));
    if w >= img.width() || h >= img.height() {
        return Ok(false);
    }
    let small = image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3);
    let has_alpha = small.pixels().any(|p| p.0[3] < 255);
    let photo = was_jpeg || !few_colours(&small, 256);
    let new_id = if photo {
        let gray = is_gray(&small);
        let mut jpeg = Vec::new();
        {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality.clamp(1, 100));
            if gray {
                let data: Vec<u8> = small.pixels().map(|p| p.0[0]).collect();
                enc.encode(&data, w, h, image::ExtendedColorType::L8).map_err(|e| format!("The image could not be re-encoded: {e}"))?;
            } else {
                let data: Vec<u8> = small.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2]]).collect();
                enc.encode(&data, w, h, image::ExtendedColorType::Rgb8).map_err(|e| format!("The image could not be re-encoded: {e}"))?;
            }
        }
        let mut d = dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
            "ColorSpace" => if gray { "DeviceGray" } else { "DeviceRGB" }, "BitsPerComponent" => 8, "Filter" => "DCTDecode",
        };
        if has_alpha {
            let alpha: Vec<u8> = small.pixels().map(|p| p.0[3]).collect();
            let mut m = Stream::new(
                dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 },
                alpha,
            );
            let _ = m.compress();
            d.set("SMask", doc.add_object(m));
        }
        doc.add_object(Stream::new(d, jpeg).with_compression(false))
    } else {
        images::object_from_rgba(doc, &small)
    };
    if stream_size(doc, new_id) >= before {
        // Not worth it: forget the copy.
        if let Ok(Object::Stream(s)) = doc.get_object(new_id) {
            if let Ok(Object::Reference(m)) = s.dict.get(b"SMask") {
                let m = *m;
                doc.objects.remove(&m);
            }
        }
        doc.objects.remove(&new_id);
        return Ok(false);
    }
    // Put the new image where the old one was, so every use of it follows.
    let Some(Object::Stream(mut new)) = doc.objects.remove(&new_id) else { return Ok(false) };
    for (k, v) in name_keys {
        new.dict.set(k, v);
    }
    doc.objects.insert(id, Object::Stream(new));
    Ok(true)
}

fn downsample_images(doc: &mut Document, opts: &Options, report: &mut Report) {
    // The highest resolution each image is drawn at anywhere in the document.
    let mut drawn: HashMap<ObjectId, f32> = HashMap::new();
    for pid in pages::page_ids(doc) {
        let Ok(scan) = scan_page(doc, pid) else { continue };
        for im in &scan.images {
            let Some(id) = im.id else { continue };
            let q = im.quad();
            let side = |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            let (wpt, hpt) = (side(q[0], q[1]), side(q[0], q[3]));
            if wpt < 0.5 || hpt < 0.5 {
                continue;
            }
            let dpi = (im.width as f32 / (wpt / 72.0)).max(im.height as f32 / (hpt / 72.0));
            let e = drawn.entry(id).or_insert(0.0);
            *e = e.max(dpi);
        }
    }
    let mut ids: Vec<(ObjectId, f32)> = drawn.into_iter().collect();
    ids.sort_by_key(|(id, _)| *id);
    for (id, dpi) in ids {
        let before = stream_size(doc, id);
        report.image_bytes_before += before;
        // A little above the target is not worth the loss of a resampling pass.
        let done = dpi > opts.image_dpi * 1.2 && resample(doc, id, opts.image_dpi / dpi, opts.jpeg_quality).unwrap_or(false);
        if done {
            report.images_resampled += 1;
        } else {
            report.images_kept += 1;
        }
        report.image_bytes_after += stream_size(doc, id);
    }
}

fn remap(o: &mut Object, map: &HashMap<ObjectId, ObjectId>) {
    match o {
        Object::Reference(id) => {
            if let Some(to) = map.get(id) {
                *id = *to;
            }
        }
        Object::Array(a) => a.iter_mut().for_each(|x| remap(x, map)),
        Object::Dictionary(d) => d.iter_mut().for_each(|(_, v)| remap(v, map)),
        Object::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| remap(v, map)),
        _ => {}
    }
}

/// Whether two copies of this object can be one: streams of content that is
/// only ever read, and the dictionaries that describe fonts and graphics states.
fn mergeable(o: &Object) -> Option<bool> {
    match o {
        Object::Stream(s) => {
            let ty = s.dict.get(b"Type").ok().and_then(|t| t.as_name().ok());
            if matches!(ty, Some(b"XRef") | Some(b"ObjStm") | Some(b"Metadata")) {
                return None;
            }
            Some(matches!(ty, Some(b"Font")) || s.dict.has(b"Length1") || s.dict.has(b"Length2") || s.dict.has(b"Length3") || matches!(s.dict.get(b"Subtype").ok().and_then(|t| t.as_name().ok()), Some(b"Type1C") | Some(b"CIDFontType0C") | Some(b"OpenType")))
        }
        Object::Dictionary(d) => {
            let ty = d.get(b"Type").ok().and_then(|t| t.as_name().ok());
            match ty {
                Some(b"Font") | Some(b"FontDescriptor") | Some(b"Encoding") => Some(true),
                Some(b"ExtGState") => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn merge_duplicates(doc: &mut Document, report: &mut Report) {
    // Merging two font files makes their descriptors equal, then their fonts: repeat until settled.
    for _ in 0..6 {
        let mut first: HashMap<Vec<u8>, ObjectId> = HashMap::new();
        let mut map: HashMap<ObjectId, ObjectId> = HashMap::new();
        let mut fonts = 0;
        for (id, obj) in &doc.objects {
            let Some(is_font) = mergeable(obj) else { continue };
            let mut key = Vec::new();
            if Writer::write_object(&mut key, obj).is_err() {
                continue;
            }
            match first.get(&key) {
                Some(keep) => {
                    map.insert(*id, *keep);
                    if is_font {
                        fonts += 1;
                    }
                }
                None => {
                    first.insert(key, *id);
                }
            }
        }
        if map.is_empty() {
            break;
        }
        for obj in doc.objects.values_mut() {
            remap(obj, &map);
        }
        remap_dict(&mut doc.trailer, &map);
        for id in map.keys() {
            doc.objects.remove(id);
        }
        report.duplicates_merged += map.len();
        report.fonts_merged += fonts;
    }
}

fn remap_dict(d: &mut lopdf::Dictionary, map: &HashMap<ObjectId, ObjectId>) {
    d.iter_mut().for_each(|(_, v)| remap(v, map));
}

fn discard_extras(doc: &mut Document, report: &mut Report) {
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        let Some(obj) = doc.objects.get_mut(&id) else { continue };
        let d = match obj {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &mut s.dict,
            _ => continue,
        };
        for key in [&b"Thumb"[..], b"PieceInfo"] {
            if d.has(key) {
                d.remove(key);
                report.extras_removed += 1;
            }
        }
    }
}

pub fn optimize(doc: &mut Document, opts: &Options) -> Result<Report, String> {
    let mut report = Report { size_before: saved_size(doc, false), ..Default::default() };
    if opts.image_dpi >= 20.0 {
        downsample_images(doc, opts, &mut report);
    }
    if opts.discard_extras {
        discard_extras(doc, &mut report);
    }
    if opts.merge_duplicates {
        merge_duplicates(doc, &mut report);
    }
    doc.prune_objects();
    report.size_after = saved_size(doc, opts.compact);
    Ok(report)
}
