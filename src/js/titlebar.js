// Custom title bar: window buttons drawn by the app.
// Dragging and double-click-to-maximize come from data-tauri-drag-region.

import { app, appWindow } from "./api.js";
import { settings } from "./settings.js";
import { icons } from "./icons.js";
import { t, onLanguageChange } from "./i18n.js";

export function initTitlebar() {
  const pin = document.getElementById("tb-pin");
  const max = document.getElementById("tb-max");
  let maximized = false;

  function label(btn, key) {
    btn.title = t(key);
    btn.setAttribute("aria-label", btn.title);
  }

  function renderPin() {
    const pinned = !!settings.get()?.alwaysOnTop;
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
  document.getElementById("tb-mini").addEventListener("click", () => app.toggleView());
  // The backend applies "always on top" when the setting is saved.
  pin.addEventListener("click", () => settings.update({ alwaysOnTop: !settings.get().alwaysOnTop }));

  appWindow.onResized(syncMaximized);
  settings.subscribe(renderPin);
  onLanguageChange(() => {
    renderPin();
    renderMax();
  });
  syncMaximized();
}
