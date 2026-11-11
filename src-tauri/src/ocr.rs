//! OCR: making scanned pages searchable.
//!
//! Recognition is done by the recogniser built into Windows
//! (`Windows.Media.Ocr`), which works offline with the language packs
//! installed on the computer. The window renders a page to grey pixels and
//! sends them here; the words that come back are laid over the page as
//! invisible text, each stretched to the place it was recognised at. The
//! scan itself is not touched.

use crate::edit::{place_hidden_words, FontChoice, HiddenWord};
use crate::util::*;
use lopdf::{Document, ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OcrWord {
    pub text: String,
    /// Bounds in image pixels, origin at the top left.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OcrLine {
    pub words: Vec<OcrWord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Recognised {
    pub lines: Vec<OcrLine>,
    /// Skew of the text in degrees, when the recogniser measured one.
    pub angle: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguage {
    pub tag: String,
    pub name: String,
}

/// What the window sends back for one page when the layer is written.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OcrPage {
    pub page: usize,
    /// Maps user space to image pixels (pdf.js `viewport.transform`).
    pub transform: [f32; 6],
    pub lines: Vec<OcrLine>,
}

#[cfg(windows)]
mod engine {
    use super::*;
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    fn failed(what: &str, e: windows::core::Error) -> String {
        format!("{what}: {}", e.message())
    }

    pub fn languages() -> Result<Vec<OcrLanguage>, String> {
        let list = OcrEngine::AvailableRecognizerLanguages().map_err(|e| failed("Windows OCR is not available", e))?;
        let mut out = Vec::new();
        for l in list {
            let tag = l.LanguageTag().map(|s| s.to_string()).unwrap_or_default();
            let name = l.DisplayName().map(|s| s.to_string()).unwrap_or_else(|_| tag.clone());
            if !tag.is_empty() {
                out.push(OcrLanguage { tag, name });
            }
        }
        Ok(out)
    }

    pub fn max_dimension() -> u32 {
        OcrEngine::MaxImageDimension().unwrap_or(10_000)
    }

    pub fn recognize(gray: &[u8], w: u32, h: u32, lang: Option<&str>) -> Result<Recognised, String> {
        if w == 0 || h == 0 || gray.len() < (w as usize) * (h as usize) {
            return Err("The page image is not the size it says.".into());
        }
        let limit = max_dimension();
        if w > limit || h > limit {
            return Err(format!("The page image is larger than Windows OCR accepts ({limit} pixels a side)."));
        }
        let engine = match lang.filter(|l| !l.is_empty()) {
            Some(tag) => {
                let language = Language::CreateLanguage(&HSTRING::from(tag)).map_err(|e| failed("That language is not known to Windows", e))?;
                OcrEngine::TryCreateFromLanguage(&language)
                    .map_err(|_| format!("Windows has no OCR language pack for {tag}. Add it under Settings › Time & language › Language & region."))?
            }
            None => OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|_| "Windows has no OCR language pack for your languages. Add one under Settings › Time & language › Language & region.".to_string())?,
        };
        let mut bgra = Vec::with_capacity(gray.len() * 4);
        for g in &gray[..(w as usize) * (h as usize)] {
            bgra.extend_from_slice(&[*g, *g, *g, 255]);
        }
        let writer = DataWriter::new().map_err(|e| failed("Windows OCR is not available", e))?;
        writer.WriteBytes(&bgra).map_err(|e| failed("The page image could not be handed to Windows", e))?;
        let buffer = writer.DetachBuffer().map_err(|e| failed("The page image could not be handed to Windows", e))?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32, BitmapAlphaMode::Premultiplied)
            .map_err(|e| failed("The page image could not be handed to Windows", e))?;
        let result = engine.RecognizeAsync(&bitmap).and_then(|op| op.join()).map_err(|e| failed("Recognition failed", e))?;
        let mut lines = Vec::new();
        for line in result.Lines().map_err(|e| failed("Recognition failed", e))? {
            let mut words = Vec::new();
            for word in line.Words().map_err(|e| failed("Recognition failed", e))? {
                let (Ok(text), Ok(r)) = (word.Text(), word.BoundingRect()) else { continue };
                let text = text.to_string();
                if text.trim().is_empty() || r.Width <= 0.0 || r.Height <= 0.0 {
                    continue;
                }
                words.push(OcrWord { text, x: r.X, y: r.Y, w: r.Width, h: r.Height });
            }
            if !words.is_empty() {
                lines.push(OcrLine { words });
            }
        }
        let angle = result.TextAngle().ok().and_then(|a| a.Value().ok());
        Ok(Recognised { lines, angle })
    }
}

#[cfg(not(windows))]
mod engine {
    use super::*;

    pub fn languages() -> Result<Vec<OcrLanguage>, String> {
        Err("OCR uses the recogniser built into Windows.".into())
    }

    pub fn max_dimension() -> u32 {
        10_000
    }

    pub fn recognize(_: &[u8], _: u32, _: u32, _: Option<&str>) -> Result<Recognised, String> {
        Err("OCR uses the recogniser built into Windows.".into())
    }
}

pub use engine::{languages, max_dimension, recognize};

/// The font the invisible layer is written in: a standard font when every
/// character is in its encoding (nothing is embedded then), otherwise an
/// installed font that covers the script.
fn layer_font(lines: &[OcrLine], lang: &str) -> FontChoice {
    let plain = lines.iter().flat_map(|l| &l.words).flat_map(|w| w.text.chars()).all(|c| crate::encoding::win_ansi_byte(c).is_some());
    if plain {
        return FontChoice::Standard { name: "Helvetica".into() };
    }
    let l = lang.to_lowercase();
    let family = if l.starts_with("ja") {
        "Yu Gothic"
    } else if l.starts_with("zh") {
        "Microsoft YaHei"
    } else if l.starts_with("ko") {
        "Malgun Gothic"
    } else {
        "Arial"
    };
    FontChoice::System { family: family.into(), bold: false, italic: false }
}

/// Lays the recognised words over the page as invisible text.
/// Returns the number of words placed and font notices.
pub fn add_text_layer(doc: &mut Document, page_id: ObjectId, page: &OcrPage, lang: &str) -> Result<(usize, Vec<String>), String> {
    let inv = invert(&page.transform).ok_or("The page image has no usable transform.")?;
    // Image axes in user space: right along a line of text, and up.
    let dx = apply_vec(&inv, 1.0, 0.0);
    let dy = apply_vec(&inv, 0.0, -1.0);
    let (sx, sy) = (len(dx).max(1e-6), len(dy).max(1e-6));
    let dir = (dx.0 / sx, dx.1 / sx);
    let up = (dy.0 / sy, dy.1 / sy);
    let mut words = Vec::new();
    for line in &page.lines {
        let top = line.words.iter().map(|w| w.y).fold(f32::MAX, f32::min);
        let bottom = line.words.iter().map(|w| w.y + w.h).fold(f32::MIN, f32::max);
        let height = (bottom - top).max(1.0);
        // The box of a line runs from its ascenders to its descenders; the baseline sits a little above the bottom.
        let baseline = bottom - 0.2 * height;
        let size = height * sy;
        for w in &line.words {
            words.push(HiddenWord { text: w.text.clone(), origin: apply(&inv, w.x, baseline), dir, up, size, width: w.w * sx });
        }
    }
    place_hidden_words(doc, page_id, &words, &layer_font(&page.lines, lang))
}
