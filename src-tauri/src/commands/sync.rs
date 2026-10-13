//! Linking devices and syncing them (see src/sync).

use std::time::Duration;

use tauri::AppHandle;

use crate::sync::app::{self, Found, Info, Pairing};

#[tauri::command]
pub fn sync_info(app: AppHandle) -> Info {
    app::info(&app)
}

/// Computer: show a pairing code for a phone to type in.
#[tauri::command]
pub fn sync_pairing_start(app: AppHandle) -> Result<Pairing, String> {
    if cfg!(mobile) {
        return Err("unsupported".into());
    }
    app::begin_pairing(&app)
}

#[tauri::command]
pub fn sync_pairing_stop() {
    app::end_pairing();
}

/// Computer: forget a linked phone.
#[tauri::command]
pub fn sync_remove_device(app: AppHandle, id: String) -> Info {
    app::remove_device(&app, &id);
    app::info(&app)
}

/// Phone: computers on this network showing a pairing code.
#[tauri::command]
pub async fn sync_find() -> Result<Vec<Found>, String> {
    tauri::async_runtime::spawn_blocking(app::find_computers).await.map_err(|e| e.to_string())?
}

/// Phone: link with a computer using the code it shows.
#[tauri::command]
pub async fn sync_pair(app: AppHandle, addr: String, code: String, name: String) -> Result<Info, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || app::pair(&handle, &addr, &code, &name))
        .await
        .map_err(|e| e.to_string())??;
    Ok(app::info(&app))
}

/// Phone: sync now (the app came to the front, or the user asked).
#[tauri::command]
pub fn sync_now() {
    if cfg!(mobile) {
        app::kick(Duration::ZERO);
    }
}

/// Phone: forget the linked computer.
#[tauri::command]
pub fn sync_unlink(app: AppHandle) -> Info {
    app::unlink(&app);
    app::info(&app)
}
