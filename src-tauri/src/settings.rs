//! User settings, stored as one JSON document in the database.
//!
//! Every field has a default (`#[serde(default)]`), so settings saved by an
//! older version load cleanly after an update, and new fields simply start
//! at their defaults.

use serde::{Deserialize, Serialize};

use crate::sound;
use crate::timer::TimerConfig;

pub const SETTINGS_KEY: &str = "settings";
pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+Space";

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
    /// First day of the week for weekly totals: "saturday", "sunday" or "monday".
    pub week_start: String,
    /// Weekly focus goal.
    pub weekly_goal_minutes: u32,
    pub daily_goal_enabled: bool,
    pub daily_goal_minutes: u32,
    /// What the window's close button does: "tray" (keep running) or "quit".
    pub close_action: String,
    /// Mirrors the Windows startup entry (changed only through its command).
    pub launch_at_startup: bool,
    /// Mini mode background opacity in percent, 30..=100.
    pub mini_opacity: u32,
    /// System-wide start/pause shortcut such as "Ctrl+Alt+Space"; empty = off.
    /// Changed only through its command, which checks it can be registered.
    pub global_shortcut: String,
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
            week_start: "saturday".to_string(),
            weekly_goal_minutes: 20 * 60,
            daily_goal_enabled: false,
            daily_goal_minutes: 4 * 60,
            close_action: "tray".to_string(),
            launch_at_startup: false,
            mini_opacity: 92,
            global_shortcut: DEFAULT_SHORTCUT.to_string(),
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
        // Weekly goal: 1-100 hours in whole hours. Daily goal: 30 min - 16 h in half hours.
        self.weekly_goal_minutes = (self.weekly_goal_minutes.clamp(60, 6000) + 30) / 60 * 60;
        self.daily_goal_minutes = (self.daily_goal_minutes.clamp(30, 960) + 15) / 30 * 30;
        if !sound::exists(&self.sound) {
            self.sound = d.sound;
        }
        if !is_theme_id(&self.theme) {
            self.theme = d.theme;
        }
        if !matches!(self.language.as_str(), "auto" | "ar" | "en") {
            self.language = d.language;
        }
        if !matches!(self.week_start.as_str(), "saturday" | "sunday" | "monday") {
            self.week_start = d.week_start;
        }
        if !matches!(self.close_action.as_str(), "tray" | "quit") {
            self.close_action = d.close_action;
        }
        self.mini_opacity = self.mini_opacity.clamp(30, 100);
        if self.global_shortcut.len() > 40
            || !self.global_shortcut.chars().all(|c| c.is_ascii_alphanumeric() || c == '+')
        {
            self.global_shortcut = d.global_shortcut;
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

/// The settings every linked device shares: the timer, goals, appearance,
/// language and alert sound. Volume and everything about windows, startup
/// and shortcuts stay with each device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Shared {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sessions_before_long: u32,
    pub auto_start: bool,
    pub sound: String,
    pub theme: String,
    pub custom_theme: CustomTheme,
    pub mode_colors: bool,
    pub language: String,
    pub week_start: String,
    pub weekly_goal_minutes: u32,
    pub daily_goal_enabled: bool,
    pub daily_goal_minutes: u32,
}

impl Default for Shared {
    fn default() -> Self {
        Settings::default().shared()
    }
}

impl Settings {
    pub fn shared(&self) -> Shared {
        Shared {
            focus_minutes: self.focus_minutes,
            short_break_minutes: self.short_break_minutes,
            long_break_minutes: self.long_break_minutes,
            sessions_before_long: self.sessions_before_long,
            auto_start: self.auto_start,
            sound: self.sound.clone(),
            theme: self.theme.clone(),
            custom_theme: self.custom_theme.clone(),
            mode_colors: self.mode_colors,
            language: self.language.clone(),
            week_start: self.week_start.clone(),
            weekly_goal_minutes: self.weekly_goal_minutes,
            daily_goal_enabled: self.daily_goal_enabled,
            daily_goal_minutes: self.daily_goal_minutes,
        }
    }

    /// These settings with the shared part taken from another device.
    pub fn with_shared(self, s: Shared) -> Settings {
        Settings {
            focus_minutes: s.focus_minutes,
            short_break_minutes: s.short_break_minutes,
            long_break_minutes: s.long_break_minutes,
            sessions_before_long: s.sessions_before_long,
            auto_start: s.auto_start,
            sound: s.sound,
            theme: s.theme,
            custom_theme: s.custom_theme,
            mode_colors: s.mode_colors,
            language: s.language,
            week_start: s.week_start,
            weekly_goal_minutes: s.weekly_goal_minutes,
            daily_goal_enabled: s.daily_goal_enabled,
            daily_goal_minutes: s.daily_goal_minutes,
            ..self
        }
        .sanitized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_part_leaves_device_settings_alone() {
        let phone = Settings { volume: 40, close_action: "quit".into(), global_shortcut: String::new(), ..Settings::default() };
        let pc = Settings { focus_minutes: 50, theme: "dark".into(), language: "ar".into(), volume: 90, ..Settings::default() };
        let merged = phone.clone().with_shared(pc.shared());
        assert_eq!((merged.focus_minutes, merged.theme.as_str(), merged.language.as_str()), (50, "dark", "ar"));
        assert_eq!((merged.volume, merged.close_action.as_str(), merged.global_shortcut.as_str()), (40, "quit", ""));
        assert_eq!(merged.shared(), pc.shared());
    }

    #[test]
    fn shared_values_from_another_device_are_checked() {
        let bad = Shared { focus_minutes: 9999, sound: "siren".into(), ..Shared::default() };
        let s = Settings::default().with_shared(bad);
        assert_eq!((s.focus_minutes, s.sound.as_str()), (180, sound::DEFAULT_SOUND));
    }

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
        let g = Settings { weekly_goal_minutes: 0, daily_goal_minutes: 100_000, ..Settings::default() }.sanitized();
        assert_eq!((g.weekly_goal_minutes, g.daily_goal_minutes), (60, 960));
        let r = Settings { weekly_goal_minutes: 125, daily_goal_minutes: 50, ..Settings::default() }.sanitized();
        assert_eq!((r.weekly_goal_minutes, r.daily_goal_minutes), (120, 60), "rounded to whole / half hours");
    }

    #[test]
    fn unknown_values_fall_back_to_defaults() {
        let s = Settings {
            sound: "nope".into(),
            theme: "Bad Theme!".into(),
            language: "fr".into(),
            week_start: "friday".into(),
            close_action: "minimize".into(),
            mini_opacity: 5,
            global_shortcut: "Ctrl+<script>".into(),
            custom_theme: CustomTheme { background: "red".into(), accent: "#ABCDEF".into(), text: "#12345".into() },
            ..Settings::default()
        }
        .sanitized();
        let d = Settings::default();
        assert_eq!(s.sound, d.sound);
        assert_eq!(s.theme, "auto");
        assert_eq!(s.language, "auto");
        assert_eq!(s.week_start, "saturday");
        assert_eq!(s.close_action, "tray");
        assert_eq!(s.mini_opacity, 30);
        assert_eq!(s.global_shortcut, DEFAULT_SHORTCUT);
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
