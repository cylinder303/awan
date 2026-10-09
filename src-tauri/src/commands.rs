//! Commands invoked from the web UI.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::i18n::Lang;
use crate::timer::{Mode, TimerView};
use crate::{apply, clock, lock, window_ext, AppState};

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
pub fn set_language(state: State<'_, AppState>, lang: String) {
    *lock(&state.lang) = Lang::parse(&lang);
}

/// The UI has painted its first frame: show the (initially hidden) window.
#[tauri::command]
pub fn app_ready(window: WebviewWindow) {
    window_ext::reveal(&window);
}
