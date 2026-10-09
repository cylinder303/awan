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
