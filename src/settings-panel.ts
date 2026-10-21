import { registerAssociations, type Settings, setSettings } from "./api";

const CHECKS: [string, keyof Settings][] = [
  ["#s-max", "open_maximized"],
  ["#s-fs", "fullscreen_on_start"],
  ["#s-autohide", "autohide_controls"],
  ["#s-sharp", "sharp_downscale"],
  ["#s-preview", "embedded_preview"],
  ["#s-3d", "filmstrip_3d"],
  ["#s-hover", "hover_preview"],
  ["#s-keep", "keep_ready"],
  ["#s-login", "start_with_windows"],
  ["#s-updates", "check_updates"],
];

const GROUPS_KEY = "vislo.groups";

/** Format groups ticked for default-app registration; remembered per viewer, all ticked by default. */
function bindGroups(panel: HTMLElement): () => string[] {
  const boxes = [...panel.querySelectorAll<HTMLInputElement>("input[data-group]")];
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(GROUPS_KEY) ?? "null");
    if (Array.isArray(saved)) for (const b of boxes) b.checked = saved.includes(b.dataset.group);
  } catch {
    /* storage unavailable: keep the defaults */
  }
  const chosen = () => boxes.filter((b) => b.checked).map((b) => b.dataset.group!);
  for (const b of boxes) {
    b.addEventListener("change", (e) => {
      e.stopPropagation(); // these are not settings.json fields
      try {
        localStorage.setItem(GROUPS_KEY, JSON.stringify(chosen()));
      } catch {
        /* remembering is a convenience only */
      }
    });
  }
  return chosen;
}

/** Binds the settings panel: every change is validated and saved by the backend, then applied. */
export function bindSettingsPanel(
  panel: HTMLElement,
  initial: Settings,
  apply: (s: Settings) => void,
  notify: (text: string) => void,
): void {
  let current = initial;
  const el = <T extends HTMLElement>(sel: string) => panel.querySelector<T>(sel)!;

  const render = () => {
    el<HTMLSelectElement>("#s-wheel").value = current.wheel;
    el<HTMLSelectElement>("#s-bg").value = current.background;
    el<HTMLInputElement>("#s-display").value = String(current.slideshow_display_ms / 1000);
    el<HTMLInputElement>("#s-fade").value = String(current.slideshow_fade_ms / 1000);
    for (const [sel, key] of CHECKS) el<HTMLInputElement>(sel).checked = current[key] as boolean;
  };

  const collect = (): Settings => {
    const next: Settings = { ...current };
    next.wheel = el<HTMLSelectElement>("#s-wheel").value as Settings["wheel"];
    next.background = el<HTMLSelectElement>("#s-bg").value as Settings["background"];
    const secs = (sel: string, fallback: number) => {
      const text = el<HTMLInputElement>(sel).value.trim();
      const n = Number(text);
      return text !== "" && Number.isFinite(n) && n >= 0 ? Math.round(n * 1000) : fallback; // a cleared field is not "0 s"
    };
    next.slideshow_display_ms = secs("#s-display", current.slideshow_display_ms);
    next.slideshow_fade_ms = secs("#s-fade", current.slideshow_fade_ms);
    for (const [sel, key] of CHECKS) (next[key] as boolean) = el<HTMLInputElement>(sel).checked;
    return next;
  };

  panel.addEventListener("change", async () => {
    try {
      current = await setSettings(collect());
      render(); // shows the backend's clamped values
      apply(current);
    } catch (err) {
      notify(String(err));
    }
  });

  const chosenGroups = bindGroups(panel);
  panel.querySelector("#btn-assoc-panel")?.addEventListener("click", async () => {
    const groups = chosenGroups();
    if (groups.length === 0) return notify("Tick at least one format first");
    try {
      await registerAssociations(groups);
      notify("Registered. Pick Vislo for your image types in Settings > Default apps.");
    } catch (err) {
      notify(String(err));
    }
  });
  render();
}
