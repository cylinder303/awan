// Local-calendar helpers. All functions take and return millisecond
// timestamps and use the computer's time zone (DST-safe: days are added
// with the calendar, not as 24-hour blocks).

const WEEK_STARTS = { sunday: 0, monday: 1, saturday: 6 };

export function startOfDay(ts) {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function addDays(ts, days) {
  const d = new Date(ts);
  d.setDate(d.getDate() + days);
  return d.getTime();
}

/** Midnight of the first day of the week containing `ts`. */
export function startOfWeek(ts, weekStart = "saturday") {
  const first = WEEK_STARTS[weekStart] ?? 6;
  const day = startOfDay(ts);
  const back = (new Date(day).getDay() - first + 7) % 7;
  return addDays(day, -back);
}

export function startOfMonth(ts) {
  const d = new Date(startOfDay(ts));
  d.setDate(1);
  return d.getTime();
}

export function addMonths(ts, months) {
  const d = new Date(ts);
  d.setMonth(d.getMonth() + months);
  return d.getTime();
}

/** [from, to) ranges for today, this week and this month. */
export function periodRanges(now, weekStart) {
  const today = startOfDay(now);
  const week = startOfWeek(now, weekStart);
  const month = startOfMonth(now);
  return {
    today: [today, addDays(today, 1)],
    week: [week, addDays(week, 7)],
    month: [month, addMonths(month, 1)],
  };
}

/** One [from, to) range per day for the last `count` days, oldest first. */
export function lastDays(now, count) {
  const today = startOfDay(now);
  return Array.from({ length: count }, (_, i) => {
    const from = addDays(today, i - count + 1);
    return [from, addDays(from, 1)];
  });
}

/** "2026-10-09" in local time, for grouping. */
export function dayKey(ts) {
  const d = new Date(ts);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
