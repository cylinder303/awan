// Entry point for the main window.

import { app, syncApi, timer } from "./api.js";
import { store } from "./store.js";
import { settings } from "./settings.js";
import { detectLanguage, getLanguage, setLanguage, t } from "./i18n.js";
import { applyTheme } from "./theme.js";
import { renderIcons } from "./icons.js";
import { formatClock, intlLocale } from "./format.js";
import { initTitlebar } from "./titlebar.js";
import { initAndroidActions, initPlatform, isMobile, showSession } from "./platform.js";
import { currentPage, initRouter, navigate } from "./router.js";
import { initShortcuts } from "./shortcuts.js";
import { openDialog } from "./components/dialog.js";
import { installGuard } from "./guard.js";

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
  initPlatform();
  installGuard();
  await settings.load();
  settings.subscribe(applySettings);
  renderIcons();
  if (!isMobile) initTitlebar();

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

  // A phone syncs with its computer whenever the app comes back to the front.
  if (isMobile) {
    document.addEventListener("visibilitychange", () => {
      if (!document.hidden) syncApi.now();
    });
    initAndroid();
  }
}

/** Android: back button, notification and tile actions, and the session notification. */
function initAndroid() {
  initAndroidActions({
    back() {
      if (document.body.dataset.modal) {
        document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
        return true;
      }
      if (currentPage() !== "timer") {
        navigate("timer");
        return true;
      }
      return false;
    },
    action(name) {
      if (name === "toggle") timer.toggle();
      else if (name === "skip") timer.skip();
    },
  });

  // Only tell Android when something it shows changes, not every second.
  let shown = "";
  const show = (view) => {
    const endsAt = view.status === "running" ? Date.now() + view.remainingMs : 0;
    const key = [view.status, view.mode, view.task, Math.round(endsAt / 5000), getLanguage()].join("|");
    if (key === shown) return;
    shown = key;
    const mode = t(`mode.${view.mode}`);
    const clock = new Intl.DateTimeFormat(intlLocale(getLanguage()), { hour: "numeric", minute: "2-digit" });
    showSession({
      status: view.status,
      title: view.task ? `${mode} · ${view.task}` : mode,
      endsAt,
      text: t("session.paused", { left: formatClock(view.remainingMs) }),
      pause: t("session.pause"),
      resume: t("session.resume"),
      skip: t("session.skip"),
      channel: t("session.channel"),
      openHint: t("session.open_hint"),
      tile:
        view.status === "running"
          ? t("session.tile_running", { mode, end: clock.format(endsAt) })
          : view.status === "paused"
            ? t("session.tile_paused")
            : t("session.tile_idle", { mode }),
    });
  };
  store.subscribe(show);
}

boot();
