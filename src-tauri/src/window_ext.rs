//! Native window touches for the frameless main window.

use tauri::WebviewWindow;

/// Ask Windows 11 for rounded corners. On Windows 10 the call is ignored and
/// the window keeps clean square edges.
#[cfg(windows)]
pub fn apply_corner_preference(window: &WebviewWindow) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    if let Ok(hwnd) = window.hwnd() {
        let pref = DWMWCP_ROUND;
        // SAFETY: valid window handle and a pointer to a 4-byte value.
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0 as _,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &pref as *const _ as *const _,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn apply_corner_preference(_window: &WebviewWindow) {}

/// Bring a window to the front, restoring it if minimized or hidden.
pub fn reveal(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}
