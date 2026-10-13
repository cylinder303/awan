//! Syncing history, goals and shared settings between a phone and a computer
//! on the same Wi-Fi, with no account and no server on the internet.
//!
//! The computer listens (host.rs); the phone connects to it now and then and
//! the two exchange what the other does not have yet (client.rs). They are
//! linked once with a code shown on the computer. See wire.rs for the
//! messages and crypto.rs for how they are protected.

pub mod app;
pub mod client;
pub mod crypto;
pub mod host;
pub mod wire;

use serde_json::Value;

use crate::db::Db;
use wire::{Changes, DeviceId, StampedSettings};

/// This device as others see it.
#[derive(Debug, Clone)]
pub struct Identity {
    pub id: DeviceId,
    pub name: String,
}

/// Where sync reads and writes this device's data.
pub trait Store: Send + Sync + 'static {
    /// What changed here after `since`, leaving out what came from `skip`.
    fn collect(&self, since: i64, skip: &str) -> Result<Changes, String>;
    /// Take in another device's changes. Returns whether anything changed.
    fn apply(&self, changes: &Changes, from: &str) -> Result<bool, String>;
    /// Sync's own small records (links, device list).
    fn load(&self, key: &str) -> Option<Value>;
    fn save(&self, key: &str, value: Value);
}

/// The part of `Store::collect` that reads the database.
pub fn collect_from(db: &Db, since: i64, skip: &str, settings: Option<StampedSettings>) -> Result<Changes, String> {
    let rev = db.current_rev()?;
    let (sessions, deleted) = db.changes_since(since, skip)?;
    Ok(Changes { sessions, deleted, week_goals: db.stamped_week_goals()?, settings, rev })
}

/// The part of `Store::apply` that writes the database. Returns whether the
/// history or the goals changed.
pub fn apply_to(db: &mut Db, changes: &Changes, from: &str) -> Result<bool, String> {
    let history = db.apply_remote(&changes.sessions, &changes.deleted, from)?;
    let goals = db.merge_stamped_goals(&changes.week_goals)?;
    Ok(history || goals)
}

#[cfg(test)]
pub(crate) mod testing {
    //! A store backed by an in-memory database, for tests.

    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;
    use crate::settings::Shared;

    pub struct MemStore {
        pub db: Mutex<Db>,
        pub settings: Mutex<Option<StampedSettings>>,
        kv: Mutex<HashMap<String, Value>>,
    }

    impl MemStore {
        pub fn new() -> Self {
            Self { db: Mutex::new(Db::open_in_memory().unwrap()), settings: Mutex::new(None), kv: Mutex::default() }
        }

        pub fn uids(&self) -> Vec<String> {
            let (sessions, _) = self.db.lock().unwrap().changes_since(0, "-").unwrap();
            let mut v: Vec<String> = sessions.into_iter().map(|s| s.uid).collect();
            v.sort();
            v
        }

        pub fn set_settings(&self, changed_at: i64, values: Shared) {
            *self.settings.lock().unwrap() = Some(StampedSettings { changed_at, values });
        }
    }

    impl Store for MemStore {
        fn collect(&self, since: i64, skip: &str) -> Result<Changes, String> {
            let settings = self.settings.lock().unwrap().clone();
            collect_from(&self.db.lock().unwrap(), since, skip, settings)
        }

        fn apply(&self, changes: &Changes, from: &str) -> Result<bool, String> {
            let mut changed = apply_to(&mut self.db.lock().unwrap(), changes, from)?;
            let mut mine = self.settings.lock().unwrap();
            if let Some(theirs) = &changes.settings {
                if mine.as_ref().map_or(true, |m| theirs.changed_at > m.changed_at) {
                    *mine = Some(theirs.clone());
                    changed = true;
                }
            }
            Ok(changed)
        }

        fn load(&self, key: &str) -> Option<Value> {
            self.kv.lock().unwrap().get(key).cloned()
        }

        fn save(&self, key: &str, value: Value) {
            self.kv.lock().unwrap().insert(key.to_string(), value);
        }
    }
}
