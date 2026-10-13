// The "Sync" section of the settings page. A computer shows the phones linked
// to it and a code to link another; a phone shows the computer it syncs with,
// or lets you link one by typing the code the computer shows.

import { syncApi } from "../api.js";
import { getLanguage, t } from "../i18n.js";
import { intlLocale } from "../format.js";
import { openDialog } from "../components/dialog.js";
import { toast } from "../components/toast.js";
import { el, row, section } from "../components/layout.js";
import { deviceName, ensureLocalNetwork } from "../platform.js";

const KNOWN_ERRORS = [
  "unreachable",
  "permission",
  "wrong_code",
  "expired",
  "not_pairing",
  "unknown_device",
  "bad_address",
  "invalid",
];

const errorText = (code) => t(`sync.error_${KNOWN_ERRORS.includes(code) ? code : "invalid"}`);

/** "5 minutes ago", in the app's language with Latin digits. */
function ago(ts) {
  const seconds = Math.max(0, (Date.now() - ts) / 1000);
  if (seconds < 60) return t("sync.just_now");
  const rtf = new Intl.RelativeTimeFormat(intlLocale(getLanguage()), { numeric: "auto" });
  const steps = [
    [60, "minute"],
    [24, "hour"],
    [30, "day"],
  ];
  let value = seconds / 60;
  for (const [size, unit] of steps) {
    if (value < size) return rtf.format(-Math.floor(value), unit);
    value /= size;
  }
  return rtf.format(-Math.floor(value), "month");
}

const lastSync = (ts) => (ts ? t("sync.last", { when: ago(ts) }) : t("sync.never"));

function button(label, onClick, kind = "quiet") {
  const btn = el("button", `btn btn--${kind} btn--sm`);
  btn.type = "button";
  btn.textContent = label;
  btn.addEventListener("click", onClick);
  return btn;
}

function note(text, className = "dialog-note") {
  const p = el("p", className);
  p.textContent = text;
  return p;
}

/**
 * Build the section. Returns its element and a function that stops its
 * live updates (call it when the page goes away or is rebuilt).
 */
export function syncSection() {
  const holder = el("div");
  let info = null;

  function render() {
    if (!info) return;
    const fresh = info.role === "computer" ? computerSection(info) : phoneSection(info);
    holder.replaceChildren(fresh);
  }

  const offChanged = syncApi.onChanged((next) => {
    info = next;
    render();
  });
  syncApi.info().then((first) => {
    info ??= first;
    render();
  });
  // Keep "5 minutes ago" current.
  const timer = setInterval(render, 30_000);

  return {
    el: holder,
    dispose() {
      clearInterval(timer);
      offChanged.then((off) => off());
    },
  };
}

// ------------------------------------------------------------ computer

function computerSection(info) {
  const rows = info.devices.map((d) =>
    row(d.name, button(t("sync.remove"), () => removeDevice(d)), lastSync(d.lastSyncAt)),
  );
  if (!rows.length) {
    rows.push(row(t("sync.no_phones"), button(t("sync.link_phone"), pairPhone, "primary")));
  } else {
    rows.push(row(t("sync.another_phone"), button(t("sync.link_phone"), pairPhone)));
  }
  return section(t("sync.title"), rows, t("sync.hint_computer"));
}

async function removeDevice(device) {
  const ok = await openDialog({
    title: t("sync.remove_title", { name: device.name }),
    body: t("sync.remove_body"),
    actions: [
      { label: t("common.cancel"), value: false },
      { label: t("sync.remove"), value: true, kind: "danger" },
    ],
  });
  if (ok) syncApi.removeDevice(device.id);
}

async function pairPhone() {
  let pairing;
  try {
    pairing = await syncApi.startPairing();
  } catch {
    toast(t("sync.pair_failed"), { error: true });
    return;
  }
  const content = el("div", "pair");
  const code = el("div", "pair__code num");
  code.textContent = `${pairing.code.slice(0, 5)} ${pairing.code.slice(5)}`;
  content.append(code, note(t("sync.pair_expiry")));
  if (pairing.address) content.append(note(t("sync.pair_address", { address: pairing.address })));
  content.append(note(t("sync.pair_firewall")));

  const offs = [];
  const result = await openDialog({
    title: t("sync.pair_title"),
    body: t("sync.pair_body"),
    content,
    actions: [{ label: t("common.cancel"), value: "cancel" }],
    onOpen(close) {
      offs.push(syncApi.onPaired((name) => close({ paired: name })));
      offs.push(syncApi.onPairingEnded(() => close("expired")));
    },
  });
  offs.forEach((off) => off.then((f) => f()));
  if (result?.paired !== undefined) toast(t("sync.paired", { name: result.paired }));
  else if (result === "expired") toast(t("sync.pair_expired"), { error: true });
  else syncApi.stopPairing();
}

// --------------------------------------------------------------- phone

function phoneSection(info) {
  if (!info.link) {
    const r = row(t("sync.no_computer"), button(t("sync.link_computer"), linkComputer, "primary"));
    return section(t("sync.title"), [r], t("sync.hint_phone"));
  }
  const { status } = info;
  let hint = lastSync(info.link.lastSyncAt);
  if (status.state === "syncing") hint = t("sync.syncing");
  else if (status.state === "error") hint = errorText(status.error);
  const main = row(info.link.name, button(t("sync.now"), syncNow), hint);
  if (status.state === "error") main.classList.add("row--error");
  const unlink = row(t("sync.linked_computer"), button(t("sync.remove"), () => unlinkComputer(info.link)));
  return section(t("sync.title"), [main, unlink], t("sync.hint_phone"));
}

async function syncNow() {
  if (!(await ensureLocalNetwork())) return toast(errorText("permission"), { error: true });
  syncApi.now();
}

async function unlinkComputer(link) {
  const ok = await openDialog({
    title: t("sync.remove_title", { name: link.name }),
    body: t("sync.remove_body"),
    actions: [
      { label: t("common.cancel"), value: false },
      { label: t("sync.remove"), value: true, kind: "danger" },
    ],
  });
  if (ok) syncApi.unlink();
}

async function linkComputer() {
  if (!(await ensureLocalNetwork())) return toast(errorText("permission"), { error: true });

  const content = el("div", "pair");
  const status = note(t("sync.searching"), "pair__status");
  const again = button(t("sync.search_again"), search);
  again.hidden = true;
  const statusRow = el("div", "pair__status-row");
  statusRow.append(status, again);

  const choice = el("select", "select pair__choice");
  choice.hidden = true;
  choice.setAttribute("aria-label", t("sync.computer"));

  const codeField = field(t("sync.code"), "numeric", "12345 67890");
  codeField.input.maxLength = 14;
  const addressField = field(t("sync.address"), "decimal", "192.168.1.5");
  addressField.el.hidden = true;
  const manual = el("button", "link-btn");
  manual.type = "button";
  manual.textContent = t("sync.address_manual");
  manual.addEventListener("click", () => {
    addressField.el.hidden = false;
    manual.hidden = true;
    addressField.input.focus();
  });
  const error = note("", "pair__error");
  error.setAttribute("role", "alert");
  content.append(statusRow, choice, codeField.el, manual, addressField.el, error);

  let found = [];
  async function search() {
    again.hidden = true;
    status.textContent = t("sync.searching");
    try {
      found = await syncApi.find();
    } catch (code) {
      found = [];
      error.textContent = errorText(code);
    }
    choice.replaceChildren(...found.map((c) => new Option(c.name, c.addr)));
    choice.hidden = found.length < 2;
    status.textContent =
      found.length === 0 ? t("sync.not_found") : found.length === 1 ? t("sync.found", { name: found[0].name }) : t("sync.choose");
    again.hidden = found.length > 0;
    if (!found.length) {
      addressField.el.hidden = false;
      manual.hidden = true;
    }
  }

  async function link() {
    error.textContent = "";
    const typedAddress = addressField.el.hidden ? "" : addressField.input.value.trim();
    const addr = typedAddress || (found.length > 1 ? choice.value : found[0]?.addr);
    if (!addr) {
      error.textContent = t("sync.not_found");
      return false;
    }
    const code = codeField.input.value;
    if (code.replace(/\D/g, "").length < 10 && !/[٠-٩۰-۹]/.test(code)) {
      error.textContent = errorText("wrong_code");
      codeField.input.focus();
      return false;
    }
    status.textContent = t("sync.linking");
    try {
      const info = await syncApi.pair(addr, code, deviceName());
      toast(t("sync.paired", { name: info.link?.name ?? "" }));
      return true;
    } catch (code) {
      error.textContent = errorText(code);
      status.textContent = "";
      return false;
    }
  }

  search();
  await openDialog({
    title: t("sync.link_title"),
    body: t("sync.link_body"),
    content,
    actions: [
      { label: t("common.cancel"), value: false },
      { label: t("sync.link"), value: true, kind: "primary", run: link },
    ],
  });
}

function field(label, inputmode, placeholder) {
  const wrap = el("label", "field");
  const span = el("span", "field__label");
  span.textContent = label;
  const input = el("input", "input");
  input.type = "text";
  input.inputMode = inputmode;
  input.placeholder = placeholder;
  input.autocomplete = "off";
  input.spellcheck = false;
  input.dir = "ltr";
  wrap.append(span, input);
  return { el: wrap, input };
}
