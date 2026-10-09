//! Local SQLite storage. The database file lives in the user's AppData folder
//! (`%APPDATA%\io.github.cylinder303.awan\awan.db`), outside the install
//! directory, so updates and reinstalls never touch it.

mod migrations;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

pub type Result<T> = std::result::Result<T, String>;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory().map_err(|e| e.to_string())?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(|e| e.to_string())?;
        let mut db = Self { conn };
        migrations::run(&mut db.conn)?;
        Ok(db)
    }

    /// Read a JSON value stored under `key`.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        match raw {
            Some(s) => serde_json::from_str(&s).map(Some).map_err(|e| e.to_string()),
            None => Ok(None),
        }
    }

    /// Store `value` as JSON under `key`.
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.conn
            .execute(
                "INSERT INTO kv (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get::<u32>("x").unwrap(), None);
        db.set("x", &42u32).unwrap();
        db.set("x", &43u32).unwrap();
        assert_eq!(db.get::<u32>("x").unwrap(), Some(43));
    }

    #[test]
    fn reopening_keeps_data_and_migrations_are_idempotent() {
        let dir = std::env::temp_dir().join(format!("awan-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("awan.db");
        {
            let db = Db::open(&path).unwrap();
            db.set("k", &"v").unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.get::<String>("k").unwrap().as_deref(), Some("v"));
        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }
}
