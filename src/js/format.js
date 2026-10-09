// Formatting helpers. Numbers always use Latin digits (0-9), in both languages.

/** Remaining time for the clock face: "25:00", or "1:05:00" past an hour. */
export function formatClock(ms) {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/** Replace {name} placeholders in a template string. */
export function interpolate(template, params = {}) {
  return template.replace(/\{(\w+)\}/g, (match, key) => (key in params ? String(params[key]) : match));
}

const UNITS = {
  ar: { h: "س", m: "د" },
  en: { h: "h", m: "min" },
};

/** Compact length: "25 min", "1 h 5 min" (Arabic: "25 د", "1 س 5 د"). */
export function formatDuration(ms, lang = "en") {
  const u = UNITS[lang] || UNITS.en;
  const minutes = Math.max(0, Math.round(ms / 60000));
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} ${u.m}`;
  if (m === 0) return `${h} ${u.h}`;
  return `${h} ${u.h} ${m} ${u.m}`;
}

/** Locale tag for Intl: Gregorian calendar, Latin digits, in both languages. */
export function intlLocale(lang) {
  return `${lang === "ar" ? "ar" : "en"}-u-ca-gregory-nu-latn`;
}

export function formatTime(ts, lang) {
  return new Intl.DateTimeFormat(intlLocale(lang), { hour: "numeric", minute: "2-digit" }).format(ts);
}
