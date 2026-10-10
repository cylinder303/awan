// Render the installer bitmaps that show the logo:
//   sidebar.bmp  164x314  welcome and finish pages
//   header.bmp   150x57   top of the other pages
//
//   npm install --no-save playwright-core@1
//   node scripts/installer-art.mjs [icon.svg] [output-dir]
//
// Defaults: scripts/icon.svg -> src-tauri/installer. Uses Microsoft Edge on
// Windows; elsewhere set CHROME_PATH to a Chromium-based browser.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const root = fileURLToPath(new URL("..", import.meta.url));
const [iconPath = join(root, "scripts/icon.svg"), out = join(root, "src-tauri/installer")] = process.argv.slice(2);
const svg = readFileSync(iconPath, "utf8");
const icon = (px) => `<img src="data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}" width="${px}" height="${px}">`;
const fonts = ["latin", "arabic"]
  .map((script) => readFileSync(join(root, `src/assets/fonts/ibm-plex-sans-arabic-${script}-500-normal.woff2`)).toString("base64"))
  .map((data) => `@font-face{font-family:Plex;font-weight:500;src:url(data:font/woff2;base64,${data})}`)
  .join("");
const TEXT = "#3a2f25";

const browser = await chromium.launch(
  process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : process.platform === "win32" ? { channel: "msedge" } : {},
);

async function render(name, width, height, body) {
  const page = await browser.newPage({ viewport: { width, height } });
  await page.setContent(`<!doctype html><style>${fonts}*{margin:0}body{width:${width}px;height:${height}px;overflow:hidden;font-family:Plex}</style>${body}`);
  await page.evaluate(() => document.fonts.ready);
  const png = await page.screenshot();
  // Read the pixels back through a canvas, then write a 24-bit BMP (the
  // format NSIS expects).
  const rgba = await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const canvas = Object.assign(document.createElement("canvas"), { width: img.width, height: img.height });
    const ctx = canvas.getContext("2d");
    ctx.drawImage(img, 0, 0);
    return Array.from(ctx.getImageData(0, 0, img.width, img.height).data);
  }, `data:image/png;base64,${png.toString("base64")}`);
  writeFileSync(join(out, `${name}.bmp`), bmp(width, height, rgba));
  await page.close();
}

function bmp(width, height, rgba) {
  const row = Math.ceil((width * 3) / 4) * 4;
  const buf = Buffer.alloc(54 + row * height);
  buf.write("BM", 0, "ascii");
  buf.writeUInt32LE(buf.length, 2);
  buf.writeUInt32LE(54, 10); // pixel data offset
  buf.writeUInt32LE(40, 14); // BITMAPINFOHEADER
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22); // positive: rows stored bottom-up
  buf.writeUInt16LE(1, 26); // planes
  buf.writeUInt16LE(24, 28); // bits per pixel
  buf.writeUInt32LE(row * height, 34);
  buf.writeInt32LE(2835, 38); // 72 dpi
  buf.writeInt32LE(2835, 42);
  for (let y = 0; y < height; y++) {
    const at = 54 + (height - 1 - y) * row;
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      buf[at + x * 3] = rgba[i + 2];
      buf[at + x * 3 + 1] = rgba[i + 1];
      buf[at + x * 3 + 2] = rgba[i];
    }
  }
  return buf;
}

await render("sidebar", 164, 314, `
  <div style="height:100%;background:linear-gradient(160deg,#f8f2e8,#e9dece);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;color:${TEXT}">
    ${icon(92)}
    <div style="font-size:26px">أوان</div>
    <div style="font-size:13px;opacity:.6;letter-spacing:2px">AWAN</div>
  </div>`);
await render("header", 150, 57, `
  <div style="height:100%;background:#fff;display:flex;align-items:center;justify-content:flex-end;padding-right:10px;gap:8px">
    <div style="font-size:15px;color:${TEXT}">Awan</div>
    ${icon(38)}
  </div>`);
await browser.close();
console.log(`Installer bitmaps written to ${out}`);
