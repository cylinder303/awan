import { test } from "node:test";
import assert from "node:assert/strict";
import { contrast, isDark, luminance, parseHex, readableOn, rotateHue, toHex } from "../src/js/color.js";

test("parseHex accepts #rrggbb and rejects anything else", () => {
  assert.deepEqual(parseHex("#ff8000"), { r: 255, g: 128, b: 0 });
  assert.deepEqual(parseHex("FF8000"), { r: 255, g: 128, b: 0 });
  assert.equal(parseHex("#fff"), null);
  assert.equal(parseHex("red"), null);
});

test("toHex round-trips and clamps", () => {
  assert.equal(toHex(parseHex("#1b2230")), "#1b2230");
  assert.equal(toHex({ r: 300, g: -5, b: 127.6 }), "#ff0080");
});

test("luminance matches WCAG reference points", () => {
  assert.equal(luminance("#000000"), 0);
  assert.equal(luminance("#ffffff"), 1);
});

test("contrast ratio matches WCAG examples", () => {
  assert.equal(contrast("#000000", "#ffffff"), 21);
  assert.equal(contrast("#ffffff", "#000000"), 21, "order does not matter");
  assert.equal(contrast("#777777", "#777777"), 1);
  // #767676 on white is the classic "just passes AA" gray.
  assert.ok(contrast("#767676", "#ffffff") >= 4.5);
  assert.ok(contrast("#777777", "#ffffff") < 4.5);
});

test("rotateHue keeps the color for full turns and changes it otherwise", () => {
  assert.equal(rotateHue("#e8a15f", 360), "#e8a15f");
  assert.equal(rotateHue("#ff0000", 120), "#00ff00");
  assert.equal(rotateHue("#808080", 90), "#808080", "grays have no hue");
});

test("readableOn picks the text color with more contrast", () => {
  assert.equal(readableOn("#ffffff"), "#000000");
  assert.equal(readableOn("#1b2230"), "#ffffff");
  assert.equal(readableOn("#e8a15f"), "#000000");
});

test("isDark separates dark and light backgrounds", () => {
  assert.ok(isDark("#1b2230"));
  assert.ok(!isDark("#f6f4f0"));
});
