//! Awan: a minimal Pomodoro timer.
//!
//! The timer runs here in the native process (not in the web UI) so it keeps
//! exact time while the window is minimized, hidden, or throttled. The UI
//! only renders `TimerView` snapshots pushed through the `timer://state` event.

#[cfg(target_os = "android")]
mod alerts;
mod backup;
mod clock;
mod commands;
mod db;
mod display;
mod fonts;
mod goals;
mod i18n;
mod power;
mod settings;
#[cfg(desktop)]
mod shortcuts;
mod sound;
mod streak;
mod sync;
mod timer;
#[cfg(desktop)]
mod tray;
mod window_ext;

/// Elsewhere the timer notifies at the moment it finishes.
#[cfg(not(target_os = "android"))]
mod alerts {
    use tauri::AppHandle;

    pub const SCHEDULED_AHEAD: bool = false;
    pub fn start(_app: &AppHandle) {}
    pub fn sync(_app: &AppHandle) {}
}

/// Phones have no tray: the calls shared with the desktop do nothing.
#[cfg(mobile)]
mod tray {
    use crate::timer::TimerView;
    use tauri::AppHandle;

    pub struct Tray;
    pub fn update(_app: &AppHandle, _view: &TimerView) {}
    pub fn refresh(_app: &AppHandle) {}
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WindowEvent};
#[cfg(desktop)]
use tauri::{CloseRequestApi, Window};
#[cfg(desktop)]
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;

use display::{Geometry, View};

use db::Db;
use i18n::Lang;
use settings::{Settings, SETTINGS_KEY};
use timer::{Engine, Event, Mode, Now, SessionRecord, Snapshot, Status, TimerView};

const SNAPSHOT_KEY: &str = "active_session";
#[cfg(desktop)]
const TRAY_HINT_KEY: &str = "tray_hint_shown";
/// Passed by the Windows startup entry: start quietly in the tray.
const AUTOSTART_ARG: &str = "--autostart";
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
    /// Full window or mini timer.
    view: Mutex<View>,
    geometry: Mutex<HashMap<String, Geometry>>,
    geometry_dirty: AtomicBool,
    tray: Mutex<Option<tray::Tray>>,
    /// Launched at Windows startup: stay in the tray until asked.
    start_hidden: bool,
}

/// A notification, with the app's status bar icon on Android.
pub(crate) fn notification(app: &AppHandle) -> tauri_plugin_notification::NotificationBuilder<tauri::Wry> {
    let builder = app.notification().builder();
    #[cfg(target_os = "android")]
    let builder = builder.icon("ic_stat_awan").icon_color("#d1764f");
    builder
}

/// Lock a mutex, recovering the data if a previous holder panicked.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // Must be registered first: a second launch focuses the running app.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        display::show(app, display::current(app));
    }));
    let builder = builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init());
    // Phones: reads and writes the documents chosen in the file dialogs.
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_fs::init());
    #[cfg(desktop)]
    let builder = builder
        .plugin(shortcuts::plugin())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![AUTOSTART_ARG])));
    builder
        .on_window_event(|window, event| match event {
            #[cfg(desktop)]
            WindowEvent::CloseRequested { api, .. } => on_close_requested(window, api),
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => display::track(window),
            _ => {}
        })
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
            commands::toggle_view,
            commands::set_global_shortcut,
            commands::set_launch_at_startup,
            commands::get_settings,
            commands::update_settings,
            commands::reset_settings,
            commands::list_sounds,
            commands::preview_sound,
            commands::fonts::system_fonts,
            commands::fonts::font_files,
            commands::fonts::add_font_file,
            commands::fonts::font_file_data,
            commands::fonts::remove_font_file,
            commands::history::list_sessions,
            commands::history::delete_session,
            commands::history::focus_totals,
            commands::history::recent_tasks,
            commands::history::export_csv,
            commands::history::export_backup,
            commands::history::import_backup,
            commands::history::week_goals,
            commands::history::first_session_at,
            commands::history::streak_overview,
            commands::sync::sync_info,
            commands::sync::sync_pairing_start,
            commands::sync::sync_pairing_stop,
            commands::sync::sync_remove_device,
            commands::sync::sync_find,
            commands::sync::sync_pair,
            commands::sync::sync_now,
            commands::sync::sync_unlink,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Awan")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                save_snapshot(app);
                display::flush(app);
            }
        });
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db = Db::open(&dir.join("awan.db"))?;
    let settings = db.get::<Settings>(SETTINGS_KEY)?.unwrap_or_default().sanitized();

    let mut engine = Engine::new(settings.timer_config());
    // A computer that goes to sleep pauses the session; a phone with its
    // screen locked is still mid-session.
    engine.set_sleep_pauses(cfg!(desktop));
    let mut pending = false;
    if let Some(snap) = db.get::<Snapshot>(SNAPSHOT_KEY)? {
        if cfg!(mobile) && snap.running {
            // The system closed the app mid-session: carry on.
            engine.restore_running(&snap, clock::now());
        } else if snap.elapsed_ms > 0 {
            engine.restore(&snap);
            pending = true;
        } else {
            engine.set_task(&snap.task);
        }
    }

    remember_week_goal(&db, &settings);
    let view = display::load_view(&db);
    app.manage(AppState {
        engine: Mutex::new(engine),
        db: Mutex::new(db),
        settings: Mutex::new(settings.clone()),
        lang: Mutex::new(Lang::En),
        pending_resume: AtomicBool::new(pending),
        view: Mutex::new(view),
        geometry: Mutex::new(HashMap::new()),
        geometry_dirty: AtomicBool::new(false),
        tray: Mutex::new(None),
        start_hidden: std::env::args().any(|a| a == AUTOSTART_ARG),
    });
    let handle = app.handle().clone();

    // The windows are created here rather than by Tauri ("create": false in
    // tauri.conf.json): Windows pumps messages while it builds each webview,
    // so a page that loads early could otherwise call a command before the
    // state above exists and fail to start.
    for config in app.config().app.windows.clone() {
        // A phone shows one full-screen window: there is no mini timer.
        if cfg!(mobile) && config.label != display::MAIN {
            continue;
        }
        let builder = tauri::WebviewWindowBuilder::from_config(&handle, &config)?;
        #[cfg(feature = "smoke-test")]
        let builder = match std::env::var("AWAN_DEVTOOLS_PORT") {
            Ok(port) => builder.additional_browser_args(&format!(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"
            )),
            Err(_) => builder,
        };
        builder.build()?;
    }

    #[cfg(desktop)]
    setup_desktop(&handle, settings);

    power::watch(app.handle().clone());
    spawn_ticker(app.handle().clone());
    sync::app::start(app.handle());
    Ok(())
}

/// Window placement, tray, shortcut and startup entry: the parts of startup
/// that only exist on a computer.
#[cfg(desktop)]
fn setup_desktop(handle: &AppHandle, settings: Settings) {
    for label in [display::MAIN, display::MINI] {
        if let Some(window) = handle.get_webview_window(label) {
            display::place(handle, &window);
        }
    }
    if let Some(window) = handle.get_webview_window(display::MAIN) {
        window_ext::apply_corner_preference(&window);
        window_ext::set_always_on_top(&window, settings.always_on_top);
    }

    // Without a tray (rare, e.g. no notification area) the app still works.
    *lock(&handle.state::<AppState>().tray) = tray::create(handle).ok();
    // If another program owns the shortcut, the app still starts; Settings
    // lets the user pick a different one.
    let _ = shortcuts::register(handle, &settings.global_shortcut);
    // The startup entry can be changed outside the app; trust the system.
    let launches = handle.autolaunch().is_enabled().unwrap_or(false);
    if launches != settings.launch_at_startup {
        commit_settings(handle, Settings { launch_at_startup: launches, ..settings });
    }

    // Each window appears once it has painted (no white flash). If the UI
    // never reports in, show the current view anyway.
    let fallback = handle.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        let state = fallback.state::<AppState>();
        let label = display::current(&fallback).label();
        let shown = fallback.get_webview_window(label).and_then(|w| w.is_visible().ok()).unwrap_or(true);
        if !state.start_hidden && !shown {
            display::show(&fallback, display::current(&fallback));
        }
    });
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
    tray::update(app, &view);
    save_snapshot(app);
    alerts::sync(app);
    wake_ticker();
    view
}

fn handle_events(app: &AppHandle, events: Vec<Event>) {
    // Goal notifications go out after the "session finished" one.
    let mut focus_ended = Vec::new();
    for event in events {
        match event {
            Event::SessionEnded(record) => {
                let _ = lock(&app.state::<AppState>().db).insert_session(&record);
                sync::app::local_change();
                if record.mode == Mode::Focus {
                    focus_ended.push(record);
                }
                let _ = app.emit(HISTORY_EVENT, ());
            }
            Event::Finished { ended, next, auto_started } => {
                // Where alerts are scheduled ahead, the system has already
                // shown this one (with its sound) at the right moment.
                if !alerts::SCHEDULED_AHEAD {
                    let state = app.state::<AppState>();
                    let lang = *lock(&state.lang);
                    let (title, body) = i18n::finished(lang, ended, next);
                    let _ = notification(app).title(title).body(body).show();
                    let (id, volume) = {
                        let s = lock(&state.settings);
                        (s.sound.clone(), s.alert_volume())
                    };
                    sound::play(&id, volume);
                }
                let _ = app.emit(
                    FINISHED_EVENT,
                    serde_json::json!({ "ended": ended, "next": next, "autoStarted": auto_started }),
                );
            }
            Event::SleepPaused => {}
        }
    }
    let any_focus = !focus_ended.is_empty();
    for record in focus_ended {
        check_goals(app, &record);
    }
    if any_focus {
        check_streak(app);
    }
}

/// Save settings changed in the UI. The global shortcut and the startup
/// entry are tied to the system and only change through their own commands.
pub(crate) fn save_settings(app: &AppHandle, next: Settings) -> Settings {
    let current = lock(&app.state::<AppState>().settings).clone();
    commit_settings(
        app,
        Settings {
            global_shortcut: current.global_shortcut,
            launch_at_startup: current.launch_at_startup,
            ..next
        },
    )
}

/// Back to defaults, including the shortcut and the startup entry.
pub(crate) fn reset_settings(app: &AppHandle) -> Settings {
    let defaults = Settings::default();
    #[cfg(desktop)]
    {
        let _ = shortcuts::register(app, &defaults.global_shortcut);
        let _ = app.autolaunch().disable();
    }
    commit_settings(app, defaults)
}

/// Validate, store and apply settings, then tell every window.
pub(crate) fn commit_settings(app: &AppHandle, next: Settings) -> Settings {
    commit_settings_at(app, next, None)
}

/// Take the shared settings from another device if they changed there more
/// recently than here. Returns whether they were taken.
pub(crate) fn adopt_shared_settings(app: &AppHandle, theirs: sync::wire::StampedSettings) -> bool {
    let state = app.state::<AppState>();
    let ours = lock(&state.db).get::<i64>(sync::app::SHARED_AT_KEY).ok().flatten().unwrap_or(0);
    if theirs.changed_at <= ours {
        return false;
    }
    let current = lock(&state.settings).clone();
    if current.shared() == theirs.values {
        // Same values: only adopt the date, so they are not compared again.
        let _ = lock(&state.db).set(sync::app::SHARED_AT_KEY, &theirs.changed_at);
        return false;
    }
    commit_settings_at(app, current.with_shared(theirs.values), Some(theirs.changed_at));
    true
}

/// `commit_settings`, dating a change to the shared settings `changed_at`
/// when it came from another device, or now when it was made here.
fn commit_settings_at(app: &AppHandle, next: Settings, changed_at: Option<i64>) -> Settings {
    let next = next.sanitized();
    let state = app.state::<AppState>();
    let shared_changed = lock(&state.settings).shared() != next.shared();
    {
        let db = lock(&state.db);
        let _ = db.set(SETTINGS_KEY, &next);
        if shared_changed {
            let _ = db.set(sync::app::SHARED_AT_KEY, &changed_at.unwrap_or_else(clock::wall_ms));
        }
        remember_week_goal(&db, &next);
    }
    if shared_changed && changed_at.is_none() {
        sync::app::local_change();
    }
    *lock(&state.settings) = next.clone();
    apply(app, |engine, _| {
        engine.set_config(next.timer_config());
        Vec::new()
    });
    if let Some(window) = app.get_webview_window(display::MAIN) {
        window_ext::set_always_on_top(&window, next.always_on_top);
    }
    let _ = app.emit(SETTINGS_EVENT, &next);
    next
}

/// Close button: the main window hides to the tray or quits (per settings);
/// closing the mini timer returns to the full window.
#[cfg(desktop)]
fn on_close_requested(window: &Window, api: &CloseRequestApi) {
    let app = window.app_handle();
    if app.try_state::<AppState>().is_none() {
        return; // still starting up: let it close normally
    }
    api.prevent_close();
    if window.label() == display::MINI {
        display::show(app, View::Full);
        return;
    }
    let state = app.state::<AppState>();
    let to_tray = lock(&state.settings).close_action == "tray";
    if !to_tray {
        app.exit(0);
        return;
    }
    let _ = window.hide();
    let first_time = {
        let db = lock(&state.db);
        let shown = db.get::<bool>(TRAY_HINT_KEY).ok().flatten().unwrap_or(false);
        let _ = db.set(TRAY_HINT_KEY, &true);
        !shown
    };
    if first_time {
        let (title, body) = i18n::still_running(*lock(&state.lang));
        let _ = notification(app).title(title).body(body).show();
    }
}

/// Store the weekly goal for the current week, so past weeks keep the goal
/// they were measured against when it is changed later.
fn remember_week_goal(db: &Db, settings: &Settings) {
    let p = goals::periods(clock::wall_ms(), &settings.week_start);
    let _ = db.set_week_goal(&p.week_key, settings.weekly_goal_minutes, clock::wall_ms());
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
        let _ = notification(app).title(title).body(body).show();
    }
}

/// A quiet note when the streak becomes the longest so far (never when it ends).
fn check_streak(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = lock(&state.settings).clone();
    let lang = *lock(&state.lang);
    let (overview, known) = {
        let db = lock(&state.db);
        let Ok(overview) = streak::overview(&db, &settings, clock::wall_ms(), 0) else { return };
        let key = format!("streak.best.{}", overview.mode);
        let known = db.get::<u32>(&key).ok().flatten();
        if known != Some(overview.streak.best) {
            let _ = db.set(&key, &overview.streak.best);
        }
        (overview, known)
    };
    let s = overview.streak;
    // The first time, just remember the record from the existing history.
    if known.is_some_and(|k| s.best > k) && s.current == s.best && s.best >= 2 {
        let (title, body) = i18n::streak_record(lang, overview.mode == "weekly", s.best);
        let _ = notification(app).title(title).body(body).channel_id("alert-silent").show();
    }
}

fn save_snapshot(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    // Until the user answers the resume prompt, keep the stored session as is.
    if state.pending_resume.load(Ordering::SeqCst) {
        return;
    }
    let snap = lock(&state.engine).snapshot(clock::now());
    let _ = lock(&state.db).set(SNAPSHOT_KEY, &snap);
}

/// Set when something changed that the heartbeat should look at right away.
static TICKER_WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// How long the heartbeat sleeps when no session is counting down. Nothing
/// needs it then except saving window positions on a computer.
const IDLE_WAIT: Duration = if cfg!(desktop) { Duration::from_secs(5) } else { Duration::from_secs(60) };

fn wake_ticker() {
    let (woken, cv) = &TICKER_WAKE;
    *lock(woken) = true;
    cv.notify_one();
}

/// Sleep for `wait`, or until `wake_ticker` is called.
fn ticker_sleep(wait: Duration) {
    let (woken, cv) = &TICKER_WAKE;
    let guard = lock(woken);
    let (mut guard, _) = cv.wait_timeout_while(guard, wait, |w| !*w).unwrap_or_else(|e| e.into_inner());
    *guard = false;
}

/// Native heartbeat: advances the engine, publishes a state update whenever
/// the displayed second changes, and checkpoints running sessions. While no
/// session is counting down it sleeps until something changes.
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
            if changed {
                alerts::sync(&app);
            }

            let second = view.remaining_ms.div_ceil(1000);
            if changed || second != last_second {
                last_second = second;
                let _ = app.emit(STATE_EVENT, &view);
                tray::update(&app, &view);
            }
            display::flush(&app);

            let running = view.status == Status::Running;
            // Checkpoint so a crash or power loss loses at most a few seconds.
            if changed || (running && now.mono.saturating_sub(last_save) >= 5_000) {
                last_save = now.mono;
                save_snapshot(&app);
            }

            // Running: wake just after the displayed second changes.
            let wait = if running {
                Duration::from_millis(((view.remaining_ms % 1000) + 5).clamp(20, 500))
            } else {
                IDLE_WAIT
            };
            ticker_sleep(wait);
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
