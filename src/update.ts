import {
  cancelUpdate,
  checkUpdate,
  installUpdate,
  onUpdateProgress,
  openReleases,
  type Update,
  type UpdateProgress,
} from "./api";
import { t } from "./i18n";

const FIRST_CHECK_MS = 5_000; // after the first picture is up, never before
const RECHECK_MS = 6 * 60 * 60 * 1000; // a resident instance keeps looking

/** `Vislo 1.2.0 is available`, with the first line of the release notes when there is one. */
export function updateText(u: Pick<Update, "version" | "notes">): string {
  const head = u.notes.split("\n").find((l) => /\w/.test(l) && !l.startsWith("#"))?.replace(/^[-*]\s*/, "").replace(/[*_`]/g, "");
  return head ? t("update.availableNotes", { version: u.version, notes: head }) : t("update.available", { version: u.version });
}

/** `Downloading… 42%`, or the megabytes so far when the server did not send a length. */
export function progressText(p: UpdateProgress): string {
  if (p.total && p.total > 0) {
    return t("update.progress", { percent: Math.min(100, Math.floor((p.received * 100) / p.total)) });
  }
  return t("update.progressBytes", { megabytes: (p.received / (1024 * 1024)).toFixed(1) });
}

/** Looks for new releases of this very repository and offers a one-click update. */
export class Updater {
  private offered: string | null = null; // version already shown, so a re-check does not nag
  private offeredText = ""; // the banner line for that version, restored when an install fails
  private busy = false;

  constructor(
    private readonly banner: HTMLElement,
    private readonly notify: (text: string) => void,
    private readonly enabled: () => boolean,
  ) {
    banner.querySelector("#update-later")!.addEventListener("click", () => (banner.hidden = true));
    banner.querySelector("#update-go")!.addEventListener("click", () => void this.install());
    banner.querySelector("#update-cancel")!.addEventListener("click", () =>
      void cancelUpdate().catch((e) => this.notify(String(e))),
    );
  }

  /** Background checks: one soon after start, then periodically while the process lives. */
  start(): void {
    const tick = () => void this.check(false);
    setTimeout(tick, FIRST_CHECK_MS);
    setInterval(tick, RECHECK_MS);
  }

  /** `manual` checks always answer, even when up to date or offline. */
  async check(manual: boolean): Promise<void> {
    if (this.busy || (!manual && !this.enabled())) return;
    this.busy = true;
    try {
      const update = await checkUpdate();
      if (!update) return manual ? this.notify(t("update.upToDate")) : undefined;
      if (manual || this.offered !== update.version) this.show(update);
    } catch (err) {
      if (manual) this.notify(String(err)); // offline at startup is not worth a message
    } finally {
      this.busy = false;
    }
  }

  private show(update: Update): void {
    this.offered = update.version;
    this.offeredText = updateText(update);
    this.banner.querySelector("#update-text")!.textContent = this.offeredText;
    const go = this.banner.querySelector<HTMLButtonElement>("#update-go")!;
    go.textContent = update.installable ? t("update.install") : t("update.download");
    go.disabled = false;
    go.dataset.installable = String(update.installable);
    this.banner.hidden = false;
  }

  private async install(): Promise<void> {
    const go = this.banner.querySelector<HTMLButtonElement>("#update-go")!;
    if (go.dataset.installable !== "true") return void openReleases().catch((e) => this.notify(String(e)));
    const text = this.banner.querySelector<HTMLElement>("#update-text")!;
    const cancel = this.banner.querySelector<HTMLButtonElement>("#update-cancel")!;
    go.disabled = true;
    go.textContent = t("update.downloading");
    cancel.hidden = false;
    let stop: (() => void) | undefined;
    try {
      stop = await onUpdateProgress((p) => (text.textContent = progressText(p)));
      await installUpdate(); // the app quits once the installer is running
    } catch (err) {
      this.notify(String(err)); // includes "the update was cancelled"
      text.textContent = this.offeredText;
      go.disabled = false;
      go.textContent = t("update.install");
    } finally {
      stop?.();
      cancel.hidden = true;
    }
  }
}
