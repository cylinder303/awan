//! The few strings shown by the native side (notifications). The UI tells
//! the backend which language is active.

use crate::timer::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ar,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Self {
        if s.starts_with("ar") {
            Lang::Ar
        } else {
            Lang::En
        }
    }
}

/// Notification title and body for a finished session.
pub fn finished(lang: Lang, ended: Mode, next: Mode) -> (&'static str, &'static str) {
    let title = match (lang, ended) {
        (Lang::Ar, Mode::Focus) => "انتهت جلسة التركيز",
        (Lang::Ar, _) => "انتهت الاستراحة",
        (Lang::En, Mode::Focus) => "Focus session complete",
        (Lang::En, _) => "Break is over",
    };
    let body = match (lang, next) {
        (Lang::Ar, Mode::Focus) => "حان وقت العودة إلى التركيز.",
        (Lang::Ar, Mode::ShortBreak) => "خذ استراحة قصيرة.",
        (Lang::Ar, Mode::LongBreak) => "أحسنت! خذ استراحة طويلة.",
        (Lang::En, Mode::Focus) => "Time to focus again.",
        (Lang::En, Mode::ShortBreak) => "Take a short break.",
        (Lang::En, Mode::LongBreak) => "Well done! Take a long break.",
    };
    (title, body)
}

/// "20 h" / "1 h 30 min" (Arabic: "20 س" / "1 س 30 د").
pub fn duration(lang: Lang, minutes: u32) -> String {
    let (h_unit, m_unit) = match lang {
        Lang::Ar => ("س", "د"),
        Lang::En => ("h", "min"),
    };
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} {m_unit}"),
        (h, 0) => format!("{h} {h_unit}"),
        (h, m) => format!("{h} {h_unit} {m} {m_unit}"),
    }
}

/// Notification for a reached goal.
pub fn goal_reached(lang: Lang, weekly: bool, goal_minutes: u32) -> (&'static str, String) {
    let goal = duration(lang, goal_minutes);
    match (lang, weekly) {
        (Lang::Ar, true) => ("أحسنت! حققت هدفك الأسبوعي", format!("أتممت {goal} من التركيز هذا الأسبوع.")),
        (Lang::Ar, false) => ("حققت هدف اليوم", format!("أتممت {goal} من التركيز اليوم.")),
        (Lang::En, true) => ("Weekly goal reached!", format!("You've focused for {goal} this week.")),
        (Lang::En, false) => ("Daily goal reached", format!("You've focused for {goal} today.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(Lang::En, 1200), "20 h");
        assert_eq!(duration(Lang::En, 90), "1 h 30 min");
        assert_eq!(duration(Lang::Ar, 45), "45 د");
    }
}
