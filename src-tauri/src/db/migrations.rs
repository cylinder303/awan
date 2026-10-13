//! Schema migrations, tracked with `PRAGMA user_version`.
//! Append new steps to `MIGRATIONS`; never edit a step that has shipped.

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // 1: key/value store for settings and the in-progress session.
    "CREATE TABLE kv (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );",
    // 2: session history. Times are UTC milliseconds since the Unix epoch.
    "CREATE TABLE sessions (
        id          INTEGER PRIMARY KEY,
        mode        TEXT    NOT NULL CHECK (mode IN ('focus', 'short_break', 'long_break')),
        started_at  INTEGER NOT NULL,
        ended_at    INTEGER NOT NULL,
        duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
        task        TEXT    NOT NULL DEFAULT '',
        completed   INTEGER NOT NULL CHECK (completed IN (0, 1))
    );
    CREATE INDEX sessions_started_at ON sessions (started_at);",
    // 3: weekly goal in effect for each week, so past results keep the goal
    // they were measured against. week_key is the local start date of the week.
    "CREATE TABLE week_goals (
        week_key     TEXT    PRIMARY KEY,
        goal_minutes INTEGER NOT NULL CHECK (goal_minutes > 0)
    );",
    // 4: syncing between devices.
    //  - uid names a session the same way on every device: its type and
    //    start time (what backups were already matched by). Two sessions
    //    sharing both get the row id appended, so existing rows stay.
    //  - rev orders local changes, so a sync sends only what is new; src is
    //    the device a row came from ('' = this one), so it is not sent back.
    //  - deleted_sessions remembers deletions, so they reach other devices
    //    and a deleted session does not come back from them.
    //  - week_goals.updated_at lets the newest change to a week's goal win.
    "ALTER TABLE sessions ADD COLUMN uid TEXT;
    ALTER TABLE sessions ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE sessions ADD COLUMN src TEXT NOT NULL DEFAULT '';
    UPDATE sessions SET uid =
        (CASE mode WHEN 'focus' THEN 'f' WHEN 'short_break' THEN 's' ELSE 'l' END) || '-' || started_at;
    UPDATE sessions SET uid = uid || '#' || id
        WHERE id NOT IN (SELECT MIN(id) FROM sessions GROUP BY uid);
    UPDATE sessions SET rev = id;
    CREATE UNIQUE INDEX sessions_uid ON sessions (uid);
    CREATE INDEX sessions_rev ON sessions (rev);
    CREATE TABLE deleted_sessions (
        uid        TEXT    PRIMARY KEY,
        deleted_at INTEGER NOT NULL,
        rev        INTEGER NOT NULL,
        src        TEXT    NOT NULL DEFAULT ''
    );
    CREATE INDEX deleted_sessions_rev ON deleted_sessions (rev);
    CREATE TABLE counters (
        name  TEXT    PRIMARY KEY,
        value INTEGER NOT NULL
    );
    INSERT INTO counters (name, value) VALUES ('rev', (SELECT COALESCE(MAX(id), 0) FROM sessions));
    ALTER TABLE week_goals ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;",
];

pub fn run(conn: &mut Connection) -> Result<(), String> {
    let current: usize = conn
        .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())? as usize;
    if current > MIGRATIONS.len() {
        return Err(format!(
            "database version {current} is newer than this app supports ({})",
            MIGRATIONS.len()
        ));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql).map_err(|e| e.to_string())?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}
