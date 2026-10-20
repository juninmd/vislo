export interface MenuItem {
  label: string;
  hint?: string;
  disabled?: boolean;
  run: () => void;
}

export type MenuEntry = MenuItem | "separator";

/** Keyboard-reachable popup menu; closes on any outside click, scroll, resize or blur. */
export class ContextMenu {
  constructor(private readonly el: HTMLElement) {
    el.addEventListener("keydown", (e) => this.onKey(e));
    document.addEventListener("pointerdown", (e) => {
      if (!el.hidden && !el.contains(e.target as Node)) this.hide();
    });
    for (const ev of ["wheel", "resize", "blur"]) addEventListener(ev, () => this.hide(), { passive: true });
  }

  get open(): boolean {
    return !this.el.hidden;
  }

  show(x: number, y: number, entries: readonly MenuEntry[]): void {
    this.el.replaceChildren(...entries.map((entry) => this.row(entry)));
    this.el.hidden = false;
    const { width, height } = this.el.getBoundingClientRect();
    this.el.style.left = `${Math.max(0, Math.min(x, innerWidth - width))}px`;
    this.el.style.top = `${Math.max(0, Math.min(y, innerHeight - height))}px`;
    this.buttons().find((b) => !b.disabled)?.focus();
  }

  hide(): void {
    if (this.el.hidden) return;
    this.el.hidden = true;
    this.el.replaceChildren();
  }

  private row(entry: MenuEntry): HTMLElement {
    if (entry === "separator") return document.createElement("hr");
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "menuitem");
    b.disabled = !!entry.disabled;
    b.append(entry.label);
    if (entry.hint) {
      const hint = document.createElement("span");
      hint.className = "hint";
      hint.textContent = entry.hint;
      b.append(hint);
    }
    b.addEventListener("click", () => {
      this.hide();
      entry.run();
    });
    return b;
  }

  private buttons(): HTMLButtonElement[] {
    return [...this.el.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
  }

  private onKey(e: KeyboardEvent): void {
    const items = this.buttons();
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    const move = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === "ArrowDown") move(at + 1);
    else if (e.key === "ArrowUp") move(at - 1);
    else if (e.key === "Home") move(0);
    else if (e.key === "End") move(-1);
    else if (e.key === "Escape" || e.key === "Tab") this.hide();
    else return; // Enter/Space activate the focused button natively
    e.preventDefault();
    e.stopPropagation(); // the global shortcuts must not see keys meant for the menu
  }
}
