// The fonts the app can show, and how a stored font choice becomes a CSS
// font-family value. No DOM here, so it can be tested on its own.
//
// A choice is stored as one of:
//   "<id>"            a bundled font below (declared in styles/fonts.css)
//   "system:<family>" a font installed on the computer
//   "file:<id>"       a font file the user added (loaded with FontFace)
//   "ui"              timer only: the same font as the interface

export const DEFAULT_FONT = "plex";
export const SAME_AS_UI = "ui";

/** Bundled fonts, in the order they are offered. */
export const BUNDLED_FONTS = [
  { id: "plex", name: "IBM Plex Sans Arabic", family: "Plex Arabic" },
  { id: "cairo", name: "Cairo", family: "Awan Cairo" },
  { id: "tajawal", name: "Tajawal", family: "Awan Tajawal" },
  { id: "readex", name: "Readex Pro", family: "Awan Readex Pro" },
  { id: "naskh", name: "Noto Naskh Arabic", family: "Awan Noto Naskh" },
];

/**
 * What every font falls back to, letter by letter: a font without Arabic
 * letters shows Arabic text in the default font.
 */
export const FALLBACK = `"Plex Arabic", "Segoe UI", system-ui, sans-serif`;

/** A family name as a CSS string, safe whatever characters it holds. */
export function quoteFamily(name) {
  const clean = String(name).replace(/[\u0000-\u001f\u007f]/g, " ");
  return `"${clean.replace(/[\\"]/g, (c) => `\\${c}`)}"`;
}

/** Family name given to an added font file when it is loaded. */
export function fileFamily(id) {
  return `Awan File ${id}`;
}

/** Split a stored choice into its kind and value. */
export function parseChoice(value) {
  const text = String(value ?? "");
  if (text === SAME_AS_UI) return { kind: "ui" };
  if (text.startsWith("system:") && text.length > 7) return { kind: "system", family: text.slice(7) };
  if (/^file:[0-9a-f]{16}$/.test(text)) return { kind: "file", id: text.slice(5) };
  const bundled = BUNDLED_FONTS.find((f) => f.id === text);
  return bundled ? { kind: "bundled", font: bundled } : { kind: "bundled", font: BUNDLED_FONTS[0] };
}

/**
 * The font a choice shows. `files` are the added fonts (from the backend);
 * a file that is no longer there falls back to the default font.
 * Returns { value, family, name, file? } where `value` is the choice as it
 * should be stored and `family` the first name in the CSS font stack.
 */
export function resolveFont(value, files = []) {
  const choice = parseChoice(value);
  if (choice.kind === "system") {
    return { value: `system:${choice.family}`, family: choice.family, name: choice.family };
  }
  if (choice.kind === "file") {
    const file = files.find((f) => f.id === choice.id);
    if (file) return { value: `file:${file.id}`, family: fileFamily(file.id), name: fileName(file), file };
    return resolveFont(DEFAULT_FONT);
  }
  if (choice.kind === "ui") return resolveFont(DEFAULT_FONT);
  const { id, name, family } = choice.font;
  return { value: id, family, name };
}

/** Display name of an added font: its family, and its style unless regular. */
export function fileName(file) {
  return file.style ? `${file.family} ${file.style}` : file.family;
}

/** The CSS font-family value for a choice, with the fallbacks after it. */
export function fontStack(value, files = []) {
  const { family } = resolveFont(value, files);
  return family === "Plex Arabic" ? FALLBACK : `${quoteFamily(family)}, ${FALLBACK}`;
}

/** The interface and timer stacks for a settings object. */
export function fontStacks({ fontUi, fontTimer }, files = []) {
  const ui = fontStack(fontUi, files);
  return { ui, timer: fontTimer === SAME_AS_UI ? ui : fontStack(fontTimer, files) };
}

/** The characters of a clock face ("25:00"), each marked digit or separator. */
export function clockParts(text) {
  return [...String(text)].map((char) => ({ char, digit: char >= "0" && char <= "9" }));
}

/** Widest of the ten digits, given a function that measures one. */
export function widestDigit(measure) {
  let widest = 0;
  for (const d of "0123456789") widest = Math.max(widest, measure(d) || 0);
  return widest;
}
