//! Alert sounds, played natively so they work while the window is hidden.
//! Sounds are embedded in the executable (fully offline).

pub const DEFAULT_SOUND: &str = "chime";

const SOUNDS: &[(&str, &[u8])] = &[
    ("chime", include_bytes!("../sounds/chime.wav")),
    ("bell", include_bytes!("../sounds/bell.wav")),
    ("marimba", include_bytes!("../sounds/marimba.wav")),
    ("pulse", include_bytes!("../sounds/pulse.wav")),
    ("harp", include_bytes!("../sounds/harp.wav")),
];

pub fn ids() -> Vec<&'static str> {
    SOUNDS.iter().map(|(id, _)| *id).collect()
}

pub fn exists(id: &str) -> bool {
    SOUNDS.iter().any(|(name, _)| *name == id)
}

/// Play a bundled sound at `volume` (0.0..=1.0). Unknown ids fall back to the default.
pub fn play(id: &str, volume: f32) {
    if volume <= 0.0 {
        return;
    }
    let bytes = SOUNDS
        .iter()
        .find(|(name, _)| *name == id)
        .or_else(|| SOUNDS.first())
        .map(|(_, b)| *b);
    if let Some(bytes) = bytes {
        platform::play(scale_wav(bytes, volume.min(1.0)));
    }
}

/// Return a copy of a 16-bit PCM WAV with every sample scaled by `volume`.
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

#[cfg(not(windows))]
mod platform {
    /// Development builds on other systems stay silent.
    pub fn play(_wav: Vec<u8>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_sounds_are_valid_wav() {
        for (id, bytes) in SOUNDS {
            let (start, len) = data_chunk(bytes).unwrap_or_else(|| panic!("{id} has no data chunk"));
            assert!(start + len <= bytes.len(), "{id} data chunk is truncated");
            assert!(len > 0);
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
