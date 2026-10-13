//! The phone's side: finds the computer, links with it once using the code
//! it shows, then exchanges changes with it.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::crypto;
use super::wire::{self, Answer, Hello, Kind, Question, SyncReply, SyncRequest, Welcome};
use super::{Identity, Store};

/// The linked computer, stored under this key.
pub const LINK_KEY: &str = "sync.link";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// The computer this phone is linked with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// Hex.
    pub id: String,
    pub name: String,
    /// The link key, hex.
    pub key: String,
    /// Where it was last reached, "ip:port".
    pub addr: String,
    /// The computer's change order position this phone has caught up to.
    #[serde(default)]
    pub since: i64,
    /// This phone's change order position the computer has.
    #[serde(default)]
    pub sent: i64,
    #[serde(default)]
    pub last_sync_at: Option<i64>,
}

pub fn link(store: &dyn Store) -> Option<Link> {
    store.load(LINK_KEY).and_then(|v| serde_json::from_value(v).ok())
}

pub fn save_link(store: &dyn Store, link: Option<&Link>) {
    store.save(LINK_KEY, serde_json::to_value(link).unwrap_or_default());
}

/// Why a sync or pairing did not happen. Shown to the user by its code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Nothing answered: a different network, or the computer is off.
    Unreachable,
    /// The phone may not use the local network (Android permission).
    Blocked,
    /// The computer said no: "wrong_code", "expired", "not_pairing", "unknown_device"...
    Refused(String),
    /// Something unexpected came back.
    Invalid,
}

impl Failure {
    pub fn code(&self) -> String {
        match self {
            Failure::Unreachable => "unreachable".into(),
            Failure::Blocked => "permission".into(),
            Failure::Refused(reason) => reason.clone(),
            Failure::Invalid => "invalid".into(),
        }
    }
}

fn io_failure(e: io::Error) -> Failure {
    match e.kind() {
        io::ErrorKind::PermissionDenied => Failure::Blocked,
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => Failure::Invalid,
        _ => Failure::Unreachable,
    }
}

/// Where to send a discovery question: the whole local network.
pub fn broadcast_targets() -> Vec<SocketAddr> {
    let mut targets = vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), wire::PORT)];
    // Some phones only send to the network's own broadcast address.
    if let Some(IpAddr::V4(ip)) = local_ip() {
        let [a, b, c, _] = ip.octets();
        targets.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(a, b, c, 255)), wire::PORT));
    }
    targets
}

/// This device's address on the local network (no packet is sent).
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

/// Ask the network and collect the computers that answer within `wait`.
pub fn find(question: Question, targets: &[SocketAddr], wait: Duration) -> Result<Vec<(Answer, SocketAddr)>, Failure> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(io_failure)?;
    socket.set_broadcast(true).map_err(io_failure)?;
    let packet = wire::encode_question(question);
    let mut sent = false;
    let mut blocked = false;
    for target in targets {
        match socket.send_to(&packet, target) {
            Ok(_) => sent = true,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => blocked = true,
            Err(_) => {}
        }
    }
    if !sent {
        return Err(if blocked { Failure::Blocked } else { Failure::Unreachable });
    }
    let deadline = Instant::now() + wait;
    let mut found: Vec<(Answer, SocketAddr)> = Vec::new();
    let mut buf = [0u8; 256];
    while let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()) {
        socket.set_read_timeout(Some(left)).map_err(io_failure)?;
        let Ok((n, from)) = socket.recv_from(&mut buf) else { break };
        if let Some(answer) = wire::decode_answer(&buf[..n]) {
            let addr = SocketAddr::new(from.ip(), answer.port);
            if !found.iter().any(|(a, _)| a.id == answer.id) {
                found.push((answer, addr));
            }
        }
    }
    Ok(found)
}

/// Read "192.168.1.5" or "192.168.1.5:47815" as typed by the user.
pub fn parse_addr(text: &str) -> Option<SocketAddr> {
    let text = text.trim();
    text.parse::<SocketAddr>()
        .ok()
        .or_else(|| text.parse::<IpAddr>().ok().map(|ip| SocketAddr::new(ip, wire::PORT)))
}

fn exchange(addr: SocketAddr, kind: Kind, body: &[u8]) -> Result<(Kind, Vec<u8>), Failure> {
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(io_failure)?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).map_err(io_failure)?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).map_err(io_failure)?;
    wire::write_frame(&mut stream, kind, body).map_err(io_failure)?;
    let (kind, body) = wire::read_frame(&mut stream).map_err(io_failure)?;
    if kind == Kind::Refused {
        return Err(Failure::Refused(String::from_utf8_lossy(&body).chars().take(40).collect()));
    }
    Ok((kind, body))
}

/// Link with the computer at `addr` using the code it shows.
pub fn pair(addr: SocketAddr, code: &str, me: &Identity) -> Result<Link, Failure> {
    let salt: [u8; 16] = crypto::random();
    let key = crypto::pairing_key(code, &salt);
    let hello = Hello { device_id: crypto::to_hex(&me.id), name: me.name.clone() };
    let json = serde_json::to_vec(&hello).map_err(|_| Failure::Invalid)?;
    let body = [salt.as_slice(), &crypto::seal(&key, b"pair", &json)].concat();
    let (kind, reply) = exchange(addr, Kind::PairHello, &body)?;
    if kind != Kind::PairWelcome {
        return Err(Failure::Invalid);
    }
    let welcome: Welcome = crypto::open(&key, b"welcome", &reply)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(Failure::Invalid)?;
    crypto::from_hex::<32>(&welcome.key).ok_or(Failure::Invalid)?;
    crypto::from_hex::<16>(&welcome.device_id).ok_or(Failure::Invalid)?;
    Ok(Link {
        id: welcome.device_id,
        name: welcome.name,
        key: welcome.key,
        addr: addr.to_string(),
        since: 0,
        sent: 0,
        last_sync_at: None,
    })
}

/// Exchange changes with the linked computer at `addr`. On success `link`
/// records how far each side has caught up. Returns whether this phone's
/// data changed.
pub fn sync(store: &dyn Store, me: &Identity, link: &mut Link, addr: SocketAddr, now: i64) -> Result<bool, Failure> {
    let key = crypto::from_hex::<32>(&link.key).ok_or(Failure::Invalid)?;
    let changes = store.collect(link.sent, &link.id).map_err(|_| Failure::Invalid)?;
    let sent_up_to = changes.rev;
    let request = SyncRequest { since: link.since, changes };
    let json = serde_json::to_vec(&request).map_err(|_| Failure::Invalid)?;
    let body = [me.id.as_slice(), &crypto::seal(&key, &[b"sync".as_slice(), &me.id].concat(), &json)].concat();

    let (kind, reply) = exchange(addr, Kind::SyncRequest, &body)?;
    if kind != Kind::SyncReply {
        return Err(Failure::Invalid);
    }
    let reply: SyncReply = crypto::open(&key, &[b"reply".as_slice(), &me.id].concat(), &reply)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(Failure::Invalid)?;
    let changed = store.apply(&reply.changes, &link.id).map_err(|_| Failure::Invalid)?;
    link.since = reply.changes.rev;
    link.sent = sent_up_to;
    link.addr = addr.to_string();
    link.last_sync_at = Some(now);
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::Arc;

    use super::*;
    use crate::settings::Settings;
    use crate::sync::host::{self, Host, HostEvent};
    use crate::sync::testing::MemStore;
    use crate::timer::{Mode, SessionRecord};

    fn record(mode: Mode, started_at: i64) -> SessionRecord {
        SessionRecord { mode, started_at, ended_at: started_at + 60_000, duration_ms: 60_000, task: "t".into(), completed: true }
    }

    fn now() -> i64 {
        1_700_000_000_000
    }

    fn free_udp() -> SocketAddr {
        UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
    }

    #[test]
    fn link_and_sync_over_the_network() {
        let pc = Arc::new(MemStore::new());
        let phone = MemStore::new();
        pc.db.lock().unwrap().insert_session(&record(Mode::Focus, 1_000)).unwrap();
        pc.set_settings(10, Settings { focus_minutes: 40, ..Settings::default() }.shared());
        phone.db.lock().unwrap().insert_session(&record(Mode::Focus, 2_000)).unwrap();
        phone.set_settings(20, Settings { focus_minutes: 50, ..Settings::default() }.shared());

        let (tx, events) = mpsc::channel();
        let udp = free_udp();
        let pc_me = Identity { id: [1; 16], name: "Desk".into() };
        let host = Host::start(pc.clone(), pc_me, "127.0.0.1:0".parse().unwrap(), Some(udp), now, move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        let addr: SocketAddr = format!("127.0.0.1:{}", host.port()).parse().unwrap();
        let wait = Duration::from_millis(400);

        // Not pairing: nobody answers the pairing question, and codes are refused.
        assert!(find(Question::Pairing, &[udp], wait).unwrap().is_empty());
        let me = Identity { id: [2; 16], name: "Phone".into() };
        assert_eq!(pair(addr, "0000000000", &me), Err(Failure::Refused("not_pairing".into())));

        let code = host.begin_pairing();
        let found = find(Question::Pairing, &[udp], wait).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0.name.as_str(), found[0].1), ("Desk", addr));

        let wrong = if code.starts_with('1') { "2" } else { "1" }.repeat(10);
        assert_eq!(pair(addr, &wrong, &me), Err(Failure::Refused("wrong_code".into())));
        let mut link = pair(addr, &code, &me).unwrap();
        assert_eq!(link.name, "Desk");
        assert_eq!(events.recv().unwrap(), HostEvent::Paired { name: "Phone".into() });
        assert_eq!(pair(addr, &code, &me), Err(Failure::Refused("not_pairing".into())), "a code works once");

        // First sync: both have both sessions; the newer settings (phone's) win.
        assert!(sync(&phone, &me, &mut link, addr, now()).unwrap());
        assert_eq!(pc.uids(), vec!["f-1000", "f-2000"]);
        assert_eq!(phone.uids(), pc.uids());
        assert_eq!(pc.settings.lock().unwrap().as_ref().unwrap().values.focus_minutes, 50);
        assert_eq!(events.recv().unwrap(), HostEvent::Synced { changed: true });
        assert_eq!(host::devices(&*pc)[0].last_sync_at, Some(now()));

        // Nothing new: nothing changes and nothing is sent back and forth.
        assert!(!sync(&phone, &me, &mut link, addr, now()).unwrap());
        assert_eq!(phone.collect(link.sent, &link.id).unwrap().sessions, vec![]);
        assert_eq!(pc.collect(link.since, "02020202020202020202020202020202").unwrap().sessions, vec![]);

        // A session deleted on the computer disappears from the phone.
        let id = pc.db.lock().unwrap().list_sessions(None, 10).unwrap()[1].id;
        pc.db.lock().unwrap().delete_session(id, now()).unwrap();
        assert!(sync(&phone, &me, &mut link, addr, now()).unwrap());
        assert_eq!(phone.uids(), vec!["f-2000"]);

        // The phone is found by id when its address changes.
        let found = find(Question::Device([1; 16]), &[udp], wait).unwrap();
        assert_eq!(found[0].1, addr);
        assert!(find(Question::Device([9; 16]), &[udp], wait).unwrap().is_empty());

        // Removed on the computer: the phone is told.
        host::save_devices(&*pc, &[]);
        assert_eq!(sync(&phone, &me, &mut link, addr, now()), Err(Failure::Refused("unknown_device".into())));
    }

    #[test]
    fn too_many_wrong_codes_end_pairing() {
        let pc = Arc::new(MemStore::new());
        let host = Host::start(pc, Identity { id: [1; 16], name: "Desk".into() }, "127.0.0.1:0".parse().unwrap(), None, now, |_| {})
            .unwrap();
        let addr: SocketAddr = format!("127.0.0.1:{}", host.port()).parse().unwrap();
        let code = host.begin_pairing();
        let wrong = if code.starts_with('1') { "2" } else { "1" }.repeat(10);
        let me = Identity { id: [2; 16], name: "Phone".into() };
        for _ in 0..4 {
            assert_eq!(pair(addr, &wrong, &me), Err(Failure::Refused("wrong_code".into())));
        }
        assert_eq!(pair(addr, &wrong, &me), Err(Failure::Refused("expired".into())));
        assert_eq!(pair(addr, &code, &me), Err(Failure::Refused("not_pairing".into())));
    }

    #[test]
    fn nothing_listening_is_unreachable() {
        let closed = TcpStream::connect("127.0.0.1:1").map(|_| ()).err();
        assert!(closed.is_some());
        let me = Identity { id: [2; 16], name: "Phone".into() };
        assert_eq!(pair("127.0.0.1:1".parse().unwrap(), "1234567890", &me), Err(Failure::Unreachable));
    }

    #[test]
    fn typed_addresses() {
        assert_eq!(parse_addr(" 192.168.1.5 "), Some(format!("192.168.1.5:{}", wire::PORT).parse().unwrap()));
        assert_eq!(parse_addr("192.168.1.5:5000"), Some("192.168.1.5:5000".parse().unwrap()));
        assert_eq!(parse_addr("my laptop"), None);
    }
}
