// Applies the chosen fonts to a window: the interface font, the timer font,
// and the width every timer digit gets, so the time never shifts while it
// counts down, whatever the font (many fonts have digits of different widths).

import { fontsApi } from "./api.js";
import { clockParts, fileFamily, fontStacks, parseChoice, widestDigit } from "./font-list.js";

let files = [];
let filesLoading = null;
const faces = new Map(); // file id -> Promise<boolean>
const canvas = document.createElement("canvas");

/** The added font files, as last read from the backend. */
export function fileFonts() {
  return files;
}

/** Read the list of added font files again. */
export async function refreshFileFonts() {
  filesLoading = fontsApi.files().then(
    (list) => (files = list),
    () => files,
  );
  return filesLoading;
}

/** Load an added font file into this window (once). Resolves false if it cannot be. */
export function loadFileFont(file) {
  if (!faces.has(file.id)) {
    const [min, max] = file.weight;
    const loading = fontsApi
      .data(file.id)
      .then((data) => new FontFace(fileFamily(file.id), data, { weight: min === max ? String(min) : `${min} ${max}` }).load())
      .then((face) => {
        document.fonts.add(face);
        return true;
      })
      .catch(() => {
        faces.delete(file.id);
        return false;
      });
    faces.set(file.id, loading);
  }
  return faces.get(file.id);
}

/** Make sure the added fonts these choices name are known and loaded. */
async function prepare(values) {
  const ids = values.map(parseChoice).filter((c) => c.kind === "file").map((c) => c.id);
  if (ids.length === 0) return;
  if (!filesLoading || ids.some((id) => !files.some((f) => f.id === id))) refreshFileFonts();
  await filesLoading;
  await Promise.all(ids.map((id) => files.find((f) => f.id === id)).filter(Boolean).map(loadFileFont));
}

/** Width of the widest digit of a font stack at a weight, in em. */
export async function digitWidth(stack, weight) {
  const font = `${weight} 100px ${stack}`;
  try {
    await document.fonts.load(font, "0123456789");
  } catch {
    // An unusable font: measure whatever the browser falls back to.
  }
  const ctx = canvas.getContext("2d");
  ctx.font = font;
  const width = widestDigit((d) => ctx.measureText(d).width) / 100;
  return width > 0.2 && width < 2 ? Math.round(width * 1000) / 1000 : 0.6;
}

let applied = 0;

/**
 * Apply the font settings to this window. `weight` is the weight of the
 * timer digits here (300 on the dial, 400 in mini mode). Also used for the
 * live preview in the font picker, then again with the saved settings.
 */
export async function applyFonts(s, { weight = 300 } = {}) {
  const id = ++applied;
  await prepare([s.fontUi, s.fontTimer]);
  if (id !== applied) return;
  const stacks = fontStacks(s, files);
  const width = await digitWidth(stacks.timer, weight);
  if (id !== applied) return;
  const root = document.documentElement.style;
  root.setProperty("--font", stacks.ui);
  root.setProperty("--font-timer", stacks.timer);
  root.setProperty("--digit-w", `${width}em`);
}

/**
 * Show a clock face ("25:00") with each digit in a box of the same width.
 * The element's text stays the plain clock, for screen readers and tests.
 */
export function setClock(el, text) {
  const parts = clockParts(text);
  if (el.children.length !== parts.length) {
    el.replaceChildren(...parts.map(() => document.createElement("span")));
  }
  parts.forEach(({ char, digit }, i) => {
    const span = el.children[i];
    const cls = digit ? "clock__digit" : "clock__sep";
    if (span.className !== cls) span.className = cls;
    if (span.textContent !== char) span.textContent = char;
  });
}
