import type { Command } from "./commands";
import type { MenuEntry } from "./context-menu";

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
    file("Copy image", "copy", "Ctrl+C"),
    { label: "Open image…", hint: "O", run: () => a.command("open") },
    file("Show in folder", "reveal"),
    file("Open with…", "openWith"),
    "separator",
    { label: "Information", hint: "I", disabled: !hasPicture, run: () => a.toggleInfo() },
    { label: "Slideshow", hint: "S", disabled: !hasPicture, run: () => a.toggleSlideshow() },
    { label: "Filmstrip", hint: "T", disabled: !hasPicture, run: () => a.toggleStrip() },
    { label: "Full screen", hint: "F", run: () => a.toggleFullscreen() },
    { label: "Settings", run: () => a.toggleSettings() },
  ];
}
