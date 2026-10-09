// Inline SVG icons (bundled, no icon font). All use currentColor.

const stroke = (body, size = 24) =>
  `<svg viewBox="0 0 ${size} ${size}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export const icons = {
  logo: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" stroke-opacity=".25" stroke-width="3"/><path d="M12 3.5A8.5 8.5 0 1 1 3.5 12" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/></svg>`,

  // Title bar glyphs are drawn on a 10x10 grid with 1px strokes, like Windows.
  minimize: `<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" stroke="currentColor" stroke-width="1"/></svg>`,
  maximize: `<svg viewBox="0 0 10 10" aria-hidden="true"><rect x=".5" y=".5" width="9" height="9" rx="1.5" fill="none" stroke="currentColor" stroke-width="1"/></svg>`,
  restore: `<svg viewBox="0 0 10 10" aria-hidden="true"><rect x=".5" y="2.5" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1"/><path d="M2.5 2.5V1.8A1.3 1.3 0 0 1 3.8.5h4.4a1.3 1.3 0 0 1 1.3 1.3v4.4a1.3 1.3 0 0 1-1.3 1.3H7.5" fill="none" stroke="currentColor" stroke-width="1"/></svg>`,
  close: `<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M.5.5l9 9M9.5.5l-9 9" stroke="currentColor" stroke-width="1"/></svg>`,
  pin: stroke('<path d="M9 4h6l-1 6 3 3v1H7v-1l3-3-1-6z"/><path d="M12 14v6"/>'),

  timer: stroke('<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 1.5M10 2h4"/>'),
  history: stroke('<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5M12 7v5l3 2"/>'),
  goals: stroke('<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1"/>'),
  settings: stroke('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>'),

  play: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l10.6-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z" fill="currentColor"/></svg>`,
  pause: `<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1.2" fill="currentColor"/><rect x="14" y="5" width="4" height="14" rx="1.2" fill="currentColor"/></svg>`,
  minus: stroke('<path d="M6 12h12"/>'),
  plus: stroke('<path d="M12 6v12M6 12h12"/>'),
  volume: stroke('<path d="M4 9.5v5h3.5L12 18V6L7.5 9.5z"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/>'),
  muted: stroke('<path d="M4 9.5v5h3.5L12 18V6L7.5 9.5z"/><path d="M16 10l4 4M20 10l-4 4"/>'),
  reset: stroke('<path d="M3 12a9 9 0 1 0 2.6-6.4L3 8"/><path d="M3 3v5h5"/>'),
  skip: stroke('<path d="M5 5l10 7-10 7z"/><path d="M19 5v14"/>'),
};

/** Fill every element carrying data-icon="name" with its SVG. */
export function renderIcons(root = document) {
  root.querySelectorAll("[data-icon]").forEach((el) => {
    const svg = icons[el.dataset.icon];
    if (svg && el.innerHTML !== svg) el.innerHTML = svg;
  });
}
