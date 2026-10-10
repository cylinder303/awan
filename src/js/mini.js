// Entry point for the mini timer window.

import { app, appWindow, timer } from "./api.js";
import { settings } from "./settings.js";
import { store } from "./store.js";
import { applyTheme } from "./theme.js";
import { detectLanguage, setLanguage, t } from "./i18n.js";
import { formatClock } from "./format.js";
import { icons, renderIcons } from "./icons.js";
import { initShortcuts } from "./shortcuts.js";

const root = document.getElementById("mini");
const time = root.querySelector(".mini__time");
const fill = root.querySelector(".mini__fill");
const toggleBtn = root.querySelector('[data-action="toggle"]');
const expandBtn = root.querySelector('[data-action="expand"]');

function label(btn, key) {
  btn.title = t(key);
  btn.setAttribute("aria-label", btn.title);
}

function render(view) {
  document.body.dataset.mode = view.mode;
  root.dataset.status = view.status;
  time.textContent = formatClock(view.remainingMs);
  const running = view.status === "running";
  toggleBtn.innerHTML = running ? icons.pause : icons.play;
  label(toggleBtn, running ? "timer.pause" : view.status === "paused" ? "timer.resume" : "timer.start");
  label(expandBtn, "titlebar.full");
  animate();
}

// Glide the progress line between the once-a-second updates.
let frame = 0;
function animate() {
  cancelAnimationFrame(frame);
  const view = store.get();
  if (!view) return;
  const elapsed = view.elapsedMs + (view.status === "running" ? store.age() : 0);
  fill.style.width = `${Math.min(100, (elapsed / view.durationMs) * 100)}%`;
  if (view.status === "running") frame = requestAnimationFrame(animate);
}

function applySettings(s) {
  applyTheme(s);
  root.style.setProperty("--mini-alpha", `${s.miniOpacity}%`);
  setLanguage(s.language === "auto" ? detectLanguage() : s.language);
}

async function boot() {
  await settings.load();
  settings.subscribe(applySettings);
  renderIcons();

  const initial = await timer.getState();
  store.set(initial.view);
  store.subscribe(render);
  await timer.onState((view) => store.set(view));

  toggleBtn.addEventListener("click", () => timer.toggle());
  expandBtn.addEventListener("click", () => app.toggleView());

  // Drag from anywhere except the buttons and the resize edges; a double
  // click returns to the full window. Same pattern as Tauri's own drag
  // regions: the second press (detail 2) is the double click, because the
  // native drag started by the first press swallows the "dblclick" event.
  root.addEventListener("mousedown", (e) => {
    if (e.button !== 0 || e.target.closest("button")) return;
    const edge = e.target.closest(".edge");
    if (edge) appWindow.startResizeDragging(edge.dataset.dir);
    else if (e.detail >= 2) app.toggleView();
    else appWindow.startDragging();
  });
  initShortcuts();

  await document.fonts.ready;
  requestAnimationFrame(() => app.ready());
}

boot();
