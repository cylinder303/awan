//! Session history queries.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{Db, Result};
use crate::timer::{Mode, SessionRecord};

/// A stored session, as sent to the UI and written to backups.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    #[serde(default)]
    pub id: i64,
    pub mode: Mode,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_ms: u64,
    #[serde(default)]
    pub task: String,
    pub completed: bool,
}

/// The id a session has on every device: its type and start time.
pub fn session_uid(mode: Mode, started_at: i64) -> String {
    let kind = match mode {
        Mode::Focus => 'f',
        Mode::ShortBreak => 's',
        Mode::LongBreak => 'l',
    };
    format!("{kind}-{started_at}")
}

pub(super) fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Focus => "focus",
        Mode::ShortBreak => "short_break",
        Mode::LongBreak => "long_break",
    }
}

pub(super) fn parse_mode(s: &str) -> Mode {
    match s {
        "short_break" => Mode::ShortBreak,
        "long_break" => Mode::LongBreak,
        _ => Mode::Focus,
    }
}

fn from_row(r: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: r.get(0)?,
        mode: parse_mode(&r.get::<_, String>(1)?),
        started_at: r.get(2)?,
        ended_at: r.get(3)?,
        duration_ms: r.get::<_, i64>(4)?.max(0) as u64,
        task: r.get(5)?,
        completed: r.get(6)?,
    })
}

const COLUMNS: &str = "id, mode, started_at, ended_at, duration_ms, task, completed";

impl Db {
    pub fn insert_session(&self, r: &SessionRecord) -> Result<i64> {
        let uid = session_uid(r.mode, r.started_at);
        let rev = Self::next_rev(&self.conn).map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO sessions (mode, started_at, ended_at, duration_ms, task, completed, uid, rev)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![mode_str(r.mode), r.started_at, r.ended_at, r.duration_ms as i64, r.task, r.completed, uid, rev],
            )
            .map_err(|e| e.to_string())?;
        self.conn
            .query_row("SELECT id FROM sessions WHERE uid = ?1", params![uid], |r| r.get(0))
            .map_err(|e| e.to_string())
    }

    /// Newest sessions first, older than `before` (UTC ms) when given.
    pub fn list_sessions(&self, before: Option<i64>, limit: u32) -> Result<Vec<Session>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM sessions WHERE started_at < ?1
             ORDER BY started_at DESC, id DESC LIMIT ?2"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![before.unwrap_or(i64::MAX), limit], from_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
    }

    /// Every session, oldest first (for export and backup).
    pub fn all_sessions(&self) -> Result<Vec<Session>> {
        let sql = format!("SELECT {COLUMNS} FROM sessions ORDER BY started_at, id");
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], from_row).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
    }

    /// Delete a session and remember the deletion for other devices.
    pub fn delete_session(&mut self, id: i64, now: i64) -> Result<bool> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let uid: Option<String> = tx
            .query_row("SELECT uid FROM sessions WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(uid) = uid else { return Ok(false) };
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
        let rev = Self::next_rev(&tx).map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO deleted_sessions (uid, deleted_at, rev, src) VALUES (?1, ?2, ?3, '')
             ON CONFLICT(uid) DO UPDATE SET deleted_at = excluded.deleted_at, rev = excluded.rev, src = ''",
            params![uid, now, rev],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// Total focus time (ms) for each [from, to) range of start times.
    /// Breaks never count; stopped sessions count for their actual length.
    pub fn focus_totals(&self, ranges: &[(i64, i64)]) -> Result<Vec<u64>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT COALESCE(SUM(duration_ms), 0) FROM sessions
                 WHERE mode = 'focus' AND started_at >= ?1 AND started_at < ?2",
            )
            .map_err(|e| e.to_string())?;
        ranges
            .iter()
            .map(|(from, to)| {
                stmt.query_row(params![from, to], |r| r.get::<_, i64>(0))
                    .map(|v| v.max(0) as u64)
                    .map_err(|e| e.to_string())
            })
            .collect()
    }

    /// Distinct task names, most recently used first.
    pub fn recent_tasks(&self, limit: u32) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT task FROM sessions WHERE task <> ''
                 GROUP BY task ORDER BY MAX(started_at) DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![limit], |r| r.get(0)).map_err(|e| e.to_string())?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())
    }

    /// Add sessions from a backup, skipping ones already present
    /// (same start time and type). Restoring a backup brings back sessions
    /// deleted since. Returns how many were added.
    pub fn merge_sessions(&mut self, sessions: &[Session]) -> Result<usize> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let mut added = 0;
        {
            let mut exists = tx
                .prepare("SELECT 1 FROM sessions WHERE uid = ?1")
                .map_err(|e| e.to_string())?;
            let mut insert = tx
                .prepare(
                    "INSERT INTO sessions (mode, started_at, ended_at, duration_ms, task, completed, uid, rev)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .map_err(|e| e.to_string())?;
            let mut undelete = tx.prepare("DELETE FROM deleted_sessions WHERE uid = ?1").map_err(|e| e.to_string())?;
            for s in sessions {
                let uid = session_uid(s.mode, s.started_at);
                if exists.exists(params![uid]).map_err(|e| e.to_string())? {
                    continue;
                }
                let task: String = s.task.chars().take(120).collect();
                let rev = Self::next_rev(&tx).map_err(|e| e.to_string())?;
                insert
                    .execute(params![
                        mode_str(s.mode),
                        s.started_at,
                        s.ended_at.max(s.started_at),
                        s.duration_ms as i64,
                        task,
                        s.completed,
                        uid,
                        rev
                    ])
                    .map_err(|e| e.to_string())?;
                undelete.execute(params![uid]).map_err(|e| e.to_string())?;
                added += 1;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    fn record(mode: Mode, started_at: i64, minutes: u64, task: &str, completed: bool) -> SessionRecord {
        SessionRecord {
            mode,
            started_at,
            ended_at: started_at + (minutes * MIN) as i64,
            duration_ms: minutes * MIN,
            task: task.into(),
            completed,
        }
    }

    #[test]
    fn insert_list_and_delete() {
        let mut db = Db::open_in_memory().unwrap();
        let a = db.insert_session(&record(Mode::Focus, 1_000, 25, "A", true)).unwrap();
        db.insert_session(&record(Mode::ShortBreak, 2_000, 5, "", true)).unwrap();
        db.insert_session(&record(Mode::Focus, 3_000, 12, "B", false)).unwrap();

        let all = db.list_sessions(None, 10).unwrap();
        assert_eq!(all.iter().map(|s| s.started_at).collect::<Vec<_>>(), vec![3_000, 2_000, 1_000]);
        assert_eq!(all[0].mode, Mode::Focus);
        assert!(!all[0].completed);
        assert_eq!(all[1].mode, Mode::ShortBreak);

        let older = db.list_sessions(Some(2_000), 10).unwrap();
        assert_eq!(older.len(), 1);
        assert_eq!(older[0].task, "A");

        assert!(db.delete_session(a, 5_000).unwrap());
        assert!(!db.delete_session(a, 5_000).unwrap());
        assert_eq!(db.all_sessions().unwrap().len(), 2);
    }

    #[test]
    fn focus_totals_count_only_focus_and_actual_minutes() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session(&record(Mode::Focus, 100, 25, "", true)).unwrap();
        db.insert_session(&record(Mode::Focus, 200, 7, "", false)).unwrap(); // stopped early
        db.insert_session(&record(Mode::LongBreak, 300, 15, "", true)).unwrap(); // never counts
        db.insert_session(&record(Mode::Focus, 1_000, 25, "", true)).unwrap(); // other range
        let totals = db.focus_totals(&[(0, 1_000), (1_000, 2_000), (5_000, 6_000)]).unwrap();
        assert_eq!(totals, vec![32 * MIN, 25 * MIN, 0]);
    }

    #[test]
    fn recent_tasks_are_distinct_and_newest_first() {
        let db = Db::open_in_memory().unwrap();
        for (t, task) in [(1, "Read"), (2, "Write"), (3, ""), (4, "Read"), (5, "Plan")] {
            db.insert_session(&record(Mode::Focus, t, 25, task, true)).unwrap();
        }
        assert_eq!(db.recent_tasks(10).unwrap(), vec!["Plan", "Read", "Write"]);
        assert_eq!(db.recent_tasks(1).unwrap(), vec!["Plan"]);
    }

    #[test]
    fn merging_a_backup_skips_duplicates() {
        let mut db = Db::open_in_memory().unwrap();
        db.insert_session(&record(Mode::Focus, 1_000, 25, "A", true)).unwrap();
        let backup = vec![
            Session { id: 9, mode: Mode::Focus, started_at: 1_000, ended_at: 2_000, duration_ms: 1, task: "A".into(), completed: true },
            Session { id: 10, mode: Mode::ShortBreak, started_at: 1_000, ended_at: 2_000, duration_ms: 5 * MIN, task: String::new(), completed: true },
            Session { id: 11, mode: Mode::Focus, started_at: 9_000, ended_at: 9_500, duration_ms: 3 * MIN, task: "C".into(), completed: false },
        ];
        assert_eq!(db.merge_sessions(&backup).unwrap(), 2);
        assert_eq!(db.merge_sessions(&backup).unwrap(), 0, "importing twice adds nothing");
        assert_eq!(db.all_sessions().unwrap().len(), 3);
    }
}
