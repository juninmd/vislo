import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { PHOTOS, svgFor } from "./scripts/screenshots/scenes.mjs";

const mock = (name: string) => fileURLToPath(new URL(`./scripts/screenshots/mock/${name}.ts`, import.meta.url));

/** Runs the real frontend against fake Tauri bindings; only used by `npm run screenshots`. */
export default defineConfig({
  resolve: {
    alias: {
      "@tauri-apps/api/core": mock("core"),
      "@tauri-apps/api/event": mock("event"),
      "@tauri-apps/api/webview": mock("webview"),
      "@tauri-apps/api/window": mock("window"),
      "@tauri-apps/api/app": mock("app"),
    },
  },
  plugins: [
    {
      name: "mock-photos",
      configureServer(server) {
        server.middlewares.use("/mock", (req, res) => {
          const url = new URL(req.url ?? "", "http://x");
          const path = url.searchParams.get("p") ?? "";
          const photo = PHOTOS.find((p) => path.endsWith(p.name)) ?? PHOTOS[0];
          // `slow` (a cookie) stands in for a slow disk or a cold decoder, so the perf harness can
          // measure what the viewer does while a picture is not yet available. Full-size pictures take
          // the whole delay, the small stand-ins and thumbnails a quarter of it, as in real life.
          const slow = Number(/slow=(\d+)/.exec(req.headers.cookie ?? "")?.[1] ?? 0);
          // The middleware is mounted on /mock, so the route is the last path segment ("/o", "/v", "/t").
          const route = url.pathname.split("/").filter(Boolean).pop() ?? "";
          setTimeout(() => {
            res.setHeader("Content-Type", "image/svg+xml");
            res.end(svgFor(photo));
          }, /^(o|c)$/.test(route) ? slow : Math.round(slow / 4));
        });
      },
    },
  ],
  server: { port: 1431, strictPort: true },
});
