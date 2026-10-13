//! Sync inside the running app: the computer's host, the phone's background
//! sync, and what the settings page shows.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use super::client::{self, Failure};
use super::host::{self, Host, HostEvent};
use super::wire::{self, Changes, Question, StampedSettings};
use super::{apply_to, collect_from, crypto, Identity, Store};
use crate::{clock, lock, AppState, HISTORY_EVENT};

/// The settings page listens for this to refresh the sync section.
pub const SYNC_EVENT: &str = "sync://changed";
/// A phone was just linked (payload: its name).
pub const PAIRED_EVENT: &str = "sync://paired";
/// The pairing code stopped working.
pub const PAIRING_ENDED_EVENT: &str = "sync://pairing-ended";
/// When the shared settings last changed on this device (or were taken from another).
pub const SHARED_AT_KEY: &str = "settings.sharedAt";
const ID_KEY: &str = "sync.id";
const NAME_KEY: &str = "sync.name";
/// How often a phone syncs while the app is open.
const PERIOD: Duration = Duration::from_secs(60);
/// Changes are sent this long after the last one, so a burst goes in one sync.
const SETTLE: Duration = Duration::from_secs(3);
const FIND_WAIT: Duration = Duration::from_millis(1500);

pub struct AppStore(pub AppHandle);

impl Store for AppStore {
    fn collect(&self, since: i64, skip: &str) -> Result<Changes, String> {
        let settings = shared_settings(&self.0);
        let state = self.0.state::<AppState>();
        let db = lock(&state.db);
        collect_from(&db, since, skip, Some(settings))
    }

    fn apply(&self, changes: &Changes, from: &str) -> Result<bool, String> {
        let data = {
            let state = self.0.state::<AppState>();
            let mut db = lock(&state.db);
            apply_to(&mut db, changes, from)?
        };
        let settings = changes.settings.clone().is_some_and(|s| crate::adopt_shared_settings(&self.0, s));
        if data {
            let _ = self.0.emit(HISTORY_EVENT, ());
        }
        Ok(data || settings)
    }

    fn load(&self, key: &str) -> Option<Value> {
        lock(&self.0.state::<AppState>().db).get::<Value>(key).ok().flatten().filter(|v| !v.is_null())
    }

    fn save(&self, key: &str, value: Value) {
        let _ = lock(&self.0.state::<AppState>().db).set(key, &value);
    }
}

fn shared_settings(app: &AppHandle) -> StampedSettings {
    let state = app.state::<AppState>();
    let changed_at = lock(&state.db).get::<i64>(SHARED_AT_KEY).ok().flatten().unwrap_or(0);
    let values = lock(&state.settings).shared();
    StampedSettings { changed_at, values }
}

fn identity(store: &dyn Store) -> Identity {
    let id = store
        .load(ID_KEY)
        .and_then(|v| v.as_str().and_then(crypto::from_hex::<16>))
        .unwrap_or_else(|| {
            let id: [u8; 16] = crypto::random();
            store.save(ID_KEY, Value::String(crypto::to_hex(&id)));
            id
        });
    let name = if cfg!(desktop) {
        std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "Awan".into())
    } else {
        store.load(NAME_KEY).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "Android".into())
    };
    Identity { id, name }
}

// ------------------------------------------------------------- status

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "idle", "syncing", "ok" or "error".
    pub state: &'static str,
    pub error: Option<String>,
}

static STATUS: Mutex<Status> = Mutex::new(Status { state: "idle", error: None });

fn set_status(state: &'static str, error: Option<String>) {
    *lock(&STATUS) = Status { state, error };
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub last_sync_at: Option<i64>,
}

/// What the settings page shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// "computer" (others link to it) or "phone" (links to a computer).
    pub role: &'static str,
    /// Phone: the linked computer.
    pub link: Option<Peer>,
    /// Computer: the linked phones.
    pub devices: Vec<Peer>,
    pub status: Status,
}

pub fn info(app: &AppHandle) -> Info {
    let store = AppStore(app.clone());
    Info {
        role: if cfg!(desktop) { "computer" } else { "phone" },
        link: client::link(&store).map(|l| Peer { id: l.id, name: l.name, last_sync_at: l.last_sync_at }),
        devices: host::devices(&store)
            .into_iter()
            .map(|d| Peer { id: d.id, name: d.name, last_sync_at: d.last_sync_at })
            .collect(),
        status: lock(&STATUS).clone(),
    }
}

fn notify(app: &AppHandle) {
    let _ = app.emit(SYNC_EVENT, info(app));
}

/// At startup: a computer with linked phones listens for them; a phone
/// starts syncing in the background.
pub fn start(app: &AppHandle) {
    if cfg!(desktop) {
        if !host::devices(&AppStore(app.clone())).is_empty() {
            let _ = ensure_host(app);
        }
    } else {
        spawn_agent(app.clone());
    }
}

/// Something changed here that the other devices should get soon.
pub fn local_change() {
    if cfg!(mobile) {
        kick(SETTLE);
    }
}

// ------------------------------------------------------------ computer

static HOST: Mutex<Option<Host>> = Mutex::new(None);

fn ensure_host(app: &AppHandle) -> Result<Host, String> {
    let mut slot = lock(&HOST);
    if let Some(host) = slot.as_ref() {
        return Ok(host.clone());
    }
    let store: Arc<dyn Store> = Arc::new(AppStore(app.clone()));
    let me = identity(&*store);
    let any = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), wire::PORT);
    let events = app.clone();
    let host = Host::start(store, me, any, Some(any), clock::wall_ms, move |event| {
        match event {
            HostEvent::Paired { name } => {
                let _ = events.emit(PAIRED_EVENT, name);
            }
            HostEvent::PairingEnded => {
                let _ = events.emit(PAIRING_ENDED_EVENT, ());
            }
            HostEvent::Synced { .. } => {}
        }
        notify(&events);
    })
    .map_err(|e| e.to_string())?;
    *slot = Some(host.clone());
    Ok(host)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pairing {
    pub code: String,
    pub name: String,
    /// This computer's address, for typing in if the phone cannot find it.
    pub address: Option<String>,
    pub expires_in_ms: u64,
}

pub fn begin_pairing(app: &AppHandle) -> Result<Pairing, String> {
    let host = ensure_host(app)?;
    let code = host.begin_pairing();
    let address = client::local_ip().map(|ip| {
        if host.port() == wire::PORT { ip.to_string() } else { SocketAddr::new(ip, host.port()).to_string() }
    });
    Ok(Pairing { code, name: host.name().to_string(), address, expires_in_ms: host::PAIRING_TIME.as_millis() as u64 })
}

pub fn end_pairing() {
    if let Some(host) = lock(&HOST).as_ref() {
        host.end_pairing();
    }
}

pub fn remove_device(app: &AppHandle, id: &str) {
    let store = AppStore(app.clone());
    let mut list = host::devices(&store);
    list.retain(|d| d.id != id);
    host::save_devices(&store, &list);
    notify(app);
}

// --------------------------------------------------------------- phone

/// When the next sync is due early (a change, or the user asked).
static DUE: (Mutex<Option<Instant>>, Condvar) = (Mutex::new(None), Condvar::new());

/// Sync within `delay` (sooner if already due sooner).
pub fn kick(delay: Duration) {
    let (due, cv) = &DUE;
    let at = Instant::now() + delay;
    let mut due = lock(due);
    *due = Some(due.map_or(at, |d| d.min(at)));
    cv.notify_one();
}

fn spawn_agent(app: AppHandle) {
    std::thread::spawn(move || {
        let mut next = Instant::now();
        loop {
            {
                let (due, cv) = &DUE;
                let mut due = lock(due);
                loop {
                    let now = Instant::now();
                    let target = due.map_or(next, |d| d.min(next));
                    if now >= target {
                        break;
                    }
                    due = cv.wait_timeout(due, target - now).unwrap_or_else(|e| e.into_inner()).0;
                }
                *due = None;
            }
            sync_once(&app);
            next = Instant::now() + PERIOD;
        }
    });
}

fn sync_once(app: &AppHandle) {
    let store = AppStore(app.clone());
    let Some(mut link) = client::link(&store) else { return };
    set_status("syncing", None);
    notify(app);
    let me = identity(&store);
    match reach_and_sync(&store, &me, &mut link) {
        Ok(_) => {
            client::save_link(&store, Some(&link));
            set_status("ok", None);
        }
        Err(failure) => set_status("error", Some(failure.code())),
    }
    notify(app);
}

/// Sync at the computer's last address, or find where it is now.
fn reach_and_sync(store: &AppStore, me: &Identity, link: &mut client::Link) -> Result<bool, Failure> {
    let now = clock::wall_ms();
    if let Some(addr) = client::parse_addr(&link.addr) {
        match client::sync(store, me, link, addr, now) {
            Err(Failure::Unreachable) => {}
            done => return done,
        }
    }
    let id = crypto::from_hex::<16>(&link.id).ok_or(Failure::Invalid)?;
    let found = client::find(Question::Device(id), &client::broadcast_targets(), FIND_WAIT)?;
    let (_, addr) = found.into_iter().next().ok_or(Failure::Unreachable)?;
    client::sync(store, me, link, addr, now)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub name: String,
    pub addr: String,
}

/// Computers on this network showing a pairing code.
pub fn find_computers() -> Result<Vec<Found>, String> {
    let found = client::find(Question::Pairing, &client::broadcast_targets(), FIND_WAIT).map_err(|f| f.code())?;
    Ok(found.into_iter().map(|(a, addr)| Found { name: a.name, addr: addr.to_string() }).collect())
}

/// Link with the computer at `addr` (as found, or typed in).
pub fn pair(app: &AppHandle, addr: &str, code: &str, name: &str) -> Result<(), String> {
    let addr = client::parse_addr(addr).ok_or("bad_address")?;
    let store = AppStore(app.clone());
    let name: String = name.trim().chars().take(60).collect();
    if !name.is_empty() {
        store.save(NAME_KEY, Value::String(name));
    }
    let me = identity(&store);
    let link = client::pair(addr, code, &me).map_err(|f| f.code())?;
    client::save_link(&store, Some(&link));
    set_status("idle", None);
    kick(Duration::ZERO);
    notify(app);
    Ok(())
}

pub fn unlink(app: &AppHandle) {
    client::save_link(&AppStore(app.clone()), None);
    set_status("idle", None);
    notify(app);
}
