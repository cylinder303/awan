// Translations and text direction. Arabic switches the whole UI to RTL.

import ar from "../locales/ar.js";
import en from "../locales/en.js";
import { interpolate } from "./format.js";

const catalogs = { ar, en };
let lang = "en";
const listeners = new Set();

export const languages = Object.keys(catalogs);

/** The system language if we have it, otherwise English. */
export function detectLanguage() {
  const preferred = (navigator.languages || [navigator.language || "en"]).map((l) => l.slice(0, 2));
  return preferred.find((l) => l in catalogs) || "en";
}

export function getLanguage() {
  return lang;
}

export function setLanguage(next) {
  lang = next in catalogs ? next : "en";
  const root = document.documentElement;
  root.lang = lang;
  root.dir = lang === "ar" ? "rtl" : "ltr";
  translate(document);
  listeners.forEach((fn) => fn(lang));
}

export function onLanguageChange(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function t(key, params) {
  const text = catalogs[lang][key] ?? catalogs.en[key] ?? key;
  return params ? interpolate(text, params) : text;
}

/** Fill data-i18n (text), data-i18n-title and data-i18n-placeholder attributes. */
export function translate(root) {
  root.querySelectorAll("[data-i18n]").forEach((el) => (el.textContent = t(el.dataset.i18n)));
  root.querySelectorAll("[data-i18n-title]").forEach((el) => {
    el.title = t(el.dataset.i18nTitle);
    el.setAttribute("aria-label", el.title);
  });
  root.querySelectorAll("[data-i18n-placeholder]").forEach((el) => (el.placeholder = t(el.dataset.i18nPlaceholder)));
}
