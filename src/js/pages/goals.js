// Goals page: weekly and daily focus goals, the streak and its day map,
// week start, and past weeks.

import { history } from "../api.js";
import { settings } from "../settings.js";
import { getLanguage, onLanguageChange, t } from "../i18n.js";
import { formatDuration, intlLocale } from "../format.js";
import { addDays, dayKey, periodRanges, startOfWeek } from "../dates.js";
import { goalForWeek } from "../goal-history.js";
import { icons } from "../icons.js";
import { createRing } from "../components/ring.js";
import { segmented, stepper, toggle } from "../components/controls.js";

const PAST_WEEKS = 12;
/** Weeks shown on the day map (about five months). */
const MAP_WEEKS = 22;
const WEEKDAYS = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];

export function mount(root) {
  let alive = true;
  let token = 0;

  async function render() {
    const my = ++token;
    const lang = getLanguage();
    const s = settings.get();
    const now = Date.now();
    const periods = periodRanges(now, s.weekStart);
    const weekStart = startOfWeek(now, s.weekStart);
    const past = Array.from({ length: PAST_WEEKS }, (_, i) => {
      const from = addDays(weekStart, -7 * (i + 1));
      return [from, addDays(from, 7)];
    });
    const [totals, recorded, first, streak] = await Promise.all([
      history.focusTotals([periods.today, periods.week, ...past]),
      history.weekGoals(),
      history.firstSessionAt(),
      history.streakOverview(MAP_WEEKS),
    ]);
    if (!alive || my !== token) return;
    const [today, week, ...pastTotals] = totals;

    const scroll = root.scrollTop;
    root.innerHTML = "";
    const header = el("div", "page-header");
    const title = el("h1", "page-title");
    title.textContent = t("nav.goals");
    header.append(title);
    root.append(header);

    if (first == null) {
      root.append(emptyCard());
    } else {
      root.append(weekCard(week, s.weeklyGoalMinutes, lang), streakCard(streak, lang), dayMap(streak, s, lang));
    }
    root.append(
      weeklyGoalControl(),
      dailyCard(today, s, lang, first != null),
      weekStartControl(lang),
      streakSettings(s, lang),
    );

    if (first != null) {
      const weeks = past
        .map(([from], i) => ({ from, total: pastTotals[i] }))
        .filter(({ from }) => addDays(from, 7) > first);
      if (weeks.length) root.append(pastWeeks(weeks, recorded, s.weeklyGoalMinutes, lang));
    }
    root.scrollTop = scroll;
  }

  function weekCard(doneMs, goalMinutes, lang) {
    const goalMs = goalMinutes * 60000;
    const card = el("section", "card goal-hero");
    const ring = createRing();
    ring.set(doneMs / goalMs);
    const ringBox = el("div", "goal-ring");
    const pct = el("span", "goal-ring__pct num");
    pct.textContent = `${Math.min(100, Math.floor((doneMs / goalMs) * 100))}%`;
    ringBox.append(ring.el, pct);

    const text = el("div", "goal-hero__text");
    const label = el("span", "goal-hero__label");
    label.textContent = t("goals.this_week");
    const value = el("span", "goal-hero__value");
    value.textContent = formatDuration(doneMs, lang);
    const of = el("span", "goal-hero__of");
    of.textContent = t("goals.of", { goal: formatDuration(goalMs, lang) });
    const left = el("span", "goal-hero__left");
    if (doneMs >= goalMs) {
      left.innerHTML = `${icons.check}<span></span>`;
      left.querySelector("span").textContent = t("goals.reached");
      left.classList.add("is-reached");
    } else {
      left.textContent = t("goals.left", { time: formatDuration(goalMs - doneMs, lang) });
    }
    text.append(label, value, of, left);
    card.append(ringBox, text);
    return card;
  }

  /** "12 days" with the right plural form ("one day" / "يوم واحد"...). */
  function span(n, weekly, lang) {
    const form = new Intl.PluralRules(intlLocale(lang)).select(n);
    return t(`streak.${weekly ? "weeks" : "days"}.${form}`, { n });
  }

  function streakCard(o, lang) {
    const weekly = o.mode === "weekly";
    const card = el("section", "card streak" + (o.current > 0 ? " is-active" : ""));
    const icon = el("span", "streak__icon");
    icon.innerHTML = icons.streak;
    const text = el("div", "streak__text");
    const label = el("span", "streak__label");
    label.textContent = t("streak.title");
    const value = el("span", "streak__value");
    const count = el("b", "num");
    count.textContent = String(o.current);
    const unit = el("span");
    const form = new Intl.PluralRules(intlLocale(lang)).select(o.current);
    unit.textContent = `${t(`streak.unit_${weekly ? "weeks" : "days"}.${form}`)} ${t("streak.in_a_row")}`;
    value.append(count, unit);
    const meta = el("span", "streak__meta");
    const parts = [t("streak.best", { span: span(o.best, weekly, lang) })];
    if (o.done) parts.push(t(weekly ? "streak.week_done" : "streak.today_done"));
    meta.textContent = parts.join(" · ");
    text.append(label, value, meta);
    card.append(icon, text);
    return card;
  }

  /** The last few months, one square per day, darker with more focus. */
  function dayMap(o, s, lang) {
    const section = el("section", "settings-section");
    const title = el("h2", "settings-section__title");
    title.textContent = t("streak.map_title");
    const card = el("div", "card day-map");
    const months = el("div", "day-map__months");
    const grid = el("div", "day-map__grid");
    const goal = s.dailyGoalEnabled ? s.dailyGoalMinutes * 60000 : 0;
    const most = Math.max(1, ...o.days.map((d) => d.focusMs));
    const monthName = new Intl.DateTimeFormat(intlLocale(lang), { month: "short" });
    const dayName = new Intl.DateTimeFormat(intlLocale(lang), { weekday: "long", day: "numeric", month: "long" });
    let lastMonth = null;
    o.days.forEach((d, i) => {
      const [y, m, dd] = d.date.split("-").map(Number);
      const date = new Date(y, m - 1, dd);
      const cell = el("span", "day-map__day");
      const level = d.focusMs === 0 ? 0 : Math.min(4, Math.max(1, Math.ceil((d.focusMs / (goal || most)) * 4)));
      cell.dataset.level = String(level);
      if (d.rest) cell.classList.add("is-rest");
      cell.title = `${dayName.format(date)} · ${formatDuration(d.focusMs, lang)}`;
      grid.append(cell);
      // Month names over the first week of each month.
      if (i % 7 === 0) {
        const label = el("span", "day-map__month");
        const monthOfWeek = new Date(y, m - 1, dd + 6).getMonth();
        if (monthOfWeek !== lastMonth) {
          label.textContent = monthName.format(new Date(y, m - 1, dd + 6));
          lastMonth = monthOfWeek;
        }
        months.append(label);
      }
    });
    const legend = el("div", "day-map__legend");
    const less = el("span");
    less.textContent = t("streak.less");
    const more = el("span");
    more.textContent = t("streak.more");
    legend.append(less);
    for (let level = 0; level <= 4; level++) {
      const sample = el("span", "day-map__day");
      sample.dataset.level = String(level);
      legend.append(sample);
    }
    legend.append(more);
    card.append(months, grid, legend);
    section.append(title, card);
    return section;
  }

  function streakSettings(s, lang) {
    const card = el("section", "card goal-settings");
    const mode = segmented({
      label: t("streak.mode"),
      options: [
        { value: "daily", label: t("streak.daily") },
        { value: "weekly", label: t("streak.weekly") },
      ],
      onChange: (streakMode) => settings.update({ streakMode }),
    });
    mode.set(s.streakMode);
    const head = el("div", "row row--stacked");
    const text = el("div", "row__text");
    const label = el("span", "row__label");
    label.textContent = t("streak.mode");
    const hint = el("span", "row__hint");
    hint.textContent = t(s.streakMode === "weekly" ? "streak.weekly_hint" : s.dailyGoalEnabled ? "streak.daily_goal_hint" : "streak.daily_hint");
    text.append(label, hint);
    head.append(text, mode.el);
    card.append(head);
    if (s.streakMode === "weekly") return card;

    // Rest days, in the order of the week as the user starts it.
    const first = WEEKDAYS.indexOf(s.weekStart);
    const order = Array.from({ length: 7 }, (_, i) => WEEKDAYS[(first + i) % 7]);
    const short = new Intl.DateTimeFormat(intlLocale(lang), { weekday: "short" });
    const chips = el("div", "day-chips");
    chips.setAttribute("role", "group");
    chips.setAttribute("aria-label", t("streak.rest_days"));
    for (const day of order) {
      const chip = el("button", "chip");
      chip.type = "button";
      // 2026-10-04 is a Sunday.
      chip.textContent = short.format(new Date(2026, 9, 4 + WEEKDAYS.indexOf(day)));
      const on = s.restDays.includes(day);
      chip.setAttribute("aria-pressed", String(on));
      chip.addEventListener("click", () => {
        const rest = settings.get().restDays;
        const next = rest.includes(day) ? rest.filter((d) => d !== day) : [...rest, day];
        if (next.length < 7) settings.update({ restDays: next });
      });
      chips.append(chip);
    }
    const restRow = el("div", "row row--stacked");
    const restText = el("div", "row__text");
    const restLabel = el("span", "row__label");
    restLabel.textContent = t("streak.rest_days");
    const restHint = el("span", "row__hint");
    restHint.textContent = t("streak.rest_hint");
    restText.append(restLabel, restHint);
    restRow.append(restText, chips);
    card.append(restRow);
    return card;
  }

  function emptyCard() {
    const box = el("div", "card empty empty--card");
    box.innerHTML = `${icons.goals}<h2></h2><p></p>`;
    box.querySelector("h2").textContent = t("goals.empty_title");
    box.querySelector("p").textContent = t("goals.empty_body");
    return box;
  }

  function weeklyGoalControl() {
    const card = el("div", "card goal-settings");
    const c = stepper({
      min: 1,
      max: 100,
      unit: t("unit.hours"),
      label: t("goals.weekly_goal"),
      onChange: (h) => settings.update({ weeklyGoalMinutes: Math.round(h * 60) }),
    });
    c.set(settings.get().weeklyGoalMinutes / 60);
    card.append(row(t("goals.weekly_goal"), c.el));
    return card;
  }

  function dailyCard(doneMs, s, lang, hasData) {
    const card = el("section", "card goal-settings");
    const sw = toggle({ label: t("goals.daily_goal"), onChange: (on) => settings.update({ dailyGoalEnabled: on }) });
    sw.set(s.dailyGoalEnabled);
    card.append(row(t("goals.daily_goal"), sw.el, t("goals.daily_hint")));
    if (!s.dailyGoalEnabled) return card;

    const goalMs = s.dailyGoalMinutes * 60000;
    const c = stepper({
      min: 0.5,
      max: 16,
      step: 0.5,
      unit: t("unit.hours"),
      label: t("goals.daily_goal_hours"),
      onChange: (h) => settings.update({ dailyGoalMinutes: Math.round(h * 60) }),
    });
    c.set(s.dailyGoalMinutes / 60);
    card.append(row(t("goals.daily_goal_hours"), c.el));

    if (hasData) {
      const progress = el("div", "meter-row");
      const meter = el("div", "meter");
      meter.setAttribute("role", "progressbar");
      meter.setAttribute("aria-valuemin", "0");
      meter.setAttribute("aria-valuemax", String(goalMs));
      meter.setAttribute("aria-valuenow", String(Math.min(doneMs, goalMs)));
      meter.setAttribute("aria-label", t("goals.today"));
      const fill = el("div", "meter__fill");
      fill.style.width = `${Math.min(100, (doneMs / goalMs) * 100)}%`;
      meter.append(fill);
      const caption = el("div", "meter-row__caption");
      const done = el("span");
      done.textContent = t("goals.today_progress", { done: formatDuration(doneMs, lang), goal: formatDuration(goalMs, lang) });
      const left = el("span", doneMs >= goalMs ? "is-reached" : "");
      left.textContent = doneMs >= goalMs ? t("goals.reached") : t("goals.left", { time: formatDuration(goalMs - doneMs, lang) });
      caption.append(done, left);
      progress.append(meter, caption);
      card.append(progress);
    }
    return card;
  }

  function weekStartControl(lang) {
    const card = el("section", "card goal-settings");
    const day = new Intl.DateTimeFormat(intlLocale(lang), { weekday: "long" });
    // 2026-10-03 is a Saturday; the next two days are Sunday and Monday.
    const names = [0, 1, 2].map((i) => day.format(new Date(2026, 9, 3 + i)));
    const c = segmented({
      label: t("goals.week_starts"),
      options: [
        { value: "saturday", label: names[0] },
        { value: "sunday", label: names[1] },
        { value: "monday", label: names[2] },
      ],
      onChange: (weekStart) => settings.update({ weekStart }),
    });
    c.set(settings.get().weekStart);
    const head = el("div", "row row--stacked");
    const label = el("span", "row__label");
    label.textContent = t("goals.week_starts");
    head.append(label, c.el);
    card.append(head);
    return card;
  }

  function pastWeeks(weeks, recorded, fallback, lang) {
    const section = el("section", "settings-section");
    const title = el("h2", "settings-section__title");
    title.textContent = t("goals.previous_weeks");
    const list = el("ul", "card week-list");
    const range = new Intl.DateTimeFormat(intlLocale(lang), { day: "numeric", month: "short" });
    for (const { from, total } of weeks) {
      const goalMs = goalForWeek(dayKey(from), recorded, fallback) * 60000;
      const achieved = total >= goalMs;
      const li = el("li", "week");
      const name = el("span", "week__name tnum");
      name.textContent = range.formatRange(from, addDays(from, 6));
      const bar = el("span", "week__bar");
      const fill = el("span", "week__fill");
      fill.style.width = `${Math.min(100, (total / goalMs) * 100)}%`;
      bar.append(fill);
      const value = el("span", "week__value");
      value.textContent = `${formatDuration(total, lang)} / ${formatDuration(goalMs, lang)}`;
      const status = el("span", "week__status" + (achieved ? " is-reached" : ""));
      status.innerHTML = achieved ? icons.check : icons.minus;
      status.title = t(achieved ? "goals.achieved" : "goals.missed");
      status.setAttribute("aria-label", status.title);
      li.append(name, bar, value, status);
      list.append(li);
    }
    section.append(title, list);
    return section;
  }

  render();
  const offs = [onLanguageChange(render)];
  // Re-render when goal-related settings change (not while typing in a stepper).
  let last = null;
  offs.push(
    settings.subscribe((s) => {
      const key = [s.weeklyGoalMinutes, s.dailyGoalEnabled, s.dailyGoalMinutes, s.weekStart, s.streakMode, s.restDays].join();
      if (last !== null && key !== last) render();
      last = key;
    }),
  );
  history.onChanged(() => alive && render()).then((off) => offs.push(off));
  return () => {
    alive = false;
    offs.forEach((off) => off());
  };
}

function el(tag, className) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}

function row(label, control, hint) {
  const r = el("div", "row");
  const text = el("div", "row__text");
  const l = el("span", "row__label");
  l.textContent = label;
  text.append(l);
  if (hint) {
    const h = el("span", "row__hint");
    h.textContent = hint;
    text.append(h);
  }
  r.append(text, control);
  return r;
}
