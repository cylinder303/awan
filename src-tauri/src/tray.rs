//! System tray icon: remaining time on hover, and a small menu.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{include_image, AppHandle, Manager, Wry};

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

    let icon = TrayIconBuilder::with_id("main")
        .icon(glyph(small_icon_size()))
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
        })
        .build(app)?;
    Ok(Tray { icon, toggle, skip, show, mini, quit })
}

/// The tray glyph (scripts/tray-icon.svg), rendered at each small-icon size
/// from 100% to 300% display scaling so Windows never has to resize it.
static GLYPHS: [(u32, Image<'static>); 6] = [
    (16, include_image!("icons/tray/16x16.png")),
    (20, include_image!("icons/tray/20x20.png")),
    (24, include_image!("icons/tray/24x24.png")),
    (32, include_image!("icons/tray/32x32.png")),
    (40, include_image!("icons/tray/40x40.png")),
    (48, include_image!("icons/tray/48x48.png")),
];

/// The smallest glyph at least `size` pixels wide, or the largest one.
fn glyph(size: u32) -> Image<'static> {
    let (_, image) = GLYPHS.iter().find(|(s, _)| *s >= size).unwrap_or(&GLYPHS[GLYPHS.len() - 1]);
    image.clone()
}

/// Width of a small icon (tray, title bar) at the current display scaling.
#[cfg(windows)]
fn small_icon_size() -> u32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    // SAFETY: plain query with no pointers.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) };
    u32::try_from(size).ok().filter(|s| *s > 0).unwrap_or(16)
}

#[cfg(not(windows))]
fn small_icon_size() -> u32 {
    32
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_matches_display_scale() {
        for (want, got) in [(16, 16), (20, 20), (24, 24), (28, 32), (32, 32), (36, 40), (48, 48), (64, 48), (0, 16)] {
            let image = glyph(want);
            assert_eq!((image.width(), image.height()), (got, got), "size {want}");
            assert_eq!(image.rgba().len(), (got * got * 4) as usize);
        }
    }
}
