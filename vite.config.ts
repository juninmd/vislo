import { defineConfig } from "vite";

// Fixed port so tauri.conf.json devUrl stays valid.
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", chunkSizeWarningLimit: 200 },
});
