// Windows smoke test: launch the built app, attach to its WebView2 through
// the DevTools protocol, and check that the UI actually renders.
//
//   node scripts/smoke-test.mjs <path-to-awan.exe> <output-dir>
//
// Needs `playwright-core` (installed on the fly in CI; no browser download).

import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { chromium } from "playwright-core";

const [exe, out = "smoke"] = process.argv.slice(2);
const PORT = 9333;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const report = [];
const log = (line) => {
  console.log(line);
  report.push(line);
};
mkdirSync(out, { recursive: true });

const app = spawn(exe, [], {
  env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
  stdio: ["ignore", "pipe", "pipe"],
});
app.stdout.on("data", (d) => log(`[app] ${d.toString().trim()}`));
app.stderr.on("data", (d) => log(`[app:err] ${d.toString().trim()}`));

let failed = false;
const check = (name, ok, detail = "") => {
  log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` - ${detail}` : ""}`);
  if (!ok) failed = true;
};

try {
  let connected = false;
  for (let i = 0; i < 60 && !connected; i++) {
    try {
      connected = (await fetch(`http://127.0.0.1:${PORT}/json/version`)).ok;
    } catch {
      await sleep(500);
    }
  }
  check("WebView2 DevTools endpoint reachable", connected);
  if (!connected) throw new Error("no DevTools endpoint");

  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  await sleep(6000); // let both windows boot and the 3 s show-fallback pass
  const pages = browser.contexts().flatMap((c) => c.pages());
  log(`pages: ${pages.map((p) => p.url()).join(", ")}`);
  const main = pages.find((p) => p.url().endsWith("/index.html") || p.url().endsWith("/"));
  check("main window page found", !!main);

  // State of the page as it booted (before any reload).
  const probe = async (page) =>
    page.evaluate(async () => {
      const withTimeout = (p, ms) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error(`timeout ${ms} ms`)), ms))]);
      const result = {
        title: document.title,
        tauri: typeof window.__TAURI__,
        theme: document.documentElement.dataset.theme,
        dir: document.documentElement.dir,
        time: document.querySelector(".dial__time, .mini__time")?.textContent ?? null,
        icons: document.querySelectorAll("svg").length,
        visibility: document.visibilityState,
      };
      for (const cmd of ["get_settings", "get_state"]) {
        try {
          await withTimeout(window.__TAURI__.core.invoke(cmd), 4000);
          result[cmd] = "ok";
        } catch (e) {
          result[cmd] = `error: ${e?.message ?? e}`;
        }
      }
      return result;
    });

  if (main) {
    // What the user sees on first launch is what matters most.
    const before = await probe(main);
    log(`main as booted: ${JSON.stringify(before)}`);
    await main.screenshot({ path: `${out}/main-as-booted.png` }).catch((e) => log(`screenshot failed: ${e.message}`));
    check("first launch: timer face rendered", before.time === "25:00", String(before.time));
    check("first launch: icons rendered", before.icons > 5, String(before.icons));

    // Reload to capture any errors raised while booting.
    const errors = [];
    main.on("console", (m) => (m.type() === "error" || m.type() === "warning") && errors.push(`${m.type()}: ${m.text()}`));
    main.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
    await main.reload();
    await sleep(5000);
    const after = await probe(main);
    log(`main after reload: ${JSON.stringify(after)}`);
    errors.forEach((e) => log(`  ${e}`));
    await main.screenshot({ path: `${out}/main-after-reload.png` }).catch((e) => log(`screenshot failed: ${e.message}`));

    check("IPC answers get_settings", after.get_settings === "ok", after.get_settings);
    check("after reload: timer face rendered", after.time === "25:00", String(after.time));
    check("no errors while booting", errors.filter((e) => !e.startsWith("warning")).length === 0);
  }

  const mini = pages.find((p) => p.url().endsWith("/mini.html"));
  if (mini) log(`mini as booted: ${JSON.stringify(await probe(mini))}`);
  await browser.close().catch(() => {});
} catch (e) {
  log(`ERROR ${e.stack || e}`);
  failed = true;
} finally {
  app.kill();
  writeFileSync(`${out}/report.txt`, report.join("\n") + "\n");
}
process.exit(failed ? 1 : 0);
