use tauri::{Emitter, Manager};

mod annots;
mod compare;
mod history;
mod ocr;
mod optimize;
mod pdfa;
mod pro;
mod security;
mod sign;
mod tags;
mod cmap;
mod edit;
mod encoding;
mod fonts;
mod forms;
mod images;
mod marks;
mod outline;
mod redact;
mod interp;
mod log;
mod pages;
mod store;
mod sysfonts;
mod util;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_v2;
#[cfg(test)]
mod tests_v3;
#[cfg(test)]
mod tests_v4;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First, so a second start hands its files over before anything else happens.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            let files: Vec<String> = args
                .iter()
                .skip(1)
                .filter(|a| a.to_lowercase().ends_with(".pdf"))
                .map(|a| {
                    let p = std::path::Path::new(a);
                    if p.is_absolute() { p.to_path_buf() } else { std::path::Path::new(&cwd).join(p) }
                })
                .filter(|p| p.is_file())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            if !files.is_empty() {
                let _ = app.emit("open-files", files);
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(store::Store::default())
        .setup(|app| {
            if let Ok(dir) = app.path().app_log_dir() {
                log::init(dir);
            }
            // The window starts hidden and the page shows it once its theme is
            // applied. Show it anyway if the page never gets that far.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(4));
                if let Some(window) = handle.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                }
            });
            // Index installed fonts in the background so the first edit is quick.
            std::thread::spawn(|| {
                let _ = sysfonts::installed();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            store::doc_open,
            store::doc_info,
            store::doc_bytes,
            store::doc_close,
            store::doc_save,
            store::doc_undo,
            store::doc_redo,
            store::doc_history_go,
            store::doc_set_meta,
            store::page_text,
            store::doc_fonts,
            store::font_program,
            store::system_fonts,
            store::text_edit,
            store::block_edit,
            store::text_add,
            store::pages_rotate,
            store::pages_delete,
            store::pages_reorder,
            store::page_insert_blank,
            store::page_duplicate,
            store::pages_insert_file,
            store::pages_extract,
            store::annots_list,
            store::annot_add,
            store::annot_delete,
            store::file_exists,
            store::startup_files,
            store::annot_update,
            store::form_fields,
            store::form_set,
            store::doc_flatten,
            store::images_list,
            store::image_insert,
            store::image_place,
            store::image_delete,
            store::image_replace,
            store::image_extract,
            store::text_find,
            store::redact,
            store::page_marks,
            store::labels_get,
            store::labels_set,
            store::outline_get,
            store::outline_set,
            store::pages_crop,
            store::pages_resize,
            store::doc_split,
            store::doc_render_info,
            store::doc_range,
            pro::doc_protect,
            pro::doc_optimize,
            pro::export_text,
            pro::file_write,
            pro::doc_compare,
            pro::tags_get,
            pro::tags_set_element,
            pro::tags_set_document,
            pro::reading_order,
            pro::pdfa_check,
            pro::pdfa_convert,
            pro::ocr_setup,
            pro::ocr_recognize,
            pro::ocr_apply,
            pro::sig_list,
            pro::sig_identities,
            pro::sig_file_identity,
            pro::sig_create_identity,
            pro::doc_sign,
            log::log_info,
            log::log_write,
            log::crashes_seen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
