use tauri::Manager;

mod annots;
mod cmap;
mod edit;
mod encoding;
mod fonts;
mod interp;
mod pages;
mod store;
mod sysfonts;
mod util;
#[cfg(test)]
mod tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(store::Store::default())
        .setup(|app| {
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
            store::doc_set_meta,
            store::page_text,
            store::doc_fonts,
            store::font_program,
            store::system_fonts,
            store::text_edit,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
