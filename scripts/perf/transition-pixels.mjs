// Proves that a step never lets the background through. The page is painted behind the two pictures
// in pure magenta and the photos are drawn in greys, so magenta in a frame is background only.
// The same held frame is measured twice: once as the transition paints it and once with its opacity
// animations removed (both pictures opaque). The difference is exactly how much of the background the
// fade lets through; 0 means none. Used while tuning `src/slide-anim.ts`; prints one JSON line.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const CANDIDATES = [
  process.env.CHROME_PATH,
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
];
const executablePath = CANDIDATES.find((p) => p && existsSync(p));
if (!executablePath) throw new Error("No Chromium/Chrome found: set CHROME_PATH");

const server = await createServer({ configFile: fileURLToPath(new URL("../../vite.screens.config.ts", import.meta.url)), logLevel: "error" });
await server.listen();
const base = `http://localhost:${server.config.server.port}/`;
const browser = await chromium.launch({ executablePath, args: ["--no-sandbox"] });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

await page.addInitScript(() => {
  window.__hold = null;
  const loop = () => {
    if (window.__hold && !window.__held) {
      const anims = document.getAnimations().filter((a) => a.playState === "running" && a.effect?.target?.classList?.contains("slide"));
      if (anims.length) {
        for (const a of anims) {
          a.pause();
          a.currentTime = window.__hold;
        }
        window.__held = anims.length;
      }
    }
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
});

await page.goto(`${base}?scene=viewer`);
await page.waitForSelector("#app");
await page.addStyleTag({ content: "#app { background: #ff00ff !important; } #stage img { filter: grayscale(1) !important; }" });
await wait(1500);

/** How much magenta is painted in the frame (0..100): the share of pure background colour. */
const magenta = async (b64) =>
  page.evaluate(async (data) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${data}`)).blob());
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const ctx = canvas.getContext("2d");
    ctx.drawImage(bitmap, 0, 0);
    const { data: px } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    let mix = 0;
    for (let i = 0; i < px.length; i += 4) mix += Math.min(px[i], px[i + 2]) - px[i + 1];
    return Math.round((mix / (px.length / 4) / 255) * 10000) / 100;
  }, b64);

const shot = async () => (await page.screenshot()).toString("base64");
const samples = [];
for (const at of [60, 120, 180]) {
  await page.evaluate((ms) => {
    window.__hold = ms;
    window.__held = null;
  }, at);
  await page.keyboard.press(at === 60 ? "ArrowRight" : "ArrowLeft");
  await wait(120);
  const painted = await magenta(await shot());
  // Drop only the opacity part of the transition: the transform stays exactly where it was.
  await page.evaluate(() => {
    for (const a of document.getAnimations()) {
      if (!a.effect?.target?.classList?.contains("slide")) continue;
      if (a.effect.getKeyframes().some((k) => "opacity" in k)) a.cancel();
    }
  });
  const opaque = await magenta(await shot());
  samples.push({ at, bleed: Math.round((painted - opaque) * 100) / 100, pace: await page.evaluate(() => window.__held ?? 0) });
  await page.evaluate(() => {
    for (const a of document.getAnimations()) a.finish();
    window.__hold = null;
  });
  await wait(250);
}

console.log(JSON.stringify(samples));
await browser.close();
await server.close();
