import { test } from "node:test";
import assert from "node:assert/strict";
import { addDays, dayKey, lastDays, periodRanges, startOfDay, startOfWeek } from "../src/js/dates.js";
import { formatDuration, formatTime } from "../src/js/format.js";
import { csvField, sessionsToCsv } from "../src/js/csv.js";

// Friday 9 October 2026, 15:30 local time.
const NOW = new Date(2026, 9, 9, 15, 30).getTime();

test("startOfDay is local midnight", () => {
  assert.equal(startOfDay(NOW), new Date(2026, 9, 9).getTime());
});

test("week start follows the chosen first day", () => {
  assert.equal(dayKey(startOfWeek(NOW, "saturday")), "2026-10-03");
  assert.equal(dayKey(startOfWeek(NOW, "sunday")), "2026-10-04");
  assert.equal(dayKey(startOfWeek(NOW, "monday")), "2026-10-05");
  // On the first day itself, the week starts that day.
  const saturday = new Date(2026, 9, 10, 8).getTime();
  assert.equal(dayKey(startOfWeek(saturday, "saturday")), "2026-10-10");
});

test("period ranges cover today, the week and the month", () => {
  const r = periodRanges(NOW, "sunday");
  assert.deepEqual(r.today.map(dayKey), ["2026-10-09", "2026-10-10"]);
  assert.deepEqual(r.week.map(dayKey), ["2026-10-04", "2026-10-11"]);
  assert.deepEqual(r.month.map(dayKey), ["2026-10-01", "2026-11-01"]);
});

test("lastDays returns consecutive days ending today, oldest first", () => {
  const days = lastDays(NOW, 7);
  assert.equal(days.length, 7);
  assert.equal(dayKey(days[0][0]), "2026-10-03");
  assert.equal(dayKey(days[6][0]), "2026-10-09");
  days.forEach(([from, to]) => assert.equal(dayKey(to), dayKey(addDays(from, 1))));
});

test("adding days crosses month ends", () => {
  assert.equal(dayKey(addDays(new Date(2026, 0, 31).getTime(), 1)), "2026-02-01");
});

test("formatDuration is compact in both languages", () => {
  assert.equal(formatDuration(25 * 60000, "en"), "25 min");
  assert.equal(formatDuration(65 * 60000, "en"), "1 h 5 min");
  assert.equal(formatDuration(120 * 60000, "en"), "2 h");
  assert.equal(formatDuration(0, "en"), "0 min");
  assert.equal(formatDuration(25 * 60000, "ar"), "25 د");
  assert.equal(formatDuration(65 * 60000, "ar"), "1 س 5 د");
});

test("times use Latin digits in Arabic too", () => {
  assert.match(formatTime(NOW, "ar"), /3:30/);
  assert.doesNotMatch(formatTime(NOW, "ar"), /[٠-٩]/);
});

test("csvField quotes only when needed", () => {
  assert.equal(csvField("plain"), "plain");
  assert.equal(csvField('say "hi", ok'), '"say ""hi"", ok"');
  assert.equal(csvField("two\nlines"), '"two\nlines"');
});

test("sessionsToCsv writes a header and one row per session, oldest first", () => {
  const t = (k) => k;
  const sessions = [
    { mode: "short_break", startedAt: NOW + 3_600_000, endedAt: NOW + 3_900_000, durationMs: 300000, task: "", completed: true },
    { mode: "focus", startedAt: NOW, endedAt: NOW + 1_500_000, durationMs: 1500000, task: "Report, draft", completed: false },
  ];
  const lines = sessionsToCsv(sessions, t, "en").trim().split("\r\n");
  assert.equal(lines.length, 3);
  assert.equal(lines[0], "csv.date,csv.start,csv.end,csv.minutes,csv.type,csv.task,csv.status");
  assert.match(lines[1], /^10\/09\/2026,3:30\s?PM,3:55\s?PM,25\.0,mode\.focus,"Report, draft",history\.stopped$/);
  assert.match(lines[2], /mode\.short_break,,history\.completed$/);
});
