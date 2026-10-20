import { copyImage, openWith, pickImage, revealInFolder } from "./api";

export type Command = "copy" | "open" | "reveal" | "openWith";

interface Host {
  path(): string | null;
  notify(text: string): void;
}

/** File-level actions that go through the backend: clipboard, open dialog, Explorer. */
export class Commands {
  constructor(
    private readonly host: Host,
    private readonly open: (path: string) => Promise<void>,
  ) {}

  async run(command: Command): Promise<void> {
    const path = this.host.path();
    try {
      if (command === "open") {
        const picked = await pickImage();
        if (picked) await this.open(picked);
      } else if (!path) {
        return;
      } else if (command === "copy") {
        await copyImage(path);
        this.host.notify("Copied to clipboard");
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
