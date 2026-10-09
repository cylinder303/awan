// Entry point for the main window.

import { app, timer } from "./api.js";
import { store } from "./store.js";
import { detectLanguage, setLanguage, t } from "./i18n.js";
import { setTheme } from "./theme.js";
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
      elapsed: formatClock(view.elapsed_ms),
      duration: formatClock(view.duration_ms),
    }),
    actions: [
      { label: t("resume.fresh"), value: false },
      { label: t("resume.continue"), value: true, kind: "primary" },
    ],
    dismissible: false,
  });
  timer.resolveResume(resume);
}

async function boot() {
  setTheme("auto");
  const lang = detectLanguage();
  setLanguage(lang);
  app.setLanguage(lang);
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
