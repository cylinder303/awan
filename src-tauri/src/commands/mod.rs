//! Commands invoked from the web UI.

pub mod history;

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::i18n::Lang;
use crate::settings::Settings;
use crate::timer::{Mode, TimerView};
use crate::display;
use crate::{apply, clock, commit_settings, lock, save_settings, shortcuts, sound, tray, window_ext, AppState};
use tauri_plugin_autostart::ManagerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialState {
    view: TimerView,
    /// True when an interrupted session is waiting for "resume or start fresh".
    pending_resume: bool,
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> InitialState {
    let view = lock(&state.engine).view(clock::now());
    InitialState { view, pending_resume: state.pending_resume.load(Ordering::SeqCst) }
}

#[tauri::command]
pub fn timer_toggle(app: AppHandle) -> TimerView {
    apply(&app, |engine, now| {
        engine.toggle(now);
        Vec::new()
    })
}

#[tauri::command]
pub fn timer_reset(app: AppHandle) -> TimerView {
    apply(&app, |engine, now| engine.reset(now))
}

#[tauri::command]
pub fn timer_skip(app: AppHandle) -> TimerView {
    apply(&app, |engine, now| engine.skip(now))
}

#[tauri::command]
pub fn timer_set_mode(app: AppHandle, mode: Mode) -> TimerView {
    apply(&app, |engine, now| engine.set_mode(mode, now))
}

#[tauri::command]
pub fn timer_set_task(app: AppHandle, task: String) -> TimerView {
    apply(&app, |engine, _| {
        engine.set_task(&task);
        Vec::new()
    })
}

/// Answer the "resume or start fresh" prompt shown after an interrupted session.
#[tauri::command]
pub fn resolve_resume(app: AppHandle, resume: bool) -> TimerView {
    app.state::<AppState>().pending_resume.store(false, Ordering::SeqCst);
    apply(&app, |engine, now| {
        if resume {
            engine.start(now);
            Vec::new()
        } else {
            engine.start_fresh(now)
        }
    })
}

#[tauri::command]
pub fn set_language(app: AppHandle, lang: String) {
    *lock(&app.state::<AppState>().lang) = Lang::parse(&lang);
    tray::refresh(&app);
}

/// A window has painted its first frame. Show it if it belongs to the
/// current view (and the app was not started quietly at login).
#[tauri::command]
pub fn app_ready(app: AppHandle, window: WebviewWindow) {
    let state = app.state::<AppState>();
    if !state.start_hidden && window.label() == display::current(&app).label() {
        window_ext::reveal(&window);
    }
}

/// Switch between the full window and the mini timer.
#[tauri::command]
pub fn toggle_view(app: AppHandle) {
    display::toggle(&app);
}

/// Change the system-wide start/pause shortcut ("" turns it off).
/// On failure the previous shortcut stays active.
#[tauri::command]
pub fn set_global_shortcut(app: AppHandle, shortcut: String) -> Result<Settings, String> {
    let current = lock(&app.state::<AppState>().settings).clone();
    if let Err(e) = shortcuts::register(&app, &shortcut) {
        let _ = shortcuts::register(&app, &current.global_shortcut);
        return Err(e);
    }
    Ok(commit_settings(&app, Settings { global_shortcut: shortcut, ..current }))
}

/// Add or remove the Windows startup entry.
#[tauri::command]
pub fn set_launch_at_startup(app: AppHandle, enabled: bool) -> Result<Settings, String> {
    let manager = app.autolaunch();
    if enabled { manager.enable() } else { manager.disable() }.map_err(|e| e.to_string())?;
    let current = lock(&app.state::<AppState>().settings).clone();
    Ok(commit_settings(&app, Settings { launch_at_startup: enabled, ..current }))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    lock(&state.settings).clone()
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: Settings) -> Settings {
    save_settings(&app, settings)
}

#[tauri::command]
pub fn reset_settings(app: AppHandle) -> Settings {
    crate::reset_settings(&app)
}

#[tauri::command]
pub fn list_sounds() -> Vec<&'static str> {
    sound::ids()
}

/// Play a sound from the settings page at the volume being chosen.
#[tauri::command]
pub fn preview_sound(id: String, volume: u32) {
    sound::play(&id, volume.min(100) as f32 / 100.0);
}
