// In-app keyboard shortcuts. Physical key codes are used so they work with
// any keyboard layout (R, S and M are "ق", "س" and "ة" on an Arabic layout).

import { app, timer } from "./api.js";

/** In-app shortcuts (also listed on the settings page). */
export const LOCAL_SHORTCUTS = [
  { code: "Space", key: "Space", action: "shortcuts.toggle", run: () => timer.toggle() },
  { code: "KeyR", key: "R", action: "shortcuts.reset", run: () => timer.reset() },
  { code: "KeyS", key: "S", action: "shortcuts.skip", run: () => timer.skip() },
  { code: "KeyM", key: "M", action: "shortcuts.view", run: () => app.toggleView() },
];

const bindings = Object.fromEntries(LOCAL_SHORTCUTS.map((s) => [s.code, s.run]));

function isTyping(el) {
  return el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

export function initShortcuts() {
  window.addEventListener("keydown", (e) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.repeat) return;
    if (document.body.dataset.modal || isTyping(e.target)) return;
    const action = bindings[e.code];
    if (!action) return;
    // Stop Space from also "clicking" a focused button.
    e.preventDefault();
    action();
  });
}
