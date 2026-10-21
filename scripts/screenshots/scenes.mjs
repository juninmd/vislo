// Procedural "photographs" for the README screenshots: layered landscapes drawn as SVG, so the
// repository needs no stock photos and the pictures are identical on every run.

const W = 3000;
const H = 2000;

function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** A ridge line across the picture (midpoint displacement) as a closed path down to the bottom edge. */
function ridge(rand, base, amp, rough = 0.55) {
  const n = 128;
  const ys = new Array(n + 1).fill(0);
  ys[0] = (rand() - 0.5) * amp;
  ys[n] = (rand() - 0.5) * amp;
  let disp = amp;
  for (let step = n; step > 1; step /= 2, disp *= rough) {
    for (let i = step / 2; i < n; i += step) ys[i] = (ys[i - step / 2] + ys[i + step / 2]) / 2 + (rand() - 0.5) * disp;
  }
  const pts = ys.map((y, i) => `${((i / n) * W).toFixed(0)} ${(base + y).toFixed(0)}`);
  return `M0 ${H} L${pts.join(" L")} L${W} ${H} Z`;
}

export const PHOTOS = [
  { name: "Golden dunes.jpg", seed: 21, sky: ["#2b1b4d", "#c2517a", "#ffb35c"], sun: { x: 2100, y: 880, r: 190, c: "#fff1c9" }, layers: ["#d9985a", "#b9703f", "#7c3f2a"], stars: false, rough: 0.42 },
  { name: "Aurora over the fjord.jpg", seed: 7, sky: ["#050b24", "#0b2a4a", "#1f6f78"], sun: null, aurora: true, layers: ["#0a1830", "#07101f", "#040a14"], stars: true },
  { name: "Alpine lake.jpg", seed: 33, sky: ["#6ab0e8", "#bfe0f5", "#f4f8ff"], sun: { x: 600, y: 520, r: 120, c: "#fffbe6" }, layers: ["#7d93b0", "#4d6a64", "#27433f"], stars: false, lake: true },
  { name: "Neon dusk.jpg", seed: 48, sky: ["#12062b", "#5b1a6e", "#ff5f8d"], sun: { x: 1500, y: 1020, r: 260, c: "#ffd2a8" }, layers: ["#2a0f45", "#1c0a33", "#0f051f"], stars: true },
  { name: "Autumn ridge.jpg", seed: 59, sky: ["#3c6a8c", "#e9c8a0", "#fbe9c4"], sun: { x: 1000, y: 800, r: 150, c: "#fff6d8" }, layers: ["#b5652f", "#8a3f1d", "#4d2511"], stars: false },
  { name: "Coral haze.jpg", seed: 71, sky: ["#0d3b66", "#2a9d8f", "#f4d58d"], sun: { x: 2400, y: 1000, r: 170, c: "#fff3c4" }, layers: ["#1d7a73", "#115e59", "#0a3f3c"], stars: false, lake: true },
];

export function svgFor(photo) {
  const rand = rng(photo.seed);
  const [a, b, c] = photo.sky;
  const parts = [];
  parts.push(`<defs>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="0.55" stop-color="${b}"/><stop offset="1" stop-color="${c}"/></linearGradient>
    <radialGradient id="sun"><stop offset="0" stop-color="${photo.sun?.c ?? "#fff"}" stop-opacity="1"/><stop offset="0.35" stop-color="${photo.sun?.c ?? "#fff"}" stop-opacity="0.45"/><stop offset="1" stop-color="${photo.sun?.c ?? "#fff"}" stop-opacity="0"/></radialGradient>
    <linearGradient id="aur" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3dffb0" stop-opacity="0"/><stop offset="0.5" stop-color="#3dffb0" stop-opacity="0.55"/><stop offset="1" stop-color="#7a5cff" stop-opacity="0"/></linearGradient>
    <filter id="soft"><feGaussianBlur stdDeviation="28"/></filter>
  </defs>`);
  parts.push(`<rect width="${W}" height="${H}" fill="url(#sky)"/>`);
  if (photo.stars) {
    for (let i = 0; i < 260; i++) {
      parts.push(`<circle cx="${(rand() * W).toFixed(0)}" cy="${(rand() * H * 0.55).toFixed(0)}" r="${(rand() * 2.6 + 0.6).toFixed(1)}" fill="#fff" opacity="${(rand() * 0.8 + 0.2).toFixed(2)}"/>`);
    }
  }
  if (photo.aurora) {
    for (let i = 0; i < 4; i++) {
      const x = 300 + i * 650 + rand() * 120;
      parts.push(`<path d="M${x} 1000 C ${x + 200} 700 ${x - 150} 420 ${x + 120} 120 L ${x + 420} 150 C ${x + 200} 450 ${x + 520} 760 ${x + 300} 1000 Z" fill="url(#aur)" filter="url(#soft)"/>`);
    }
  }
  if (photo.sun) {
    parts.push(`<circle cx="${photo.sun.x}" cy="${photo.sun.y}" r="${photo.sun.r * 4}" fill="url(#sun)"/>`);
    parts.push(`<circle cx="${photo.sun.x}" cy="${photo.sun.y}" r="${photo.sun.r}" fill="${photo.sun.c}"/>`);
  }
  const bases = [1080, 1330, 1580];
  photo.layers.forEach((color, i) => {
    parts.push(`<path d="${ridge(rand, bases[i], 260 - i * 50, photo.rough ?? 0.58)}" fill="${color}"/>`);
    if (i === 0) parts.push(`<rect y="${bases[i] + 120}" width="${W}" height="${H}" fill="${color}" opacity="0.0"/>`);
  });
  if (photo.lake) {
    parts.push(`<rect y="1500" width="${W}" height="${H - 1500}" fill="${a}" opacity="0.28"/>`);
    for (let i = 0; i < 40; i++) {
      parts.push(`<rect x="${(rand() * W).toFixed(0)}" y="${(1520 + rand() * 440).toFixed(0)}" width="${(rand() * 320 + 60).toFixed(0)}" height="3" fill="#fff" opacity="${(rand() * 0.25).toFixed(2)}"/>`);
    }
  }
  // soft vignette, like a lens
  parts.push(`<radialGradient id="v" cx="0.5" cy="0.5" r="0.75"><stop offset="0.6" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.45"/></radialGradient><rect width="${W}" height="${H}" fill="url(#v)"/>`);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">${parts.join("")}</svg>`;
}

export const FOLDER = "C:\\Users\\you\\Pictures\\Trip";
export const files = () => PHOTOS.map((p) => `${FOLDER}\\${p.name}`);
export const size = [W, H];
