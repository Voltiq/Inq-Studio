//! Open documents and the commands the window calls.
//!
//! Each open PDF lives here as a parsed object graph. Every change takes a
//! snapshot first, so undo and redo are exact, and bumps a revision number:
//! edits name the revision they were made against, so a line id from a stale
//! page scan can never edit the wrong text.

use crate::annots::{self, AnnotInfo, NewAnnot};
use crate::edit::{self, LineEdit, NewText};
use crate::fonts::{FontKind, PdfFont, ProgramKind};
use crate::interp::{page_resources, resource_fonts, scan_page, TextLine};
use crate::pages;
use crate::sysfonts;
use crate::util::*;
use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::ipc::Response;
use tauri::State;

const UNDO_LIMIT: usize = 40;

pub struct OpenDoc {
    pub path: Option<PathBuf>,
    pub name: String,
    pub doc: Document,
    pub undo: Vec<Document>,
    pub redo: Vec<Document>,
    pub revision: u64,
    pub dirty: bool,
    pub was_encrypted: bool,
}

#[derive(Default, Clone)]
pub struct Store(Arc<Mutex<StoreInner>>);

#[derive(Default)]
pub struct StoreInner {
    next: u32,
    docs: HashMap<u32, OpenDoc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocState {
    pub id: u32,
    pub name: String,
    pub path: Option<String>,
    pub revision: u64,
    pub pages: usize,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
    pub creator: String,
    pub producer: String,
    pub created: String,
    pub modified: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocInfo {
    pub state: DocState,
    pub version: String,
    pub was_encrypted: bool,
    pub meta: Meta,
    pub file_size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub state: DocState,
    pub notice: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageText {
    pub revision: u64,
    pub lines: Vec<TextLine>,
    pub rotation: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontSummary {
    pub key: String,
    pub name: String,
    pub raw_name: String,
    pub family: String,
    pub style: String,
    pub kind: String,
    pub encoding: String,
    pub embedded: bool,
    pub subset: bool,
    pub program: Option<ProgramKind>,
    pub glyphs: usize,
    pub charset: String,
    pub has_to_unicode: bool,
    pub extendable: bool,
    pub editable: bool,
    pub bold: bool,
    pub italic: bool,
    pub serif: bool,
    pub fixed: bool,
    pub pages: Vec<usize>,
    pub system_match: Option<String>,
    pub previewable: bool,
}

fn state(id: u32, d: &OpenDoc) -> DocState {
    DocState {
        id,
        name: d.name.clone(),
        path: d.path.as_ref().map(|p| p.to_string_lossy().to_string()),
        revision: d.revision,
        pages: d.doc.get_pages().len(),
        dirty: d.dirty,
        can_undo: !d.undo.is_empty(),
        can_redo: !d.redo.is_empty(),
    }
}

fn read_meta(doc: &Document) -> Meta {
    let info = doc.trailer.get(b"Info").ok().and_then(|o| dict(doc, o));
    let Some(info) = info else { return Meta::default() };
    let t = |k: &[u8]| get(doc, info, k).and_then(|o| lopdf::decode_text_string(o).ok()).unwrap_or_default();
    Meta {
        title: t(b"Title"),
        author: t(b"Author"),
        subject: t(b"Subject"),
        keywords: t(b"Keywords"),
        creator: t(b"Creator"),
        producer: t(b"Producer"),
        created: t(b"CreationDate"),
        modified: t(b"ModDate"),
    }
}

fn info_dict_mut(doc: &mut Document) -> &mut lopdf::Dictionary {
    let id = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) if doc.get_dictionary(*id).is_ok() => *id,
        Ok(Object::Dictionary(d)) => {
            let d = d.clone();
            let id = doc.add_object(d);
            doc.trailer.set("Info", Object::Reference(id));
            id
        }
        _ => {
            let id = doc.add_object(lopdf::Dictionary::new());
            doc.trailer.set("Info", Object::Reference(id));
            id
        }
    };
    doc.get_dictionary_mut(id).expect("info dictionary")
}

fn write_meta(doc: &mut Document, m: &Meta) {
    let info = info_dict_mut(doc);
    for (k, v) in [
        ("Title", &m.title),
        ("Author", &m.author),
        ("Subject", &m.subject),
        ("Keywords", &m.keywords),
        ("Creator", &m.creator),
    ] {
        if v.is_empty() {
            info.remove(k.as_bytes());
        } else {
            info.set(k, lopdf::text_string(v));
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled.pdf".into())
}

impl Store {
    fn with<R>(&self, id: u32, f: impl FnOnce(&mut OpenDoc) -> Result<R, String>) -> Result<R, String> {
        let mut inner = self.0.lock().map_err(|_| "Internal error: store poisoned")?;
        let d = inner.docs.get_mut(&id).ok_or("That document is no longer open.")?;
        f(d)
    }

    /// Runs a change with an undo snapshot; a failed change leaves the document as it was.
    fn mutate<R>(&self, id: u32, f: impl FnOnce(&mut Document) -> Result<R, String>) -> Result<(DocState, R), String> {
        self.with(id, |d| {
            let snapshot = d.doc.clone();
            match f(&mut d.doc) {
                Ok(r) => {
                    d.undo.push(snapshot);
                    if d.undo.len() > UNDO_LIMIT {
                        d.undo.remove(0);
                    }
                    d.redo.clear();
                    d.revision += 1;
                    d.dirty = true;
                    Ok((state(id, d), r))
                }
                Err(e) => {
                    d.doc = snapshot;
                    Err(e)
                }
            }
        })
    }

    fn check_revision(&self, id: u32, revision: u64) -> Result<(), String> {
        self.with(id, |d| {
            if d.revision != revision {
                Err("The document changed since this page was read. Try again.".into())
            } else {
                Ok(())
            }
        })
    }
}

pub(crate) async fn off_thread<R, F>(f: F) -> Result<R, String>
where
    F: FnOnce() -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| format!("Internal error: {e}"))?
}

fn load(path: &Path, password: Option<&str>) -> Result<Document, String> {
    let result = match password {
        Some(p) if !p.is_empty() => Document::load_with_password(path, p),
        _ => Document::load(path),
    };
    result.map_err(|e| {
        let s = e.to_string();
        if s.to_lowercase().contains("password") || s.to_lowercase().contains("decrypt") {
            "PASSWORD_REQUIRED".to_string()
        } else {
            format!("This file could not be opened as a PDF: {s}")
        }
    })
}

fn info_of(id: u32, d: &OpenDoc) -> DocInfo {
    DocInfo {
        state: state(id, d),
        version: d.doc.version.clone(),
        was_encrypted: d.was_encrypted,
        meta: read_meta(&d.doc),
        file_size: d.path.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()).unwrap_or(0),
    }
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn doc_open(store: State<'_, Store>, path: String, password: Option<String>) -> Result<DocInfo, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let p = PathBuf::from(&path);
        let doc = load(&p, password.as_deref())?;
        let was_encrypted = doc.encryption_state.is_some() || doc.trailer.has(b"Encrypt");
        let mut inner = store.0.lock().map_err(|_| "Internal error")?;
        inner.next += 1;
        let id = inner.next;
        let d = OpenDoc {
            name: file_name(&p),
            path: Some(p),
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 1,
            dirty: false,
            was_encrypted,
        };
        let info = info_of(id, &d);
        inner.docs.insert(id, d);
        Ok(info)
    })
    .await
}

#[tauri::command]
pub async fn doc_info(store: State<'_, Store>, id: u32) -> Result<DocInfo, String> {
    let store = store.inner().clone();
    off_thread(move || store.with(id, |d| Ok(info_of(id, d)))).await
}

/// The current document as PDF bytes, for rendering.
#[tauri::command]
pub async fn doc_bytes(store: State<'_, Store>, id: u32) -> Result<Response, String> {
    let store = store.inner().clone();
    let bytes = off_thread(move || {
        store.with(id, |d| {
            let mut buf = Vec::new();
            d.doc.save_to(&mut buf).map_err(err)?;
            Ok(buf)
        })
    })
    .await?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn doc_close(store: State<'_, Store>, id: u32) -> Result<(), String> {
    let mut inner = store.0.lock().map_err(|_| "Internal error")?;
    inner.docs.remove(&id);
    Ok(())
}

#[tauri::command]
pub async fn doc_save(store: State<'_, Store>, id: u32, path: Option<String>) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            let target = path.map(PathBuf::from).or_else(|| d.path.clone()).ok_or("Choose where to save.")?;
            let mut out = d.doc.clone();
            info_dict_mut(&mut out).set("ModDate", lopdf::text_string(&annots::pdf_date()));
            out.prune_objects();
            out.compress();
            // Write next to the target first, so a failed save never destroys the original.
            let tmp = target.with_extension("inq-saving");
            out.save(&tmp).map_err(|e| format!("Saving failed: {e}"))?;
            std::fs::rename(&tmp, &target).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("Saving failed: {e}. Is the file open in another program?")
            })?;
            d.name = file_name(&target);
            d.path = Some(target);
            d.dirty = false;
            Ok(state(id, d))
        })
    })
    .await
}

#[tauri::command]
pub async fn doc_undo(store: State<'_, Store>, id: u32) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            let prev = d.undo.pop().ok_or("Nothing to undo.")?;
            let cur = std::mem::replace(&mut d.doc, prev);
            d.redo.push(cur);
            d.revision += 1;
            d.dirty = true;
            Ok(state(id, d))
        })
    })
    .await
}

#[tauri::command]
pub async fn doc_redo(store: State<'_, Store>, id: u32) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            let next = d.redo.pop().ok_or("Nothing to redo.")?;
            let cur = std::mem::replace(&mut d.doc, next);
            d.undo.push(cur);
            d.revision += 1;
            d.dirty = true;
            Ok(state(id, d))
        })
    })
    .await
}

#[tauri::command]
pub async fn doc_set_meta(store: State<'_, Store>, id: u32, meta: Meta) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store
            .mutate(id, |doc| {
                write_meta(doc, &meta);
                Ok(())
            })
            .map(|(s, _)| s)
    })
    .await
}

// ---------------------------------------------------------------------------
// Text and fonts
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn page_text(store: State<'_, Store>, id: u32, page: usize) -> Result<PageText, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            let pid = pages::page_id(&d.doc, page)?;
            let scan = scan_page(&d.doc, pid)?;
            Ok(PageText { revision: d.revision, lines: scan.lines, rotation: edit::page_rotation(&d.doc, pid) })
        })
    })
    .await
}

fn summarize(doc: &Document, f: &PdfFont, pages: Vec<usize>) -> FontSummary {
    let program = f.program.map(|(k, _)| k);
    let previewable = matches!(program, Some(ProgramKind::TrueType | ProgramKind::OpenType))
        && f.program_bytes(doc)
            .and_then(|(_, b)| {
                ttf_parser::Face::parse(&b, 0).ok().map(|face| {
                    let sample = f.charset();
                    let chars: Vec<char> = sample.chars().filter(|c| c.is_alphanumeric()).take(12).collect();
                    !chars.is_empty() && chars.iter().all(|c| face.glyph_index(*c).is_some())
                })
            })
            .unwrap_or(false);
    let system_match = sysfonts::find(&f.ps_name, f.bold, f.italic).map(|l| l.ps_name);
    FontSummary {
        key: f.key.clone(),
        name: f.ps_name.clone(),
        raw_name: f.raw_name.clone(),
        family: f.family.clone(),
        style: f.style.clone(),
        kind: f.kind_label().to_string(),
        encoding: f.encoding_name.clone(),
        embedded: f.program.is_some(),
        subset: f.subset,
        program,
        glyphs: f.reverse.len(),
        charset: f.charset().chars().take(400).collect(),
        has_to_unicode: f.has_to_unicode,
        extendable: f.extendable,
        editable: f.kind != FontKind::Type3,
        bold: f.bold,
        italic: f.italic,
        serif: f.serif,
        fixed: f.fixed,
        pages,
        system_match,
        previewable,
    }
}

#[tauri::command]
pub async fn doc_fonts(store: State<'_, Store>, id: u32) -> Result<Vec<FontSummary>, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            let mut found: BTreeMap<String, (Arc<PdfFont>, Vec<usize>)> = BTreeMap::new();
            for (i, pid) in pages::page_ids(&d.doc).into_iter().enumerate() {
                let res = page_resources(&d.doc, pid);
                for (_, f) in resource_fonts(&d.doc, &res, &format!("p{}", pid.0)) {
                    let e = found.entry(f.key.clone()).or_insert_with(|| (f.clone(), Vec::new()));
                    e.1.push(i);
                }
            }
            let mut out: Vec<FontSummary> =
                found.into_values().map(|(f, pages)| summarize(&d.doc, &f, pages)).collect();
            out.sort_by(|a, b| a.family.cmp(&b.family).then(a.style.cmp(&b.style)));
            Ok(out)
        })
    })
    .await
}

/// The embedded program of a font, when a browser can use it to preview edits.
#[tauri::command]
pub async fn font_program(store: State<'_, Store>, id: u32, key: String) -> Result<Response, String> {
    let store = store.inner().clone();
    let bytes = off_thread(move || {
        store.with(id, |d| {
            let mut parts = key.split(' ');
            let n: u32 = parts.next().and_then(|s| s.parse().ok()).ok_or("Bad font key")?;
            let g: u16 = parts.next().and_then(|s| s.parse().ok()).ok_or("Bad font key")?;
            let f = PdfFont::load(&d.doc, &Object::Reference((n, g)), key.clone());
            match f.program_bytes(&d.doc) {
                Some((ProgramKind::TrueType | ProgramKind::OpenType, b)) => Ok(b),
                _ => Ok(Vec::new()),
            }
        })
    })
    .await?;
    Ok(Response::new(bytes))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontCatalog {
    pub families: Vec<String>,
    pub standard: Vec<&'static str>,
}

#[tauri::command]
pub async fn system_fonts() -> Result<FontCatalog, String> {
    off_thread(|| Ok(FontCatalog { families: sysfonts::families(), standard: sysfonts::STANDARD_FONTS.to_vec() })).await
}

#[tauri::command]
pub async fn text_edit(
    store: State<'_, Store>,
    id: u32,
    page: usize,
    revision: u64,
    line: usize,
    edit: LineEdit,
) -> Result<EditResult, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.check_revision(id, revision)?;
        let (state, notice) = store.mutate(id, |doc| {
            let pid = pages::page_id(doc, page)?;
            edit::edit_line(doc, pid, line, &edit)
        })?;
        Ok(EditResult { state, notice })
    })
    .await
}

#[tauri::command]
pub async fn text_add(store: State<'_, Store>, id: u32, page: usize, spec: NewText) -> Result<EditResult, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let (state, notice) = store.mutate(id, |doc| {
            let pid = pages::page_id(doc, page)?;
            edit::add_text(doc, pid, &spec)
        })?;
        Ok(EditResult { state, notice })
    })
    .await
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn pages_rotate(store: State<'_, Store>, id: u32, pages: Vec<usize>, delta: i64) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || store.mutate(id, |doc| pages::rotate(doc, &pages, delta)).map(|(s, _)| s)).await
}

#[tauri::command]
pub async fn pages_delete(store: State<'_, Store>, id: u32, pages: Vec<usize>) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || store.mutate(id, |doc| pages::delete(doc, &pages)).map(|(s, _)| s)).await
}

#[tauri::command]
pub async fn pages_reorder(store: State<'_, Store>, id: u32, order: Vec<usize>) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || store.mutate(id, |doc| pages::reorder(doc, &order)).map(|(s, _)| s)).await
}

#[tauri::command]
pub async fn page_insert_blank(store: State<'_, Store>, id: u32, at: usize) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || store.mutate(id, |doc| pages::insert_blank(doc, at)).map(|(s, _)| s)).await
}

#[tauri::command]
pub async fn page_duplicate(store: State<'_, Store>, id: u32, index: usize) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || store.mutate(id, |doc| pages::duplicate(doc, index)).map(|(s, _)| s)).await
}

#[tauri::command]
pub async fn pages_insert_file(
    store: State<'_, Store>,
    id: u32,
    path: String,
    at: usize,
) -> Result<EditResult, String> {
    let store = store.inner().clone();
    off_thread(move || {
        let other = load(Path::new(&path), None).map_err(|e| {
            if e == "PASSWORD_REQUIRED" { "That PDF is password protected.".to_string() } else { e }
        })?;
        let (state, count) = store.mutate(id, |doc| pages::insert_document(doc, other, at))?;
        Ok(EditResult {
            state,
            notice: Some(format!("Inserted {count} page{}.", if count == 1 { "" } else { "s" })),
        })
    })
    .await
}

#[tauri::command]
pub async fn pages_extract(store: State<'_, Store>, id: u32, pages: Vec<usize>, path: String) -> Result<(), String> {
    let store = store.inner().clone();
    off_thread(move || {
        let mut out = store.with(id, |d| pages::extract(&d.doc, &pages))?;
        out.compress();
        out.save(&path).map_err(|e| format!("Saving failed: {e}"))?;
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------
// Annotations
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn annots_list(store: State<'_, Store>, id: u32, page: usize) -> Result<Vec<AnnotInfo>, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store.with(id, |d| {
            let pid = pages::page_id(&d.doc, page)?;
            Ok(annots::list(&d.doc, pid))
        })
    })
    .await
}

#[tauri::command]
pub async fn annot_add(store: State<'_, Store>, id: u32, page: usize, annot: NewAnnot) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store
            .mutate(id, |doc| {
                let pid = pages::page_id(doc, page)?;
                annots::add(doc, pid, &annot)
            })
            .map(|(s, _)| s)
    })
    .await
}

#[tauri::command]
pub async fn annot_delete(store: State<'_, Store>, id: u32, page: usize, index: usize) -> Result<DocState, String> {
    let store = store.inner().clone();
    off_thread(move || {
        store
            .mutate(id, |doc| {
                let pid = pages::page_id(doc, page)?;
                annots::delete(doc, pid, index)
            })
            .map(|(s, _)| s)
    })
    .await
}

/// Size of a file on disk, for the recent files list.
#[tauri::command]
pub fn file_exists(path: String) -> bool {
    Path::new(&path).is_file()
}

/// PDFs passed on the command line ("Open with", double-click on a file).
#[tauri::command]
pub fn startup_files() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|a| a.to_lowercase().ends_with(".pdf") && Path::new(a).is_file())
        .collect()
}
