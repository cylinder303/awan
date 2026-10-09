// Small form controls used by the settings page. Each returns
// { el, set(value) } so the page can refresh values without rebuilding.

import { icons } from "../icons.js";

/** Number field with − / + buttons. */
export function stepper({ min, max, unit, label, onChange }) {
  const el = document.createElement("div");
  el.className = "stepper";
  el.innerHTML = `
    <button type="button" class="stepper__btn" data-step="-1" tabindex="-1" aria-hidden="true">${icons.minus}</button>
    <input class="stepper__input num" type="text" inputmode="numeric" maxlength="3" />
    <button type="button" class="stepper__btn" data-step="1" tabindex="-1" aria-hidden="true">${icons.plus}</button>
    <span class="stepper__unit"></span>`;
  const input = el.querySelector("input");
  input.setAttribute("aria-label", label);
  el.querySelector(".stepper__unit").textContent = unit;
  let value = min;

  const clamp = (v) => Math.min(max, Math.max(min, v));
  function commit(v) {
    const next = clamp(Number.isFinite(v) ? Math.round(v) : value);
    input.value = String(next);
    if (next !== value) {
      value = next;
      onChange(next);
    }
  }

  el.querySelectorAll("[data-step]").forEach((btn) =>
    btn.addEventListener("click", () => commit(value + Number(btn.dataset.step))),
  );
  input.addEventListener("change", () => commit(parseInt(input.value.replace(/[^\d]/g, ""), 10)));
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      commit(value + (e.key === "ArrowUp" ? 1 : -1));
    } else if (e.key === "Enter") {
      input.blur();
    }
  });

  return {
    el,
    set(v) {
      value = v;
      if (document.activeElement !== input) input.value = String(v);
    },
  };
}

/** On/off switch. */
export function toggle({ label, onChange }) {
  const el = document.createElement("button");
  el.type = "button";
  el.className = "switch";
  el.setAttribute("role", "switch");
  el.setAttribute("aria-label", label);
  el.innerHTML = '<span class="switch__thumb"></span>';
  let on = false;
  el.addEventListener("click", () => onChange(!on));
  return {
    el,
    set(v) {
      on = !!v;
      el.setAttribute("aria-checked", String(on));
    },
  };
}

/** Row of mutually exclusive options. */
export function segmented({ options, label, onChange }) {
  const el = document.createElement("div");
  el.className = "segmented";
  el.setAttribute("role", "radiogroup");
  el.setAttribute("aria-label", label);
  const buttons = options.map(({ value, label: text }) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.setAttribute("role", "radio");
    btn.className = "segmented__item";
    btn.textContent = text;
    btn.addEventListener("click", () => onChange(value));
    el.append(btn);
    return [value, btn];
  });
  return {
    el,
    set(v) {
      buttons.forEach(([value, btn]) => btn.setAttribute("aria-checked", String(value === v)));
    },
  };
}
