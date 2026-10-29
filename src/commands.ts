import { copyImage, openWith, pickImage, revealInFolder } from "./api";
import { t } from "./i18n";

export type Command = "copy" | "open" | "reveal" | "openWith";

interface Host {
  path(): string | null;
  notify(text: string): void;
}

/** File-level actions that go through the backend: clipboard, open dialog, Explorer. */
export class Commands {
  private picking = false; // one open dialog at a time, whatever the input path

  constructor(
    private readonly host: Host,
    private readonly open: (path: string) => Promise<void>,
  ) {}

  async run(command: Command): Promise<void> {
    const path = this.host.path();
    try {
      if (command === "open") {
        if (this.picking) return;
        this.picking = true;
        const picked = await pickImage().finally(() => (this.picking = false));
        if (picked) await this.open(picked);
      } else if (!path) {
        return;
      } else if (command === "copy") {
        await copyImage(path);
        this.host.notify(t("msg.copied"));
      } else if (command === "reveal") {
        await revealInFolder(path);
      } else {
        await openWith(path);
      }
    } catch (err) {
      this.host.notify(String(err));
    }
  }
}
