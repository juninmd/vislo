import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { type Album, getSettings, initialPath, openPath, registerAssociations } from "./api";
import { boot as bootData, prefetchLaunchImage } from "./boot";
import { showFitted } from "./chrome";
import { bindFolderSync } from "./folder-sync";
import { bindSettingsPanel } from "./settings-panel";
import "./style.css";
import { Viewer } from "./viewer";

const app = document.getElementById("app")!;

async function open(viewer: Viewer, path: string, listing: Promise<Album> = openPath(path)): Promise<void> {
  try {
    await viewer.load(await listing);
  } catch (err) {
    viewer.notify(String(err));
    if (viewer.files().length === 0) viewer.showEmpty();
    await showFitted(null); // never leave the window hidden on a failed open
  }
}

async function boot(): Promise<void> {
  prefetchLaunchImage(); // decoding starts now, in parallel with everything below
  // The backend injects settings and the launch file, saving two IPC round trips.
  const settings = bootData?.settings ?? (await getSettings());
  const initial = bootData ? bootData.path : await initialPath();
  const listing = initial ? openPath(initial) : undefined;
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

  let opened: Promise<void> | undefined;
  if (initial && listing) opened = open(viewer, initial, listing);
  else if (bootData?.background) viewer.park(); // login start: stay hidden until a file arrives
  else {
    viewer.showEmpty();
    opened = showFitted(null);
  }
  // Registered after the first open is under way: they are not needed for the first frame.
  await Promise.all([
    listen<string>("open-path", (e) => void open(viewer, e.payload)),
    listen("parked", () => viewer.park()),
    listen("reveal", () => {
      viewer.showEmpty(); // a plain launch of a parked instance: same as starting with no file
      void showFitted(null);
    }),
    getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      app.classList.toggle("dragover", p.type === "enter" || p.type === "over");
      if (p.type === "drop" && p.paths[0]) void open(viewer, p.paths[0]);
    }),
  ]);
  await opened;
}

void boot();
