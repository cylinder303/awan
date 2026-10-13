// Building blocks of the settings page: sections of cards made of rows.

export function el(tag, className) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}

export function section(title, children, hint) {
  const s = el("section", "settings-section");
  const h = el("h2", "settings-section__title");
  h.textContent = title;
  const card = el("div", "card");
  card.append(...children);
  s.append(h, card);
  if (hint) {
    const p = el("p", "settings-section__hint");
    p.textContent = hint;
    s.append(p);
  }
  return s;
}

/** A row whose control sits below the label (for wide controls). */
export function stackedRow(label, control, hint) {
  const r = row(label, control, hint);
  r.classList.add("row--stacked");
  return r;
}

export function row(label, control, hint) {
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
