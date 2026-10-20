import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getSettings, initialPath, openPath, registerAssociations } from "./api";
import { showFitted } from "./chrome";
import { bindFolderSync } from "./folder-sync";
import { bindSettingsPanel } from "./settings-panel";
import "./style.css";
import { Viewer } from "./viewer";

const app = document.getElementById("app")!;

async function open(viewer: Viewer, path: string): Promise<void> {
  try {
    await viewer.load(await openPath(path));
  } catch (err) {
    viewer.notify(String(err));
    if (viewer.files().length === 0) viewer.showEmpty();
    await showFitted(null); // never leave the window hidden on a failed open
  }
}

async function boot(): Promise<void> {
  const settings = await getSettings();
  const viewer = new Viewer(app, settings, (p) => open(viewer, p));
  bindSettingsPanel(document.getElementById("panel")!, settings, (s) => viewer.applySettings(s), (t) => viewer.notify(t));

  document.getElementById("btn-assoc")!.addEventListener("click", async () => {
    try {
      await registerAssociations();
      viewer.notify("Registered. Pick Vislo for your image types in Settings > Default apps.");
    } catch (err) {
      viewer.notify(String(err));
    }
  });

  bindFolderSync(viewer, (p) => open(viewer, p));
  await listen<string>("open-path", (e) => void open(viewer, e.payload));
  await getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    app.classList.toggle("dragover", p.type === "enter" || p.type === "over");
    if (p.type === "drop" && p.paths[0]) void open(viewer, p.paths[0]);
  });
  const initial = await initialPath();
  if (initial) await open(viewer, initial);
  else {
    viewer.showEmpty();
    await showFitted(null);
  }
}

void boot();
