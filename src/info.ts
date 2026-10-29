import { fileInfo, type Info } from "./api";
import { buildGroups } from "./info-rows";
import { currentLanguage, t } from "./i18n";

/** Side panel with file facts and EXIF, grouped; a click on a value copies it. */
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
      if (mine === this.token) this.el.replaceChildren(row(t("info.error"), String(err)));
      return;
    }
    if (mine !== this.token) return; // a newer image was requested meanwhile
    const fmt = new Intl.DateTimeFormat(currentLanguage(), { dateStyle: "medium", timeStyle: "short" });
    const groups = buildGroups(info, t, (ms) => fmt.format(ms));
    this.el.replaceChildren(
      ...groups.flatMap((g) => [heading(g.title), ...g.lines.map((l) => row(l.label, l.value))]),
    );
  }
}

function heading(title: string): HTMLElement {
  const h = document.createElement("div");
  h.className = "info-group";
  h.textContent = title;
  return h;
}

function row(label: string, value: string): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "info-row";
  const k = document.createElement("span");
  k.textContent = label;
  const v = document.createElement("span");
  v.textContent = value; // textContent: EXIF strings are untrusted
  v.title = `${value}\n${t("info.copy")}`;
  v.addEventListener("click", () => void copy(v, value));
  wrap.append(k, v);
  return wrap;
}

async function copy(el: HTMLElement, value: string): Promise<void> {
  if (getSelection()?.toString()) return; // a selection drag ends in a click: leave it alone
  try {
    await navigator.clipboard.writeText(value);
  } catch {
    return; // clipboard blocked: the text stays selectable
  }
  el.classList.add("copied");
  setTimeout(() => el.classList.remove("copied"), 700);
}
