// Switches between the four pages. Each page module exports
// mount(root) -> cleanup().

import * as timerPage from "./pages/timer.js";
import * as settingsPage from "./pages/settings.js";
import * as historyPage from "./pages/history.js";
import { create as soon } from "./pages/soon.js";

const pages = {
  timer: timerPage.mount,
  history: historyPage.mount,
  goals: soon("goals"),
  settings: settingsPage.mount,
};

let current = null;
let cleanup = () => {};

export function navigate(name) {
  if (!(name in pages) || name === current) return;
  cleanup();
  current = name;
  const view = document.getElementById("view");
  // Re-insert the node so the page entrance animation replays.
  const fresh = view.cloneNode(false);
  view.replaceWith(fresh);
  cleanup = pages[name](fresh) || (() => {});
  document.querySelectorAll(".nav__item").forEach((item) => {
    if (item.dataset.page === name) item.setAttribute("aria-current", "page");
    else item.removeAttribute("aria-current");
  });
}

export function initRouter() {
  document.querySelectorAll(".nav__item").forEach((item) =>
    item.addEventListener("click", () => navigate(item.dataset.page)),
  );
}
