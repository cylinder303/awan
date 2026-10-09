// Settings page: timer, sound, appearance and language.

import { history, sounds } from "../api.js";
import { settings } from "../settings.js";
import { getLanguage, onLanguageChange, t } from "../i18n.js";
import { icons } from "../icons.js";
import { builtInThemes, themeName } from "../theme.js";
import { contrast, MIN_TEXT_CONTRAST } from "../color.js";
import { openDialog } from "../components/dialog.js";
import { toast } from "../components/toast.js";
import { dayKey } from "../dates.js";
import { segmented, stepper, toggle } from "../components/controls.js";

let soundIds = null;

export function mount(root) {
  let controls = [];
  let unsubscribe = () => {};

  async function render() {
    soundIds ??= await sounds.list();
    const scroll = root.scrollTop;
    unsubscribe();
    controls = [];
    root.innerHTML = "";
    root.append(
      section(t("settings.timer"), [
        row(t("settings.focus"), numberControl("focusMinutes", 1, 180, t("unit.min"), t("settings.focus"))),
        row(t("settings.short_break"), numberControl("shortBreakMinutes", 1, 60, t("unit.min"), t("settings.short_break"))),
        row(t("settings.long_break"), numberControl("longBreakMinutes", 1, 120, t("unit.min"), t("settings.long_break"))),
        row(t("settings.sessions_before_long"), numberControl("sessionsBeforeLong", 1, 12, t("unit.sessions"), t("settings.sessions_before_long"))),
        row(t("settings.auto_start"), switchControl("autoStart", t("settings.auto_start")), t("settings.auto_start_hint")),
      ], t("settings.timer_hint")),
      section(t("settings.sound"), [row(t("settings.alert_sound"), soundPicker()), row(t("settings.volume"), volumeControl())]),
      section(t("settings.appearance"), [
        themePicker(),
        customThemeEditor(),
        row(t("settings.mode_colors"), switchControl("modeColors", t("settings.mode_colors")), t("settings.mode_colors_hint")),
      ]),
      section(t("settings.language"), [languageControl()]),
      section(t("data.title"), [
        row(t("data.backup"), actionButton(icons.download, t("data.backup_action"), backup), t("data.backup_hint")),
        row(t("data.restore"), actionButton(icons.upload, t("data.restore_action"), restore), t("data.restore_hint")),
      ]),
      resetButton(),
    );
    unsubscribe = settings.subscribe((s) => controls.forEach((c) => c(s)));
    root.scrollTop = scroll;
  }

  // ------------------------------------------------------------ controls

  function numberControl(key, min, max, unit, label) {
    const c = stepper({ min, max, unit, label, onChange: (v) => settings.update({ [key]: v }) });
    controls.push((s) => c.set(s[key]));
    return c.el;
  }

  function switchControl(key, label) {
    const c = toggle({ label, onChange: (v) => settings.update({ [key]: v }) });
    controls.push((s) => c.set(s[key]));
    return c.el;
  }

  function soundPicker() {
    const wrap = el("div", "sound-picker");
    const select = el("select", "select");
    select.setAttribute("aria-label", t("settings.alert_sound"));
    for (const id of soundIds) select.append(new Option(t(`sound.${id}`), id));
    select.addEventListener("change", () => {
      settings.update({ sound: select.value });
      preview();
    });
    const play = el("button", "icon-btn icon-btn--sm");
    play.type = "button";
    play.innerHTML = icons.play;
    play.title = t("settings.preview");
    play.setAttribute("aria-label", play.title);
    const preview = () => {
      const s = settings.get();
      sounds.preview(s.sound, s.muted ? 0 : s.volume);
    };
    play.addEventListener("click", preview);
    wrap.append(select, play);
    controls.push((s) => (select.value = s.sound));
    return wrap;
  }

  function volumeControl() {
    const wrap = el("div", "volume");
    const mute = el("button", "icon-btn icon-btn--sm");
    mute.type = "button";
    const range = el("input", "range");
    range.type = "range";
    range.min = "0";
    range.max = "100";
    range.step = "5";
    range.setAttribute("aria-label", t("settings.volume"));
    const value = el("span", "volume__value num");
    mute.addEventListener("click", () => settings.update({ muted: !settings.get().muted }));
    range.addEventListener("input", () => (value.textContent = `${range.value}%`));
    range.addEventListener("change", () => settings.update({ volume: Number(range.value), muted: false }));
    wrap.append(mute, range, value);
    controls.push((s) => {
      mute.innerHTML = s.muted ? icons.muted : icons.volume;
      mute.title = s.muted ? t("settings.unmute") : t("settings.mute");
      mute.setAttribute("aria-label", mute.title);
      mute.setAttribute("aria-pressed", String(s.muted));
      if (document.activeElement !== range) range.value = String(s.volume);
      value.textContent = s.muted ? t("settings.muted") : `${s.volume}%`;
      wrap.classList.toggle("is-muted", s.muted);
    });
    return wrap;
  }

  function themePicker() {
    const grid = el("div", "theme-grid");
    grid.setAttribute("role", "radiogroup");
    grid.setAttribute("aria-label", t("settings.theme"));
    const lang = getLanguage();
    const options = [
      { id: "auto", name: t("settings.theme_auto"), preview: ["light", "dark"] },
      ...builtInThemes().map((id) => ({ id, name: themeName(id, lang), preview: [id] })),
      { id: "custom", name: t("settings.theme_custom"), preview: ["custom"] },
    ];
    const tiles = options.map(({ id, name, preview }) => {
      const tile = el("button", "theme-tile");
      tile.type = "button";
      tile.setAttribute("role", "radio");
      tile.innerHTML = `<span class="theme-tile__preview">${preview
        .map((p) => `<span class="theme-tile__half" data-theme="${p}">${miniRing}</span>`)
        .join("")}</span><span class="theme-tile__name"></span>`;
      tile.querySelector(".theme-tile__name").textContent = name;
      tile.addEventListener("click", () => settings.update({ theme: id }));
      grid.append(tile);
      return [id, tile];
    });
    controls.push((s) => tiles.forEach(([id, tile]) => tile.setAttribute("aria-checked", String(id === s.theme))));
    return grid;
  }

  function customThemeEditor() {
    const box = el("div", "custom-theme");
    const fields = [
      ["background", t("settings.color_background")],
      ["accent", t("settings.color_accent")],
      ["text", t("settings.color_text")],
    ].map(([key, label]) => {
      const field = el("label", "color-field");
      const input = el("input", "color-field__input");
      input.type = "color";
      const name = el("span", "color-field__label");
      name.textContent = label;
      input.addEventListener("input", () =>
        settings.update({ customTheme: { ...settings.get().customTheme, [key]: input.value } }),
      );
      field.append(input, name);
      box.append(field);
      return [key, input];
    });
    const note = el("p", "custom-theme__note");
    box.append(note);
    controls.push((s) => {
      box.hidden = s.theme !== "custom";
      fields.forEach(([key, input]) => {
        if (document.activeElement !== input) input.value = s.customTheme[key];
      });
      const ratio = contrast(s.customTheme.text, s.customTheme.background);
      const low = ratio < MIN_TEXT_CONTRAST;
      // Isolate "4.8:1" as left-to-right text so RTL layout cannot reorder it.
      const shown = `\u2066${ratio.toFixed(1)}:1\u2069`;
      note.textContent = t(low ? "settings.contrast_low" : "settings.contrast_ok", { ratio: shown });
      note.classList.toggle("is-warning", low);
    });
    return box;
  }

  function languageControl() {
    const c = segmented({
      label: t("settings.language"),
      options: [
        { value: "auto", label: t("settings.language_auto") },
        { value: "ar", label: "العربية" },
        { value: "en", label: "English" },
      ],
      onChange: (language) => settings.update({ language }),
    });
    controls.push((s) => c.set(s.language));
    return c.el;
  }

  function actionButton(icon, label, onClick) {
    const btn = el("button", "btn btn--quiet btn--sm");
    btn.type = "button";
    btn.innerHTML = `${icon}<span></span>`;
    btn.querySelector("span").textContent = label;
    btn.addEventListener("click", onClick);
    return btn;
  }

  async function backup() {
    try {
      if (await history.exportBackup(`awan-backup-${dayKey(Date.now())}.json`)) toast(t("data.backup_done"));
    } catch {
      toast(t("data.write_failed"), { error: true });
    }
  }

  async function restore() {
    const ok = await openDialog({
      title: t("data.restore_title"),
      body: t("data.restore_body"),
      actions: [
        { label: t("common.cancel"), value: false },
        { label: t("data.restore_action"), value: true, kind: "primary" },
      ],
    });
    if (!ok) return;
    try {
      const result = await history.importBackup();
      if (result) toast(t("data.restore_done", { added: result.added, skipped: result.skipped }));
    } catch (error) {
      const known = ["not_backup", "newer_backup", "unreadable"];
      toast(t(`data.error_${known.includes(error) ? error : "unreadable"}`), { error: true });
    }
  }

  function resetButton() {
    const btn = el("button", "btn btn--quiet reset-btn");
    btn.type = "button";
    btn.textContent = t("settings.reset");
    btn.addEventListener("click", async () => {
      const ok = await openDialog({
        title: t("settings.reset_title"),
        body: t("settings.reset_body"),
        actions: [
          { label: t("common.cancel"), value: false },
          { label: t("settings.reset_confirm"), value: true, kind: "danger" },
        ],
      });
      if (ok) settings.reset();
    });
    return btn;
  }

  render();
  const offLanguage = onLanguageChange(render);
  return () => {
    unsubscribe();
    offLanguage();
  };
}

// ---------------------------------------------------------------- helpers

const miniRing = `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8" fill="none" stroke="var(--ring-track)" stroke-width="2.5"/><path d="M12 4a8 8 0 1 1-8 8" fill="none" stroke="var(--focus)" stroke-width="2.5" stroke-linecap="round"/></svg>`;

function el(tag, className) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}

function section(title, children, hint) {
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
