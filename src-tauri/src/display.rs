//! The two views (full window and mini timer), switching between them, and
//! remembering each window's position and size.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::{lock, window_ext, AppState};

pub const MAIN: &str = "main";
pub const MINI: &str = "mini";
const VIEW_KEY: &str = "view";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    Full,
    Mini,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Full => MAIN,
            View::Mini => MINI,
        }
    }
}

/// Window position and size in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

/// A rectangle on the desktop (a monitor's work area), physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Is enough of the window on screen to grab and move it? The top strip
/// (where you drag) must lie inside a monitor, at least 80 px wide.
pub fn is_reachable(g: &Geometry, areas: &[Area]) -> bool {
    areas.iter().any(|a| {
        let left = g.x.max(a.x);
        let right = (g.x + g.width as i32).min(a.x + a.width as i32);
        let top_ok = g.y >= a.y && g.y + 24 <= a.y + a.height as i32;
        top_ok && right - left >= 80.min(g.width as i32)
    })
}

/// Centre a window of the given size in an area, shrinking it to fit.
pub fn centered(width: u32, height: u32, area: &Area) -> Geometry {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Geometry {
        x: area.x + (area.width - width) as i32 / 2,
        y: area.y + (area.height - height) as i32 / 2,
        width,
        height,
        maximized: false,
    }
}

fn work_areas(window: &WebviewWindow) -> (Vec<Area>, Option<Area>) {
    let to_area = |m: &tauri::Monitor| {
        let r = m.work_area();
        Area { x: r.position.x, y: r.position.y, width: r.size.width, height: r.size.height }
    };
    let all = window.available_monitors().unwrap_or_default().iter().map(to_area).collect();
    let primary = window.primary_monitor().ok().flatten().map(|m| to_area(&m));
    (all, primary)
}

fn geometry_key(label: &str) -> String {
    format!("window.{label}")
}

/// Put a window where it was last time, or somewhere visible if that spot
/// is no longer on any screen (e.g. a monitor was disconnected).
pub fn place(app: &AppHandle, window: &WebviewWindow) {
    let state = app.state::<AppState>();
    let saved = lock(&state.geometry).get(window.label()).copied().or_else(|| {
        lock(&state.db).get::<Geometry>(&geometry_key(window.label())).ok().flatten()
    });
    let current = window.outer_size().ok();
    let (areas, primary) = work_areas(window);
    let target = match saved {
        Some(g) if areas.is_empty() || is_reachable(&g, &areas) => Some(g),
        Some(g) => primary.or(areas.first().copied()).map(|a| centered(g.width, g.height, &a)),
        None => None,
    };
    if let Some(g) = target {
        let _ = window.set_size(PhysicalSize::new(g.width, g.height));
        let _ = window.set_position(PhysicalPosition::new(g.x, g.y));
        if g.maximized {
            let _ = window.maximize();
        }
    } else if let (Some(size), Some(area)) = (current, primary) {
        // First run: keep the default size, centred on the main screen.
        let g = centered(size.width, size.height, &area);
        let _ = window.set_position(PhysicalPosition::new(g.x, g.y));
    }
}

/// Remember a window's geometry after it moved or was resized.
pub fn track(window: &tauri::Window) {
    if window.is_minimized().unwrap_or(false) || !window.is_visible().unwrap_or(false) {
        return; // Windows reports a far-off position for minimized windows
    }
    let app = window.app_handle();
    let state = app.state::<AppState>();
    let maximized = window.is_maximized().unwrap_or(false);
    let mut map = lock(&state.geometry);
    let previous = map.get(window.label()).copied();
    let g = if maximized {
        // Keep the normal size so un-maximizing next time restores it.
        match previous {
            Some(p) => Geometry { maximized: true, ..p },
            None => return,
        }
    } else {
        let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else { return };
        Geometry { x: pos.x, y: pos.y, width: size.width, height: size.height, maximized: false }
    };
    if previous != Some(g) {
        map.insert(window.label().to_string(), g);
        state.geometry_dirty.store(true, Ordering::SeqCst);
    }
}

/// Write remembered geometry to the database (called from the heartbeat).
pub fn flush(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.geometry_dirty.swap(false, Ordering::SeqCst) {
        return;
    }
    let map: HashMap<String, Geometry> = lock(&state.geometry).clone();
    let db = lock(&state.db);
    for (label, g) in map {
        let _ = db.set(&geometry_key(&label), &g);
    }
}

pub fn current(app: &AppHandle) -> View {
    *lock(&app.state::<AppState>().view)
}

/// Load the view used last time (full or mini).
pub fn load_view(db: &crate::db::Db) -> View {
    db.get::<View>(VIEW_KEY).ok().flatten().unwrap_or(View::Full)
}

/// Show one view and hide the other.
pub fn show(app: &AppHandle, view: View) {
    let state = app.state::<AppState>();
    *lock(&state.view) = view;
    let _ = lock(&state.db).set(VIEW_KEY, &view);
    let (show, hide) = match view {
        View::Full => (MAIN, MINI),
        View::Mini => (MINI, MAIN),
    };
    if let Some(w) = app.get_webview_window(hide) {
        let _ = w.hide();
    }
    if let Some(w) = app.get_webview_window(show) {
        // Re-check placement: the screen layout may have changed while hidden.
        if !w.is_visible().unwrap_or(false) {
            place(app, &w);
        }
        window_ext::reveal(&w);
    }
    crate::tray::refresh(app);
}

pub fn toggle(app: &AppHandle) {
    let next = match current(app) {
        View::Full => View::Mini,
        View::Mini => View::Full,
    };
    show(app, next);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Area = Area { x: 0, y: 0, width: 1920, height: 1040 };
    const SECOND: Area = Area { x: 1920, y: 0, width: 1920, height: 1040 };

    fn g(x: i32, y: i32) -> Geometry {
        Geometry { x, y, width: 380, height: 580, maximized: false }
    }

    #[test]
    fn on_screen_windows_stay_put() {
        assert!(is_reachable(&g(100, 100), &[SCREEN]));
        assert!(is_reachable(&g(2500, 100), &[SCREEN, SECOND]));
        assert!(is_reachable(&g(1800, 100), &[SCREEN]), "partly off the right edge is still reachable");
    }

    #[test]
    fn windows_on_a_missing_monitor_are_unreachable() {
        assert!(!is_reachable(&g(2500, 100), &[SCREEN]));
        assert!(!is_reachable(&g(100, -400), &[SCREEN]), "title strip above the screen");
        assert!(!is_reachable(&g(100, 1030), &[SCREEN]), "title strip below the screen");
        assert!(!is_reachable(&g(1880, 100), &[SCREEN]), "only 40 px visible");
    }

    #[test]
    fn centering_fits_the_window_into_the_area() {
        let c = centered(380, 580, &SECOND);
        assert_eq!((c.x, c.y), (1920 + 770, 230));
        let big = centered(4000, 3000, &SCREEN);
        assert_eq!((big.x, big.y, big.width, big.height), (0, 0, 1920, 1040));
    }
}
