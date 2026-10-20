import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { showFitted } from "./chrome";
import "./style.css";
import { type Album, Viewer } from "./viewer";

const app = document.getElementById("app")!;
const viewer = new Viewer(app, document.getElementById("stage")!);

async function openPath(path: string): Promise<void> {
  try {
    await viewer.load(await invoke<Album>("open_path", { path }));
  } catch (err) {
    viewer.notify(String(err));
    if (!viewer.hasAlbum) viewer.showEmpty();
    await showFitted(null); // never leave the window hidden on a failed open
  }
}

async function boot(): Promise<void> {
  await listen<string>("open-path", (e) => void openPath(e.payload));
  await getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    app.classList.toggle("dragover", p.type === "enter" || p.type === "over");
    if (p.type === "drop" && p.paths[0]) void openPath(p.paths[0]);
  });
  const initial = await invoke<string | null>("initial_path");
  if (initial) await openPath(initial);
  else {
    viewer.showEmpty();
    await showFitted(null);
  }
}

void boot();
