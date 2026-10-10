// Make the web view behave like a desktop app: no browser context menu
// (except copy/paste in text fields) and no reload, print, find or
// back/forward shortcuts.

const BLOCKED = new Set(["F3", "F5", "F7", "F12", "BrowserBack", "BrowserForward", "BrowserRefresh"]);
const BLOCKED_WITH_CTRL = new Set(["KeyR", "KeyP", "KeyF", "KeyG", "KeyU", "KeyJ"]);

function isEditable(el) {
  return el?.closest?.("input, textarea, [contenteditable='true']");
}

export function installGuard() {
  document.addEventListener("contextmenu", (e) => {
    if (!isEditable(e.target)) e.preventDefault();
  });
  document.addEventListener(
    "keydown",
    (e) => {
      const ctrl = e.ctrlKey || e.metaKey;
      if (
        BLOCKED.has(e.code) ||
        (ctrl && BLOCKED_WITH_CTRL.has(e.code)) ||
        (ctrl && e.shiftKey && e.code === "KeyI") ||
        (e.altKey && (e.code === "ArrowLeft" || e.code === "ArrowRight"))
      ) {
        e.preventDefault();
      }
    },
    true,
  );
}
