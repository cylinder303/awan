// Timer page: mode tabs, progress ring, task field and controls.

import { history, timer } from "../api.js";
import { store } from "../store.js";
import { t, translate, onLanguageChange } from "../i18n.js";
import { formatClock } from "../format.js";
import { icons, renderIcons } from "../icons.js";
import { createRing } from "../components/ring.js";
import { openDialog } from "../components/dialog.js";
import { attachSuggestions } from "../components/suggest.js";

const MODES = ["focus", "short_break", "long_break"];

const template = `
  <section class="timer">
    <div class="modes" role="tablist">
      ${MODES.map((m) => `<button class="modes__item" role="tab" data-value="${m}" data-i18n="mode.${m}"></button>`).join("")}
    </div>

    <div class="dial">
      <div class="dial__center">
        <div class="dial__time num" role="timer" aria-live="off"></div>
        <div class="dial__cycle"></div>
      </div>
    </div>

    <div class="task-wrap">
      <input class="task" type="text" maxlength="120" spellcheck="false" autocomplete="off"
             data-i18n-placeholder="timer.task_placeholder" />
    </div>

    <div class="controls">
      <button class="icon-btn" data-action="reset" data-i18n-title="timer.reset" data-icon="reset"></button>
      <button class="play-btn" data-action="toggle"></button>
      <button class="icon-btn" data-action="skip" data-i18n-title="timer.skip" data-icon="skip"></button>
    </div>

    <p class="notice" hidden data-i18n="timer.sleep_notice"></p>
  </section>`;

export function mount(root) {
  root.innerHTML = template;
  translate(root);
  renderIcons(root);

  const dial = root.querySelector(".dial");
  const time = root.querySelector(".dial__time");
  const cycle = root.querySelector(".dial__cycle");
  const task = root.querySelector(".task");
  const play = root.querySelector(".play-btn");
  const notice = root.querySelector(".notice");
  const tabs = [...root.querySelectorAll(".modes__item")];
  const ring = createRing();
  dial.prepend(ring.el);

  let frame = 0;

  function render(view) {
    time.textContent = formatClock(view.remainingMs);
    cycle.textContent = t("timer.cycle", { index: view.cycleIndex, total: view.cycleLen });
    tabs.forEach((tab) => tab.setAttribute("aria-selected", String(tab.dataset.value === view.mode)));

    const running = view.status === "running";
    play.innerHTML = running ? icons.pause : icons.play;
    const label = running ? t("timer.pause") : view.status === "paused" ? t("timer.resume") : t("timer.start");
    play.title = label;
    play.setAttribute("aria-label", label);
    notice.hidden = !view.pausedBySleep;

    if (document.activeElement !== task && task.value !== view.task) task.value = view.task;

    cancelAnimationFrame(frame);
    if (running) animate();
    else ring.set(view.elapsedMs / view.durationMs);
  }

  // Between backend updates, glide the ring using the time since the last update.
  function animate() {
    const view = store.get();
    if (!view || view.status !== "running") return;
    ring.set((view.elapsedMs + store.age()) / view.durationMs);
    frame = requestAnimationFrame(animate);
  }

  async function chooseMode(mode) {
    const view = store.get();
    if (!view || mode === view.mode) return;
    if (view.status !== "idle") {
      const ok = await openDialog({
        title: t("switch.title"),
        body: t("switch.body"),
        actions: [
          { label: t("common.cancel"), value: false },
          { label: t("switch.confirm"), value: true, kind: "primary" },
        ],
      });
      if (!ok) return;
    }
    timer.setMode(mode);
  }

  // Registered first so it can claim Enter/Escape while its list is open.
  attachSuggestions(task, () => history.recentTasks(), (value) => timer.setTask(value));
  let typing = 0;
  task.addEventListener("input", () => {
    clearTimeout(typing);
    typing = setTimeout(() => timer.setTask(task.value), 400);
  });
  task.addEventListener("change", () => {
    clearTimeout(typing);
    timer.setTask(task.value);
  });
  task.addEventListener("keydown", (e) => {
    if (e.defaultPrevented) return; // handled by the suggestion list
    if (e.key === "Enter" || e.key === "Escape") task.blur();
  });

  root.querySelector('[data-action="toggle"]').addEventListener("click", () => timer.toggle());
  root.querySelector('[data-action="reset"]').addEventListener("click", () => timer.reset());
  root.querySelector('[data-action="skip"]').addEventListener("click", () => timer.skip());
  tabs.forEach((tab) => tab.addEventListener("click", () => chooseMode(tab.dataset.value)));

  let flash = 0;
  const unsubscribe = [
    store.subscribe(render),
    store.onFinished(() => {
      dial.classList.remove("is-finished");
      void dial.offsetWidth; // restart the animation
      dial.classList.add("is-finished");
      clearTimeout(flash);
      flash = setTimeout(() => dial.classList.remove("is-finished"), 2000);
    }),
    onLanguageChange(() => store.get() && render(store.get())),
  ];

  return () => {
    cancelAnimationFrame(frame);
    clearTimeout(typing);
    clearTimeout(flash);
    unsubscribe.forEach((off) => off());
  };
}
