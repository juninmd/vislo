/** Where the focus was before a popup took it, handed back when the popup closes so keyboard users resume there. */
export class FocusReturn {
  private opener: HTMLElement | null = null;

  /** Call before the popup moves the focus into itself. */
  save(): void {
    this.opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  }

  /** Call once the popup is hidden; `hadFocus` says whether the focus was inside it, so a click elsewhere keeps its target. */
  restore(hadFocus: boolean): void {
    if (hadFocus && this.opener?.isConnected) this.opener.focus();
    this.opener = null;
  }
}
