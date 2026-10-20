import { registerAssociations, type Settings, setSettings } from "./api";

const CHECKS: [string, keyof Settings][] = [
  ["#s-fs", "fullscreen_on_start"],
  ["#s-autohide", "autohide_controls"],
  ["#s-sharp", "sharp_downscale"],
  ["#s-preview", "embedded_preview"],
  ["#s-3d", "filmstrip_3d"],
  ["#s-hover", "hover_preview"],
];

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
      const n = Number(el<HTMLInputElement>(sel).value);
      return Number.isFinite(n) && n >= 0 ? Math.round(n * 1000) : fallback;
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

  panel.querySelector("#btn-assoc-panel")?.addEventListener("click", async () => {
    try {
      await registerAssociations();
      notify("Registered. Pick Vislo for your image types in Settings > Default apps.");
    } catch (err) {
      notify(String(err));
    }
  });
  render();
}
