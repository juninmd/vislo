// Opens the gallery on a folder of 3000 pictures (a fifth of them RAW) and measures how long until every cell on
// screen shows its picture, how many stay empty, and how much work the backend was asked for. Then it jumps a third of
// the way down, and flings through five screens, and measures the same for the final screen.
//
// The backend is a model of the real job pool (src-tauri/src/pool.rs), served from request interception so that the
// browser's six-connections-per-host limit does not hide it (the vislo: scheme has no such limit): WORKERS threads,
// newest job first, and a backlog of 256 whose oldest job is answered 503 when it overflows, at a fixed cost per route.
// `npm run perf:gallery` (env: GSIZE cell edge, VIEW WxH, RUNS); prints the median of RUNS passes as JSON.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";
import { PHOTOS, svgFor } from "../screenshots/scenes.mjs";

const RUNS = Number(process.env.RUNS ?? 3);
const WORKERS = 4;
const BACKLOG = 256; // JOB_BACKLOG in src-tauri/src/lib.rs
/** Milliseconds of backend work per route: a thumbnail of a 24 MP JPEG, reading a file, a full RAW conversion. */
const COST = { t: 60, o: 40, c: 400, v: 10, f: 250 };
const SIZE = Number(process.env.GSIZE ?? 200); // gallery cell edge; 96 is the smallest the slider allows
const [VW, VH] = (process.env.VIEW ?? "1360x900").split("x").map(Number);
const CANDIDATES = [process.env.CHROME_PATH, "/usr/bin/google-chrome", "/usr/bin/chromium",
  "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"];
const executablePath = CANDIDATES.find((p) => p && existsSync(p));
if (!executablePath) throw new Error("No Chromium/Chrome found: set CHROME_PATH");
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

const server = await createServer({ configFile: fileURLToPath(new URL("../../vite.screens.config.ts", import.meta.url)), logLevel: "error" });
await server.listen();
const base = `http://localhost:${server.config.server.port}/`;
const browser = await chromium.launch({ executablePath, args: ["--no-sandbox"] });
const svg = svgFor(PHOTOS[0]);

function backend() {
  const queue = [];
  let busy = 0;
  let stats = { t: 0, o: 0, c: 0, shed: 0 };
  const pump = () => {
    while (busy < WORKERS && queue.length) {
      const job = queue.pop(); // newest first
      busy++;
      setTimeout(() => {
        busy--;
        job.route.fulfill({ status: 200, contentType: "image/svg+xml", body: svg }).catch(() => {});
        pump();
      }, COST[job.name] ?? 50);
    }
  };
  return {
    handle(route) {
      const name = new URL(route.request().url()).pathname.split("/").pop();
      stats[name] = (stats[name] ?? 0) + 1;
      queue.push({ route, name });
      if (queue.length > BACKLOG) {
        stats.shed++;
        queue.shift().route.fulfill({ status: 503, body: "request superseded" }).catch(() => {});
      }
      pump();
    },
    take() {
      const s = stats;
      stats = { t: 0, o: 0, c: 0, shed: 0 };
      return s;
    },
  };
}

/** Cells inside the grid's viewport, and how many of them show their picture. */
const onScreen = () => {
  const grid = document.querySelector(".g-grid");
  const box = grid.getBoundingClientRect();
  const cells = [...grid.querySelectorAll(".g-cell")].filter((c) => {
    const r = c.getBoundingClientRect();
    return r.bottom > box.top && r.top < box.bottom;
  });
  const shown = cells.filter((c) => c.querySelector("img")?.classList.contains("loaded")).length;
  return { cells: cells.length, shown };
};

async function settle(page, limitMs) {
  const start = Date.now();
  for (;;) {
    const s = await page.evaluate(onScreen);
    if (s.shown === s.cells || Date.now() - start > limitMs) return { ms: Date.now() - start, ...s };
    await page.waitForTimeout(20);
  }
}

async function pass() {
  const page = await browser.newPage({ viewport: { width: VW, height: VH }, deviceScaleFactor: 1 });
  const be = backend();
  await page.route((url) => url.pathname.startsWith("/mock/"), (route) => be.handle(route));
  await page.goto(`${base}?scene=gperf&gsize=${SIZE}`);
  await page.waitForSelector("#stage .slide");
  await page.waitForTimeout(1000);
  be.take(); // only the gallery's own requests count
  const out = {};
  const measure = async (name, act) => {
    await act();
    const s = await settle(page, 20000);
    await page.waitForTimeout(3000); // the backlog drains, so the counters include all the work this step queued
    out[name] = { ...s, ...be.take() };
  };
  await measure("open", async () => {
    await page.keyboard.press("g");
    await page.waitForSelector(".g-cell");
  });
  await measure("jump", () => page.evaluate(() => {
    const grid = document.querySelector(".g-grid");
    grid.scrollTo(0, grid.scrollHeight / 3);
  }));
  // A fling: ten half screens in a second, as a wheel or a dragged scrollbar does. Only the last screen matters.
  await measure("fling", async () => {
    for (let i = 0; i < 10; i++) {
      await page.evaluate(() => document.querySelector(".g-grid").scrollBy(0, innerHeight / 2));
      await page.waitForTimeout(100);
    }
  });
  await page.close();
  return out;
}

const runs = [];
for (let i = 0; i < RUNS; i++) runs.push(await pass());
const pick = (f) => median(runs.map(f));
const summary = (k) => ({
  msUntilScreenComplete: pick((r) => r[k].ms),
  cellsOnScreen: pick((r) => r[k].cells),
  emptyCells: pick((r) => r[k].cells - r[k].shown),
  thumbRequests: pick((r) => r[k].t),
  fullSizeRequests: pick((r) => r[k].o + r[k].c),
  shed: pick((r) => r[k].shed),
});
console.log(JSON.stringify({ cell: SIZE, view: `${VW}x${VH}`, open: summary("open"), jump: summary("jump"), fling: summary("fling") }, null, 2));
await browser.close();
await server.close();
