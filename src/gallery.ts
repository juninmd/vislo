import { srcFor, thumbSrc } from "./api";
import { clampSize, GALLERY_MAX, GALLERY_MIN, GALLERY_STEP, type GalleryKey, moveIndex, thumbPx } from "./gallery-layout";

export interface GalleryHost {
  files(): readonly string[];
  position(): number;
  /** The user picked a picture: show it. */
  pick(index: number): void;
  /** The size or shape was changed: remember it. */
  save(size: number, square: boolean): void;
}

const KEYS = new Set<string>(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);
const basename = (p: string) => p.slice(Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/")) + 1);

/**
 * Every picture of the folder as a grid of thumbnails. Cells are plain buttons whose images load lazily as they
 * scroll into view, so a folder of thousands costs one DOM node each and a request only for what is on screen.
 */
export class Gallery {
  private cells: HTMLButtonElement[] = [];
  private built: readonly string[] | null = null;
  private size = 200;
  private square = true;
  private bucket = 0;
  private readonly grid: HTMLElement;
  private readonly slider: HTMLInputElement;
  private readonly crop: HTMLInputElement;

  constructor(
    private readonly root: HTMLElement,
    private readonly host: GalleryHost,
  ) {
    this.grid = root.querySelector<HTMLElement>(".g-grid")!;
    this.slider = root.querySelector<HTMLInputElement>("#g-size")!;
    this.crop = root.querySelector<HTMLInputElement>("#g-square")!;
    this.slider.min = String(GALLERY_MIN);
    this.slider.max = String(GALLERY_MAX);
    this.slider.step = String(GALLERY_STEP);
    this.slider.addEventListener("input", () => this.resize(Number(this.slider.value), false));
    this.slider.addEventListener("change", () => this.host.save(this.size, this.square));
    this.crop.addEventListener("change", () => {
      this.reshape(this.crop.checked);
      this.host.save(this.size, this.square);
    });
    this.grid.addEventListener("click", (e) => {
      const i = this.indexOf(e.target);
      if (i !== null) this.host.pick(i);
    });
    root.addEventListener("keydown", (e) => this.onKey(e));
    root.addEventListener(
      "wheel",
      (e) => {
        if (!e.ctrlKey) return; // a plain wheel scrolls the grid
        e.preventDefault();
        this.resize(this.size + (e.deltaY < 0 ? GALLERY_STEP : -GALLERY_STEP), true);
      },
      { passive: false },
    );
  }

  get visible(): boolean {
    return !this.root.hidden;
  }

  /** The folder changed: the cells are rebuilt the next time the gallery opens. */
  invalidate(): void {
    this.built = null;
    if (this.visible) this.show();
  }

  /** Size and shape from the settings; they apply at once, whether or not the gallery is open. */
  configure(size: number, square: boolean): void {
    this.size = clampSize(size);
    this.square = square;
    this.slider.value = String(this.size);
    this.crop.checked = square;
    this.grid.style.setProperty("--g-size", `${this.size}px`);
    this.grid.classList.toggle("fit", !square);
    this.retarget();
  }

  toggle(): void {
    if (this.visible) this.close();
    else this.show();
  }

  show(): void {
    const files = this.host.files();
    if (!files.length) return;
    if (this.built !== files) this.build(files);
    this.root.hidden = false;
    const at = this.host.position();
    this.cells.forEach((c, i) => c.classList.toggle("active", i === at));
    this.focusCell(at, "center");
  }

  close(): void {
    this.root.hidden = true;
  }

  private build(files: readonly string[]): void {
    this.built = files;
    this.bucket = thumbPx(this.size, devicePixelRatio);
    this.cells = files.map((path, i) => {
      const cell = document.createElement("button");
      cell.type = "button";
      cell.className = "g-cell";
      cell.dataset.i = String(i);
      cell.title = basename(path);
      cell.tabIndex = -1;
      const img = new Image();
      img.loading = "lazy";
      img.decoding = "async";
      img.draggable = false;
      img.alt = "";
      img.onload = () => img.classList.add("loaded");
      img.onerror = () => {
        img.onerror = null;
        img.src = srcFor(path); // formats the thumbnailer cannot decode fall back to the picture itself
      };
      img.src = thumbSrc(path, this.bucket);
      cell.append(img);
      return cell;
    });
    this.grid.replaceChildren(...this.cells);
  }

  /** Another size bucket needs other thumbnails; the lazy images fetch them only once they are on screen. */
  private retarget(): void {
    const bucket = thumbPx(this.size, devicePixelRatio);
    if (!this.built || bucket === this.bucket) return;
    this.bucket = bucket;
    this.cells.forEach((cell, i) => {
      const img = cell.firstElementChild as HTMLImageElement;
      img.classList.remove("loaded");
      img.src = thumbSrc(this.built![i], bucket);
    });
  }

  private resize(px: number, persist: boolean): void {
    this.configure(px, this.square);
    if (persist) this.host.save(this.size, this.square);
  }

  private reshape(square: boolean): void {
    this.configure(this.size, square);
  }

  private indexOf(t: EventTarget | null): number | null {
    const cell = (t as HTMLElement | null)?.closest<HTMLElement>(".g-cell");
    return cell ? Number(cell.dataset.i) : null;
  }

  private columns(): number {
    return Math.max(1, getComputedStyle(this.grid).gridTemplateColumns.split(" ").length);
  }

  private focusCell(index: number, block: ScrollLogicalPosition): void {
    const cell = this.cells[index];
    if (!cell) return;
    cell.focus({ preventScroll: true });
    cell.scrollIntoView({ block, inline: "nearest" });
  }

  private onKey(e: KeyboardEvent): void {
    if (e.target instanceof HTMLInputElement && e.key !== "Home" && e.key !== "End") {
      if (e.key === "Escape") return; // closes the gallery through the viewer
      if (e.target.type === "range" || e.key === " ") return; // the slider and the checkbox keep their own keys
    }
    if (e.key === "+" || e.key === "=" || e.key === "-" || e.key === "_") {
      this.resize(this.size + (e.key === "-" || e.key === "_" ? -GALLERY_STEP : GALLERY_STEP), true);
    } else if (KEYS.has(e.key) && !(e.target instanceof HTMLInputElement)) {
      const from = this.indexOf(document.activeElement) ?? this.host.position();
      const rows = Math.max(1, Math.floor(this.grid.clientHeight / (this.size + 8)));
      this.focusCell(moveIndex(from, e.key as GalleryKey, this.columns(), this.cells.length, rows), "nearest");
    } else return;
    e.preventDefault();
  }
}
