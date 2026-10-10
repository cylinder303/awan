// Goals page: weekly and daily focus goals, week start, and past weeks.

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
    const [totals, recorded, first] = await Promise.all([
      history.focusTotals([periods.today, periods.week, ...past]),
      history.weekGoals(),
      history.firstSessionAt(),
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
      root.append(weekCard(week, s.weeklyGoalMinutes, lang));
    }
    root.append(weeklyGoalControl(), dailyCard(today, s, lang, first != null), weekStartControl(lang));

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
      const key = [s.weeklyGoalMinutes, s.dailyGoalEnabled, s.dailyGoalMinutes, s.weekStart].join();
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
