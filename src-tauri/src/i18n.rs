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

/// Name of the Android notification channel for one alert sound, as shown
/// in the phone's notification settings.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn alert_channel(lang: Lang, sound: Option<&str>) -> String {
    let label = match (lang, sound) {
        (Lang::Ar, Some("drops")) => "قطرات ماء",
        (Lang::Ar, Some("knock")) => "طرقات على الباب",
        (Lang::Ar, Some("birds")) => "عصافير",
        (Lang::Ar, Some("pebble")) => "حصاة",
        (Lang::Ar, Some("wave")) => "موج البحر",
        (Lang::Ar, _) => "بلا صوت",
        (Lang::En, Some("drops")) => "Water drops",
        (Lang::En, Some("knock")) => "Door knock",
        (Lang::En, Some("birds")) => "Birds",
        (Lang::En, Some("pebble")) => "Pebble",
        (Lang::En, Some("wave")) => "Sea wave",
        (Lang::En, _) => "Silent",
    };
    match lang {
        Lang::Ar => format!("نهاية الجلسة: {label}"),
        Lang::En => format!("Session end: {label}"),
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


pub fn mode_name(lang: Lang, mode: Mode) -> &'static str {
    match (lang, mode) {
        (Lang::Ar, Mode::Focus) => "تركيز",
        (Lang::Ar, Mode::ShortBreak) => "استراحة قصيرة",
        (Lang::Ar, Mode::LongBreak) => "استراحة طويلة",
        (Lang::En, Mode::Focus) => "Focus",
        (Lang::En, Mode::ShortBreak) => "Short break",
        (Lang::En, Mode::LongBreak) => "Long break",
    }
}

/// "Awan · Focus · 12:34" (adds "paused" when paused).
pub fn tray_tooltip(lang: Lang, view: &crate::timer::TimerView) -> String {
    let secs = view.remaining_ms.div_ceil(1000);
    let time = format!("{:02}:{:02}", secs / 60, secs % 60);
    let name = match lang {
        Lang::Ar => "أوان",
        Lang::En => "Awan",
    };
    let paused = match (lang, view.status) {
        (Lang::Ar, crate::timer::Status::Paused) => " (متوقف مؤقتًا)",
        (Lang::En, crate::timer::Status::Paused) => " (paused)",
        _ => "",
    };
    // Isolate the time so it reads left-to-right inside Arabic text.
    format!("{name} · {} · \u{2066}{time}\u{2069}{paused}", mode_name(lang, view.mode))
}

pub struct TrayLabels {
    pub start: &'static str,
    pub pause: &'static str,
    pub resume: &'static str,
    pub skip: &'static str,
    pub show: &'static str,
    pub mini: &'static str,
    pub full: &'static str,
    pub quit: &'static str,
}

pub fn tray_labels(lang: Lang) -> TrayLabels {
    match lang {
        Lang::Ar => TrayLabels {
            start: "ابدأ",
            pause: "إيقاف مؤقت",
            resume: "استئناف",
            skip: "تخطٍّ",
            show: "إظهار النافذة",
            mini: "الوضع المصغّر",
            full: "الوضع الكامل",
            quit: "إنهاء",
        },
        Lang::En => TrayLabels {
            start: "Start",
            pause: "Pause",
            resume: "Resume",
            skip: "Skip",
            show: "Show window",
            mini: "Mini mode",
            full: "Full mode",
            quit: "Quit",
        },
    }
}

/// Shown once, the first time closing the window hides it to the tray.
pub fn still_running(lang: Lang) -> (&'static str, &'static str) {
    match lang {
        Lang::Ar => ("أوان ما زال يعمل", "المؤقت مستمر في علبة النظام. يمكنك تغيير ذلك من الإعدادات."),
        Lang::En => ("Awan is still running", "The timer keeps going in the system tray. You can change this in Settings."),
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
