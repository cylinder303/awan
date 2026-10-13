//! What travels on the network.
//!
//! TCP (the phone connects to the computer): one request frame, one reply
//! frame, then the connection closes. A frame is "AWN1", a kind byte, a
//! 4-byte length and the body. Everything in a body except the phone's id or
//! salt is sealed (see crypto.rs).
//!
//! UDP (finding the computer on the network): the phone broadcasts a short
//! question, a computer that fits answers with its id, port and name.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

use crate::db::{StampedGoal, SyncSession, Tombstone};
use crate::settings::Shared;

/// Port for both the TCP service and UDP discovery.
pub const PORT: u16 = 47815;
const MAGIC: &[u8; 4] = b"AWN1";
/// Large enough for years of history in one message.
const MAX_FRAME: usize = 64 << 20;

pub type DeviceId = [u8; 16];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    /// Phone to computer: salt (16) + sealed `Hello` under the code's key.
    PairHello = 1,
    /// Computer to phone: sealed `Welcome` under the code's key.
    PairWelcome = 2,
    /// Phone to computer: phone id (16) + sealed `SyncRequest` under the link key.
    SyncRequest = 3,
    /// Computer to phone: sealed `SyncReply` under the link key.
    SyncReply = 4,
    /// Either way: a short reason in plain text.
    Refused = 9,
}

impl Kind {
    fn from_u8(b: u8) -> Option<Kind> {
        Some(match b {
            1 => Kind::PairHello,
            2 => Kind::PairWelcome,
            3 => Kind::SyncRequest,
            4 => Kind::SyncReply,
            9 => Kind::Refused,
            _ => return None,
        })
    }
}

pub fn write_frame(w: &mut impl Write, kind: Kind, body: &[u8]) -> io::Result<()> {
    let mut head = [0u8; 9];
    head[..4].copy_from_slice(MAGIC);
    head[4] = kind as u8;
    head[5..].copy_from_slice(&(body.len() as u32).to_be_bytes());
    w.write_all(&head)?;
    w.write_all(body)?;
    w.flush()
}

pub fn read_frame(r: &mut impl Read) -> io::Result<(Kind, Vec<u8>)> {
    let mut head = [0u8; 9];
    r.read_exact(&mut head)?;
    if &head[..4] != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not an Awan message"));
    }
    let kind = Kind::from_u8(head[4]).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unknown kind"))?;
    let len = u32::from_be_bytes(head[5..].try_into().unwrap()) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok((kind, body))
}

// ------------------------------------------------------------- messages

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub device_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Welcome {
    pub device_id: String,
    pub name: String,
    /// The link key, hex.
    pub key: String,
}

/// Shared settings and when they last changed (milliseconds since 1970).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StampedSettings {
    pub changed_at: i64,
    pub values: Shared,
}

/// Everything one side has that the other may not.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Changes {
    pub sessions: Vec<SyncSession>,
    pub deleted: Vec<Tombstone>,
    pub week_goals: Vec<StampedGoal>,
    pub settings: Option<StampedSettings>,
    /// The sender's position in its own change order when this was put together.
    pub rev: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    /// The computer's change order position the phone already has.
    pub since: i64,
    pub changes: Changes,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReply {
    pub changes: Changes,
}

// ------------------------------------------------------------ discovery

const ASK: &[u8; 5] = b"AWN1?";
const ANSWER: &[u8; 5] = b"AWN1!";

/// What a phone asks the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Computers showing a pairing code.
    Pairing,
    /// The computer with this id.
    Device(DeviceId),
}

pub fn encode_question(q: Question) -> Vec<u8> {
    let mut out = ASK.to_vec();
    match q {
        Question::Pairing => out.push(0),
        Question::Device(id) => {
            out.push(1);
            out.extend_from_slice(&id);
        }
    }
    out
}

pub fn decode_question(b: &[u8]) -> Option<Question> {
    let rest = b.strip_prefix(&ASK[..])?;
    match rest {
        [0] => Some(Question::Pairing),
        [1, id @ ..] if id.len() == 16 => Some(Question::Device(id.try_into().ok()?)),
        _ => None,
    }
}

/// A computer's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub id: DeviceId,
    pub port: u16,
    pub pairing: bool,
    pub name: String,
}

pub fn encode_answer(a: &Answer) -> Vec<u8> {
    let mut out = ANSWER.to_vec();
    out.extend_from_slice(&a.id);
    out.extend_from_slice(&a.port.to_be_bytes());
    out.push(u8::from(a.pairing));
    let name: String = a.name.chars().take(60).collect();
    out.extend_from_slice(name.as_bytes());
    out
}

pub fn decode_answer(b: &[u8]) -> Option<Answer> {
    let rest = b.strip_prefix(&ANSWER[..])?;
    if rest.len() < 19 {
        return None;
    }
    Some(Answer {
        id: rest[..16].try_into().ok()?,
        port: u16::from_be_bytes([rest[16], rest[17]]),
        pairing: rest[18] == 1,
        name: String::from_utf8_lossy(&rest[19..]).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_reject_garbage() {
        let mut buf = Vec::new();
        write_frame(&mut buf, Kind::SyncReply, b"body").unwrap();
        assert_eq!(read_frame(&mut &buf[..]).unwrap(), (Kind::SyncReply, b"body".to_vec()));
        assert!(read_frame(&mut &b"HTTP/1.1 200 OK\r\n"[..]).is_err());
        let mut huge = b"AWN1\x04".to_vec();
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(read_frame(&mut &huge[..]).is_err());
    }

    #[test]
    fn discovery_packets_round_trip() {
        let id = [7u8; 16];
        for q in [Question::Pairing, Question::Device(id)] {
            assert_eq!(decode_question(&encode_question(q)), Some(q));
        }
        assert_eq!(decode_question(b"AWN1?\x01short"), None);
        let a = Answer { id, port: PORT, pairing: true, name: "مكتب".into() };
        assert_eq!(decode_answer(&encode_answer(&a)), Some(a));
        assert_eq!(decode_answer(b"AWN1!"), None);
    }
}
