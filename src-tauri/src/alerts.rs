//! End-of-session alerts on phones.
//!
//! When a session ends the app may not be running at all (screen off, or
//! closed by the system), so each end is handed to the system ahead of time
//! as a scheduled notification, which rings with the chosen sound. The plan is
//! refreshed whenever the timer or the settings change. Computers notify at
//! the moment the timer finishes instead (see `handle_events` in lib.rs).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{Channel, Importance, NotificationExt, PermissionState, Schedule, Visibility};

use crate::i18n::{self, Lang};
use crate::timer::Mode;
use crate::{clock, lock, sound, AppState};

/// Session ends are handed to the system ahead of time on this platform.
pub const SCHEDULED_AHEAD: bool = true;

/// Two scheduled ends count as the same if they are this close.
const SAME_TIME_MS: i64 = 1_500;

#[derive(Debug, Clone, PartialEq)]
struct Alert {
    id: i32,
    at_ms: i64,
    ended: Mode,
    next: Mode,
    channel: String,
    lang: Lang,
}

impl Alert {
    fn same_as(&self, other: &Alert) -> bool {
        (self.at_ms - other.at_ms).abs() <= SAME_TIME_MS
            && (self.ended, self.next, self.lang) == (other.ended, other.next, other.lang)
            && self.channel == other.channel
    }
}

/// What is scheduled with the system right now.
static SCHEDULED: Mutex<Vec<Alert>> = Mutex::new(Vec::new());
static STARTED: Once = Once::new();
/// Permission asked and channels registered: notifications can be scheduled.
static READY: AtomicBool = AtomicBool::new(false);

fn channel_id(sound: Option<&str>) -> String {
    format!("alert-{}", sound.unwrap_or("silent"))
}

/// Once the window is up: ask for permission to notify, register a channel
/// per sound (Android ties the sound to the channel), and schedule.
pub fn start(app: &AppHandle) {
    STARTED.call_once(|| {
        let app = app.clone();
        // Off the main thread: the permission prompt waits for an answer.
        std::thread::spawn(move || {
            let notification = app.notification();
            if notification.permission_state().ok() != Some(PermissionState::Granted) {
                let _ = notification.request_permission();
            }
            let lang = *lock(&app.state::<AppState>().lang);
            for id in sound::ids().into_iter().map(Some).chain([None]) {
                let channel = Channel::builder(channel_id(id), i18n::alert_channel(lang, id))
                    .importance(if id.is_some() { Importance::High } else { Importance::Low })
                    .visibility(Visibility::Public)
                    .vibration(true);
                let channel = match id {
                    Some(id) => channel.sound(id),
                    None => channel,
                };
                let _ = notification.create_channel(channel.build());
            }
            READY.store(true, Ordering::SeqCst);
            sync(&app);
        });
    });
}

/// Bring the scheduled notifications in line with the timer: the current
/// session's end and, with auto-start, the one after it.
pub fn sync(app: &AppHandle) {
    if !READY.load(Ordering::SeqCst) {
        return;
    }
    let state = app.state::<AppState>();
    let now = clock::now();
    let endings = lock(&state.engine).upcoming(now);
    let (sound_id, volume) = {
        let s = lock(&state.settings);
        (s.sound.clone(), s.alert_volume())
    };
    let channel = channel_id((volume > 0.0).then_some(sound_id.as_str()));
    let lang = *lock(&state.lang);
    let wanted: Vec<Alert> = endings
        .iter()
        .map(|e| {
            let at_ms = now.wall + e.in_ms as i64;
            Alert {
                id: (at_ms / 1000 % i32::MAX as i64) as i32,
                at_ms,
                ended: e.ended,
                next: e.next,
                channel: channel.clone(),
                lang,
            }
        })
        .collect();

    let mut scheduled = lock(&SCHEDULED);
    let notification = app.notification();
    // Withdraw ends that no longer apply. Ones already due are left alone:
    // they are on screen now and the user should see them.
    let stale: Vec<i32> = scheduled
        .iter()
        .filter(|old| old.at_ms > now.wall + SAME_TIME_MS && !wanted.iter().any(|w| w.same_as(old)))
        .map(|old| old.id)
        .collect();
    if !stale.is_empty() {
        let _ = notification.cancel(stale);
    }
    let mut kept: Vec<Alert> = scheduled.iter().filter(|old| wanted.iter().any(|w| w.same_as(old))).cloned().collect();
    for alert in wanted {
        if kept.iter().any(|k| k.same_as(&alert)) || alert.at_ms <= now.wall {
            continue;
        }
        let Ok(date) = time::OffsetDateTime::from_unix_timestamp_nanos(alert.at_ms as i128 * 1_000_000) else {
            continue;
        };
        let (title, body) = i18n::finished(alert.lang, alert.ended, alert.next);
        let mut builder = crate::notification(app)
            .id(alert.id)
            .channel_id(&alert.channel)
            .title(title)
            .body(body)
            .auto_cancel()
            .schedule(Schedule::At { date, repeating: false, allow_while_idle: true });
        if volume > 0.0 {
            // Android 7 and older take the sound from the notification itself.
            builder = builder.sound(&sound_id);
        }
        if builder.show().is_ok() {
            kept.push(alert);
        }
    }
    *scheduled = kept;
}
