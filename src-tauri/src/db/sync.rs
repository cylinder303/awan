//! What other devices need to stay in step: sessions added and deleted since
//! a point in the local change order, and the weekly goals with their dates.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::sessions::{mode_str, parse_mode};
use super::{Db, Result};
use crate::timer::Mode;

/// A session as it travels between devices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncSession {
    pub uid: String,
    pub mode: Mode,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_ms: u64,
    #[serde(default)]
    pub task: String,
    pub completed: bool,
}

/// A deleted session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tombstone {
    pub uid: String,
    pub deleted_at: i64,
}

/// A week's goal and when it was last changed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StampedGoal {
    pub week_key: String,
    pub goal_minutes: u32,
    pub updated_at: i64,
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

impl Db {
    /// The latest point in the local change order.
    pub fn current_rev(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT value FROM counters WHERE name = 'rev'", [], |r| r.get(0))
            .map_err(err)
    }

    /// Sessions added and deleted after `since`, leaving out those that came
    /// from device `skip_src` (it already has them).
    pub fn changes_since(&self, since: i64, skip_src: &str) -> Result<(Vec<SyncSession>, Vec<Tombstone>)> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT uid, mode, started_at, ended_at, duration_ms, task, completed FROM sessions
                 WHERE rev > ?1 AND src <> ?2 ORDER BY rev",
            )
            .map_err(err)?;
        let sessions = stmt
            .query_map(params![since, skip_src], |r| {
                Ok(SyncSession {
                    uid: r.get(0)?,
                    mode: parse_mode(&r.get::<_, String>(1)?),
                    started_at: r.get(2)?,
                    ended_at: r.get(3)?,
                    duration_ms: r.get::<_, i64>(4)?.max(0) as u64,
                    task: r.get(5)?,
                    completed: r.get(6)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        let mut stmt = self
            .conn
            .prepare("SELECT uid, deleted_at FROM deleted_sessions WHERE rev > ?1 AND src <> ?2 ORDER BY rev")
            .map_err(err)?;
        let deleted = stmt
            .query_map(params![since, skip_src], |r| Ok(Tombstone { uid: r.get(0)?, deleted_at: r.get(1)? }))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok((sessions, deleted))
    }

    /// Take in sessions and deletions from device `src`. Deletions win over
    /// sessions; anything already known is skipped. Returns whether the
    /// history changed.
    pub fn apply_remote(&mut self, sessions: &[SyncSession], deleted: &[Tombstone], src: &str) -> Result<bool> {
        let tx = self.conn.transaction().map_err(err)?;
        let mut changed = false;
        {
            let mut known_deletion = tx.prepare("SELECT 1 FROM deleted_sessions WHERE uid = ?1").map_err(err)?;
            let mut add_deletion = tx
                .prepare("INSERT INTO deleted_sessions (uid, deleted_at, rev, src) VALUES (?1, ?2, ?3, ?4)")
                .map_err(err)?;
            let mut remove = tx.prepare("DELETE FROM sessions WHERE uid = ?1").map_err(err)?;
            for d in deleted {
                if known_deletion.exists(params![d.uid]).map_err(err)? {
                    continue;
                }
                let rev = Self::next_rev(&tx).map_err(err)?;
                add_deletion.execute(params![d.uid, d.deleted_at, rev, src]).map_err(err)?;
                changed |= remove.execute(params![d.uid]).map_err(err)? > 0;
            }

            let mut known = tx.prepare("SELECT 1 FROM sessions WHERE uid = ?1").map_err(err)?;
            let mut insert = tx
                .prepare(
                    "INSERT INTO sessions (mode, started_at, ended_at, duration_ms, task, completed, uid, rev, src)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(err)?;
            for s in sessions {
                if known.exists(params![s.uid]).map_err(err)? || known_deletion.exists(params![s.uid]).map_err(err)? {
                    continue;
                }
                let task: String = s.task.chars().take(120).collect();
                let rev = Self::next_rev(&tx).map_err(err)?;
                insert
                    .execute(params![
                        mode_str(s.mode),
                        s.started_at,
                        s.ended_at.max(s.started_at),
                        s.duration_ms as i64,
                        task,
                        s.completed,
                        s.uid,
                        rev,
                        src
                    ])
                    .map_err(err)?;
                changed = true;
            }
        }
        tx.commit().map_err(err)?;
        Ok(changed)
    }

    pub fn stamped_week_goals(&self) -> Result<Vec<StampedGoal>> {
        let mut stmt = self
            .conn
            .prepare("SELECT week_key, goal_minutes, updated_at FROM week_goals ORDER BY week_key")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| Ok(StampedGoal { week_key: r.get(0)?, goal_minutes: r.get(1)?, updated_at: r.get(2)? }))
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Take in weekly goals from another device; per week, the most recent
    /// change wins. Returns whether anything changed.
    pub fn merge_stamped_goals(&self, goals: &[StampedGoal]) -> Result<bool> {
        let mut changed = false;
        for g in goals {
            let current: Option<(u32, i64)> = self
                .conn
                .query_row(
                    "SELECT goal_minutes, updated_at FROM week_goals WHERE week_key = ?1",
                    params![g.week_key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(err)?;
            let newer = match current {
                None => true,
                Some((minutes, at)) => g.updated_at > at && minutes != g.goal_minutes,
            };
            if newer {
                self.conn
                    .execute(
                        "INSERT INTO week_goals (week_key, goal_minutes, updated_at) VALUES (?1, ?2, ?3)
                         ON CONFLICT(week_key) DO UPDATE SET goal_minutes = excluded.goal_minutes, updated_at = excluded.updated_at",
                        params![g.week_key, g.goal_minutes.max(1), g.updated_at],
                    )
                    .map_err(err)?;
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Session;
    use crate::timer::SessionRecord;

    const MIN: u64 = 60_000;

    fn record(mode: Mode, started_at: i64) -> SessionRecord {
        SessionRecord {
            mode,
            started_at,
            ended_at: started_at + 25 * MIN as i64,
            duration_ms: 25 * MIN,
            task: "Read".into(),
            completed: true,
        }
    }

    /// Leave nothing out when listing changes.
    const NOBODY: &str = "-";

    fn uids(db: &Db) -> Vec<String> {
        let (s, _) = db.changes_since(0, NOBODY).unwrap();
        let mut v: Vec<String> = s.into_iter().map(|s| s.uid).collect();
        v.sort();
        v
    }

    /// Run one exchange the way the sync does: `a` sends what is new since
    /// `a_sent`, `b` answers with what is new since `b_seen`.
    fn exchange(a: &mut Db, a_id: &str, a_sent: &mut i64, b: &mut Db, b_id: &str, b_seen: &mut i64) {
        let a_rev = a.current_rev().unwrap();
        let (s, d) = a.changes_since(*a_sent, b_id).unwrap();
        b.apply_remote(&s, &d, a_id).unwrap();
        let b_rev = b.current_rev().unwrap();
        let (s, d) = b.changes_since(*b_seen, a_id).unwrap();
        a.apply_remote(&s, &d, b_id).unwrap();
        *a_sent = a_rev;
        *b_seen = b_rev;
    }

    #[test]
    fn two_devices_end_up_with_the_same_history() {
        let (mut pc, mut phone) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        let (mut sent, mut seen) = (0, 0);
        pc.insert_session(&record(Mode::Focus, 1_000)).unwrap();
        pc.insert_session(&record(Mode::ShortBreak, 2_000)).unwrap();
        let gone = phone.insert_session(&record(Mode::Focus, 3_000)).unwrap();
        phone.insert_session(&record(Mode::Focus, 4_000)).unwrap();

        exchange(&mut phone, "phone", &mut sent, &mut pc, "pc", &mut seen);
        assert_eq!(uids(&pc), vec!["f-1000", "f-3000", "f-4000", "s-2000"]);
        assert_eq!(uids(&phone), uids(&pc));

        // Nothing new: nothing travels, and nothing bounces back.
        assert_eq!(phone.changes_since(sent, "pc").unwrap(), (vec![], vec![]));
        assert_eq!(pc.changes_since(seen, "phone").unwrap(), (vec![], vec![]));

        // A deletion on the phone reaches the computer and stays deleted.
        phone.delete_session(gone, 9_000).unwrap();
        exchange(&mut phone, "phone", &mut sent, &mut pc, "pc", &mut seen);
        assert_eq!(uids(&pc), vec!["f-1000", "f-4000", "s-2000"]);
        let readd = SyncSession {
            uid: "f-3000".into(),
            mode: Mode::Focus,
            started_at: 3_000,
            ended_at: 4_000,
            duration_ms: 1,
            task: String::new(),
            completed: true,
        };
        assert!(!pc.apply_remote(&[readd], &[], "old-phone").unwrap(), "a deleted session does not come back");
    }

    #[test]
    fn deletions_on_both_sides_and_repeats_are_harmless() {
        let (mut a, mut b) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        let id_a = a.insert_session(&record(Mode::Focus, 1_000)).unwrap();
        let id_b = b.insert_session(&record(Mode::Focus, 1_000)).unwrap(); // same session (from a backup)
        a.delete_session(id_a, 5).unwrap();
        b.delete_session(id_b, 6).unwrap();
        let (s, d) = a.changes_since(0, NOBODY).unwrap();
        assert_eq!(d.len(), 1);
        assert!(!b.apply_remote(&s, &d, "a").unwrap());
        assert!(!b.apply_remote(&s, &d, "a").unwrap());
        assert!(uids(&b).is_empty());
    }

    #[test]
    fn newest_week_goal_wins() {
        let (pc, phone) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        pc.set_week_goal("2026-10-10", 600, 100).unwrap();
        phone.set_week_goal("2026-10-10", 900, 200).unwrap();
        phone.set_week_goal("2026-10-03", 300, 50).unwrap();
        assert!(pc.merge_stamped_goals(&phone.stamped_week_goals().unwrap()).unwrap());
        assert!(!phone.merge_stamped_goals(&pc.stamped_week_goals().unwrap()).unwrap(), "phone was already newest");
        assert_eq!(pc.stamped_week_goals().unwrap(), phone.stamped_week_goals().unwrap());
        assert_eq!(pc.week_goals().unwrap()[1].goal_minutes, 900);
        // Saving the same goal again does not make it newer.
        pc.set_week_goal("2026-10-10", 900, 999).unwrap();
        assert_eq!(pc.stamped_week_goals().unwrap()[1].updated_at, 200);
    }

    #[test]
    fn restoring_a_backup_brings_back_a_deleted_session() {
        let mut db = Db::open_in_memory().unwrap();
        let id = db.insert_session(&record(Mode::Focus, 1_000)).unwrap();
        let backup = db.all_sessions().unwrap();
        db.delete_session(id, 2).unwrap();
        assert_eq!(db.merge_sessions(&backup).unwrap(), 1);
        let (sessions, deleted) = db.changes_since(0, NOBODY).unwrap();
        assert_eq!((sessions.len(), deleted.len()), (1, 0));
        let _: Vec<Session> = backup;
    }

    #[test]
    fn existing_history_gets_ids_when_upgrading() {
        let dir = std::env::temp_dir().join(format!("awan-upgrade-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("awan.db");
        {
            // A database as version 3 left it, with two sessions sharing a start.
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE sessions (id INTEGER PRIMARY KEY, mode TEXT NOT NULL, started_at INTEGER NOT NULL,
                   ended_at INTEGER NOT NULL, duration_ms INTEGER NOT NULL, task TEXT NOT NULL DEFAULT '',
                   completed INTEGER NOT NULL);
                 CREATE INDEX sessions_started_at ON sessions (started_at);
                 CREATE TABLE week_goals (week_key TEXT PRIMARY KEY, goal_minutes INTEGER NOT NULL);
                 INSERT INTO sessions (mode, started_at, ended_at, duration_ms, task, completed) VALUES
                   ('focus', 10, 20, 10, 'a', 1), ('focus', 10, 30, 20, 'b', 0), ('long_break', 40, 50, 10, '', 1);
                 INSERT INTO week_goals VALUES ('2026-10-03', 600);
                 PRAGMA user_version = 3;",
            )
            .unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(uids(&db), vec!["f-10", "f-10#2", "l-40"]);
        assert_eq!(db.current_rev().unwrap(), 3);
        db.insert_session(&record(Mode::Focus, 60)).unwrap();
        assert_eq!(db.current_rev().unwrap(), 4);
        assert_eq!(db.stamped_week_goals().unwrap()[0].updated_at, 0);
        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }
}
