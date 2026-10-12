//! System clocks used by the timer.
//!
//! On Windows the monotonic clock is `QueryUnbiasedInterruptTime`: the system
//! interrupt clock with time spent in sleep and hibernation removed, so a
//! sleeping machine never eats into a session. On Android it is the opposite:
//! a locked phone is still mid-session, so the clock includes sleep.

use crate::timer::Now;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> Now {
    Now { mono: mono_ms(), wall: wall_ms() }
}

pub fn wall_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(windows)]
fn mono_ms() -> u64 {
    use windows_sys::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTime;
    let mut t: u64 = 0;
    // SAFETY: valid out pointer; the call cannot fail on Windows 7+.
    unsafe { QueryUnbiasedInterruptTime(&mut t) };
    t / 10_000 // 100 ns units -> ms
}

/// Android: time since boot including deep sleep, so a session keeps
/// counting while the phone's screen is off.
#[cfg(target_os = "android")]
fn mono_ms() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: valid out pointer; CLOCK_BOOTTIME exists on every Android version.
    unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    ts.tv_sec as u64 * 1000 + ts.tv_nsec as u64 / 1_000_000
}

#[cfg(not(any(windows, target_os = "android")))]
fn mono_ms() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}
