import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { type Album, errorMessage, getSettings, initialPath, openPath, registerAssociations } from "./api";
import { boot as bootData, prefetchLaunchImage } from "./boot";
import { showFitted } from "./chrome";
import { bindFolderSync } from "./folder-sync";
import { setLanguage, t } from "./i18n";
import { bindSettingsPanel } from "./settings-panel";
import "./style.css";
import { Updater } from "./update";
import { Viewer } from "./viewer";

const app = document.getElementById("app")!;

async function open(viewer: Viewer, path: string, listing: Promise<Album> = openPath(path)): Promise<void> {
  try {
    await viewer.load(await listing);
  } catch (err) {
    viewer.notify(errorMessage(err));
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
  // The window was created without a title bar (see window.rs `frameless`): show our own buttons.
  app.classList.toggle("frameless", settings.open_maximized);
  await setLanguage(settings.language); // English loads nothing; another language is one small chunk
  const viewer = new Viewer(app, settings, (p) => open(viewer, p));
  let current = settings;
  bindSettingsPanel(
    document.getElementById("panel")!,
    settings,
    (s) => {
      if (s.language !== current.language) void setLanguage(s.language);
      viewer.applySettings((current = s));
    },
    (t) => viewer.notify(t),
  );
  const updater = new Updater(document.getElementById("update")!, (t) => viewer.notify(t), () => current.check_updates);
  document.getElementById("btn-update-check")!.addEventListener("click", () => void updater.check(true));
  getVersion().then(
    (v) => (document.getElementById("s-version")!.textContent = `Vislo ${v}`),
    () => undefined,
  );

  document.getElementById("btn-assoc")!.addEventListener("click", async () => {
    try {
      await registerAssociations();
      viewer.notify(t("msg.registered"));
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
  updater.start(); // never competes with the first picture
}

void boot();
