// CSV export of the session history.

import { formatTime, intlLocale } from "./format.js";

/** Quote a field when it contains a comma, quote or line break. */
export function csvField(value) {
  const s = String(value ?? "");
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/**
 * @param {object[]} sessions stored sessions (any order)
 * @param {(key: string) => string} t translation function
 * @param {string} lang "ar" or "en"
 */
export function sessionsToCsv(sessions, t, lang) {
  const date = new Intl.DateTimeFormat(intlLocale(lang), { year: "numeric", month: "2-digit", day: "2-digit" });
  const header = ["date", "start", "end", "minutes", "type", "task", "status"].map((k) => t(`csv.${k}`));
  const rows = [...sessions]
    .sort((a, b) => a.startedAt - b.startedAt)
    .map((s) => [
      date.format(s.startedAt),
      formatTime(s.startedAt, lang),
      formatTime(s.endedAt, lang),
      (s.durationMs / 60000).toFixed(1),
      t(`mode.${s.mode}`),
      s.task,
      t(s.completed ? "history.completed" : "history.stopped"),
    ]);
  return [header, ...rows].map((r) => r.map(csvField).join(",")).join("\r\n") + "\r\n";
}
