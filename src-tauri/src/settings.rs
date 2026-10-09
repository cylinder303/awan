//! User settings, stored as one JSON document in the database.
//!
//! Every field has a default (`#[serde(default)]`), so settings saved by an
//! older version load cleanly after an update, and new fields simply start
//! at their defaults.

use serde::{Deserialize, Serialize};

use crate::sound;
use crate::timer::TimerConfig;

pub const SETTINGS_KEY: &str = "settings";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sessions_before_long: u32,
    pub auto_start: bool,

    pub sound: String,
    /// 0..=100
    pub volume: u32,
    pub muted: bool,

    /// "auto" follows Windows; otherwise a theme id from themes.css or "custom".
    pub theme: String,
    pub custom_theme: CustomTheme,
    /// Give focus, short break and long break their own accent colors.
    pub mode_colors: bool,
    /// "auto" follows the Windows display language; or "ar" / "en".
    pub language: String,
    pub always_on_top: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomTheme {
    pub background: String,
    pub accent: String,
    pub text: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            sessions_before_long: 4,
            auto_start: false,
            sound: sound::DEFAULT_SOUND.to_string(),
            volume: 80,
            muted: false,
            theme: "auto".to_string(),
            custom_theme: CustomTheme::default(),
            mode_colors: true,
            language: "auto".to_string(),
            always_on_top: false,
        }
    }
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self {
            background: "#1b2230".to_string(),
            accent: "#e8a15f".to_string(),
            text: "#ece7df".to_string(),
        }
    }
}

impl Settings {
    /// Clamp every value into its valid range and replace unknown values
    /// with defaults, so a bad value can never break the app.
    pub fn sanitized(mut self) -> Self {
        let d = Settings::default();
        self.focus_minutes = self.focus_minutes.clamp(1, 180);
        self.short_break_minutes = self.short_break_minutes.clamp(1, 60);
        self.long_break_minutes = self.long_break_minutes.clamp(1, 120);
        self.sessions_before_long = self.sessions_before_long.clamp(1, 12);
        self.volume = self.volume.min(100);
        if !sound::exists(&self.sound) {
            self.sound = d.sound;
        }
        if !is_theme_id(&self.theme) {
            self.theme = d.theme;
        }
        if !matches!(self.language.as_str(), "auto" | "ar" | "en") {
            self.language = d.language;
        }
        let dc = CustomTheme::default();
        for (value, fallback) in [
            (&mut self.custom_theme.background, dc.background),
            (&mut self.custom_theme.accent, dc.accent),
            (&mut self.custom_theme.text, dc.text),
        ] {
            if !is_hex_color(value) {
                *value = fallback;
            }
            value.make_ascii_lowercase();
        }
        self
    }

    pub fn timer_config(&self) -> TimerConfig {
        TimerConfig {
            focus_ms: self.focus_minutes as u64 * 60_000,
            short_break_ms: self.short_break_minutes as u64 * 60_000,
            long_break_ms: self.long_break_minutes as u64 * 60_000,
            sessions_before_long: self.sessions_before_long,
            auto_start: self.auto_start,
        }
    }

    /// Effective alert volume, 0.0 when muted.
    pub fn alert_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume as f32 / 100.0
        }
    }
}

/// Theme ids are defined in themes.css, so any short slug is accepted here.
fn is_theme_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let d = Settings::default();
        assert_eq!(d.clone().sanitized(), d);
        assert_eq!(d.timer_config(), TimerConfig::default());
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let s = Settings {
            focus_minutes: 0,
            short_break_minutes: 999,
            sessions_before_long: 50,
            volume: 300,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!((s.focus_minutes, s.short_break_minutes, s.sessions_before_long, s.volume), (1, 60, 12, 100));
    }

    #[test]
    fn unknown_values_fall_back_to_defaults() {
        let s = Settings {
            sound: "nope".into(),
            theme: "Bad Theme!".into(),
            language: "fr".into(),
            custom_theme: CustomTheme { background: "red".into(), accent: "#ABCDEF".into(), text: "#12345".into() },
            ..Settings::default()
        }
        .sanitized();
        let d = Settings::default();
        assert_eq!(s.sound, d.sound);
        assert_eq!(s.theme, "auto");
        assert_eq!(s.language, "auto");
        assert_eq!(s.custom_theme.background, d.custom_theme.background);
        assert_eq!(s.custom_theme.accent, "#abcdef");
        assert_eq!(s.custom_theme.text, d.custom_theme.text);
    }

    #[test]
    fn partial_json_from_older_versions_loads() {
        let s: Settings = serde_json::from_str(r#"{"focusMinutes": 50, "theme": "olive"}"#).unwrap();
        assert_eq!(s.focus_minutes, 50);
        assert_eq!(s.theme, "olive");
        assert_eq!(s.short_break_minutes, 5);
    }

    #[test]
    fn mute_silences_alerts() {
        let s = Settings { volume: 60, ..Settings::default() };
        assert!((s.alert_volume() - 0.6).abs() < 1e-6);
        assert_eq!(Settings { muted: true, ..s }.alert_volume(), 0.0);
    }
}
