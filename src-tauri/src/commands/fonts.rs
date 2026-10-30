//! Font commands: the fonts installed on Windows, and font files the user
//! adds. Added files are copied to `<app data>/fonts/<id>.<ext>` and listed
//! in the database, so they stay usable after the original file moves.

use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::fonts::{self, FileFont, SystemFont};
use crate::settings::Settings;
use crate::{clock, commit_settings, lock, AppState};

/// The font families installed on this computer (none on a phone).
/// Read afresh each time, so a font installed meanwhile shows up.
#[tauri::command]
pub async fn system_fonts() -> Vec<SystemFont> {
    if cfg!(mobile) {
        return Vec::new();
    }
    tauri::async_runtime::spawn_blocking(|| fonts::scan(&fonts::system_dirs())).await.unwrap_or_default()
}

/// The font files added so far, oldest first.
#[tauri::command]
pub fn font_files(state: State<'_, AppState>) -> Vec<FileFont> {
    stored(&state)
}

fn stored(state: &AppState) -> Vec<FileFont> {
    lock(&state.db).get::<Vec<FileFont>>(fonts::FILES_KEY).ok().flatten().unwrap_or_default()
}

/// Ask for a TTF or OTF file and add it. Returns None if cancelled; adding
/// a file that is already there returns the existing entry.
#[tauri::command]
pub async fn add_font_file(app: AppHandle) -> Result<Option<FileFont>, String> {
    let mut dialog = app.dialog().file();
    // Phones filter by file type, which a font copied from a computer does
    // not always carry: list every file and let the check below decide.
    #[cfg(desktop)]
    {
        dialog = dialog.add_filter("TrueType / OpenType", &["ttf", "otf"]);
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
    }
    let Some(file) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let data = read_bytes(&app, file)?;
    let font = fonts::inspect(&data, clock::wall_ms()).map_err(String::from)?;

    let state = app.state::<AppState>();
    let mut list = stored(&state);
    if let Some(existing) = list.iter().find(|f| f.id == font.id) {
        // Restore the copy in case it went missing.
        let path = dir(&app)?.join(existing.file_name());
        if !path.exists() {
            std::fs::write(path, &data).map_err(|_| "write_failed".to_string())?;
        }
        return Ok(Some(existing.clone()));
    }
    let folder = dir(&app)?;
    std::fs::create_dir_all(&folder).map_err(|_| "write_failed".to_string())?;
    std::fs::write(folder.join(font.file_name()), &data).map_err(|_| "write_failed".to_string())?;
    list.push(font.clone());
    lock(&state.db).set(fonts::FILES_KEY, &list)?;
    Ok(Some(font))
}

/// The bytes of an added font, for the FontFace API (an ArrayBuffer in JS).
#[tauri::command]
pub fn font_file_data(app: AppHandle, id: String) -> Result<Response, String> {
    let font = stored(&app.state::<AppState>()).into_iter().find(|f| f.id == id).ok_or("missing")?;
    let data = std::fs::read(dir(&app)?.join(font.file_name())).map_err(|_| "missing".to_string())?;
    Ok(Response::new(data))
}

/// Remove an added font. Settings that used it go back to their defaults.
#[tauri::command]
pub fn remove_font_file(app: AppHandle, id: String) -> Result<Settings, String> {
    if !fonts::is_file_id(&id) {
        return Err("missing".into());
    }
    let state = app.state::<AppState>();
    let mut list = stored(&state);
    if let Some(i) = list.iter().position(|f| f.id == id) {
        let font = list.remove(i);
        lock(&state.db).set(fonts::FILES_KEY, &list)?;
        let _ = std::fs::remove_file(dir(&app)?.join(font.file_name()));
    }
    let mut settings = lock(&state.settings).clone();
    let used = format!("file:{id}");
    let defaults = Settings::default();
    if settings.font_ui == used {
        settings.font_ui = defaults.font_ui;
    }
    if settings.font_timer == used {
        settings.font_timer = defaults.font_timer;
    }
    Ok(commit_settings(&app, settings))
}

fn dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("fonts"))
}

/// Read a file chosen in an open dialog (a path, or a document link on a phone).
fn read_bytes(app: &AppHandle, file: FilePath) -> Result<Vec<u8>, String> {
    #[cfg(mobile)]
    if let FilePath::Url(_) = file {
        use tauri_plugin_fs::FsExt;
        return app.fs().read(file).map_err(|_| "unreadable".to_string());
    }
    let _ = app;
    let path = file.into_path().map_err(|_| "unreadable".to_string())?;
    // Refuse a huge file before reading it all.
    let size = std::fs::metadata(&path).map_err(|_| "unreadable".to_string())?.len();
    if size > fonts::MAX_FILE_BYTES as u64 {
        return Err("too_big".into());
    }
    std::fs::read(path).map_err(|_| "unreadable".to_string())
}
