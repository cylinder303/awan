//! Streak: how many days (or weeks) in a row the goal was met, counted from
//! the history, so it shows up right away and is the same on every linked
//! device.
//!
//! - Daily: a day counts when its focus time reaches the daily goal, or,
//!   without a daily goal, when it has at least one completed focus session.
//! - Rest days the user picked never break a streak (and count if met).
//! - Weekly: a week counts when its focus time reaches that week's goal.
//! - Today (or this week) never breaks a streak before it is over.

use std::collections::HashMap;

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use serde::Serialize;

use crate::db::Db;
use crate::goals;
use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Daily,
    Weekly,
}

impl Mode {
    pub fn parse(s: &str) -> Mode {
        if s == "weekly" {
            Mode::Weekly
        } else {
            Mode::Daily
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Mode::Daily => "daily",
            Mode::Weekly => "weekly",
        }
    }
}

pub struct Rules {
    pub mode: Mode,
    /// Focus a day needs; None: one completed focus session is enough.
    pub daily_goal_ms: Option<u64>,
    pub rest_days: Vec<Weekday>,
    pub week_start: Weekday,
    /// Goal of weeks with no recorded goal (the current setting).
    pub weekly_goal_ms: u64,
    /// Goal recorded for each week, by its start date "YYYY-MM-DD".
    pub week_goals: HashMap<String, u64>,
}

/// Focus in one local day.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Day {
    pub focus_ms: u64,
    pub completed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Streak {
    pub current: u32,
    pub best: u32,
    /// Today (daily) or this week (weekly) already counts.
    pub done: bool,
}

pub fn weekday(name: &str) -> Option<Weekday> {
    Some(match name {
        "saturday" => Weekday::Sat,
        "sunday" => Weekday::Sun,
        "monday" => Weekday::Mon,
        "tuesday" => Weekday::Tue,
        "wednesday" => Weekday::Wed,
        "thursday" => Weekday::Thu,
        "friday" => Weekday::Fri,
        _ => return None,
    })
}

pub fn local_date(ms: i64) -> NaiveDate {
    Local.timestamp_millis_opt(ms).single().map(|dt| dt.date_naive()).unwrap_or_default()
}

/// Group focus sessions (start, length, completed) by the local day they started.
pub fn days_from(sessions: &[(i64, u64, bool)]) -> HashMap<NaiveDate, Day> {
    let mut days: HashMap<NaiveDate, Day> = HashMap::new();
    for &(started_at, duration_ms, completed) in sessions {
        let day = days.entry(local_date(started_at)).or_default();
        day.focus_ms += duration_ms;
        day.completed += u32::from(completed);
    }
    days
}

impl Rules {
    pub fn day_met(&self, day: Day) -> bool {
        match self.daily_goal_ms {
            Some(goal) => day.focus_ms >= goal.max(1),
            None => day.completed > 0,
        }
    }

    pub fn is_rest(&self, date: NaiveDate) -> bool {
        self.rest_days.contains(&date.weekday())
    }

    fn week_goal(&self, week: NaiveDate) -> u64 {
        let key = week.format("%Y-%m-%d").to_string();
        self.week_goals.get(&key).copied().unwrap_or(self.weekly_goal_ms).max(1)
    }
}

pub fn compute(days: &HashMap<NaiveDate, Day>, today: NaiveDate, rules: &Rules) -> Streak {
    let Some(&first) = days.keys().min() else {
        return Streak { current: 0, best: 0, done: false };
    };
    match rules.mode {
        Mode::Daily => {
            let (mut run, mut best) = (0, 0);
            let mut date = first;
            while date <= today {
                let met = rules.day_met(days.get(&date).copied().unwrap_or_default());
                if met {
                    run += 1;
                    best = best.max(run);
                } else if !rules.is_rest(date) && date != today {
                    run = 0;
                }
                date += Duration::days(1);
            }
            let done = rules.day_met(days.get(&today).copied().unwrap_or_default());
            Streak { current: run, best, done }
        }
        Mode::Weekly => {
            let mut weeks: HashMap<NaiveDate, u64> = HashMap::new();
            for (date, day) in days {
                *weeks.entry(goals::week_start_date(*date, rules.week_start)).or_default() += day.focus_ms;
            }
            let this_week = goals::week_start_date(today, rules.week_start);
            let met = |week: NaiveDate| weeks.get(&week).copied().unwrap_or(0) >= rules.week_goal(week);
            let (mut run, mut best) = (0, 0);
            let mut week = goals::week_start_date(first, rules.week_start);
            while week <= this_week {
                if met(week) {
                    run += 1;
                    best = best.max(run);
                } else if week != this_week {
                    run = 0;
                }
                week += Duration::days(7);
            }
            Streak { current: run, best, done: met(this_week) }
        }
    }
}

/// One day of the map on the goals page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapDay {
    pub date: String,
    pub focus_ms: u64,
    pub met: bool,
    pub rest: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub mode: &'static str,
    #[serde(flatten)]
    pub streak: Streak,
    /// The last `weeks` weeks, oldest first, up to today.
    pub days: Vec<MapDay>,
}

pub fn rules(settings: &Settings, db: &Db) -> Rules {
    Rules {
        mode: Mode::parse(&settings.streak_mode),
        daily_goal_ms: settings.daily_goal_enabled.then(|| settings.daily_goal_minutes as u64 * 60_000),
        rest_days: settings.rest_days.iter().filter_map(|d| weekday(d)).collect(),
        week_start: goals::first_weekday(&settings.week_start),
        weekly_goal_ms: settings.weekly_goal_minutes as u64 * 60_000,
        week_goals: db
            .week_goals()
            .unwrap_or_default()
            .into_iter()
            .map(|g| (g.week_key, g.goal_minutes as u64 * 60_000))
            .collect(),
    }
}

/// The streak, plus the day map for the last `weeks` weeks.
pub fn overview(db: &Db, settings: &Settings, now_ms: i64, weeks: u32) -> Result<Overview, String> {
    let rules = rules(settings, db);
    let days = days_from(&db.focus_sessions()?);
    let today = local_date(now_ms);
    let streak = compute(&days, today, &rules);
    let mut map = Vec::new();
    if weeks > 0 {
        let mut date = goals::week_start_date(today, rules.week_start) - Duration::days(7 * (weeks as i64 - 1));
        while date <= today {
            let day = days.get(&date).copied().unwrap_or_default();
            map.push(MapDay {
                date: date.format("%Y-%m-%d").to_string(),
                focus_ms: day.focus_ms,
                met: rules.day_met(day),
                rest: rules.is_rest(date),
            });
            date += Duration::days(1);
        }
    }
    Ok(Overview { mode: rules.mode.key(), streak, days: map })
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u64 = 3_600_000;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn rules(mode: Mode) -> Rules {
        Rules {
            mode,
            daily_goal_ms: None,
            rest_days: vec![],
            week_start: Weekday::Sat,
            weekly_goal_ms: 10 * H,
            week_goals: HashMap::new(),
        }
    }

    /// One completed focus session on each listed day (2026-10-01 is a Thursday).
    fn sessions_on(dates: &[&str]) -> HashMap<NaiveDate, Day> {
        dates.iter().map(|d| (date(d), Day { focus_ms: H, completed: 1 })).collect()
    }

    #[test]
    fn counts_days_in_a_row_and_today_does_not_break_it() {
        let days = sessions_on(&["2026-10-01", "2026-10-02", "2026-10-03", "2026-10-05", "2026-10-06"]);
        let r = rules(Mode::Daily);
        // Today (7th) has nothing yet: the streak is still 2 (5th, 6th).
        assert_eq!(compute(&days, date("2026-10-07"), &r), Streak { current: 2, best: 3, done: false });
        // A full day without focus ends it.
        assert_eq!(compute(&days, date("2026-10-08"), &r), Streak { current: 0, best: 3, done: false });
        assert_eq!(compute(&days, date("2026-10-06"), &r), Streak { current: 2, best: 3, done: true });
    }

    #[test]
    fn rest_days_do_not_break_a_streak() {
        // The 2nd is a Friday, left out.
        let days = sessions_on(&["2026-10-01", "2026-10-03", "2026-10-04"]);
        let mut r = rules(Mode::Daily);
        assert_eq!(compute(&days, date("2026-10-04"), &r).current, 2);
        r.rest_days = vec![Weekday::Fri];
        assert_eq!(compute(&days, date("2026-10-04"), &r), Streak { current: 3, best: 3, done: true });
    }

    #[test]
    fn with_a_daily_goal_only_days_that_reach_it_count() {
        let mut days = sessions_on(&["2026-10-01", "2026-10-02"]);
        days.insert(date("2026-10-03"), Day { focus_ms: 3 * H, completed: 3 });
        days.insert(date("2026-10-04"), Day { focus_ms: 2 * H, completed: 0 }); // stopped early, still focus
        let r = Rules { daily_goal_ms: Some(2 * H), ..rules(Mode::Daily) };
        assert_eq!(compute(&days, date("2026-10-04"), &r), Streak { current: 2, best: 2, done: true });
    }

    #[test]
    fn weekly_streak_uses_each_weeks_own_goal() {
        // Weeks start on Saturday: 2026-09-19, 09-26, 10-03, 10-10.
        let mut days = HashMap::new();
        days.insert(date("2026-09-20"), Day { focus_ms: 12 * H, completed: 10 });
        days.insert(date("2026-09-27"), Day { focus_ms: 6 * H, completed: 5 });
        days.insert(date("2026-10-04"), Day { focus_ms: 11 * H, completed: 9 });
        let mut r = rules(Mode::Weekly);
        let today = date("2026-10-11");
        assert_eq!(compute(&days, today, &r), Streak { current: 1, best: 1, done: false });
        // The week of 09-26 had a lower goal back then: it counted.
        r.week_goals.insert("2026-09-26".into(), 5 * H);
        assert_eq!(compute(&days, today, &r), Streak { current: 3, best: 3, done: false });
    }

    #[test]
    fn no_history_no_streak() {
        assert_eq!(compute(&HashMap::new(), date("2026-10-01"), &rules(Mode::Daily)), Streak { current: 0, best: 0, done: false });
    }

    #[test]
    fn sessions_are_grouped_by_local_day() {
        let noon = Local.from_local_datetime(&date("2026-10-01").and_hms_opt(12, 0, 0).unwrap()).single().unwrap();
        let ms = noon.timestamp_millis();
        let days = days_from(&[(ms, H, true), (ms + 1_000, H / 2, false)]);
        assert_eq!(days.get(&date("2026-10-01")), Some(&Day { focus_ms: H + H / 2, completed: 1 }));
    }
}
