//! Keys and sealed messages.
//!
//! Pairing: the computer shows a 10-digit code. Both sides turn it into a key
//! with PBKDF2 (slow on purpose, so the code cannot be guessed from recorded
//! traffic in any useful time), and the computer sends a fresh random 256-bit
//! key under it. Every message after that is sealed with that key using
//! ChaCha20-Poly1305: unreadable to others on the network, and any change
//! to it is detected.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::Sha256;

pub type SecretKey = [u8; 32];

const NONCE_LEN: usize = 12;
const PAIRING_ROUNDS: u32 = if cfg!(test) { 1_000 } else { 200_000 };
/// Digits in a pairing code.
pub const CODE_LEN: usize = 10;

/// Random bytes from the operating system.
pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the system random number generator is unavailable");
    bytes
}

/// A new pairing code: 10 random digits.
pub fn new_code() -> String {
    // Rejection sampling keeps every digit equally likely.
    let mut code = String::with_capacity(CODE_LEN);
    while code.len() < CODE_LEN {
        for b in random::<16>() {
            if b < 250 && code.len() < CODE_LEN {
                code.push(char::from(b'0' + b % 10));
            }
        }
    }
    code
}

/// The digits of a typed code (spaces, dashes and Arabic-Indic digits allowed).
pub fn normalize_code(typed: &str) -> String {
    typed
        .chars()
        .filter_map(|c| match c {
            '0'..='9' => Some(c),
            '٠'..='٩' => char::from_u32('0' as u32 + (c as u32 - '٠' as u32)),
            '۰'..='۹' => char::from_u32('0' as u32 + (c as u32 - '۰' as u32)),
            _ => None,
        })
        .collect()
}

/// The key both sides derive from a pairing code and the phone's salt.
pub fn pairing_key(code: &str, salt: &[u8]) -> SecretKey {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(normalize_code(code).as_bytes(), salt, PAIRING_ROUNDS, &mut key);
    key
}

/// Encrypt `plaintext`; `context` is authenticated but not sent, so a message
/// cannot be replayed as a different kind of message.
pub fn seal(key: &SecretKey, context: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let nonce = random::<NONCE_LEN>();
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let sealed = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plaintext, aad: context })
        .expect("encryption cannot fail for in-memory data");
    let mut out = Vec::with_capacity(NONCE_LEN + sealed.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    out
}

/// Decrypt what `seal` produced, or None if the key, context or data is wrong.
pub fn open(key: &SecretKey, context: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < NONCE_LEN {
        return None;
    }
    let (nonce, body) = sealed.split_at(NONCE_LEN);
    ChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(Nonce::from_slice(nonce), Payload { msg: body, aad: context })
        .ok()
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 || !text.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_messages_open_only_with_the_same_key_and_context() {
        let key = random::<32>();
        let sealed = seal(&key, b"sync", b"hello");
        assert_eq!(open(&key, b"sync", &sealed).as_deref(), Some(&b"hello"[..]));
        assert_eq!(open(&key, b"reply", &sealed), None, "other context");
        assert_eq!(open(&random::<32>(), b"sync", &sealed), None, "other key");
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(open(&key, b"sync", &tampered), None, "changed in transit");
        assert_ne!(seal(&key, b"sync", b"hello"), sealed, "fresh nonce every time");
    }

    #[test]
    fn codes_are_ten_digits_and_typing_variants_match() {
        let code = new_code();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(normalize_code("12345 67890"), "1234567890");
        assert_eq!(normalize_code("١٢٣٤٥-٦٧٨٩٠"), "1234567890");
        let salt = random::<16>();
        assert_eq!(pairing_key("12345-67890", &salt), pairing_key("١٢٣٤٥٦٧٨٩٠", &salt));
        assert_ne!(pairing_key("1234567890", &salt), pairing_key("1234567891", &salt));
    }

    #[test]
    fn hex_round_trip() {
        let id = random::<16>();
        assert_eq!(from_hex::<16>(&to_hex(&id)), Some(id));
        assert_eq!(from_hex::<16>("zz"), None);
    }
}
