import { copyImage, openWith, pickImage, revealInFolder } from "./api";
import { CopyError, copyFromPage } from "./copy-page";
import { isPageCopy } from "./formats";
import { t } from "./i18n";

export type Command = "copy" | "open" | "reveal" | "openWith";
export type Shrunk = { w: number; h: number } | null;

interface Host {
  path(): string | null;
  notify(text: string): void;
}

/**
 * Formats the backend cannot decode (AVIF, SVG) are copied from the page; everything else goes through the
 * backend, and its errors are reported as they are rather than retried by the page.
 */
export function copyPicture(
  path: string,
  backend: (path: string) => Promise<Shrunk>,
  page: (path: string) => Promise<Shrunk>,
  pageOnly: (path: string) => boolean,
): Promise<Shrunk> {
  return pageOnly(path) ? page(path) : backend(path);
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
        const shrunk = await copyPicture(path, copyImage, copyFromPage, isPageCopy);
        this.host.notify(shrunk ? t("msg.copiedDownscaled", { w: shrunk.w, h: shrunk.h }) : t("msg.copied"));
      } else if (command === "reveal") {
        await revealInFolder(path);
      } else {
        await openWith(path);
      }
    } catch (err) {
      this.host.notify(err instanceof CopyError ? t(err.key) : String(err));
    }
  }
}
