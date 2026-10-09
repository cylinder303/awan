// Entry point for the main window.

import { app, timer } from "./api.js";
import { store } from "./store.js";
import { settings } from "./settings.js";
import { detectLanguage, setLanguage, t } from "./i18n.js";
import { applyTheme } from "./theme.js";
import { renderIcons } from "./icons.js";
import { formatClock } from "./format.js";
import { initTitlebar } from "./titlebar.js";
import { initRouter, navigate } from "./router.js";
import { initShortcuts } from "./shortcuts.js";
import { openDialog } from "./components/dialog.js";

async function askResume(view) {
  const resume = await openDialog({
    title: t("resume.title"),
    body: t("resume.body", {
      mode: t(`mode.${view.mode}`),
      elapsed: formatClock(view.elapsedMs),
      duration: formatClock(view.durationMs),
    }),
    actions: [
      { label: t("resume.fresh"), value: false },
      { label: t("resume.continue"), value: true, kind: "primary" },
    ],
    dismissible: false,
  });
  timer.resolveResume(resume);
}

let appliedLanguage = null;

/** Apply appearance and language settings to the window. */
function applySettings(s) {
  applyTheme(s);
  const lang = s.language === "auto" ? detectLanguage() : s.language;
  if (lang !== appliedLanguage) {
    appliedLanguage = lang;
    setLanguage(lang);
    app.setLanguage(lang);
  }
}

async function boot() {
  await settings.load();
  settings.subscribe(applySettings);
  renderIcons();
  initTitlebar();

  // The body's data-mode drives the accent color everywhere.
  store.subscribe((view) => (document.body.dataset.mode = view.mode));

  const initial = await timer.getState();
  store.set(initial.view);
  await timer.onState((view) => store.set(view));
  await timer.onFinished((event) => store.finished(event));

  initRouter();
  navigate("timer");
  initShortcuts();

  // Show the window only once fonts and the first frame are ready (no flash).
  await document.fonts.ready;
  requestAnimationFrame(() => app.ready());

  if (initial.pendingResume) askResume(initial.view);
}

boot();
