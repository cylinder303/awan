//! System-wide start/pause shortcut (works while the app is in the background).

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::apply(app, |engine, now| {
                    engine.toggle(now);
                    Vec::new()
                });
            }
        })
        .build()
}

/// Parse "Ctrl+Alt+Space" and friends. A shortcut must include Ctrl, Alt or
/// the Windows key, so it can never swallow ordinary typing.
pub fn parse(text: &str) -> Result<Shortcut, String> {
    let shortcut: Shortcut = text.parse().map_err(|_| "invalid".to_string())?;
    if !shortcut.mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER) {
        return Err("needs_modifier".into());
    }
    Ok(shortcut)
}

/// Replace the registered shortcut. An empty string turns it off.
/// Fails with "taken" if another program already uses the combination.
pub fn register(app: &AppHandle, text: &str) -> Result<(), String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    if text.is_empty() {
        return Ok(());
    }
    let shortcut = parse(text)?;
    manager.register(shortcut).map_err(|_| "taken".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DEFAULT_SHORTCUT;

    #[test]
    fn default_and_typical_shortcuts_parse() {
        assert!(parse(DEFAULT_SHORTCUT).is_ok());
        assert!(parse("Ctrl+Shift+P").is_ok());
        assert!(parse("Alt+F9").is_ok());
        assert!(parse("Super+1").is_ok());
    }

    #[test]
    fn shortcuts_without_ctrl_alt_or_win_are_refused() {
        assert_eq!(parse("Shift+P").unwrap_err(), "needs_modifier");
        assert_eq!(parse("P").unwrap_err(), "needs_modifier");
        assert_eq!(parse("Ctrl+Nonsense").unwrap_err(), "invalid");
    }
}
