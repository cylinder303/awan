import { test } from "node:test";
import assert from "node:assert/strict";
import { goalForWeek } from "../src/js/goal-history.js";
import { parseNumber } from "../src/js/components/controls.js";

const recorded = [
  { weekKey: "2026-09-12", goalMinutes: 600 },
  { weekKey: "2026-09-26", goalMinutes: 1200 },
];

test("a week uses the goal in effect at the time", () => {
  assert.equal(goalForWeek("2026-09-12", recorded, 900), 600);
  assert.equal(goalForWeek("2026-09-19", recorded, 900), 600, "no record that week: previous goal");
  assert.equal(goalForWeek("2026-10-03", recorded, 900), 1200);
});

test("weeks before any record use the oldest record; no records use the fallback", () => {
  assert.equal(goalForWeek("2026-08-01", recorded, 900), 600);
  assert.equal(goalForWeek("2026-08-01", [], 900), 900);
});

test("numbers typed with Arabic-Indic digits are understood", () => {
  assert.equal(parseNumber("٤٫٥"), 4.5);
  assert.equal(parseNumber("25"), 25);
  assert.equal(parseNumber("1,5"), 1.5);
  assert.ok(Number.isNaN(parseNumber("abc")));
});

import { comboFromEvent, formatCombo, keyName } from "../src/js/keys.js";

test("key presses become shortcut strings independent of layout", () => {
  assert.equal(comboFromEvent({ code: "Space", ctrlKey: true, altKey: true }), "Ctrl+Alt+Space");
  assert.equal(comboFromEvent({ code: "KeyP", ctrlKey: true, shiftKey: true }), "Ctrl+Shift+P");
  assert.equal(comboFromEvent({ code: "F9", altKey: true }), "Alt+F9");
  assert.equal(comboFromEvent({ code: "Digit1", metaKey: true }), "Super+1");
  assert.equal(comboFromEvent({ code: "ControlLeft", ctrlKey: true }), null, "modifier alone");
  assert.equal(keyName("IntlBackslash"), null);
});

test("shortcuts display with friendly key names", () => {
  assert.deepEqual(formatCombo("Ctrl+Super+ArrowUp"), ["Ctrl", "Win", "↑"]);
  assert.deepEqual(formatCombo(""), []);
});
