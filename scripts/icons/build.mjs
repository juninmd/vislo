// Regenerates the Windows icon set from app-icon.svg / app-icon-small.svg: `npm run icons`.
// Chromium rasterises the SVGs at each exact pixel size (crisper than scaling a bitmap), the PNG frames are
// packed into icon.ico (16-256 px) and the Tauri PNGs are rewritten. Set CHROME_PATH to pick a browser.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { chromium } from "playwright-core";
import { ICO_SIZES, packIco, SMALL_MAX } from "./ico.mjs";

const ROOT = new URL("../../", import.meta.url);
const OUT = new URL("src-tauri/icons/", ROOT);
const CANDIDATES = [
  process.env.CHROME_PATH,
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
];
const executablePath = CANDIDATES.find((p) => p && existsSync(p));
if (!executablePath) throw new Error("No Chromium/Chrome found: set CHROME_PATH");

const svg = (name) => readFileSync(new URL(name, ROOT), "utf8");
const full = svg("app-icon.svg");
const small = svg("app-icon-small.svg");
const pick = (size) => (size <= SMALL_MAX ? small : full);

const browser = await chromium.launch({ executablePath });
async function render(source, size) {
  const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 });
  await page.setContent(
    `<style>html,body{margin:0;background:transparent}svg{display:block;width:${size}px;height:${size}px}</style>${source}`,
  );
  const png = await page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
  await page.close();
  return png;
}

const frames = [];
for (const size of ICO_SIZES) frames.push({ size, png: await render(pick(size), size) });
writeFileSync(new URL("icon.ico", OUT), packIco(frames));
// The PNG set Tauri bundles; unused Square*Logo.png (Store tiles) are left as they were.
for (const [name, size] of [["32x32.png", 32], ["64x64.png", 64], ["128x128.png", 128], ["128x128@2x.png", 256], ["icon.png", 512]]) {
  writeFileSync(new URL(name, OUT), await render(pick(size), size));
}
await browser.close();
console.log(`wrote icon.ico (${ICO_SIZES.join(", ")} px) and PNGs`);
