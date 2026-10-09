// Theme handling. Themes are discovered from themes.css, so adding a theme
// there is enough for it to appear in the picker.

import { isDark, readableOn, rotateHue } from "./color.js";

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
const THEME_SELECTOR = /^\[data-theme="([a-z0-9-]+)"\]$/;
let current = { theme: "auto", customTheme: null, modeColors: true };
let cached = null;

/** Built-in theme ids, in the order they appear in themes.css. */
export function builtInThemes() {
  if (cached) return cached;
  const ids = [];
  for (const sheet of document.styleSheets) {
    let rules;
    try {
      rules = sheet.cssRules;
    } catch {
      continue; // not our stylesheet
    }
    for (const rule of rules) {
      const id = THEME_SELECTOR.exec(rule.selectorText || "")?.[1];
      if (id && id !== "custom" && !ids.includes(id)) ids.push(id);
    }
  }
  cached = ids;
  return ids;
}

/** Display name of a theme, read from its --theme-name-<lang> variable. */
export function themeName(id, lang) {
  const probe = document.createElement("span");
  probe.dataset.theme = id;
  probe.hidden = true;
  document.body.append(probe);
  const style = getComputedStyle(probe);
  const raw = style.getPropertyValue(`--theme-name-${lang}`) || style.getPropertyValue("--theme-name-en");
  probe.remove();
  return raw.trim().replace(/^["']|["']$/g, "") || id;
}

/** The theme actually shown, after resolving "auto" and unknown ids. */
export function resolveTheme(theme = current.theme) {
  if (theme === "custom") return "custom";
  if (theme !== "auto" && builtInThemes().includes(theme)) return theme;
  return systemDark.matches ? "dark" : "light";
}

/** Apply theme-related settings to the whole window. */
export function applyTheme({ theme, customTheme, modeColors }) {
  current = { theme, customTheme, modeColors };
  const root = document.documentElement;
  if (customTheme) {
    const { background, accent, text } = customTheme;
    root.style.setProperty("--c-bg", background);
    root.style.setProperty("--c-text", text);
    root.style.setProperty("--c-accent", accent);
    root.style.setProperty("--c-short", rotateHue(accent, 140));
    root.style.setProperty("--c-long", rotateHue(accent, 220));
    root.style.setProperty("--c-on-accent", readableOn(accent));
    root.style.setProperty("--c-scheme", isDark(background) ? "dark" : "light");
  }
  root.dataset.theme = resolveTheme(theme);
  root.dataset.modeColors = modeColors ? "on" : "off";
}

systemDark.addEventListener("change", () => {
  if (current.theme === "auto") applyTheme(current);
});
