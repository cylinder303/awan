//! Goal periods in local time, and detecting when a goal is reached.

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};

/// Local day and week containing a moment, as [from, to) UTC milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Periods {
    pub today: (i64, i64),
    pub week: (i64, i64),
    /// Start date of the week, "YYYY-MM-DD" (matches the UI's week keys).
    pub week_key: String,
}

pub fn first_weekday(week_start: &str) -> Weekday {
    match week_start {
        "sunday" => Weekday::Sun,
        "monday" => Weekday::Mon,
        _ => Weekday::Sat,
    }
}

/// The date the week containing `date` starts on.
pub fn week_start_date(date: NaiveDate, first: Weekday) -> NaiveDate {
    let back = (7 + date.weekday().num_days_from_monday() - first.num_days_from_monday()) % 7;
    date - Duration::days(back as i64)
}

/// Local midnight of `date` in UTC ms. Where midnight is skipped by a
/// daylight-saving change, the first valid moment of that day is used.
fn local_midnight_ms(date: NaiveDate) -> i64 {
    let mut t = date.and_hms_opt(0, 0, 0).expect("valid midnight");
    for _ in 0..4 {
        if let Some(dt) = Local.from_local_datetime(&t).earliest() {
            return dt.timestamp_millis();
        }
        t += Duration::minutes(30);
    }
    Local.from_utc_datetime(&t).timestamp_millis()
}

pub fn periods(now_ms: i64, week_start: &str) -> Periods {
    let today = Local
        .timestamp_millis_opt(now_ms)
        .single()
        .map(|dt| dt.date_naive())
        .unwrap_or_default();
    let week = week_start_date(today, first_weekday(week_start));
    Periods {
        today: (local_midnight_ms(today), local_midnight_ms(today + Duration::days(1))),
        week: (local_midnight_ms(week), local_midnight_ms(week + Duration::days(7))),
        week_key: week.format("%Y-%m-%d").to_string(),
    }
}

/// True when adding focus time moved a total from below `goal_ms` to at or above it.
pub fn crossed(before_ms: u64, after_ms: u64, goal_ms: u64) -> bool {
    goal_ms > 0 && before_ms < goal_ms && after_ms >= goal_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn week_start_for_each_choice() {
        let friday = d(2026, 10, 9);
        assert_eq!(week_start_date(friday, Weekday::Sat), d(2026, 10, 3));
        assert_eq!(week_start_date(friday, Weekday::Sun), d(2026, 10, 4));
        assert_eq!(week_start_date(friday, Weekday::Mon), d(2026, 10, 5));
        let saturday = d(2026, 10, 10);
        assert_eq!(week_start_date(saturday, Weekday::Sat), saturday);
    }

    #[test]
    fn periods_are_ordered_and_contain_now() {
        let now = 1_791_590_000_000; // October 2026
        let p = periods(now, "sunday");
        assert!(p.today.0 <= now && now < p.today.1);
        assert!(p.week.0 <= now && now < p.week.1);
        assert!(p.week.0 <= p.today.0 && p.today.1 <= p.week.1);
        assert_eq!(p.week_key.len(), 10);
    }

    #[test]
    fn crossing_happens_once() {
        let h = 3_600_000;
        assert!(crossed(19 * h, 20 * h, 20 * h));
        assert!(crossed(19 * h, 21 * h, 20 * h));
        assert!(!crossed(20 * h, 21 * h, 20 * h), "already reached before");
        assert!(!crossed(10 * h, 19 * h, 20 * h), "not yet reached");
    }
}
