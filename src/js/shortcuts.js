// In-app keyboard shortcuts. Physical key codes are used so they work with
// any keyboard layout (R and S are "ق" and "س" on an Arabic layout).

import { timer } from "./api.js";

const bindings = {
  Space: () => timer.toggle(),
  KeyR: () => timer.reset(),
  KeyS: () => timer.skip(),
};

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
