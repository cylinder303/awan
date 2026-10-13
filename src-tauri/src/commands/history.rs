//! History, statistics, CSV export and backup commands.

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::backup::{Backup, ImportSummary};
use crate::db::{Session, WeekGoal};
use crate::{clock, lock, save_settings, AppState, HISTORY_EVENT};

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>, before: Option<i64>, limit: u32) -> Result<Vec<Session>, String> {
    lock(&state.db).list_sessions(before, limit.min(1000))
}

#[tauri::command]
pub fn delete_session(app: AppHandle, id: i64) -> Result<bool, String> {
    let deleted = lock(&app.state::<AppState>().db).delete_session(id, clock::wall_ms())?;
    crate::sync::app::local_change();
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
pub fn week_goals(state: State<'_, AppState>) -> Result<Vec<WeekGoal>, String> {
    lock(&state.db).week_goals()
}

#[tauri::command]
pub fn first_session_at(state: State<'_, AppState>) -> Result<Option<i64>, String> {
    lock(&state.db).first_session_at()
}

#[tauri::command]
pub fn recent_tasks(state: State<'_, AppState>, limit: u32) -> Result<Vec<String>, String> {
    lock(&state.db).recent_tasks(limit.min(50))
}

/// Ask where to save, then write the CSV the UI prepared (it formats dates
/// in the user's time zone and language). Returns false if cancelled.
#[tauri::command]
pub async fn export_csv(app: AppHandle, contents: String, file_name: String) -> Result<bool, String> {
    let Some(file) = save_dialog(&app, &file_name, "CSV", "csv") else {
        return Ok(false);
    };
    // A byte-order mark lets Excel open Arabic text correctly.
    write_file(&app, file, "csv", format!("\u{feff}{contents}").as_bytes())?;
    Ok(true)
}

#[tauri::command]
pub async fn export_backup(app: AppHandle, file_name: String) -> Result<bool, String> {
    let backup = {
        let state = app.state::<AppState>();
        let settings = lock(&state.settings).clone();
        let db = lock(&state.db);
        let mut backup = Backup::new(settings, db.all_sessions()?, clock::wall_ms());
        backup.week_goals = db.week_goals()?;
        backup
    };
    let Some(file) = save_dialog(&app, &file_name, "Awan backup", "json") else {
        return Ok(false);
    };
    write_file(&app, file, "json", backup.to_json()?.as_bytes())?;
    Ok(true)
}

/// Restore from a backup: settings are replaced, sessions are merged
/// (sessions already present are skipped). Returns None if cancelled.
#[tauri::command]
pub async fn import_backup(app: AppHandle) -> Result<Option<ImportSummary>, String> {
    let mut dialog = app.dialog().file();
    // Phones filter by file type, which a backup copied over from a computer
    // (a download, a chat attachment) does not always carry: list every file
    // there and let the backup check below reject the wrong ones.
    #[cfg(desktop)]
    {
        dialog = dialog.add_filter("Awan backup", &["json"]);
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
    }
    let Some(file) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let text = read_file(&app, file)?;
    let backup = Backup::from_json(&text)?;

    let state = app.state::<AppState>();
    let added = {
        let mut db = lock(&state.db);
        let added = db.merge_sessions(&backup.sessions)?;
        db.merge_week_goals(&backup.week_goals)?;
        added
    };
    save_settings(&app, backup.settings);
    crate::sync::app::local_change();
    let _ = app.emit(HISTORY_EVENT, ());
    Ok(Some(ImportSummary { added, skipped: backup.sessions.len() - added }))
}

fn save_dialog(app: &AppHandle, file_name: &str, filter: &str, ext: &str) -> Option<FilePath> {
    let mut dialog = app.dialog().file().set_file_name(file_name).add_filter(filter, &[ext]);
    #[cfg(desktop)]
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    dialog.blocking_save_file()
}

/// Write a file chosen in a save dialog. On a phone the dialog hands back a
/// document link (content://) rather than a path, opened through the system.
fn write_file(app: &AppHandle, file: FilePath, ext: &str, contents: &[u8]) -> Result<(), String> {
    #[cfg(mobile)]
    if let FilePath::Url(_) = file {
        use std::io::Write;
        use tauri_plugin_fs::{FsExt, OpenOptions};
        let mut options = OpenOptions::new();
        options.read(false).write(true).truncate(true).create(true);
        let mut out = app.fs().open(file, options).map_err(|e| e.to_string())?;
        return out.write_all(contents).map_err(|e| e.to_string());
    }
    let _ = app;
    let mut path = file.into_path().map_err(|e| e.to_string())?;
    if path.extension().is_none() {
        path.set_extension(ext);
    }
    std::fs::write(path, contents).map_err(|e| e.to_string())
}

/// Read a text file chosen in an open dialog (a path, or a document link on a phone).
fn read_file(app: &AppHandle, file: FilePath) -> Result<String, String> {
    #[cfg(mobile)]
    if let FilePath::Url(_) = file {
        use tauri_plugin_fs::FsExt;
        return app.fs().read_to_string(file).map_err(|_| "unreadable".to_string());
    }
    let _ = app;
    let path = file.into_path().map_err(|_| "unreadable".to_string())?;
    std::fs::read_to_string(path).map_err(|_| "unreadable".to_string())
}
