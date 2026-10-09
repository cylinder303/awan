// Custom title bar: window buttons drawn by the app.
// Dragging and double-click-to-maximize come from data-tauri-drag-region.

import { appWindow } from "./api.js";
import { icons } from "./icons.js";
import { t, onLanguageChange } from "./i18n.js";

export function initTitlebar() {
  const pin = document.getElementById("tb-pin");
  const max = document.getElementById("tb-max");
  let pinned = false;
  let maximized = false;

  function label(btn, key) {
    btn.title = t(key);
    btn.setAttribute("aria-label", btn.title);
  }

  function renderPin() {
    pin.setAttribute("aria-pressed", String(pinned));
    label(pin, pinned ? "titlebar.unpin" : "titlebar.pin");
  }

  function renderMax() {
    max.innerHTML = maximized ? icons.restore : icons.maximize;
    label(max, maximized ? "titlebar.restore" : "titlebar.maximize");
  }

  async function syncMaximized() {
    maximized = await appWindow.isMaximized();
    renderMax();
  }

  document.getElementById("tb-min").addEventListener("click", () => appWindow.minimize());
  document.getElementById("tb-close").addEventListener("click", () => appWindow.close());
  max.addEventListener("click", () => appWindow.toggleMaximize());
  pin.addEventListener("click", async () => {
    pinned = !pinned;
    await appWindow.setAlwaysOnTop(pinned);
    renderPin();
  });

  appWindow.onResized(syncMaximized);
  onLanguageChange(() => {
    renderPin();
    renderMax();
  });
  renderPin();
  syncMaximized();
}
