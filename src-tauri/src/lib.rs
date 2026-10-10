//! Awan: a minimal Pomodoro timer.
//!
//! The timer runs here in the native process (not in the web UI) so it keeps
//! exact time while the window is minimized, hidden, or throttled. The UI
//! only renders `TimerView` snapshots pushed through the `timer://state` event.

mod backup;
mod clock;
mod commands;
mod db;
mod goals;
mod i18n;
mod power;
mod settings;
mod sound;
mod timer;
mod window_ext;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use db::Db;
use i18n::Lang;
use settings::{Settings, SETTINGS_KEY};
use timer::{Engine, Event, Mode, Now, SessionRecord, Snapshot, Status, TimerView};

const SNAPSHOT_KEY: &str = "active_session";
pub const STATE_EVENT: &str = "timer://state";
pub const FINISHED_EVENT: &str = "timer://finished";
pub const SETTINGS_EVENT: &str = "settings://changed";
pub const HISTORY_EVENT: &str = "history://changed";

pub struct AppState {
    engine: Mutex<Engine>,
    db: Mutex<Db>,
    settings: Mutex<Settings>,
    lang: Mutex<Lang>,
    /// An interrupted session was restored and the user has not yet chosen
    /// between resuming it and starting fresh.
    pending_resume: AtomicBool,
}

/// Lock a mutex, recovering the data if a previous holder panicked.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn run() {
    tauri::Builder::default()
        // Must be registered first: a second launch focuses the running app.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                window_ext::reveal(&w);
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::timer_toggle,
            commands::timer_reset,
            commands::timer_skip,
            commands::timer_set_mode,
            commands::timer_set_task,
            commands::resolve_resume,
            commands::set_language,
            commands::app_ready,
            commands::get_settings,
            commands::update_settings,
            commands::reset_settings,
            commands::list_sounds,
            commands::preview_sound,
            commands::history::list_sessions,
            commands::history::delete_session,
            commands::history::focus_totals,
            commands::history::recent_tasks,
            commands::history::export_csv,
            commands::history::export_backup,
            commands::history::import_backup,
            commands::history::week_goals,
            commands::history::first_session_at,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Awan")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                save_snapshot(app);
            }
        });
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db = Db::open(&dir.join("awan.db"))?;
    let settings = db.get::<Settings>(SETTINGS_KEY)?.unwrap_or_default().sanitized();

    let mut engine = Engine::new(settings.timer_config());
    let mut pending = false;
    if let Some(snap) = db.get::<Snapshot>(SNAPSHOT_KEY)? {
        if snap.elapsed_ms > 0 {
            engine.restore(&snap);
            pending = true;
        } else {
            engine.set_task(&snap.task);
        }
    }

    remember_week_goal(&db, &settings);
    app.manage(AppState {
        engine: Mutex::new(engine),
        db: Mutex::new(db),
        settings: Mutex::new(settings.clone()),
        lang: Mutex::new(Lang::En),
        pending_resume: AtomicBool::new(pending),
    });

    if let Some(window) = app.get_webview_window("main") {
        window_ext::apply_corner_preference(&window);
        let _ = window.set_always_on_top(settings.always_on_top);
        // The UI shows the window once it has painted (no white flash). If it
        // never reports in, show it anyway.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            if !window.is_visible().unwrap_or(true) {
                window_ext::reveal(&window);
            }
        });
    }

    power::watch(app.handle().clone());
    spawn_ticker(app.handle().clone());
    Ok(())
}

/// Run an engine operation, then perform its side effects and publish the new state.
pub(crate) fn apply<F>(app: &AppHandle, op: F) -> TimerView
where
    F: FnOnce(&mut Engine, Now) -> Vec<Event>,
{
    let state = app.state::<AppState>();
    let now = clock::now();
    let (events, view) = {
        let mut engine = lock(&state.engine);
        let events = op(&mut engine, now);
        (events, engine.view(now))
    };
    handle_events(app, events);
    let _ = app.emit(STATE_EVENT, &view);
    save_snapshot(app);
    view
}

fn handle_events(app: &AppHandle, events: Vec<Event>) {
    // Goal notifications go out after the "session finished" one.
    let mut focus_ended = Vec::new();
    for event in events {
        match event {
            Event::SessionEnded(record) => {
                let _ = lock(&app.state::<AppState>().db).insert_session(&record);
                if record.mode == Mode::Focus {
                    focus_ended.push(record);
                }
                let _ = app.emit(HISTORY_EVENT, ());
            }
            Event::Finished { ended, next, auto_started } => {
                let state = app.state::<AppState>();
                let lang = *lock(&state.lang);
                let (title, body) = i18n::finished(lang, ended, next);
                let _ = app.notification().builder().title(title).body(body).show();
                let (id, volume) = {
                    let s = lock(&state.settings);
                    (s.sound.clone(), s.alert_volume())
                };
                sound::play(&id, volume);
                let _ = app.emit(
                    FINISHED_EVENT,
                    serde_json::json!({ "ended": ended, "next": next, "autoStarted": auto_started }),
                );
            }
            Event::SleepPaused => {}
        }
    }
    for record in focus_ended {
        check_goals(app, &record);
    }
}

/// Validate, store and apply new settings, then tell every window.
pub(crate) fn save_settings(app: &AppHandle, next: Settings) -> Settings {
    let next = next.sanitized();
    let state = app.state::<AppState>();
    {
        let db = lock(&state.db);
        let _ = db.set(SETTINGS_KEY, &next);
        remember_week_goal(&db, &next);
    }
    *lock(&state.settings) = next.clone();
    apply(app, |engine, _| {
        engine.set_config(next.timer_config());
        Vec::new()
    });
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(next.always_on_top);
    }
    let _ = app.emit(SETTINGS_EVENT, &next);
    next
}

/// Store the weekly goal for the current week, so past weeks keep the goal
/// they were measured against when it is changed later.
fn remember_week_goal(db: &Db, settings: &Settings) {
    let p = goals::periods(clock::wall_ms(), &settings.week_start);
    let _ = db.set_week_goal(&p.week_key, settings.weekly_goal_minutes);
}

/// Notify when a just-recorded focus session completes today's or this week's goal.
fn check_goals(app: &AppHandle, record: &SessionRecord) {
    let state = app.state::<AppState>();
    let settings = lock(&state.settings).clone();
    let lang = *lock(&state.lang);
    let p = goals::periods(record.ended_at, &settings.week_start);
    let totals = {
        let db = lock(&state.db);
        remember_week_goal(&db, &settings);
        db.focus_totals(&[p.today, p.week]).unwrap_or_default()
    };
    let [today, week] = totals[..] else { return };
    // Sessions count toward the day and week they started in.
    let added = |(from, to): (i64, i64)| {
        if (from..to).contains(&record.started_at) { record.duration_ms } else { 0 }
    };
    let mut reached = Vec::new();
    if settings.daily_goal_enabled {
        let goal = settings.daily_goal_minutes as u64 * 60_000;
        if goals::crossed(today.saturating_sub(added(p.today)), today, goal) {
            reached.push(i18n::goal_reached(lang, false, settings.daily_goal_minutes));
        }
    }
    let goal = settings.weekly_goal_minutes as u64 * 60_000;
    if goals::crossed(week.saturating_sub(added(p.week)), week, goal) {
        reached.push(i18n::goal_reached(lang, true, settings.weekly_goal_minutes));
    }
    for (title, body) in reached {
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

fn save_snapshot(app: &AppHandle) {
    let state = app.state::<AppState>();
    // Until the user answers the resume prompt, keep the stored session as is.
    if state.pending_resume.load(Ordering::SeqCst) {
        return;
    }
    let snap = lock(&state.engine).snapshot(clock::now());
    let _ = lock(&state.db).set(SNAPSHOT_KEY, &snap);
}

/// Native heartbeat: advances the engine, publishes a state update whenever
/// the displayed second changes, and checkpoints running sessions.
fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_second = u64::MAX;
        let mut last_save = 0u64;
        loop {
            let state = app.state::<AppState>();
            let now = clock::now();
            let (events, view) = {
                let mut engine = lock(&state.engine);
                let events = engine.tick(now);
                (events, engine.view(now))
            };
            let changed = !events.is_empty();
            handle_events(&app, events);

            let second = view.remaining_ms.div_ceil(1000);
            if changed || second != last_second {
                last_second = second;
                let _ = app.emit(STATE_EVENT, &view);
            }

            let running = view.status == Status::Running;
            // Checkpoint so a crash or power loss loses at most a few seconds.
            if changed || (running && now.mono.saturating_sub(last_save) >= 5_000) {
                last_save = now.mono;
                save_snapshot(&app);
            }

            // Wake just after the displayed second changes.
            let wait = if running { (view.remaining_ms % 1000) + 5 } else { 500 };
            std::thread::sleep(Duration::from_millis(wait.clamp(20, 500)));
        }
    });
}

/// Called from the power-notification thread when the machine is about to sleep.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn on_suspend(app: &AppHandle) {
    apply(app, |engine, now| engine.suspend(now));
}

/// Called after the machine wakes up. The session stays paused until the user resumes.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn on_resume(app: &AppHandle) {
    apply(app, |engine, now| engine.tick(now));
}
