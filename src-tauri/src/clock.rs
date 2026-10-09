//! System clocks used by the timer.
//!
//! On Windows the monotonic clock is `QueryUnbiasedInterruptTime`: the system
//! interrupt clock with time spent in sleep and hibernation removed, so a
//! sleeping machine never eats into a session.

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

#[cfg(not(windows))]
fn mono_ms() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}
