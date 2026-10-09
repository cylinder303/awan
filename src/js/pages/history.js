// History page: focus totals, a 7-day chart and past sessions by day.

import { history } from "../api.js";
import { settings } from "../settings.js";
import { getLanguage, onLanguageChange, t } from "../i18n.js";
import { formatDuration, formatTime, intlLocale } from "../format.js";
import { dayKey, lastDays, periodRanges, startOfDay, addDays } from "../dates.js";
import { icons } from "../icons.js";
import { sessionsToCsv } from "../csv.js";
import { columnChart } from "../components/bars.js";
import { openDialog } from "../components/dialog.js";
import { toast } from "../components/toast.js";

const PAGE = 200;

export function mount(root) {
  let sessions = [];
  let hasMore = false;
  let alive = true;

  async function load() {
    const first = await history.list(null, PAGE);
    if (!alive) return;
    sessions = first;
    hasMore = first.length === PAGE;
    await render();
  }

  async function loadMore() {
    const older = await history.list(sessions[sessions.length - 1].startedAt, PAGE);
    sessions = sessions.concat(older);
    hasMore = older.length === PAGE;
    await render();
  }

  async function render() {
    const lang = getLanguage();
    const scroll = root.scrollTop;
    root.innerHTML = "";
    if (sessions.length === 0) {
      root.append(emptyState());
      return;
    }

    const now = Date.now();
    const periods = periodRanges(now, settings.get().weekStart);
    const days = lastDays(now, 7);
    const totals = await history.focusTotals([periods.today, periods.week, periods.month, ...days]);
    if (!alive) return;
    const [today, week, month, ...perDay] = totals;

    const header = el("div", "page-header");
    const title = el("h1", "page-title");
    title.textContent = t("nav.history");
    const exportBtn = el("button", "btn btn--quiet btn--sm");
    exportBtn.type = "button";
    exportBtn.innerHTML = `${icons.download}<span></span>`;
    exportBtn.querySelector("span").textContent = t("history.export_csv");
    exportBtn.addEventListener("click", exportCsv);
    header.append(title, exportBtn);

    const tiles = el("div", "stat-row");
    for (const [label, value] of [
      [t("history.today"), today],
      [t("history.this_week"), week],
      [t("history.this_month"), month],
    ]) {
      const tile = el("div", "stat");
      tile.innerHTML = `<span class="stat__value"></span><span class="stat__label"></span>`;
      tile.querySelector(".stat__value").textContent = formatDuration(value, lang);
      tile.querySelector(".stat__label").textContent = label;
      tiles.append(tile);
    }

    const chartCard = el("section", "card chart-card");
    const chartTitle = el("h2", "chart-card__title");
    chartTitle.textContent = t("history.last_7_days");
    const weekday = new Intl.DateTimeFormat(intlLocale(lang), { weekday: lang === "ar" ? "narrow" : "short" });
    const weekdayLong = new Intl.DateTimeFormat(intlLocale(lang), { weekday: "long", day: "numeric", month: "short" });
    chartCard.append(
      chartTitle,
      columnChart({
        caption: t("history.last_7_days"),
        valueHeader: t("history.focus_time"),
        items: days.map(([from], i) => ({
          label: weekday.format(from),
          fullLabel: weekdayLong.format(from),
          value: perDay[i],
          display: formatDuration(perDay[i], lang),
        })),
      }),
    );

    const list = el("div", "history-list");
    for (const [key, group] of groupByDay(sessions)) list.append(dayGroup(key, group, lang));

    root.append(header, tiles, chartCard, list);
    if (hasMore) {
      const more = el("button", "btn btn--quiet load-more");
      more.type = "button";
      more.textContent = t("history.load_more");
      more.addEventListener("click", loadMore);
      root.append(more);
    }
    root.scrollTop = scroll;
  }

  function dayGroup(key, group, lang) {
    const section = el("section", "day");
    const head = el("div", "day__head");
    const name = el("h2", "day__name");
    name.textContent = dayTitle(group[0].startedAt, lang);
    const total = el("span", "day__total");
    const focusMs = group.filter((s) => s.mode === "focus").reduce((sum, s) => sum + s.durationMs, 0);
    total.textContent = t("history.day_focus", { time: formatDuration(focusMs, lang) });
    head.append(name, total);

    const card = el("ul", "card day__list");
    for (const s of group) card.append(sessionRow(s, lang));
    section.append(head, card);
    section.dataset.day = key;
    return section;
  }

  function sessionRow(s, lang) {
    const li = el("li", "session");
    li.dataset.mode = s.mode;
    li.innerHTML = `
      <span class="session__dot" aria-hidden="true"></span>
      <span class="session__main">
        <span class="session__top"><span class="session__time tnum"></span><span class="session__type"></span></span>
        <span class="session__task"></span>
      </span>
      <span class="session__len"></span>
      <span class="session__status"></span>
      <button type="button" class="icon-btn icon-btn--sm session__delete"></button>`;
    li.querySelector(".session__time").textContent = formatTime(s.startedAt, lang);
    li.querySelector(".session__type").textContent = t(`mode.${s.mode}`);
    const task = li.querySelector(".session__task");
    task.textContent = s.task;
    task.hidden = !s.task;
    li.querySelector(".session__len").textContent = formatDuration(s.durationMs, lang);
    const status = li.querySelector(".session__status");
    status.innerHTML = s.completed ? icons.check : icons.stop;
    status.title = t(s.completed ? "history.completed" : "history.stopped");
    status.setAttribute("aria-label", status.title);
    status.classList.toggle("is-stopped", !s.completed);
    const del = li.querySelector(".session__delete");
    del.innerHTML = icons.trash;
    del.title = t("history.delete");
    del.setAttribute("aria-label", del.title);
    del.addEventListener("click", () => confirmDelete(s));
    return li;
  }

  async function confirmDelete(s) {
    const ok = await openDialog({
      title: t("history.delete_title"),
      body: t("history.delete_body"),
      actions: [
        { label: t("common.cancel"), value: false },
        { label: t("history.delete_confirm"), value: true, kind: "danger" },
      ],
    });
    if (ok) await history.remove(s.id); // history://changed reloads the page
  }

  async function exportCsv() {
    const lang = getLanguage();
    let all = [];
    let before = null;
    for (;;) {
      const page = await history.list(before, 1000);
      all = all.concat(page);
      if (page.length < 1000) break;
      before = page[page.length - 1].startedAt;
    }
    const name = `awan-history-${dayKey(Date.now())}.csv`;
    try {
      if (await history.exportCsv(sessionsToCsv(all, t, lang), name)) toast(t("history.exported"));
    } catch {
      toast(t("data.write_failed"), { error: true });
    }
  }

  function emptyState() {
    const box = el("div", "empty");
    box.innerHTML = `${icons.history}<h2></h2><p></p>`;
    box.querySelector("h2").textContent = t("history.empty_title");
    box.querySelector("p").textContent = t("history.empty_body");
    return box;
  }

  load();
  const offs = [];
  history.onChanged(() => alive && load()).then((off) => offs.push(off));
  offs.push(onLanguageChange(() => render()));
  offs.push(settings.subscribe(() => sessions.length && render()));
  return () => {
    alive = false;
    offs.forEach((off) => off());
  };
}

/** Group sessions (newest first) into [dayKey, sessions] pairs. */
function groupByDay(sessions) {
  const groups = new Map();
  for (const s of sessions) {
    const key = dayKey(s.startedAt);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(s);
  }
  return groups;
}

function dayTitle(ts, lang) {
  const day = startOfDay(ts);
  const today = startOfDay(Date.now());
  if (day === today) return t("history.today");
  if (day === addDays(today, -1)) return t("history.yesterday");
  const sameYear = new Date(day).getFullYear() === new Date(today).getFullYear();
  return new Intl.DateTimeFormat(intlLocale(lang), {
    weekday: "long",
    day: "numeric",
    month: "long",
    ...(sameYear ? {} : { year: "numeric" }),
  }).format(day);
}

function el(tag, className) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}
