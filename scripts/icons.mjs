// Regenerate the app and tray icons from their SVG sources:
//
//   scripts/icon.svg       -> src-tauri/icons/       (window, taskbar, installer)
//   scripts/tray-icon.svg  -> src-tauri/icons/tray/  (system tray, one PNG per
//                             display scale so it stays sharp)
//   scripts/android-icon.json -> the Android launcher icons, adaptive: the
//                             ring (android-icon-fg.svg) over the tile color
//                             (android-icon-bg.svg)
//
//   npm run icons
//
// The installer bitmaps also show the logo: see scripts/installer-art.mjs.

import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const tauri = join(root, "node_modules/@tauri-apps/cli/tauri.js");
const icons = join(root, "src-tauri/icons");

// Only what the Windows build uses; `tauri icon` also writes macOS, iOS,
// Android and Microsoft Store variants.
const APP_FILES = ["32x32.png", "128x128.png", "128x128@2x.png", "icon.png", "icon.ico"];
// Small-icon sizes at 100%, 125%, 150%, 200%, 250% and 300% scaling.
const TRAY_SIZES = [16, 20, 24, 32, 40, 48];

function generate(source, extraArgs, files, dest) {
  const tmp = mkdtempSync(join(tmpdir(), "awan-icons-"));
  try {
    execFileSync(process.execPath, [tauri, "icon", join(root, source), "-o", tmp, ...extraArgs], { stdio: "inherit" });
    mkdirSync(dest, { recursive: true });
    for (const file of files) copyFileSync(join(tmp, file), join(dest, file));
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

generate("scripts/icon.svg", [], APP_FILES, icons);
generate(
  "scripts/tray-icon.svg",
  ["--png", TRAY_SIZES.join(",")],
  TRAY_SIZES.map((s) => `${s}x${s}.png`),
  join(icons, "tray"),
);

const androidRes = join(root, "src-tauri/gen/android/app/src/main/res");
const tmp = mkdtempSync(join(tmpdir(), "awan-icons-"));
try {
  execFileSync(process.execPath, [tauri, "icon", join(root, "scripts/android-icon.json"), "-o", tmp], { stdio: "inherit" });
  cpSync(join(tmp, "android"), androidRes, { recursive: true });
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
console.log("Icons updated in src-tauri/icons and the Android project");
