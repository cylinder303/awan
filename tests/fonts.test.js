import { test } from "node:test";
import assert from "node:assert/strict";
import {
  BUNDLED_FONTS,
  DEFAULT_FONT,
  FALLBACK,
  clockParts,
  fileFamily,
  fontStack,
  fontStacks,
  parseChoice,
  quoteFamily,
  resolveFont,
  widestDigit,
} from "../src/js/font-list.js";

const file = { id: "0123456789abcdef", family: "My Font", style: "Bold", weight: [700, 700], arabic: false, digits: true };

test("bundled fonts have unique ids and the default comes first", () => {
  const ids = BUNDLED_FONTS.map((f) => f.id);
  assert.equal(new Set(ids).size, ids.length);
  assert.equal(ids[0], DEFAULT_FONT);
  for (const id of ids) assert.match(id, /^[a-z0-9-]{1,32}$/, "the backend accepts it");
});

test("every bundled font is declared in fonts.css", async () => {
  const { readFile } = await import("node:fs/promises");
  const css = await readFile(new URL("../src/styles/fonts.css", import.meta.url), "utf8");
  for (const f of BUNDLED_FONTS) assert.ok(css.includes(`font-family: "${f.family}";`), f.family);
});

test("choices are read by kind", () => {
  assert.equal(parseChoice("cairo").font.id, "cairo");
  assert.deepEqual(parseChoice("system:Segoe UI"), { kind: "system", family: "Segoe UI" });
  assert.deepEqual(parseChoice("file:0123456789abcdef"), { kind: "file", id: "0123456789abcdef" });
  assert.deepEqual(parseChoice("ui"), { kind: "ui" });
  // Anything unknown is the default font.
  for (const bad of ["", "nope", "system:", "file:123", undefined]) assert.equal(parseChoice(bad).font.id, DEFAULT_FONT);
});

test("an added font resolves while it is there, then falls back", () => {
  const found = resolveFont("file:0123456789abcdef", [file]);
  assert.equal(found.family, fileFamily(file.id));
  assert.equal(found.name, "My Font Bold");
  assert.equal(resolveFont("file:0123456789abcdef", []).value, DEFAULT_FONT);
});

test("font stacks always end with the default fallbacks", () => {
  assert.equal(fontStack("plex"), FALLBACK);
  assert.equal(fontStack("cairo"), `"Awan Cairo", ${FALLBACK}`);
  assert.equal(fontStack("system:Segoe UI"), `"Segoe UI", ${FALLBACK}`);
  assert.equal(fontStack("file:0123456789abcdef", [file]), `"Awan File 0123456789abcdef", ${FALLBACK}`);
});

test("the timer follows the interface font unless it has its own", () => {
  assert.deepEqual(fontStacks({ fontUi: "tajawal", fontTimer: "ui" }), {
    ui: fontStack("tajawal"),
    timer: fontStack("tajawal"),
  });
  assert.equal(fontStacks({ fontUi: "tajawal", fontTimer: "cairo" }).timer, fontStack("cairo"));
});

test("family names cannot break out of the CSS string", () => {
  assert.equal(quoteFamily('Evil", red; x'), '"Evil\\", red; x"');
  assert.equal(quoteFamily("Back\\slash"), '"Back\\\\slash"');
  assert.equal(quoteFamily("a\nb"), '"a b"');
  assert.equal(quoteFamily("خط النسخ"), '"خط النسخ"');
});

test("clock parts mark digits and separators", () => {
  assert.deepEqual(clockParts("1:05:00").map((p) => p.digit), [true, false, true, true, false, true, true]);
  assert.equal(clockParts("25:00").map((p) => p.char).join(""), "25:00");
});

test("the widest digit sets the width of every digit", () => {
  const widths = { 1: 40, 7: 52, 0: 61 };
  assert.equal(widestDigit((d) => widths[d] ?? 55), 61);
  assert.equal(widestDigit(() => NaN), 0);
});
