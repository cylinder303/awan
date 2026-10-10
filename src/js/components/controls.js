// Small form controls used by the settings page. Each returns
// { el, set(value) } so the page can refresh values without rebuilding.

import { icons } from "../icons.js";

/** Read a number typed with Latin or Arabic-Indic digits ("٤٫٥" → 4.5). */
export function parseNumber(text) {
  const latin = String(text)
    .replace(/[٠-٩]/g, (d) => String(d.charCodeAt(0) - 0x0660))
    .replace(/[٫,]/g, ".")
    .replace(/[^\d.]/g, "");
  return parseFloat(latin);
}

/** Number field with − / + buttons. */
export function stepper({ min, max, step = 1, unit, label, onChange }) {
  const el = document.createElement("div");
  el.className = "stepper";
  el.innerHTML = `
    <button type="button" class="stepper__btn" data-step="-1" tabindex="-1" aria-hidden="true">${icons.minus}</button>
    <input class="stepper__input num" type="text" inputmode="decimal" maxlength="4" />
    <button type="button" class="stepper__btn" data-step="1" tabindex="-1" aria-hidden="true">${icons.plus}</button>
    <span class="stepper__unit"></span>`;
  const input = el.querySelector("input");
  input.setAttribute("aria-label", label);
  el.querySelector(".stepper__unit").textContent = unit;
  let value = min;

  const clamp = (v) => Math.min(max, Math.max(min, v));
  const show = (v) => String(Number.isInteger(v) ? v : v.toFixed(1));
  function commit(v) {
    const next = clamp(Number.isFinite(v) ? Math.round(v / step) * step : value);
    input.value = show(next);
    if (next !== value) {
      value = next;
      onChange(next);
    }
  }

  el.querySelectorAll("[data-step]").forEach((btn) =>
    btn.addEventListener("click", () => commit(value + Number(btn.dataset.step) * step)),
  );
  input.addEventListener("change", () => commit(parseNumber(input.value)));
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      commit(value + (e.key === "ArrowUp" ? step : -step));
    } else if (e.key === "Enter") {
      input.blur();
    }
  });

  return {
    el,
    set(v) {
      value = v;
      if (document.activeElement !== input) input.value = show(v);
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
