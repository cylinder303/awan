//! History, statistics, CSV export and backup commands.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::backup::{Backup, ImportSummary};
use crate::db::Session;
use crate::{clock, lock, save_settings, AppState, HISTORY_EVENT};

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>, before: Option<i64>, limit: u32) -> Result<Vec<Session>, String> {
    lock(&state.db).list_sessions(before, limit.min(1000))
}

#[tauri::command]
pub fn delete_session(app: AppHandle, id: i64) -> Result<bool, String> {
    let deleted = lock(&app.state::<AppState>().db).delete_session(id)?;
    let _ = app.emit(HISTORY_EVENT, ());
    Ok(deleted)
}

/// Focus totals (ms) for several [from, to) ranges of UTC milliseconds.
/// The UI computes the ranges because it knows the local calendar.
#[tauri::command]
pub fn focus_totals(state: State<'_, AppState>, ranges: Vec<(i64, i64)>) -> Result<Vec<u64>, String> {
    lock(&state.db).focus_totals(&ranges)
}

#[tauri::command]
pub fn recent_tasks(state: State<'_, AppState>, limit: u32) -> Result<Vec<String>, String> {
    lock(&state.db).recent_tasks(limit.min(50))
}

/// Ask where to save, then write the CSV the UI prepared (it formats dates
/// in the user's time zone and language). Returns false if cancelled.
#[tauri::command]
pub async fn export_csv(app: AppHandle, contents: String, file_name: String) -> Result<bool, String> {
    let Some(path) = save_dialog(&app, &file_name, "CSV", "csv") else {
        return Ok(false);
    };
    // A byte-order mark lets Excel open Arabic text correctly.
    std::fs::write(path, format!("\u{feff}{contents}")).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn export_backup(app: AppHandle, file_name: String) -> Result<bool, String> {
    let backup = {
        let state = app.state::<AppState>();
        let settings = lock(&state.settings).clone();
        let sessions = lock(&state.db).all_sessions()?;
        Backup::new(settings, sessions, clock::wall_ms())
    };
    let Some(path) = save_dialog(&app, &file_name, "Awan backup", "json") else {
        return Ok(false);
    };
    std::fs::write(path, backup.to_json()?).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Restore from a backup: settings are replaced, sessions are merged
/// (sessions already present are skipped). Returns None if cancelled.
#[tauri::command]
pub async fn import_backup(app: AppHandle) -> Result<Option<ImportSummary>, String> {
    let mut dialog = app.dialog().file().add_filter("Awan backup", &["json"]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_pick_file().and_then(to_path) else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path).map_err(|_| "unreadable".to_string())?;
    let backup = Backup::from_json(&text)?;

    let added = lock(&app.state::<AppState>().db).merge_sessions(&backup.sessions)?;
    save_settings(&app, backup.settings);
    let _ = app.emit(HISTORY_EVENT, ());
    Ok(Some(ImportSummary { added, skipped: backup.sessions.len() - added }))
}

fn save_dialog(app: &AppHandle, file_name: &str, filter: &str, ext: &str) -> Option<PathBuf> {
    let mut dialog = app.dialog().file().set_file_name(file_name).add_filter(filter, &[ext]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let mut path = dialog.blocking_save_file().and_then(to_path)?;
    if path.extension().is_none() {
        path.set_extension(ext);
    }
    Some(path)
}

fn to_path(file: FilePath) -> Option<PathBuf> {
    file.into_path().ok()
}
