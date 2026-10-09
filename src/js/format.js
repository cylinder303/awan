// Formatting helpers. Numbers always use Latin digits (0-9), in both languages.

/** Remaining time for the clock face: "25:00", or "1:05:00" past an hour. */
export function formatClock(ms) {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/** Replace {name} placeholders in a template string. */
export function interpolate(template, params = {}) {
  return template.replace(/\{(\w+)\}/g, (match, key) => (key in params ? String(params[key]) : match));
}
