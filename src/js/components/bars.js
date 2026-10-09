// Column chart for a single series (e.g. focus minutes per day).
// One color: the latest column in the accent, earlier ones muted. No legend
// (the title names the series); values on hover/focus and a hidden table
// for screen readers.

/**
 * @param {object} opts
 * @param {{label: string, fullLabel: string, value: number, display: string}[]} opts.items oldest first
 * @param {string} opts.caption accessible name of the chart
 * @param {string} opts.valueHeader table header for the values
 */
export function columnChart({ items, caption, valueHeader }) {
  const root = document.createElement("figure");
  root.className = "bars";
  const max = Math.max(...items.map((i) => i.value), 1);
  const last = items.length - 1;

  const plot = document.createElement("div");
  plot.className = "bars__plot";
  plot.setAttribute("aria-hidden", "true");

  const tip = document.createElement("div");
  tip.className = "bars__tip";
  tip.hidden = true;

  items.forEach((item, i) => {
    const col = document.createElement("div");
    col.className = "bars__col" + (i === last ? " is-current" : "");
    col.tabIndex = 0;
    const bar = document.createElement("div");
    bar.className = "bars__bar";
    bar.style.height = item.value > 0 ? `max(3px, ${(item.value / max) * 100}%)` : "0";
    const track = document.createElement("div");
    track.className = "bars__track";
    // Label the latest value directly; the rest are on hover.
    if (i === last && item.value > 0) {
      const v = document.createElement("span");
      v.className = "bars__value";
      v.textContent = item.display;
      track.append(v);
    }
    track.append(bar);
    const label = document.createElement("span");
    label.className = "bars__label";
    label.textContent = item.label;
    col.append(track, label);

    const show = () => {
      tip.textContent = `${item.fullLabel} · ${item.display}`;
      tip.hidden = false;
      const c = col.getBoundingClientRect();
      const p = plot.getBoundingClientRect();
      const left = c.left - p.left + c.width / 2;
      tip.style.left = `${Math.min(Math.max(left, 60), p.width - 60)}px`;
    };
    const hide = () => (tip.hidden = true);
    col.addEventListener("pointerenter", show);
    col.addEventListener("pointerleave", hide);
    col.addEventListener("focus", show);
    col.addEventListener("blur", hide);
    plot.append(col);
  });
  plot.append(tip);

  // Same data as a table for assistive technology.
  const table = document.createElement("table");
  table.className = "sr-only";
  table.innerHTML = `<caption></caption><thead><tr><th scope="col"></th><th scope="col"></th></tr></thead><tbody></tbody>`;
  table.querySelector("caption").textContent = caption;
  table.querySelectorAll("th")[1].textContent = valueHeader;
  const body = table.querySelector("tbody");
  for (const item of items) {
    const tr = document.createElement("tr");
    const th = document.createElement("th");
    th.scope = "row";
    th.textContent = item.fullLabel;
    const td = document.createElement("td");
    td.textContent = item.display;
    tr.append(th, td);
    body.append(tr);
  }

  root.append(plot, table);
  return root;
}
