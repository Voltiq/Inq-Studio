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
    /// The language it was recognised in, when that was chosen automatically.
    #[serde(default)]
    pub language: Option<String>,
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
        Ok(Recognised { lines, angle, language: None })
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

/// Words that are very common in a language and rare in the others.
const COMMON: [(&str, &[&str]); 10] = [
    ("de", &["der", "die", "und", "das", "nicht", "ist", "mit", "den", "von", "für", "auf", "dem", "ein", "eine", "sich", "auch", "wird", "sind", "zu", "im"]),
    ("en", &["the", "and", "of", "to", "is", "that", "for", "with", "this", "are", "was", "be", "by", "on", "not", "have", "from", "or", "as", "at"]),
    ("fr", &["le", "la", "les", "et", "des", "est", "une", "que", "dans", "pour", "pas", "sur", "avec", "du", "au", "ce", "qui", "sont", "nous", "vous"]),
    ("es", &["el", "los", "las", "que", "del", "por", "con", "una", "para", "es", "como", "más", "pero", "sus", "se", "al", "lo", "este", "son", "no"]),
    ("it", &["il", "che", "di", "per", "una", "con", "non", "sono", "della", "gli", "nel", "alla", "più", "come", "anche", "le", "del", "dei", "si", "da"]),
    ("nl", &["de", "het", "een", "van", "en", "niet", "dat", "voor", "met", "zijn", "op", "te", "aan", "ook", "als", "bij", "wordt", "naar", "om", "er"]),
    ("pt", &["os", "que", "do", "da", "em", "para", "com", "não", "uma", "por", "mais", "como", "dos", "ao", "das", "são", "foi", "seu", "se", "na"]),
    ("pl", &["nie", "się", "jest", "na", "to", "że", "do", "jak", "ale", "po", "co", "dla", "od", "przez", "tak", "są", "lub", "oraz", "czy", "być"]),
    ("sv", &["och", "att", "det", "som", "en", "på", "är", "av", "för", "med", "till", "den", "har", "inte", "om", "ett", "var", "jag", "de", "men"]),
    ("tr", &["bir", "ve", "bu", "için", "ile", "da", "de", "olarak", "çok", "daha", "gibi", "en", "ne", "var", "ama", "kadar", "olan", "sonra", "ya", "her"]),
];

/// How much a recognised page reads like the language `tag`: the share of
/// its words that are among the language's most common ones, and, for
/// languages without a list, a small credit for every word found.
fn fits(found: &Recognised, tag: &str) -> f32 {
    let words: Vec<String> = found.lines.iter().flat_map(|l| &l.words).map(|w| w.text.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase()).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return 0.0;
    }
    let short = tag.split('-').next().unwrap_or(tag).to_lowercase();
    match COMMON.iter().find(|(l, _)| *l == short) {
        Some((_, list)) => words.iter().filter(|w| list.contains(&w.as_str())).count() as f32 / words.len() as f32 + 0.001,
        None => 0.02,
    }
}

/// Recognises a page in every installed language (one per language, not per
/// region) and keeps the result that reads most like its language.
pub fn recognize_auto(gray: &[u8], w: u32, h: u32) -> Result<Recognised, String> {
    let mut tried: Vec<String> = Vec::new();
    let mut best: Option<(f32, Recognised)> = None;
    for l in languages()? {
        let short = l.tag.split('-').next().unwrap_or(&l.tag).to_lowercase();
        if tried.contains(&short) {
            continue;
        }
        tried.push(short);
        let Ok(mut found) = recognize(gray, w, h, Some(&l.tag)) else { continue };
        let score = fits(&found, &l.tag);
        found.language = Some(l.tag.clone());
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, found));
        }
    }
    best.map(|(_, r)| r).ok_or_else(|| "Windows has no OCR language pack installed.".to_string())
}

/// Asks Windows to install the OCR pack of a language. Windows shows its
/// administrator prompt; the download comes from Windows Update.
pub fn install_language(tag: &str) -> Result<Vec<OcrLanguage>, String> {
    let ok = tag.len() <= 16 && tag.split('-').all(|p| !p.is_empty() && p.len() <= 8 && p.chars().all(|c| c.is_ascii_alphanumeric()));
    if !ok {
        return Err("That is not a language tag.".into());
    }
    if !cfg!(windows) {
        return Err("OCR languages are installed through Windows.".into());
    }
    let inner = format!("Add-WindowsCapability -Online -Name Language.OCR~~~{tag}~0.0.1.0");
    let outer = format!("Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-Command','{inner}'");
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-Command", &outer]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window of our own.
        command.creation_flags(0x0800_0000);
    }
    let status = command.status().map_err(|e| format!("Windows PowerShell could not be started: {e}"))?;
    let now = languages()?;
    if now.iter().any(|l| l.tag.eq_ignore_ascii_case(tag)) {
        Ok(now)
    } else if !status.success() {
        Err("The installation was cancelled or not allowed. It needs administrator rights.".into())
    } else {
        Err("Windows did not install the language. It needs administrator rights and a connection to Windows Update; on a managed computer the administrator may have to add it.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str) -> Recognised {
        Recognised { lines: vec![OcrLine { words: text.split(' ').map(|w| OcrWord { text: w.into(), x: 0.0, y: 0.0, w: 1.0, h: 1.0 }).collect() }], angle: None, language: None }
    }

    #[test]
    fn tells_languages_apart() {
        let german = page("Der Schüler hat das Zeugnis mit der Note gut erhalten und ist nicht versetzt.");
        let english = page("The student received the report with a good mark and is not moved up.");
        assert!(fits(&german, "de-DE") > fits(&german, "en-US"));
        assert!(fits(&english, "en-GB") > fits(&english, "de-DE"));
        assert_eq!(fits(&page(""), "de-DE"), 0.0);
        assert!(install_language("de-DE; rm").is_err());
    }
}

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
