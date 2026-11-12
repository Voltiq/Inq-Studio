//! Commands of the v0.4 tools: protection, optimising, export, comparing,
//! accessibility, PDF/A, OCR and signatures.

use crate::security::{ProtectRequest, Protection};
use crate::store::{self, off_thread, DocInfo, DocState, EditResult, Store};
use crate::{annots, compare, ocr, optimize, pages, pdfa, sign, tags};
use serde::Serialize;
use std::path::PathBuf;
use tauri::ipc::{InvokeBody, Request};
use tauri::State;

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

/// Sets the password protection the next save writes; `None` removes it.
#[tauri::command]
pub async fn doc_protect(store: State<'_, Store>, id: u32, request: Option<ProtectRequest>) -> Result<DocInfo, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            match request {
                Some(req) => {
                    req.check()?;
                    d.protection = Protection::New(req);
                }
                None => d.protection = Protection::None,
            }
            d.dirty = true;
            Ok(store::info_of(id, d))
        })
    })
    .await
}

// ---------------------------------------------------------------------------
// Optimise
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Optimised {
    pub state: DocState,
    pub report: optimize::Report,
}

#[tauri::command]
pub async fn doc_optimize(store: State<'_, Store>, id: u32, options: optimize::Options) -> Result<Optimised, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let compact = options.compact;
        let (_, report) = store.mutate(id, "Optimise", |doc| optimize::optimize(doc, &options))?;
        let state = store.with(id, |d| {
            d.compact = compact;
            Ok(store::state(id, d))
        })?;
        Ok(Optimised { state, report })
    })
    .await
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_text(store: State<'_, Store>, id: u32, pages: Vec<usize>, path: String) -> Result<usize, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let text = store.with(id, |d| compare::document_text(&d.doc, &pages))?;
        std::fs::write(&path, text.as_bytes()).map_err(|e| format!("The text could not be saved: {e}"))?;
        Ok(text.chars().filter(|c| !c.is_whitespace()).count())
    })
    .await
}

fn header(request: &Request<'_>, name: &str) -> Result<String, String> {
    let raw = request.headers().get(name).and_then(|v| v.to_str().ok()).ok_or(format!("Missing {name}"))?;
    // Values are percent-encoded UTF-8.
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|_| format!("Bad {name}"))
}

fn body<'a>(request: &'a Request<'_>) -> Result<&'a [u8], String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes),
        _ => Err("Expected binary data.".into()),
    }
}

/// Writes the request body to the file named in the `x-path` header (a page image the window rendered).
#[tauri::command]
pub fn file_write(request: Request<'_>) -> Result<(), String> {
    let path = header(&request, "x-path")?;
    std::fs::write(PathBuf::from(path), body(&request)?).map_err(|e| format!("The file could not be saved: {e}"))
}

// ---------------------------------------------------------------------------
// Compare
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn doc_compare(store: State<'_, Store>, a: u32, b: u32) -> Result<compare::Comparison, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let first = store.with(a, |d| Ok(d.doc.clone()))?;
        let second = store.with(b, |d| Ok(d.doc.clone()))?;
        compare::compare(&first, &second)
    })
    .await
}

// ---------------------------------------------------------------------------
// Accessibility
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn tags_get(store: State<'_, Store>, id: u32) -> Result<tags::TagInfo, String> {
    let store = store.inner().clone();
    off_thread(move || store.with(id, |d| Ok(tags::read(&d.doc)))).await
}

#[tauri::command]
pub async fn tags_set_element(
    store: State<'_, Store>,
    id: u32,
    element: String,
    alt: Option<String>,
    actual_text: Option<String>,
    title: Option<String>,
) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store
            .mutate(id, "Alternative text", |doc| tags::set_element(doc, &element, alt.as_deref(), actual_text.as_deref(), title.as_deref()))
            .map(|(s, _)| s)
    })
    .await
}

#[tauri::command]
pub async fn tags_set_document(store: State<'_, Store>, id: u32, lang: Option<String>, display_title: Option<bool>) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || store.mutate(id, "Accessibility settings", |doc| tags::set_document(doc, lang.as_deref(), display_title)).map(|(s, _)| s)).await
}

#[tauri::command]
pub async fn reading_order(store: State<'_, Store>, id: u32, page: usize) -> Result<Vec<tags::OrderItem>, String> {
    let store = store.inner().clone();
    off_thread(move || store.with(id, |d| tags::reading_order(&d.doc, page))).await
}

// ---------------------------------------------------------------------------
// PDF/A
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn pdfa_check(store: State<'_, Store>, id: u32, part: u8) -> Result<pdfa::Report, String> {
    let store = store.inner().clone();
    off_thread(move || store.with(id, |d| Ok(pdfa::validate(&d.doc, pdfa::Level { part: part.clamp(1, 3) }, !matches!(d.protection, Protection::None))))).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Converted {
    pub state: DocState,
    pub done: Vec<String>,
    pub report: pdfa::Report,
}

#[tauri::command]
pub async fn pdfa_convert(store: State<'_, Store>, id: u32, part: u8) -> Result<Converted, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let level = pdfa::Level { part: part.clamp(1, 3) };
        let (_, mut done) = store.mutate(id, format!("Convert to {}", level.label()), |doc| pdfa::convert(doc, level))?;
        store.with(id, |d| {
            if !matches!(d.protection, Protection::None) {
                d.protection = Protection::None;
                done.insert(0, "Removed the password protection.".into());
            }
            if level.part == 1 {
                d.compact = false;
            }
            let report = pdfa::validate(&d.doc, level, false);
            Ok(Converted { state: store::state(id, d), done, report })
        })
    })
    .await
}

// ---------------------------------------------------------------------------
// OCR
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrSetup {
    pub languages: Vec<ocr::OcrLanguage>,
    pub max_dimension: u32,
}

#[tauri::command]
pub async fn ocr_setup() -> Result<OcrSetup, String> {
    off_thread(|| Ok(OcrSetup { languages: ocr::languages()?, max_dimension: ocr::max_dimension() })).await
}

/// Installs the Windows OCR pack of a language (Windows asks for administrator rights) and returns what is installed then.
#[tauri::command]
pub async fn ocr_install(tag: String) -> Result<Vec<ocr::OcrLanguage>, String> {
    off_thread(move || ocr::install_language(&tag)).await
}

/// Recognises the text of one page image: the body is `width × height` grey
/// pixels, the size and language come in headers.
#[tauri::command]
pub async fn ocr_recognize(request: Request<'_>) -> Result<ocr::Recognised, String> {
    let width: u32 = header(&request, "x-width")?.parse().map_err(|_| "Bad width")?;
    let height: u32 = header(&request, "x-height")?.parse().map_err(|_| "Bad height")?;
    let lang = header(&request, "x-lang").unwrap_or_default();
    let pixels = body(&request)?.to_vec();
    off_thread(move || {
        if lang == "auto" {
            ocr::recognize_auto(&pixels, width, height)
        } else {
            ocr::recognize(&pixels, width, height, Some(lang.as_str()).filter(|l| !l.is_empty()))
        }
    })
    .await
}

/// Writes the recognised text of several pages into the document as one change.
#[tauri::command]
pub async fn ocr_apply(store: State<'_, Store>, id: u32, pages: Vec<ocr::OcrPage>, lang: String) -> Result<EditResult, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let label = format!("Recognise text on {}", store::pages_label(&pages.iter().map(|p| p.page).collect::<Vec<_>>()));
        let (state, (words, notices)) = store.mutate(id, label, |doc| {
            let mut words = 0;
            let mut notices: Vec<String> = Vec::new();
            for p in &pages {
                let pid = pages::page_id(doc, p.page)?;
                let (n, notes) = ocr::add_text_layer(doc, pid, p, &lang)?;
                words += n;
                for note in notes {
                    if !notices.contains(&note) {
                        notices.push(note);
                    }
                }
            }
            Ok((words, notices))
        })?;
        let mut notice = format!("Added {} of searchable text on {}.", plural(words, "word"), plural(pages.len(), "page"));
        if !notices.is_empty() {
            notice.push(' ');
            notice.push_str(&notices.join(" "));
        }
        Ok(EditResult { state, notice: Some(notice) })
    })
    .await
}

// ---------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signatures {
    pub signatures: Vec<sign::SigInfo>,
    /// Empty signature fields that can be signed.
    pub empty_fields: Vec<String>,
    /// The document has changes that are not in the file the signatures were checked against.
    pub unsaved: bool,
}

#[tauri::command]
pub async fn sig_list(store: State<'_, Store>, id: u32) -> Result<Signatures, String> {
    let store = store.inner().clone();
    off_thread(move || {
        // Checking hashes the whole file: work on copies, outside the lock.
        let (doc, file, unsaved) = store.with(id, |d| Ok((d.doc.clone(), d.base.as_ref().map(|b| b.0.clone()), d.dirty)))?;
        Ok(Signatures { signatures: sign::list(&doc, file.as_deref().map(|v| v.as_slice())), empty_fields: sign::empty_fields(&doc), unsaved })
    })
    .await
}

#[tauri::command]
pub async fn sig_identities() -> Result<Vec<sign::Identity>, String> {
    off_thread(sign::store_identities).await
}

#[tauri::command]
pub async fn sig_file_identity(path: String, password: String) -> Result<sign::Identity, String> {
    off_thread(move || sign::file_identity(&path, &password)).await
}

#[tauri::command]
pub async fn sig_create_identity(request: sign::NewIdentity) -> Result<sign::Identity, String> {
    off_thread(move || sign::create_identity(&request)).await
}

/// Signs the document and writes it to `path` (or over its own file).
#[tauri::command]
pub async fn doc_sign(store: State<'_, Store>, id: u32, identity: sign::IdentityRef, options: sign::SignOptions, path: Option<String>) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let generation = store.next_generation();
        sign::sign_with(&identity, |who, signer| {
            store.with(id, |d| {
                let target = path.clone().map(PathBuf::from).or_else(|| d.path.clone()).ok_or("Choose where to save the signed document.")?;
                if !matches!(d.protection, Protection::None) {
                    return Err("Remove the password protection before signing; a protected document cannot be signed here yet.".into());
                }
                let mut out = d.doc.clone();
                let date = annots::pdf_date();
                sign::add_signature(&mut out, &options, &who.name, &date)?;
                store::info_dict_mut(&mut out).set("ModDate", lopdf::text_string(&date));
                if pdfa::claimed(&out).is_some() {
                    pdfa::sync_metadata(&mut out);
                }
                // Earlier signatures stay valid only when this one is appended.
                let bytes = match (&d.base, store::is_signed(&d.doc)) {
                    (Some(base), true) => store::incremental_bytes(base, &out)?,
                    (None, true) => return Err("This document is signed already but has no file to append to. Save it first.".into()),
                    _ => store::full_bytes(&mut out, &Protection::None, &None, false)?.0,
                };
                let tsa = options.timestamp_url.trim().to_string();
                let signed = sign::finish(bytes, |a, b| {
                    let cms = signer(a, b)?;
                    if tsa.is_empty() {
                        Ok(cms)
                    } else {
                        sign::timestamp(cms, &tsa)
                    }
                })?;
                store::write_safely(&target, &signed)?;
                // The document in memory becomes exactly what the file holds, value included.
                let reread = store::load_bytes(&signed, None)?;
                store::saved(d, reread, signed, None, target, generation);
                d.undo.clear();
                d.redo.clear();
                d.changed = None;
                Ok(store::state(id, d))
            })
        })
    })
    .await
}
