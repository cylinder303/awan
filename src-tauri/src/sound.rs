//! Alert sounds, played natively so they work while the window is hidden.
//! On a computer they are embedded in the executable (fully offline). On
//! Android the app's resources hold them (res/raw), where the system plays
//! them with the notification, so the executable carries no second copy.

pub const DEFAULT_SOUND: &str = "drops";

/// Natural sounds only, no instruments or melodies (see scripts/gen_sounds.py).
const IDS: [&str; 5] = ["drops", "knock", "birds", "pebble", "wave"];

#[cfg(not(target_os = "android"))]
const SOUNDS: [(&str, &[u8]); 5] = [
    ("drops", include_bytes!("../sounds/drops.wav")),
    ("knock", include_bytes!("../sounds/knock.wav")),
    ("birds", include_bytes!("../sounds/birds.wav")),
    ("pebble", include_bytes!("../sounds/pebble.wav")),
    ("wave", include_bytes!("../sounds/wave.wav")),
];

pub fn ids() -> Vec<&'static str> {
    IDS.to_vec()
}

pub fn exists(id: &str) -> bool {
    IDS.contains(&id)
}

/// A bundled sound by id; unknown ids fall back to the default.
#[cfg(not(target_os = "android"))]
fn bytes(id: &str) -> &'static [u8] {
    SOUNDS.iter().find(|(name, _)| *name == id).unwrap_or(&SOUNDS[0]).1
}

/// Play a bundled sound at `volume` (0.0..=1.0). On Android the page plays
/// previews through the app's own player (MainActivity.kt) instead.
pub fn play(id: &str, volume: f32) {
    if volume <= 0.0 {
        return;
    }
    #[cfg(not(target_os = "android"))]
    platform::play(scale_wav(bytes(id), volume.min(1.0)));
    #[cfg(target_os = "android")]
    let _ = id;
}

/// Return a copy of a 16-bit PCM WAV with every sample scaled by `volume`.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn scale_wav(wav: &[u8], volume: f32) -> Vec<u8> {
    let mut out = wav.to_vec();
    if let Some((start, len)) = data_chunk(wav) {
        let end = (start + len).min(out.len());
        for s in out[start..end].chunks_exact_mut(2) {
            let v = i16::from_le_bytes([s[0], s[1]]) as f32 * volume;
            s.copy_from_slice(&(v as i16).to_le_bytes());
        }
    }
    out
}

/// Locate the `data` chunk of a RIFF/WAVE file: (offset, length).
#[cfg_attr(target_os = "android", allow(dead_code))]
fn data_chunk(wav: &[u8]) -> Option<(usize, usize)> {
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12;
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let len = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().ok()?) as usize;
        if id == b"data" {
            return Some((pos + 8, len));
        }
        pos += 8 + len + (len & 1);
    }
    None
}

#[cfg(windows)]
mod platform {
    use std::sync::Mutex;
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    /// PlaySound reads from our buffer while playing asynchronously, so the
    /// buffer must stay alive until the next sound replaces it.
    static BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

    pub fn play(wav: Vec<u8>) {
        let mut buf = BUFFER.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: stop any current sound before the old buffer is released.
        unsafe { PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0) };
        *buf = wav;
        // SAFETY: with SND_MEMORY the first argument points to an in-memory
        // WAV image, kept alive in BUFFER for the duration of playback.
        unsafe {
            PlaySoundW(
                buf.as_ptr() as *const u16,
                std::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            )
        };
    }
}

#[cfg(not(any(windows, target_os = "android")))]
mod platform {
    /// Development builds on other systems stay silent.
    pub fn play(_wav: Vec<u8>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_the_embedded_sounds() {
        assert_eq!(IDS.to_vec(), SOUNDS.iter().map(|(id, _)| *id).collect::<Vec<_>>());
    }

    #[test]
    fn bundled_sounds_are_valid_wav() {
        for (id, bytes) in SOUNDS {
            let (start, len) = data_chunk(bytes).unwrap_or_else(|| panic!("{id} has no data chunk"));
            assert!(start + len <= bytes.len(), "{id} data chunk is truncated");
            assert!(len > 0);
        }
    }

    /// Android plays the alerts from its own copy (res/raw); both are
    /// written by scripts/gen_sounds.py and must stay identical.
    #[test]
    fn android_copies_match() {
        let raw = concat!(env!("CARGO_MANIFEST_DIR"), "/gen/android/app/src/main/res/raw/");
        for (id, bytes) in SOUNDS {
            let copy = std::fs::read(format!("{raw}{id}.wav")).unwrap_or_else(|_| panic!("{id}.wav missing from res/raw"));
            assert!(copy == *bytes, "{id}.wav differs from src-tauri/sounds; run scripts/gen_sounds.py");
        }
    }

    #[test]
    fn volume_scales_samples() {
        let wav = SOUNDS[0].1;
        let (start, _) = data_chunk(wav).unwrap();
        let half = scale_wav(wav, 0.5);
        let silent = scale_wav(wav, 0.0);
        assert_eq!(&half[..start], &wav[..start], "header untouched");
        let peak = |b: &[u8]| {
            b[start..].chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]).unsigned_abs()).max().unwrap()
        };
        assert!(peak(&half) <= peak(wav) / 2 + 1);
        assert_eq!(peak(&silent), 0);
    }
}
