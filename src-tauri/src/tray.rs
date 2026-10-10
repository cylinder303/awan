//! System tray icon: remaining time on hover, and a small menu.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::display::{self, View};
use crate::i18n;
use crate::timer::{Status, TimerView};
use crate::{apply, clock, lock, AppState};

/// Handles are cheap to clone; they are cloned out of the app state before
/// use so no lock is held while Tauri hops to the main thread.
#[derive(Clone)]
pub struct Tray {
    icon: TrayIcon,
    toggle: MenuItem<Wry>,
    skip: MenuItem<Wry>,
    show: MenuItem<Wry>,
    mini: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<Tray> {
    let toggle = MenuItem::with_id(app, "toggle", "Start", true, None::<&str>)?;
    let skip = MenuItem::with_id(app, "skip", "Skip", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show window", true, None::<&str>)?;
    let mini = MenuItem::with_id(app, "mini", "Mini mode", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &skip, &separator, &show, &mini, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Awan")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => {
                apply(app, |engine, now| {
                    engine.toggle(now);
                    Vec::new()
                });
            }
            "skip" => {
                apply(app, |engine, now| engine.skip(now));
            }
            "show" => display::show(app, View::Full),
            // Labelled "Mini mode" or "Full mode" depending on what is open.
            "mini" => display::toggle(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // A left click brings the app back.
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                display::show(app, display::current(app));
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let icon = builder.build(app)?;
    Ok(Tray { icon, toggle, skip, show, mini, quit })
}

/// Update the tooltip and menu labels from the current timer state.
pub fn update(app: &AppHandle, view: &TimerView) {
    let state = app.state::<AppState>();
    let lang = *lock(&state.lang);
    let mini_open = display::current(app) == View::Mini;
    let tray = lock(&state.tray).clone();
    if let Some(tray) = tray {
        let _ = tray.icon.set_tooltip(Some(i18n::tray_tooltip(lang, view)));
        let labels = i18n::tray_labels(lang);
        let toggle = match view.status {
            Status::Running => labels.pause,
            Status::Paused => labels.resume,
            Status::Idle => labels.start,
        };
        let _ = tray.toggle.set_text(toggle);
        let _ = tray.skip.set_text(labels.skip);
        let _ = tray.show.set_text(labels.show);
        let _ = tray.mini.set_text(if mini_open { labels.full } else { labels.mini });
        let _ = tray.quit.set_text(labels.quit);
    }
}

/// Refresh after something other than the timer changed (language, view).
pub fn refresh(app: &AppHandle) {
    let view = lock(&app.state::<AppState>().engine).view(clock::now());
    update(app, &view);
}
