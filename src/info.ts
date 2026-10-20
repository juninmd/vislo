import { fileInfo, type Info } from "./api";

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[u]}`;
}

export function formatDimensions(w: number | null, h: number | null): string | null {
  if (!w || !h) return null;
  return `${w} × ${h} px (${((w * h) / 1e6).toFixed(1)} MP)`;
}

/** Side panel with file facts and EXIF, the usual photo properties. */
export class InfoPanel {
  private token = 0;

  constructor(private readonly el: HTMLElement) {}

  get visible(): boolean {
    return this.el.classList.contains("show");
  }

  toggle(path: string | null): void {
    const on = this.el.classList.toggle("show");
    if (on && path) void this.load(path);
  }

  close(): void {
    this.el.classList.remove("show");
  }

  /** Called on every navigation; cheap when the panel is hidden. */
  refresh(path: string): void {
    if (this.visible) void this.load(path);
  }

  private async load(path: string): Promise<void> {
    const mine = ++this.token;
    let info: Info;
    try {
      info = await fileInfo(path);
    } catch (err) {
      if (mine === this.token) this.el.replaceChildren(row("Error", String(err)));
      return;
    }
    if (mine !== this.token) return; // a newer image was requested meanwhile
    const rows: [string, string | null][] = [
      ["File", info.name],
      ["Folder", info.folder],
      ["Size", formatBytes(info.size)],
      ["Dimensions", formatDimensions(info.width, info.height)],
      ["Modified", info.modified_ms === null ? null : new Date(info.modified_ms).toLocaleString()],
      ...info.rows,
    ];
    this.el.replaceChildren(...rows.filter((r): r is [string, string] => r[1] !== null).map(([k, v]) => row(k, v)));
  }
}

function row(label: string, value: string): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "info-row";
  const k = document.createElement("span");
  k.textContent = label;
  const v = document.createElement("span");
  v.textContent = value; // textContent: EXIF strings are untrusted
  v.title = value;
  wrap.append(k, v);
  return wrap;
}
