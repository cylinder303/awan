// Font settings: a button per font (interface, timer digits) that opens a
// picker. Picking a font previews it on the whole window at once; "Apply"
// keeps it and "Cancel" puts the saved fonts back.

import { fontsApi } from "../api.js";
import { settings } from "../settings.js";
import { t } from "../i18n.js";
import { icons } from "../icons.js";
import { formatClock } from "../format.js";
import { isMobile } from "../platform.js";
import { applyFonts, fileFonts, loadFileFont, refreshFileFonts, setClock } from "../fonts.js";
import {
  BUNDLED_FONTS,
  DEFAULT_FONT,
  SAME_AS_UI,
  fileName,
  fontStack,
  fontStacks,
  resolveFont,
} from "../font-list.js";
import { openDialog } from "../components/dialog.js";
import { toast } from "../components/toast.js";
import { el } from "../components/layout.js";

const SAMPLE_AR = "جلسة تركيز، ثم استراحة قصيرة.";
/** Shown beside each font in the list: Arabic letters, or the timer's digits. */
const ITEM_SAMPLE_AR = "أوان";
const ITEM_SAMPLE_TIMER = "25:00";
const SAMPLE_EN = "A focus session, then a short break.";

/** `kind` is the settings key: "fontUi" or "fontTimer". */
export function fontButton(kind, controls) {
  const btn = el("button", "font-btn");
  btn.type = "button";
  const label = t(kind === "fontUi" ? "fonts.ui" : "fonts.timer");
  btn.setAttribute("aria-label", label);
  btn.addEventListener("click", () => chooseFont(kind));
  const show = (s) => {
    const value = s[kind];
    const files = fileFonts();
    btn.textContent = value === SAME_AS_UI ? t("fonts.same_as_ui") : resolveFont(value, files).name;
    btn.style.fontFamily = fontStacks(s, files)[kind === "fontUi" ? "ui" : "timer"];
    btn.title = btn.textContent;
  };
  controls.push(show);
  // Names of added fonts arrive after the first render.
  refreshFileFonts().then(() => settings.get() && show(settings.get()));
  return btn;
}

async function chooseFont(kind) {
  const timer = kind === "fontTimer";
  let saved = { ...settings.get() };
  let selected = saved[kind];
  let files = await refreshFileFonts();
  // Each added font shows its name in itself.
  files.forEach(loadFileFont);
  let system = null; // loaded below on a computer
  let query = "";

  const content = el("div", "font-picker");
  const preview = el("div", `font-preview${timer ? " font-preview--timer" : ""}`);
  const search = el("input", "font-search");
  search.type = "search";
  search.placeholder = t("fonts.search");
  search.setAttribute("aria-label", t("fonts.search"));
  search.spellcheck = false;
  const list = el("div", "font-list");
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", t(timer ? "fonts.timer" : "fonts.ui"));
  const add = el("button", "btn btn--quiet btn--sm font-add");
  add.type = "button";
  add.innerHTML = `${icons.plus}<span></span>`;
  add.querySelector("span").textContent = t("fonts.add_file");
  add.title = t("fonts.add_hint");
  content.append(preview, ...(isMobile ? [] : [search]), list, add);

  // ------------------------------------------------------------ preview

  let clock = 10 * 60_000;
  let ticking = 0;
  function renderPreview() {
    const next = { ...saved, [kind]: selected };
    applyFonts(next); // the whole window shows it, behind the picker
    preview.innerHTML = "";
    if (timer) {
      const face = el("div", "font-preview__clock num");
      face.style.fontFamily = fontStacks(next, files).timer;
      setClock(face, formatClock(clock));
      preview.append(face);
      return;
    }
    const stack = fontStack(selected, files);
    for (const [text, dir] of [[SAMPLE_AR, "rtl"], [SAMPLE_EN, "ltr"]]) {
      const line = el("p", "font-preview__line");
      line.dir = dir;
      line.textContent = text;
      line.style.fontFamily = stack;
      preview.append(line);
    }
    if (!supportsArabic(selected)) {
      const note = el("p", "font-preview__note");
      note.textContent = t("fonts.no_arabic_note");
      preview.append(note);
    }
  }
  if (timer) {
    // A running clock shows that the digits keep still in the chosen font.
    ticking = setInterval(() => {
      clock = clock > 1000 ? clock - 1000 : 10 * 60_000;
      const face = preview.querySelector(".font-preview__clock");
      if (face) setClock(face, formatClock(clock));
    }, 1000);
  }

  function supportsArabic(value) {
    if (value.startsWith("file:")) return files.find((f) => `file:${f.id}` === value)?.arabic ?? true;
    if (value.startsWith("system:")) return system?.find((f) => `system:${f.family}` === value)?.arabic ?? true;
    return true;
  }

  // --------------------------------------------------------------- list

  /** A font in the list. `arabic: false` tags it instead of showing Arabic letters. */
  function item(value, name, { stack, tag, arabic = true, remove } = {}) {
    const option = el("button", "font-item");
    option.type = "button";
    option.setAttribute("role", "option");
    option.dataset.value = value;
    option.dataset.search = name.toLowerCase();
    const label = el("span", "font-item__name");
    label.textContent = name;
    label.style.fontFamily = stack;
    option.append(label);
    if (timer || arabic) {
      const sample = el("span", `font-item__sample${timer ? " num" : ""}`);
      sample.textContent = timer ? ITEM_SAMPLE_TIMER : ITEM_SAMPLE_AR;
      sample.dir = timer ? "ltr" : "rtl";
      sample.style.fontFamily = stack;
      option.append(sample);
    }
    if (!timer && !arabic) tag = t("fonts.no_arabic");
    if (tag) {
      const chip = el("span", "font-item__tag");
      chip.textContent = tag;
      option.append(chip);
    }
    option.addEventListener("click", () => select(value));
    if (!remove) return option;
    const row = el("div", "font-item-row");
    row.dataset.search = option.dataset.search;
    const del = el("button", "icon-btn icon-btn--sm font-item__remove");
    del.type = "button";
    del.innerHTML = icons.trash;
    del.title = t("fonts.remove", { name });
    del.setAttribute("aria-label", del.title);
    del.addEventListener("click", remove);
    row.append(option, del);
    return row;
  }

  function group(title, items) {
    const box = el("div", "font-group");
    box.setAttribute("role", "group");
    const h = el("h3", "font-group__title");
    h.textContent = title;
    box.setAttribute("aria-label", title);
    box.append(h, ...items);
    return box;
  }

  function renderList() {
    const groups = [];
    const bundled = BUNDLED_FONTS.map((f) =>
      item(f.id, f.name, { stack: fontStack(f.id), tag: f.id === DEFAULT_FONT ? t("fonts.default") : "" }),
    );
    if (timer) bundled.unshift(item(SAME_AS_UI, t("fonts.same_as_ui"), { stack: fontStack(saved.fontUi, files) }));
    groups.push(group(t("fonts.group_bundled"), bundled));

    const usable = files.filter((f) => !timer || f.digits);
    if (usable.length) {
      groups.push(
        group(
          t("fonts.group_files"),
          usable.map((f) =>
            item(`file:${f.id}`, fileName(f), {
              stack: fontStack(`file:${f.id}`, files),
              arabic: f.arabic,
              remove: () => removeFile(f),
            }),
          ),
        ),
      );
    }

    if (!isMobile) {
      const installed = system
        ? system
            .filter((f) => !timer || f.digits)
            .map((f) =>
              item(`system:${f.family}`, f.family, { stack: fontStack(`system:${f.family}`), arabic: f.arabic }),
            )
        : [Object.assign(el("p", "font-list__status"), { textContent: t("fonts.loading") })];
      groups.push(group(t("fonts.group_system"), installed));
    }

    const empty = el("p", "font-list__status font-list__empty");
    empty.textContent = t("fonts.no_results");
    list.replaceChildren(...groups, empty);
    mark();
    filter();
  }

  function mark() {
    list.querySelectorAll(".font-item").forEach((option) => {
      option.setAttribute("aria-selected", String(option.dataset.value === selected));
    });
  }

  function filter() {
    let shown = 0;
    list.querySelectorAll(".font-group").forEach((box) => {
      let any = false;
      box.querySelectorAll(":scope > .font-item, :scope > .font-item-row").forEach((node) => {
        const match = !query || node.dataset.search.includes(query);
        node.hidden = !match;
        any ||= match;
      });
      const loading = box.querySelector(".font-list__status");
      box.hidden = !any && !(loading && !query);
      shown += any ? 1 : 0;
    });
    list.querySelector(".font-list__empty").hidden = shown > 0 || !query;
  }

  function select(value) {
    selected = value;
    mark();
    renderPreview();
  }

  // ------------------------------------------------------------ actions

  async function addFile() {
    let font;
    try {
      font = await fontsApi.add();
    } catch (error) {
      const known = ["not_font", "too_big", "unreadable", "write_failed"];
      toast(t(`fonts.error_${known.includes(error) ? error : "unreadable"}`), { error: true });
      return;
    }
    if (!font) return;
    files = await refreshFileFonts();
    selected = `file:${font.id}`;
    renderList();
    renderPreview();
    list.querySelector(`[data-value="file:${font.id}"]`)?.scrollIntoView({ block: "nearest" });
    toast(t("fonts.added", { name: fileName(font) }));
  }

  async function removeFile(font) {
    try {
      settings.adopt(await fontsApi.remove(font.id));
    } catch {
      return;
    }
    files = await refreshFileFonts();
    // The saved settings no longer use it; neither does the choice here.
    saved = { ...saved, fontUi: settings.get().fontUi, fontTimer: settings.get().fontTimer };
    if (selected === `file:${font.id}`) selected = saved[kind];
    renderList();
    renderPreview();
    toast(t("fonts.removed", { name: fileName(font) }));
  }

  // Up and down arrows move through the fonts shown (the dialog keeps
  // other keys from the page, so listen before it does).
  function onArrow(e) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const options = [...list.querySelectorAll(".font-item")].filter((o) => !o.closest("[hidden]"));
    if (options.length === 0) return;
    e.preventDefault();
    const at = options.findIndex((o) => o.dataset.value === selected);
    const next = options[Math.max(0, Math.min(options.length - 1, at + (e.key === "ArrowDown" ? 1 : -1)))];
    select(next.dataset.value);
    next.scrollIntoView({ block: "nearest" });
  }
  window.addEventListener("keydown", onArrow, true);

  add.addEventListener("click", addFile);
  search.addEventListener("input", () => {
    query = search.value.trim().toLowerCase();
    filter();
  });

  renderList();
  renderPreview();
  if (!isMobile) {
    fontsApi.system().then(
      (fonts) => {
        system = fonts;
        renderList();
        if (!timer) renderPreview(); // the Arabic note may change
      },
      () => {
        system = [];
        renderList();
      },
    );
  }

  const ok = await openDialog({
    title: t(timer ? "fonts.timer" : "fonts.ui"),
    body: t("fonts.choose_body"),
    content,
    actions: [
      { label: t("common.cancel"), value: false },
      { label: t("fonts.apply"), value: true, kind: "primary" },
    ],
    onOpen: () => {
      content.closest(".dialog")?.classList.add("dialog--fonts");
      list.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "center" });
      (isMobile ? list.querySelector('[aria-selected="true"]') : search)?.focus();
    },
  });
  clearInterval(ticking);
  window.removeEventListener("keydown", onArrow, true);
  if (ok && selected !== settings.get()[kind]) await settings.update({ [kind]: selected });
  applyFonts(settings.get());
}
