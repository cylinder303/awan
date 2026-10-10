// Keyboard combinations for the global shortcut, in the format the native
// side understands: "Ctrl+Alt+Space", "Ctrl+Shift+P", "Alt+F9".

const NAMED = new Set([
  "Space", "Enter", "Tab", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "PageDown",
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Minus", "Equal", "BracketLeft",
  "BracketRight", "Backslash", "Semicolon", "Quote", "Backquote", "Comma", "Period", "Slash",
]);

/** The main key of a KeyboardEvent.code, or null for modifiers/unsupported keys. */
export function keyName(code) {
  let m;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1];
  if ((m = /^Digit(\d)$/.exec(code))) return m[1];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  if (/^Numpad\d$/.test(code)) return code;
  return NAMED.has(code) ? code : null;
}

/**
 * Build a combination from a key press, or null while only modifiers are
 * held. Physical key codes make it independent of the keyboard layout.
 */
export function comboFromEvent(e) {
  const key = keyName(e.code);
  if (!key) return null;
  const parts = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(key);
  return parts.join("+");
}

const LABELS = { Super: "Win", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→" };

/** Split a combination into labels for display: ["Ctrl", "Alt", "Space"]. */
export function formatCombo(combo) {
  return combo.split("+").filter(Boolean).map((p) => LABELS[p] ?? p);
}
