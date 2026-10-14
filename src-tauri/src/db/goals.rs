//! Weekly goal history.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{Db, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WeekGoal {
    /// Local start date of the week, "YYYY-MM-DD".
    pub week_key: String,
    pub goal_minutes: u32,
}

impl Db {
    /// Remember the goal in effect for a week (the latest value wins).
    /// `now` dates the change, for syncing; an unchanged goal keeps its date.
    pub fn set_week_goal(&self, week_key: &str, goal_minutes: u32, now: i64) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO week_goals (week_key, goal_minutes, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(week_key) DO UPDATE SET goal_minutes = excluded.goal_minutes, updated_at = excluded.updated_at
                 WHERE goal_minutes <> excluded.goal_minutes",
                params![week_key, goal_minutes.max(1), now],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn week_goals(&self) -> Result<Vec<WeekGoal>> {
        let mut stmt = self
            .conn
            .prepare("SELECT week_key, goal_minutes FROM week_goals ORDER BY week_key")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok(WeekGoal { week_key: r.get(0)?, goal_minutes: r.get(1)? }))
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
    }

    /// Add goals from a backup without overwriting weeks already known.
    pub fn merge_week_goals(&self, goals: &[WeekGoal]) -> Result<()> {
        for g in goals {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO week_goals (week_key, goal_minutes) VALUES (?1, ?2)",
                    params![g.week_key, g.goal_minutes.max(1)],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Every focus session as (start, length, completed), for the streak.
    pub fn focus_sessions(&self) -> Result<Vec<(i64, u64, bool)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT started_at, duration_ms, completed FROM sessions WHERE mode = 'focus'")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)?.max(0) as u64, r.get(2)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
    }

    /// Start time of the oldest session, if any.
    pub fn first_session_at(&self) -> Result<Option<i64>> {
        self.conn
            .query_row("SELECT MIN(started_at) FROM sessions", [], |r| r.get(0))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_goal_for_a_week_wins_and_merge_keeps_existing() {
        let db = Db::open_in_memory().unwrap();
        db.set_week_goal("2026-10-03", 1200, 1).unwrap();
        db.set_week_goal("2026-10-03", 900, 2).unwrap();
        db.set_week_goal("2026-09-26", 600, 3).unwrap();
        db.merge_week_goals(&[
            WeekGoal { week_key: "2026-10-03".into(), goal_minutes: 60 },
            WeekGoal { week_key: "2026-09-19".into(), goal_minutes: 300 },
        ])
        .unwrap();
        let goals = db.week_goals().unwrap();
        assert_eq!(
            goals.iter().map(|g| (g.week_key.as_str(), g.goal_minutes)).collect::<Vec<_>>(),
            vec![("2026-09-19", 300), ("2026-09-26", 600), ("2026-10-03", 900)]
        );
    }

    #[test]
    fn first_session_is_none_when_empty() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.first_session_at().unwrap(), None);
    }
}
