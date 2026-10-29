import type { Command } from "./commands";
import type { MenuEntry } from "./context-menu";
import { t } from "./i18n";

export interface MenuActions {
  command(command: Command): void;
  toggleInfo(): void;
  toggleSlideshow(): void;
  toggleStrip(): void;
  toggleFullscreen(): void;
  toggleSettings(): void;
}

/** The right-click menu; file actions are greyed out until a picture is open. */
export function menuItems(a: MenuActions, hasPicture: boolean): MenuEntry[] {
  const file = (label: string, command: Command, hint?: string): MenuEntry => ({ label, hint, disabled: !hasPicture, run: () => a.command(command) });
  return [
    file(t("menu.copy"), "copy", "Ctrl+C"),
    { label: t("action.open"), hint: "O", run: () => a.command("open") },
    file(t("menu.reveal"), "reveal"),
    file(t("menu.openWith"), "openWith"),
    "separator",
    { label: t("hud.info"), hint: "I", disabled: !hasPicture, run: () => a.toggleInfo() },
    { label: t("hud.slideshow"), hint: "S", disabled: !hasPicture, run: () => a.toggleSlideshow() },
    { label: t("hud.filmstrip"), hint: "T", disabled: !hasPicture, run: () => a.toggleStrip() },
    { label: t("hud.fullscreen"), hint: "F", run: () => a.toggleFullscreen() },
    { label: t("hud.settings"), run: () => a.toggleSettings() },
  ];
}
