// Counts layout/style work and frame pacing while a fixed UI scenario runs: `npm run perf:ui`.
// Same real frontend and mocked backend as the screenshots. Run it on two checkouts to compare them;
// the medians of RUNS passes are printed as JSON. Set CHROME_PATH to use a specific browser.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const RUNS = Number(process.env.RUNS ?? 5);
const CANDIDATES = [
  process.env.CHROME_PATH,
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
];
const executablePath = CANDIDATES.find((p) => p && existsSync(p));
if (!executablePath) throw new Error("No Chromium/Chrome found: set CHROME_PATH");

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

const server = await createServer({ configFile: fileURLToPath(new URL("../../vite.screens.config.ts", import.meta.url)), logLevel: "error" });
await server.listen();
const base = `http://localhost:${server.config.server.port}/`;
const browser = await chromium.launch({ executablePath, args: ["--no-sandbox"] });

async function pass() {
  const page = await browser.newPage({ viewport: { width: 1360, height: 900 }, deviceScaleFactor: 1 });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Performance.enable");
  await page.goto(`${base}?scene=viewer`);
  await page.waitForSelector("#app");
  await wait(1200);
  const metrics = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]));
  await page.evaluate(() => {
    window.__dt = [];
    let last = performance.now();
    const tick = (t) => {
      window.__dt.push(t - last);
      last = t;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const a = await metrics();
  // Every overlay in and out, plus navigation with the filmstrip open.
  await page.keyboard.press("t");
  await wait(400);
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press("ArrowRight");
    await wait(350);
  }
  await page.keyboard.press("t");
  await wait(400);
  await page.keyboard.press("i");
  await wait(350);
  await page.keyboard.press("i");
  await page.click("#btn-gear");
  await wait(350);
  await page.click("#btn-gear");
  await page.mouse.click(700, 420, { button: "right" });
  await wait(300);
  await page.keyboard.press("Escape");
  await wait(300);
  const b = await metrics();
  const dt = await page.evaluate(() => window.__dt.slice(2));
  await page.close();
  const d = (k) => b[k] - a[k];
  return {
    layouts: d("LayoutCount"),
    styleRecalcs: d("RecalcStyleCount"),
    layoutMs: d("LayoutDuration") * 1000,
    styleMs: d("RecalcStyleDuration") * 1000,
    scriptMs: d("ScriptDuration") * 1000,
    taskMs: d("TaskDuration") * 1000,
    slowFrames: dt.filter((x) => x > 20).length,
    worstFrameMs: Math.max(...dt),
  };
}

const runs = [];
for (let i = 0; i < RUNS; i++) runs.push(await pass());
const out = Object.fromEntries(Object.keys(runs[0]).map((k) => [k, Number(median(runs.map((r) => r[k])).toFixed(2))]));
console.log(JSON.stringify(out));
await browser.close();
await server.close();
