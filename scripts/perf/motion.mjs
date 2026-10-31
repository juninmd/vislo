// Measures what the user feels: how long the first picture takes to appear, how long a step or a jump
// to another picture takes, how evenly the frames are paced while a transition runs, and how much of
// the stage the two pictures cover together while they cross (below 100% the background shows through).
// `npm run perf:motion`; it prints the median of RUNS passes as JSON.
// The frontend and backend bindings are the same ones the screenshots use (vite.screens.config.ts).
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

// Runs before any module: performance.now() is counted from navigation start, so these marks are
// milliseconds since the page started loading. A picture landing on the stage is the DOM change that
// makes the viewer feel responsive; the paint mark follows two frames later.
const probe = () => {
  const m = { firstSlide: null, firstPainted: null, nav: {}, coverage: [] };
  window.__m = m;
  let waiting = null;
  new MutationObserver(() => {
    const stage = document.querySelector("#stage");
    if (!stage) return;
    if (m.firstSlide === null && stage.querySelector(".slide")) {
      m.firstSlide = performance.now();
      requestAnimationFrame(() => requestAnimationFrame(() => (m.firstPainted = performance.now())));
    }
    if (waiting && stage.childElementCount > 1) {
      m.nav[waiting].slideAt = performance.now();
      waiting = null;
    }
  }).observe(document, { childList: true, subtree: true }); // document: this runs before <html> exists
  addEventListener(
    "keydown",
    (e) => {
      if (waiting || !["ArrowRight", "ArrowLeft", "PageDown", "End", "Home"].includes(e.key)) return;
      waiting = e.key;
      m.nav[e.key] = { keyAt: performance.now(), slideAt: null };
      // While the two pictures cross, read back the opacity the browser is actually painting. The last
      // child is on top, so the stage is `in + (1 - in) * out` covered: 1 means it is never dimmed.
      const until = performance.now() + 500;
      const sample = () => {
        const opacities = [...document.querySelectorAll("#stage .slide")].map((s) => Number(getComputedStyle(s).opacity));
        if (opacities.length > 1) {
          const top = opacities[opacities.length - 1];
          m.coverage.push(top + (1 - top) * opacities[0]);
        }
        if (performance.now() < until) requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
    },
    true,
  );
};

async function pass({ slow = 0 } = {}) {
  const page = await browser.newPage({ viewport: { width: 1360, height: 900 }, deviceScaleFactor: 1 });
  await page.context().addCookies([{ name: "slow", value: String(slow), url: base }]);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Performance.enable");
  await page.addInitScript(probe);
  await page.goto(`${base}?scene=perf`);
  await page.waitForSelector("#app");
  const metrics = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]));
  const nav = async (key, settle = 500) => {
    await page.keyboard.press(key);
    await wait(settle); // the transition ends at 240 ms; the marks are taken when it starts
    return page.evaluate((k) => window.__m.nav[k], key);
  };
  await wait(1400 + slow * 4);

  const boot = await page.evaluate(() => ({ firstSlide: window.__m.firstSlide, firstPainted: window.__m.firstPainted }));
  const m0 = await metrics();

  // A step to the neighbour, which start-up already decoded in the background.
  const step = await nav("ArrowRight");
  const stepCoverage = await page.evaluate(() => window.__m.coverage.slice());
  const m1 = await metrics();

  // Home, then a jump across the folder: that picture was never decoded and has no stand-in prepared.
  await page.keyboard.press("Home");
  await wait(800 + slow * 2);
  const frames = page.evaluate(
    () =>
      new Promise((done) => {
        const gaps = [];
        let last = performance.now();
        const tick = (t) => {
          gaps.push(t - last);
          last = t;
          if (gaps.length < 45) requestAnimationFrame(tick);
          else done(gaps);
        };
        requestAnimationFrame(tick);
      }),
  );
  const before = await metrics();
  const jump = await nav("End");
  const after = await metrics();
  const gaps = (await frames).slice(2);

  // Stepping back: that picture is still decoding, but its stand-in was prepared when the jump landed.
  // The wait between the key press and the picture starting to move is what the viewer feels.
  const beforeStaged = await metrics();
  const staged = await nav("ArrowLeft");
  const afterStaged = await metrics();
  const warm = await nav("ArrowLeft");
  await page.close();

  const d = (a, b, k) => Number((b[k] - a[k]).toFixed(2));
  const latency = (s) => (s?.keyAt == null || s?.slideAt == null ? null : Number((s.slideAt - s.keyAt).toFixed(2)));
  return {
    startupToSlide: boot.firstSlide === null ? null : Number(boot.firstSlide.toFixed(2)),
    startupToPaint: boot.firstPainted === null ? null : Number(boot.firstPainted.toFixed(2)),
    stepLatency: latency(step),
    stagedStepLatency: latency(staged),
    jumpLatency: latency(jump),
    warmStepLatency: latency(warm),
    stepMinCoverage: stepCoverage.length ? Number(Math.min(...stepCoverage).toFixed(4)) : null,
    stepLayouts: d(m0, m1, "LayoutCount"),
    stepStyleRecalcs: d(m0, m1, "RecalcStyleCount"),
    jumpStyleRecalcs: d(before, after, "RecalcStyleCount"),
    jumpStyleMs: Number((d(before, after, "RecalcStyleDuration") * 1000).toFixed(2)),
    stagedStyleRecalcs: d(beforeStaged, afterStaged, "RecalcStyleCount"),
    jumpFrames: gaps.length,
    jumpWorstFrame: Number(Math.max(...gaps).toFixed(2)),
    jumpSlowFrames: gaps.filter((x) => x > 20).length,
  };
}

/** Median of `RUNS` passes; `slow` models a backend where a full picture takes that many ms to arrive. */
async function report(options) {
  const runs = [];
  for (let i = 0; i < RUNS; i++) runs.push(await pass(options));
  return Object.fromEntries(
    Object.keys(runs[0]).map((k) => {
      const values = runs.map((r) => r[k]).filter((v) => v !== null && Number.isFinite(v));
      return [k, values.length ? Number(median(values).toFixed(2)) : null];
    }),
  );
}

console.log(JSON.stringify({ fast: await report(), slow: await report({ slow: 600 }) }));
await browser.close();
await server.close();
