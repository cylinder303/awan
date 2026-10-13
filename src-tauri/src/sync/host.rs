//! The computer's side: answers phones on the network, links new ones with
//! a pairing code, and exchanges changes with linked ones.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::crypto::{self, SecretKey};
use super::wire::{self, Answer, Hello, Kind, Question, SyncReply, SyncRequest, Welcome};
use super::{Identity, Store};

/// Linked phones, stored by the store under this key.
pub const DEVICES_KEY: &str = "sync.devices";
/// How long a pairing code works.
pub const PAIRING_TIME: Duration = Duration::from_secs(5 * 60);
/// Wrong codes allowed before the code stops working.
const MAX_TRIES: u32 = 5;
/// Phones served at the same time.
const MAX_CONNECTIONS: usize = 8;
const IO_TIMEOUT: Duration = Duration::from_secs(20);

/// A linked phone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Hex.
    pub id: String,
    pub name: String,
    /// The link key, hex.
    pub key: String,
    pub linked_at: i64,
    #[serde(default)]
    pub last_sync_at: Option<i64>,
}

pub fn devices(store: &dyn Store) -> Vec<Device> {
    store.load(DEVICES_KEY).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

pub fn save_devices(store: &dyn Store, list: &[Device]) {
    store.save(DEVICES_KEY, serde_json::to_value(list).unwrap_or_default());
}

/// Something the app shows.
#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Paired { name: String },
    /// A phone synced; `changed` when it brought something new.
    Synced { changed: bool },
    /// The pairing code expired or was guessed wrong too often.
    PairingEnded,
}

struct Pairing {
    code: String,
    until: Instant,
    tries: u32,
}

struct Inner {
    store: Arc<dyn Store>,
    me: Identity,
    port: u16,
    pairing: Mutex<Option<Pairing>>,
    on_event: Box<dyn Fn(HostEvent) + Send + Sync>,
    now: fn() -> i64,
    busy: AtomicUsize,
}

#[derive(Clone)]
pub struct Host {
    inner: Arc<Inner>,
}

impl Host {
    /// Listen on `tcp`, and answer discovery questions on `udp` if given (a
    /// phone can still link by typing the address if that port is taken).
    pub fn start(
        store: Arc<dyn Store>,
        me: Identity,
        tcp: SocketAddr,
        udp: Option<SocketAddr>,
        now: fn() -> i64,
        on_event: impl Fn(HostEvent) + Send + Sync + 'static,
    ) -> io::Result<Host> {
        let listener = TcpListener::bind(tcp).or_else(|_| TcpListener::bind(SocketAddr::new(tcp.ip(), 0)))?;
        let port = listener.local_addr()?.port();
        let host = Host {
            inner: Arc::new(Inner {
                store,
                me,
                port,
                pairing: Mutex::new(None),
                on_event: Box::new(on_event),
                now,
                busy: AtomicUsize::new(0),
            }),
        };
        let h = host.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if h.inner.busy.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                    h.inner.busy.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let h = h.clone();
                std::thread::spawn(move || {
                    let _ = h.serve(stream);
                    h.inner.busy.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        if let Some(udp) = udp.and_then(|a| UdpSocket::bind(a).ok()) {
            let h = host.clone();
            std::thread::spawn(move || h.answer_questions(udp));
        }
        Ok(host)
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    pub fn name(&self) -> &str {
        &self.inner.me.name
    }

    /// Show a new pairing code for the next few minutes.
    pub fn begin_pairing(&self) -> String {
        let code = crypto::new_code();
        *lock(&self.inner.pairing) = Some(Pairing { code: code.clone(), until: Instant::now() + PAIRING_TIME, tries: 0 });
        code
    }

    pub fn end_pairing(&self) {
        lock(&self.inner.pairing).take();
    }

    /// The code being shown, if it still works.
    fn pairing_code(&self) -> Option<String> {
        let mut pairing = lock(&self.inner.pairing);
        match pairing.as_ref() {
            Some(p) if Instant::now() < p.until => Some(p.code.clone()),
            Some(_) => {
                *pairing = None;
                drop(pairing);
                (self.inner.on_event)(HostEvent::PairingEnded);
                None
            }
            None => None,
        }
    }

    fn answer_questions(&self, socket: UdpSocket) {
        let mut buf = [0u8; 64];
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            let fits = match wire::decode_question(&buf[..n]) {
                Some(Question::Pairing) => self.pairing_code().is_some(),
                Some(Question::Device(id)) => id == self.inner.me.id,
                None => false,
            };
            if fits {
                let answer = Answer {
                    id: self.inner.me.id,
                    port: self.inner.port,
                    pairing: self.pairing_code().is_some(),
                    name: self.inner.me.name.clone(),
                };
                let _ = socket.send_to(&wire::encode_answer(&answer), from);
            }
        }
    }

    fn serve(&self, mut stream: TcpStream) -> io::Result<()> {
        stream.set_read_timeout(Some(IO_TIMEOUT))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;
        let (kind, body) = wire::read_frame(&mut stream)?;
        let (kind, reply) = match kind {
            Kind::PairHello => self.pair(&body),
            Kind::SyncRequest => self.sync(&body),
            _ => Err("invalid".to_string()),
        }
        .unwrap_or_else(|reason| (Kind::Refused, reason.into_bytes()));
        wire::write_frame(&mut stream, kind, &reply)
    }

    fn pair(&self, body: &[u8]) -> Result<(Kind, Vec<u8>), String> {
        let code = self.pairing_code().ok_or("not_pairing")?;
        if body.len() < 16 {
            return Err("invalid".into());
        }
        let (salt, sealed) = body.split_at(16);
        let key = crypto::pairing_key(&code, salt);
        let Some(hello) = crypto::open(&key, b"pair", sealed).and_then(|b| serde_json::from_slice::<Hello>(&b).ok())
        else {
            let mut pairing = lock(&self.inner.pairing);
            if let Some(p) = pairing.as_mut() {
                p.tries += 1;
                if p.tries >= MAX_TRIES {
                    *pairing = None;
                    drop(pairing);
                    (self.inner.on_event)(HostEvent::PairingEnded);
                    return Err("expired".into());
                }
            }
            return Err("wrong_code".into());
        };
        crypto::from_hex::<16>(&hello.device_id).ok_or("invalid")?;
        let link_key: SecretKey = crypto::random();
        let name: String = hello.name.trim().chars().take(60).collect();
        let store = &*self.inner.store;
        let mut list = devices(store);
        list.retain(|d| d.id != hello.device_id);
        list.push(Device {
            id: hello.device_id,
            name: name.clone(),
            key: crypto::to_hex(&link_key),
            linked_at: (self.inner.now)(),
            last_sync_at: None,
        });
        save_devices(store, &list);
        self.end_pairing();
        (self.inner.on_event)(HostEvent::Paired { name });
        let welcome = Welcome {
            device_id: crypto::to_hex(&self.inner.me.id),
            name: self.inner.me.name.clone(),
            key: crypto::to_hex(&link_key),
        };
        let json = serde_json::to_vec(&welcome).map_err(|e| e.to_string())?;
        Ok((Kind::PairWelcome, crypto::seal(&key, b"welcome", &json)))
    }

    fn sync(&self, body: &[u8]) -> Result<(Kind, Vec<u8>), String> {
        if body.len() < 16 {
            return Err("invalid".into());
        }
        let (id, sealed) = body.split_at(16);
        let id_hex = crypto::to_hex(id);
        let store = &*self.inner.store;
        let device = devices(store).into_iter().find(|d| d.id == id_hex).ok_or("unknown_device")?;
        let key = crypto::from_hex::<32>(&device.key).ok_or("unknown_device")?;
        let request: SyncRequest = crypto::open(&key, &[b"sync".as_slice(), id].concat(), sealed)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or("unknown_device")?;

        let changed = store.apply(&request.changes, &id_hex)?;
        let reply = SyncReply { changes: store.collect(request.since, &id_hex)? };
        let json = serde_json::to_vec(&reply).map_err(|e| e.to_string())?;

        let mut list = devices(store);
        if let Some(d) = list.iter_mut().find(|d| d.id == id_hex) {
            d.last_sync_at = Some((self.inner.now)());
        }
        save_devices(store, &list);
        (self.inner.on_event)(HostEvent::Synced { changed });
        Ok((Kind::SyncReply, crypto::seal(&key, &[b"reply".as_slice(), id].concat(), &json)))
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
