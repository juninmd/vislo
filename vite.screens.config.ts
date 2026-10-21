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
          const path = new URL(req.url ?? "", "http://x").searchParams.get("p") ?? "";
          const photo = PHOTOS.find((p) => path.endsWith(p.name)) ?? PHOTOS[0];
          res.setHeader("Content-Type", "image/svg+xml");
          res.end(svgFor(photo));
        });
      },
    },
  ],
  server: { port: 1431, strictPort: true },
});
