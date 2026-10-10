//! Backup file: all settings and history in one JSON document.

use serde::{Deserialize, Serialize};

use crate::db::{Session, WeekGoal};
use crate::settings::Settings;

pub const APP_ID: &str = "awan";
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub app: String,
    pub format: u32,
    pub exported_at: i64,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub week_goals: Vec<WeekGoal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// Sessions added from the backup.
    pub added: usize,
    /// Sessions already present and skipped.
    pub skipped: usize,
}

impl Backup {
    pub fn new(settings: Settings, sessions: Vec<Session>, exported_at: i64) -> Self {
        Self { app: APP_ID.into(), format: FORMAT, exported_at, settings, sessions, week_goals: Vec::new() }
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Parse and check a backup file. Newer formats are refused rather than
    /// half-read.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let backup: Backup =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "not_backup".to_string())?;
        if backup.app != APP_ID {
            return Err("not_backup".into());
        }
        if backup.format > FORMAT {
            return Err("newer_backup".into());
        }
        Ok(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::Mode;

    fn sample() -> Backup {
        let settings = Settings { focus_minutes: 40, theme: "olive".into(), ..Settings::default() };
        let sessions = vec![Session {
            id: 1,
            mode: Mode::Focus,
            started_at: 10,
            ended_at: 20,
            duration_ms: 25 * 60_000,
            task: "Read".into(),
            completed: true,
        }];
        Backup::new(settings, sessions, 99)
    }

    #[test]
    fn round_trip() {
        let b = sample();
        assert_eq!(Backup::from_json(&b.to_json().unwrap()).unwrap(), b);
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(Backup::from_json("{}").unwrap_err(), "not_backup");
        assert_eq!(Backup::from_json("hello").unwrap_err(), "not_backup");
        assert_eq!(
            Backup::from_json(r#"{"app":"other","format":1,"exportedAt":0}"#).unwrap_err(),
            "not_backup"
        );
    }

    #[test]
    fn rejects_newer_format() {
        let json = sample().to_json().unwrap().replace("\"format\": 1", "\"format\": 99");
        assert_eq!(Backup::from_json(&json).unwrap_err(), "newer_backup");
    }

    #[test]
    fn tolerates_bom_and_missing_sections() {
        let b = Backup::from_json("\u{feff}{\"app\":\"awan\",\"format\":1,\"exportedAt\":5}").unwrap();
        assert!(b.sessions.is_empty());
        assert_eq!(b.settings, Settings::default());
    }
}
