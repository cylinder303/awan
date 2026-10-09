import { test } from "node:test";
import assert from "node:assert/strict";
import { formatClock, interpolate } from "../src/js/format.js";

test("formatClock pads minutes and seconds", () => {
  assert.equal(formatClock(25 * 60_000), "25:00");
  assert.equal(formatClock(5_000), "00:05");
  assert.equal(formatClock(0), "00:00");
});

test("formatClock rounds partial seconds up so 0 shows only at the end", () => {
  assert.equal(formatClock(59_001), "01:00");
  assert.equal(formatClock(1), "00:01");
});

test("formatClock shows hours for long sessions", () => {
  assert.equal(formatClock(65 * 60_000), "1:05:00");
});

test("formatClock never goes negative", () => {
  assert.equal(formatClock(-3_000), "00:00");
});

test("formatClock uses Latin digits only", () => {
  assert.match(formatClock(123_456), /^[0-9:]+$/);
});

test("interpolate fills known placeholders and keeps unknown ones", () => {
  assert.equal(interpolate("{a} of {b}", { a: 2, b: 4 }), "2 of 4");
  assert.equal(interpolate("{a} {missing}", { a: 1 }), "1 {missing}");
});
