//! The log file and crash reports.
//!
//! Everything stays on this computer. The log (`inq-studio.log` in the app's
//! log folder, one earlier file kept) records what the app did and what went
//! wrong: versions, failed commands, errors from the window. It never holds
//! document content or passwords. When the app panics, a crash report with
//! the message, the place and a stack trace is written next to it, and the
//! next start offers to show it.

use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_LOG: u64 = 1024 * 1024;
const KEEP_CRASHES: usize = 10;

static DIR: OnceLock<PathBuf> = OnceLock::new();
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// "2026-11-11 08:15:00" in UTC, from the system clock.
fn stamp(sep: [&str; 2]) -> String {
    let d = crate::annots::pdf_date();
    // D:YYYYMMDDHHMMSSZ
    let p = |a: usize, b: usize| d.get(a..b).unwrap_or("00").to_string();
    format!("{}-{}-{}{}{}{}{}{}{}", p(2, 6), p(6, 8), p(8, 10), sep[0], p(10, 12), sep[1], p(12, 14), sep[1], p(14, 16))
}

fn log_path(dir: &Path) -> PathBuf {
    dir.join("inq-studio.log")
}

/// Opens the log in `dir`, moving a full one aside, and installs the crash handler.
pub fn init(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    let path = log_path(&dir);
    if std::fs::metadata(&path).map(|m| m.len() > MAX_LOG).unwrap_or(false) {
        let _ = std::fs::rename(&path, dir.join("inq-studio.previous.log"));
    }
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(mut slot) = FILE.lock() {
            *slot = Some(f);
        }
    }
    let _ = DIR.set(dir);
    write("info", &format!("Inq Studio {} started on {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH));

    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        crash(&message, &place, &std::backtrace::Backtrace::force_capture().to_string());
        default(info);
    }));
}

/// Appends a line to the log. Line breaks in the message are kept on one line.
pub fn write(level: &str, message: &str) {
    let line = format!("{} {:<5} {}\n", stamp([" ", ":"]), level, message.replace(['\r', '\n'], " ⏎ "));
    if let Ok(mut slot) = FILE.lock() {
        if let Some(f) = slot.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

fn crash(message: &str, place: &str, trace: &str) {
    write("crash", &format!("{message} at {place}"));
    let Some(dir) = DIR.get() else { return };
    let report = format!(
        "Inq Studio crash report\n\nVersion: {}\nSystem: {} {}\nTime (UTC): {}\n\nWhat happened: {message}\nWhere: {place}\n\nStack trace:\n{trace}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        stamp([" ", ":"]),
    );
    let _ = std::fs::write(dir.join(format!("crash-{}.txt", stamp(["-", ""]).replace('-', ""))), report);
}

fn crash_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.retain(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("crash-") && n.ends_with(".txt")).unwrap_or(false));
    files.sort();
    files
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogInfo {
    pub folder: String,
    pub file: String,
    /// Crash reports not seen yet, newest last.
    pub new_crashes: Vec<String>,
}

/// Where the log is, and which crash reports are new since the user last looked.
#[tauri::command]
pub fn log_info() -> Result<LogInfo, String> {
    let dir = DIR.get().ok_or("The log is not available.")?;
    let files = crash_files(dir);
    // Old reports go; the list stays short.
    for old in files.iter().rev().skip(KEEP_CRASHES) {
        let _ = std::fs::remove_file(old);
    }
    let seen = std::fs::read_to_string(dir.join("crashes-seen.txt")).unwrap_or_default();
    let new_crashes = files
        .iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n > seen.trim()).unwrap_or(false))
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    Ok(LogInfo { folder: dir.to_string_lossy().to_string(), file: log_path(dir).to_string_lossy().to_string(), new_crashes })
}

/// Remembers that the crash reports so far have been shown.
#[tauri::command]
pub fn crashes_seen() {
    let Some(dir) = DIR.get() else { return };
    if let Some(last) = crash_files(dir).last().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
        let _ = std::fs::write(dir.join("crashes-seen.txt"), last);
    }
}

/// A line from the window: a failed command or an error in the interface.
#[tauri::command]
pub fn log_write(level: String, message: String) {
    let level = match level.as_str() {
        "error" | "warn" | "info" => level,
        _ => "info".to_string(),
    };
    let short: String = message.chars().take(2000).collect();
    write(&level, &short);
}

#[cfg(test)]
mod tests {
    #[test]
    fn stamps_are_sortable() {
        let s = super::stamp([" ", ":"]);
        assert_eq!(s.len(), 19, "{s}");
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[13..14], ":");
        let f = super::stamp(["-", ""]).replace('-', "");
        assert_eq!(f.len(), 14, "{f}");
        assert!(f.chars().all(|c| c.is_ascii_digit()));
    }
}
