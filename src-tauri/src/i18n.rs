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
