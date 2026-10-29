// Regenerates the README screenshots: `npm run screenshots`.
// Starts the real frontend against mocked Tauri bindings (vite.screens.config.ts) and drives it
// with headless Chromium. Set CHROME_PATH to use a specific browser binary.
import { existsSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

// fileURLToPath, not .pathname: on Windows the latter yields "/D:/..." and mkdir fails.
const OUT = fileURLToPath(new URL("../../assets/screenshots/", import.meta.url));
const CANDIDATES = [
  process.env.CHROME_PATH,
  "/opt/pw-browsers/chromium",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
];
const executablePath = CANDIDATES.find((p) => p && existsSync(p));
if (!executablePath) throw new Error("No Chromium/Chrome found: set CHROME_PATH");

mkdirSync(OUT, { recursive: true });
const server = await createServer({ configFile: fileURLToPath(new URL("../../vite.screens.config.ts", import.meta.url)) });
await server.listen();
const base = `http://localhost:${server.config.server.port}/`;
const browser = await chromium.launch({ executablePath, args: ["--no-sandbox"] });

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

async function scene(name, setup) {
  // English whatever the host language is, so the README images do not depend on who ran the script
  const page = await browser.newPage({ locale: "en-US", viewport: { width: 1360, height: 900 }, deviceScaleFactor: 1 });
  await page.goto(`${base}?scene=${name.split("-")[0]}`);
  await page.waitForSelector("#app");
  await wait(900); // first picture decoded and laid out
  await setup?.(page);
  await wait(700);
  await page.screenshot({ path: `${OUT}${name}.png` });
  await page.close();
  console.log("wrote", `${name}.png`);
}

await scene("viewer");
await scene("filmstrip", async (page) => {
  await page.keyboard.press("t");
  await wait(500);
  for (let i = 0; i < 2; i++) {
    await page.keyboard.press("ArrowRight");
    await wait(450);
  }
});
await scene("info", async (page) => {
  await page.keyboard.press("i");
});
await scene("update", async (page) => {
  await page.keyboard.press("t");
  await page.click("#btn-gear");
  await wait(7000); // the background update check fires 5 s after start
});
await scene("zoom", async (page) => {
  await page.mouse.move(820, 440);
  for (let i = 0; i < 5; i++) {
    await page.mouse.wheel(0, -240);
    await wait(60);
  }
});
await scene("menu", async (page) => {
  await page.mouse.click(700, 420, { button: "right" });
});
await scene("empty");
await scene("formats", async (page) => {
  await page.click("#btn-gear");
  await page.evaluate(() => document.querySelector("#panel fieldset")?.scrollIntoView({ block: "center" }));
});

// The same screen in other languages: the browser locale drives "Automatic".
for (const [name, locale] of [["ja", "ja-JP"], ["de", "de-DE"], ["ru", "ru-RU"]]) {
  const context = await browser.newContext({ locale, viewport: { width: 1360, height: 900 }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  await page.goto(`${base}?scene=viewer`);
  await page.waitForSelector("#app");
  await wait(900);
  await page.keyboard.press("i");
  await page.click("#btn-gear");
  await wait(700);
  await page.screenshot({ path: `${OUT}lang-${name}.png`, clip: { x: 0, y: 0, width: 1360, height: 900 } });
  await context.close();
  console.log("wrote", `lang-${name}.png`);
}

await browser.close();
await server.close();
