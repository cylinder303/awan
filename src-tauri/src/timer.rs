//! Pomodoro timer engine.
//!
//! The engine is pure state: every method receives the current time (`Now`)
//! instead of reading a clock, so it is fully deterministic and unit-testable.
//! Elapsed time is always derived from clock readings, never from counting
//! ticks, so a throttled or delayed tick cannot make the timer drift.

use serde::{Deserialize, Serialize};

/// Sessions stopped early shorter than this are not worth recording.
pub const MIN_RECORD_MS: u64 = 60_000;

/// If the wall clock jumps forward by more than this between two ticks while
/// running, the machine was asleep (ticks run every <= 500 ms otherwise).
pub const SLEEP_GAP_MS: i64 = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Idle,
    Running,
    Paused,
}

/// A point in time as seen by two clocks.
#[derive(Debug, Clone, Copy)]
pub struct Now {
    /// Monotonic milliseconds that do not advance while the machine sleeps.
    pub mono: u64,
    /// Wall-clock milliseconds since the Unix epoch (for records and sleep detection).
    pub wall: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimerConfig {
    pub focus_ms: u64,
    pub short_break_ms: u64,
    pub long_break_ms: u64,
    pub sessions_before_long: u32,
    pub auto_start: bool,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            focus_ms: 25 * 60_000,
            short_break_ms: 5 * 60_000,
            long_break_ms: 15 * 60_000,
            sessions_before_long: 4,
            auto_start: false,
        }
    }
}

impl TimerConfig {
    pub fn duration(&self, mode: Mode) -> u64 {
        match mode {
            Mode::Focus => self.focus_ms,
            Mode::ShortBreak => self.short_break_ms,
            Mode::LongBreak => self.long_break_ms,
        }
    }
}

/// A finished (or abandoned) session, ready to be stored in history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub mode: Mode,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_ms: u64,
    pub task: String,
    pub completed: bool,
}

/// Side effects the caller should perform after an engine call.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A session ended and should be written to history.
    SessionEnded(SessionRecord),
    /// The countdown reached zero (notify + play sound).
    Finished { ended: Mode, next: Mode, auto_started: bool },
    /// The timer was paused because the machine went to sleep.
    SleepPaused,
}

/// What the UI needs to render. Sent to the frontend as JSON.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimerView {
    pub mode: Mode,
    pub status: Status,
    pub duration_ms: u64,
    pub elapsed_ms: u64,
    pub remaining_ms: u64,
    /// 1-based position in the cycle, e.g. 2 in "2 of 4".
    pub cycle_index: u32,
    pub cycle_len: u32,
    pub task: String,
    pub paused_by_sleep: bool,
    pub auto_start: bool,
}

/// State persisted to disk so an interrupted session can be resumed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub mode: Mode,
    pub elapsed_ms: u64,
    pub cycle_pos: u32,
    pub task: String,
    pub started_at: Option<i64>,
    pub saved_at: i64,
    /// Length the session started with (absent in older snapshots).
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Engine {
    cfg: TimerConfig,
    mode: Mode,
    status: Status,
    /// Elapsed time banked before the current running segment.
    banked_ms: u64,
    /// Monotonic time when the current running segment began.
    segment_start: Option<u64>,
    last_tick: Option<Now>,
    /// Focus sessions completed in the current cycle.
    cycle_pos: u32,
    task: String,
    paused_by_sleep: bool,
    /// Wall time the current session was first started.
    started_at: Option<i64>,
    /// Length fixed when the session started, so changing settings
    /// mid-session only affects the sessions that follow.
    locked_ms: Option<u64>,
}

impl Engine {
    pub fn new(cfg: TimerConfig) -> Self {
        Self {
            cfg,
            mode: Mode::Focus,
            status: Status::Idle,
            banked_ms: 0,
            segment_start: None,
            last_tick: None,
            cycle_pos: 0,
            task: String::new(),
            paused_by_sleep: false,
            started_at: None,
            locked_ms: None,
        }
    }

    /// Length of the current session.
    pub fn duration_ms(&self) -> u64 {
        self.locked_ms.unwrap_or_else(|| self.cfg.duration(self.mode))
    }

    pub fn elapsed_ms(&self, now: Now) -> u64 {
        let running = self
            .segment_start
            .map(|s| now.mono.saturating_sub(s))
            .unwrap_or(0);
        self.banked_ms + running
    }

    pub fn remaining_ms(&self, now: Now) -> u64 {
        self.duration_ms().saturating_sub(self.elapsed_ms(now))
    }

    pub fn view(&self, now: Now) -> TimerView {
        let len = self.cfg.sessions_before_long.max(1);
        let cycle_index = match self.mode {
            Mode::Focus => (self.cycle_pos + 1).min(len),
            Mode::ShortBreak => self.cycle_pos.clamp(1, len),
            Mode::LongBreak => len,
        };
        let duration = self.duration_ms();
        let elapsed = self.elapsed_ms(now).min(duration);
        TimerView {
            mode: self.mode,
            status: self.status,
            duration_ms: duration,
            elapsed_ms: elapsed,
            remaining_ms: duration - elapsed,
            cycle_index,
            cycle_len: len,
            task: self.task.clone(),
            paused_by_sleep: self.paused_by_sleep,
            auto_start: self.cfg.auto_start,
        }
    }

    pub fn start(&mut self, now: Now) {
        if self.status == Status::Running {
            return;
        }
        if self.started_at.is_none() {
            self.started_at = Some(now.wall);
        }
        if self.locked_ms.is_none() {
            self.locked_ms = Some(self.cfg.duration(self.mode));
        }
        self.segment_start = Some(now.mono);
        self.last_tick = Some(now);
        self.status = Status::Running;
        self.paused_by_sleep = false;
    }

    pub fn pause(&mut self, now: Now) {
        if self.status != Status::Running {
            return;
        }
        self.bank(now.mono);
        self.status = Status::Paused;
    }

    pub fn toggle(&mut self, now: Now) {
        if self.status == Status::Running {
            self.pause(now);
        } else {
            self.start(now);
        }
    }

    /// Restart the current session from zero. The abandoned part is returned
    /// as a history record if it was long enough.
    pub fn reset(&mut self, now: Now) -> Vec<Event> {
        let events = self.abandon(now);
        self.clear_session();
        events
    }

    /// Jump to the next session in the cycle.
    pub fn skip(&mut self, now: Now) -> Vec<Event> {
        let events = self.abandon(now);
        self.advance();
        if self.cfg.auto_start {
            self.start(now);
        }
        events
    }

    /// Switch to a specific mode chosen by the user.
    pub fn set_mode(&mut self, mode: Mode, now: Now) -> Vec<Event> {
        if mode == self.mode {
            return Vec::new();
        }
        let events = self.abandon(now);
        self.clear_session();
        self.mode = mode;
        events
    }

    pub fn set_task(&mut self, task: &str) {
        self.task = task.trim().chars().take(120).collect();
    }

    /// Apply new settings. A session already under way keeps its length.
    pub fn set_config(&mut self, cfg: TimerConfig) {
        self.cfg = cfg;
        self.cycle_pos = self.cycle_pos.min(self.cfg.sessions_before_long.max(1));
    }

    /// The machine is about to sleep: pause and remember why.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn suspend(&mut self, now: Now) -> Vec<Event> {
        if self.status != Status::Running {
            return Vec::new();
        }
        self.pause(now);
        self.paused_by_sleep = true;
        vec![Event::SleepPaused]
    }

    /// Advance time. Call regularly (every <= 500 ms) and after waking.
    pub fn tick(&mut self, now: Now) -> Vec<Event> {
        let mut events = Vec::new();
        let last = self.last_tick.replace(now);
        if self.status != Status::Running {
            return events;
        }

        // Fallback sleep detection: a large wall-clock jump between ticks means
        // the process was frozen (sleep, hibernate, modern standby). Count time
        // only up to the last tick we actually observed.
        if let Some(last) = last {
            if now.wall - last.wall > SLEEP_GAP_MS {
                self.bank(last.mono.max(self.segment_start.unwrap_or(0)));
                self.status = Status::Paused;
                self.paused_by_sleep = true;
                events.push(Event::SleepPaused);
                return events;
            }
        }

        if self.remaining_ms(now) == 0 {
            events.extend(self.finish(now));
        }
        events
    }

    pub fn snapshot(&self, now: Now) -> Snapshot {
        Snapshot {
            mode: self.mode,
            elapsed_ms: self.elapsed_ms(now).min(self.duration_ms()),
            cycle_pos: self.cycle_pos,
            task: self.task.clone(),
            started_at: self.started_at,
            saved_at: now.wall,
            duration_ms: self.locked_ms,
        }
    }

    /// Load an interrupted session in the paused state.
    pub fn restore(&mut self, snap: &Snapshot) {
        self.mode = snap.mode;
        self.cycle_pos = snap.cycle_pos.min(self.cfg.sessions_before_long.max(1));
        self.task = snap.task.clone();
        self.locked_ms = Some(snap.duration_ms.unwrap_or_else(|| self.cfg.duration(snap.mode)));
        self.banked_ms = snap.elapsed_ms.min(self.duration_ms());
        self.segment_start = None;
        self.started_at = snap.started_at;
        self.status = if self.banked_ms > 0 { Status::Paused } else { Status::Idle };
        if self.status == Status::Idle {
            self.locked_ms = None;
        }
        self.paused_by_sleep = false;
    }

    /// Discard the restored session and begin a fresh cycle.
    pub fn start_fresh(&mut self, now: Now) -> Vec<Event> {
        let events = self.abandon(now);
        self.clear_session();
        self.mode = Mode::Focus;
        self.cycle_pos = 0;
        events
    }

    // ----------------------------------------------------------------- private

    fn bank(&mut self, mono: u64) {
        if let Some(start) = self.segment_start.take() {
            self.banked_ms += mono.saturating_sub(start);
        }
    }

    fn clear_session(&mut self) {
        self.banked_ms = 0;
        self.segment_start = None;
        self.started_at = None;
        self.locked_ms = None;
        self.status = Status::Idle;
        self.paused_by_sleep = false;
    }

    /// Record the current session as stopped early, if it is worth keeping.
    fn abandon(&mut self, now: Now) -> Vec<Event> {
        let elapsed = self.elapsed_ms(now).min(self.duration_ms());
        match self.started_at {
            Some(started_at) if elapsed >= MIN_RECORD_MS => {
                vec![Event::SessionEnded(SessionRecord {
                    mode: self.mode,
                    started_at,
                    ended_at: now.wall,
                    duration_ms: elapsed,
                    task: self.task.clone(),
                    completed: false,
                })]
            }
            _ => Vec::new(),
        }
    }

    fn finish(&mut self, now: Now) -> Vec<Event> {
        let ended = self.mode;
        let record = SessionRecord {
            mode: ended,
            started_at: self.started_at.unwrap_or(now.wall),
            ended_at: now.wall,
            duration_ms: self.duration_ms(),
            task: self.task.clone(),
            completed: true,
        };
        let next = self.advance();
        let auto_started = self.cfg.auto_start;
        if auto_started {
            self.start(now);
        }
        vec![
            Event::SessionEnded(record),
            Event::Finished { ended, next, auto_started },
        ]
    }

    /// Move to the next mode in the cycle and leave it idle.
    fn advance(&mut self) -> Mode {
        let len = self.cfg.sessions_before_long.max(1);
        self.mode = match self.mode {
            Mode::Focus => {
                self.cycle_pos += 1;
                if self.cycle_pos >= len {
                    Mode::LongBreak
                } else {
                    Mode::ShortBreak
                }
            }
            Mode::ShortBreak => Mode::Focus,
            Mode::LongBreak => {
                self.cycle_pos = 0;
                Mode::Focus
            }
        };
        self.clear_session();
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    /// A fake clock where both clocks advance together unless told otherwise.
    struct Clock {
        mono: u64,
        wall: i64,
    }

    impl Clock {
        fn new() -> Self {
            Self { mono: 1_000, wall: 1_700_000_000_000 }
        }
        fn now(&self) -> Now {
            Now { mono: self.mono, wall: self.wall }
        }
        fn advance(&mut self, ms: u64) -> Now {
            self.mono += ms;
            self.wall += ms as i64;
            self.now()
        }
        /// Advance in small tick-sized steps, collecting events.
        fn run(&mut self, eng: &mut Engine, ms: u64) -> Vec<Event> {
            let mut events = Vec::new();
            let mut left = ms;
            while left > 0 {
                let step = left.min(500);
                let now = self.advance(step);
                events.extend(eng.tick(now));
                left -= step;
            }
            events
        }
    }

    fn short_cfg() -> TimerConfig {
        TimerConfig {
            focus_ms: 10 * MIN,
            short_break_ms: 2 * MIN,
            long_break_ms: 5 * MIN,
            sessions_before_long: 4,
            auto_start: false,
        }
    }

    #[test]
    fn defaults_match_classic_pomodoro() {
        let cfg = TimerConfig::default();
        assert_eq!(cfg.focus_ms, 25 * MIN);
        assert_eq!(cfg.short_break_ms, 5 * MIN);
        assert_eq!(cfg.long_break_ms, 15 * MIN);
        assert_eq!(cfg.sessions_before_long, 4);
        assert!(!cfg.auto_start);
    }

    #[test]
    fn counts_down_from_clock_not_ticks() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        // No ticks at all for 3 minutes (heavily throttled): still exact.
        let now = c.advance(3 * MIN);
        assert_eq!(e.remaining_ms(now), 7 * MIN);
        assert_eq!(e.view(now).status, Status::Running);
    }

    #[test]
    fn pause_freezes_time() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 2 * MIN);
        e.pause(c.now());
        c.run(&mut e, 5 * MIN);
        assert_eq!(e.elapsed_ms(c.now()), 2 * MIN);
        e.start(c.now());
        c.run(&mut e, MIN);
        assert_eq!(e.elapsed_ms(c.now()), 3 * MIN);
    }

    #[test]
    fn finishing_focus_records_and_moves_to_short_break() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.set_task("Write report");
        e.start(c.now());
        let events = c.run(&mut e, 10 * MIN);
        assert_eq!(events.len(), 2);
        match &events[0] {
            Event::SessionEnded(r) => {
                assert_eq!(r.mode, Mode::Focus);
                assert!(r.completed);
                assert_eq!(r.duration_ms, 10 * MIN);
                assert_eq!(r.task, "Write report");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(
            events[1],
            Event::Finished { ended: Mode::Focus, next: Mode::ShortBreak, auto_started: false }
        );
        let v = e.view(c.now());
        assert_eq!(v.mode, Mode::ShortBreak);
        assert_eq!(v.status, Status::Idle);
        assert_eq!(v.remaining_ms, 2 * MIN);
        assert_eq!(v.cycle_index, 1);
    }

    #[test]
    fn long_break_after_four_focus_sessions_then_cycle_restarts() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        let mut modes = Vec::new();
        for _ in 0..9 {
            modes.push((e.mode, e.view(c.now()).cycle_index));
            e.start(c.now());
            let d = e.cfg.duration(e.mode);
            c.run(&mut e, d);
        }
        use Mode::*;
        assert_eq!(
            modes,
            vec![
                (Focus, 1), (ShortBreak, 1),
                (Focus, 2), (ShortBreak, 2),
                (Focus, 3), (ShortBreak, 3),
                (Focus, 4), (LongBreak, 4),
                (Focus, 1),
            ]
        );
    }

    #[test]
    fn auto_start_begins_next_session() {
        let mut c = Clock::new();
        let mut e = Engine::new(TimerConfig { auto_start: true, ..short_cfg() });
        e.start(c.now());
        let events = c.run(&mut e, 10 * MIN);
        assert!(events.contains(&Event::Finished {
            ended: Mode::Focus,
            next: Mode::ShortBreak,
            auto_started: true
        }));
        assert_eq!(e.status, Status::Running);
        c.run(&mut e, MIN);
        assert_eq!(e.remaining_ms(c.now()), MIN);
    }

    #[test]
    fn skip_records_only_sessions_longer_than_a_minute() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 30_000);
        assert!(e.skip(c.now()).is_empty(), "30 s is too short to record");
        assert_eq!(e.mode, Mode::ShortBreak);

        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 3 * MIN);
        let events = e.skip(c.now());
        match events.as_slice() {
            [Event::SessionEnded(r)] => {
                assert!(!r.completed);
                assert_eq!(r.duration_ms, 3 * MIN);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(e.view(c.now()).cycle_index, 1, "skipped focus still advances the cycle");
    }

    #[test]
    fn reset_restarts_same_mode() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 4 * MIN);
        let events = e.reset(c.now());
        assert_eq!(events.len(), 1);
        let v = e.view(c.now());
        assert_eq!((v.mode, v.status, v.remaining_ms), (Mode::Focus, Status::Idle, 10 * MIN));
    }

    #[test]
    fn suspend_pauses_and_sleep_time_is_not_counted() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 2 * MIN);
        assert_eq!(e.suspend(c.now()), vec![Event::SleepPaused]);
        // Sleep for an hour: the unbiased clock stands still, the wall clock moves.
        c.wall += 3_600_000;
        assert!(e.tick(c.now()).is_empty());
        let v = e.view(c.now());
        assert_eq!(v.status, Status::Paused);
        assert!(v.paused_by_sleep);
        assert_eq!(v.elapsed_ms, 2 * MIN);
        // Stays paused until the user resumes.
        c.run(&mut e, MIN);
        assert_eq!(e.elapsed_ms(c.now()), 2 * MIN);
        e.start(c.now());
        assert!(!e.view(c.now()).paused_by_sleep);
    }

    #[test]
    fn wall_clock_gap_is_detected_as_sleep() {
        // Modern standby: no suspend notification, and both clocks jump.
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 3 * MIN);
        let events = e.tick(c.advance(40 * MIN));
        assert_eq!(events, vec![Event::SleepPaused]);
        let v = e.view(c.now());
        assert_eq!(v.status, Status::Paused);
        assert_eq!(v.elapsed_ms, 3 * MIN, "only time up to the last observed tick counts");
    }

    #[test]
    fn snapshot_restore_round_trip() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.set_task("Read");
        e.start(c.now());
        c.run(&mut e, 4 * MIN);
        let snap = e.snapshot(c.now());

        let mut e2 = Engine::new(short_cfg());
        e2.restore(&snap);
        let v = e2.view(c.now());
        assert_eq!((v.status, v.elapsed_ms, v.task.as_str()), (Status::Paused, 4 * MIN, "Read"));
        e2.start(c.now());
        let events = c.run(&mut e2, 6 * MIN);
        match &events[0] {
            Event::SessionEnded(r) => assert_eq!(r.started_at, snap.started_at.unwrap()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn start_fresh_records_partial_and_resets_cycle() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.restore(&Snapshot {
            mode: Mode::ShortBreak,
            elapsed_ms: 90_000,
            cycle_pos: 2,
            task: String::new(),
            started_at: Some(c.wall - 90_000),
            saved_at: c.wall,
            duration_ms: None,
        });
        let events = e.start_fresh(c.advance(1_000));
        assert_eq!(events.len(), 1);
        let v = e.view(c.now());
        assert_eq!((v.mode, v.status, v.cycle_index), (Mode::Focus, Status::Idle, 1));
    }

    #[test]
    fn new_durations_apply_to_next_session_not_the_running_one() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, 3 * MIN);
        e.set_config(TimerConfig { focus_ms: 2 * MIN, short_break_ms: 7 * MIN, ..short_cfg() });
        let v = e.view(c.now());
        assert_eq!((v.duration_ms, v.remaining_ms), (10 * MIN, 7 * MIN), "running session keeps its length");
        let events = c.run(&mut e, 7 * MIN);
        match &events[0] {
            Event::SessionEnded(r) => assert_eq!(r.duration_ms, 10 * MIN),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(e.view(c.now()).duration_ms, 7 * MIN, "next session uses the new length");
    }

    #[test]
    fn idle_session_picks_up_new_duration_immediately() {
        let c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.set_config(TimerConfig { focus_ms: 50 * MIN, ..short_cfg() });
        assert_eq!(e.view(c.now()).remaining_ms, 50 * MIN);
    }

    #[test]
    fn fewer_sessions_per_cycle_clamps_position() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        for _ in 0..3 {
            e.start(c.now());
            let d = e.duration_ms();
            c.run(&mut e, d);
            e.skip(c.now()); // skip the break
        }
        e.set_config(TimerConfig { sessions_before_long: 2, ..short_cfg() });
        let v = e.view(c.now());
        assert_eq!((v.cycle_index, v.cycle_len), (2, 2));
        e.start(c.now());
        c.run(&mut e, 10 * MIN);
        assert_eq!(e.mode, Mode::LongBreak);
    }

    #[test]
    fn restoring_keeps_original_length_and_reads_old_snapshots() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        c.run(&mut e, MIN);
        let snap = e.snapshot(c.now());
        assert_eq!(snap.duration_ms, Some(10 * MIN));

        let mut e2 = Engine::new(TimerConfig { focus_ms: 30 * MIN, ..short_cfg() });
        e2.restore(&snap);
        assert_eq!(e2.view(c.now()).duration_ms, 10 * MIN);

        let old: Snapshot = serde_json::from_str(
            r#"{"mode":"focus","elapsed_ms":60000,"cycle_pos":0,"task":"","started_at":1,"saved_at":2}"#,
        )
        .unwrap();
        e2.restore(&old);
        assert_eq!(e2.view(c.now()).duration_ms, 30 * MIN);
    }

    #[test]
    fn view_never_reports_negative_or_overflowing_time() {
        let mut c = Clock::new();
        let mut e = Engine::new(short_cfg());
        e.start(c.now());
        let now = c.advance(11 * MIN); // ticks were late
        let v = e.view(now);
        assert_eq!(v.remaining_ms, 0);
        assert_eq!(v.elapsed_ms, 10 * MIN);
    }
}
