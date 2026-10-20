import { convertFileSrc } from "@tauri-apps/api/core";

/** Bottom thumbnail strip; cells exist for every file but only visible ones load an image. */
export class Filmstrip {
  private cells: HTMLButtonElement[] = [];
  private observer: IntersectionObserver;
  private active = -1;

  constructor(
    private readonly root: HTMLElement,
    private readonly onSelect: (index: number) => void,
  ) {
    this.observer = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          const img = (e.target as HTMLElement).firstElementChild as HTMLImageElement;
          img.src = img.dataset.src ?? "";
          this.observer.unobserve(e.target);
        }
      },
      { root, rootMargin: "0px 400px" },
    );
    root.addEventListener("click", (e) => {
      const cell = (e.target as HTMLElement).closest<HTMLElement>(".cell");
      if (cell) this.onSelect(Number(cell.dataset.i));
    });
  }

  build(files: string[]): void {
    this.observer.disconnect();
    this.root.replaceChildren();
    const frag = document.createDocumentFragment();
    this.cells = files.map((file, i) => {
      const cell = document.createElement("button");
      cell.className = "cell";
      cell.dataset.i = String(i);
      cell.tabIndex = -1;
      const img = new Image();
      img.decoding = "async";
      img.draggable = false;
      img.alt = "";
      img.dataset.src = convertFileSrc(file);
      cell.append(img);
      frag.append(cell);
      return cell;
    });
    this.root.append(frag);
    this.cells.forEach((c) => this.observer.observe(c));
    this.active = -1;
  }

  setActive(index: number, smooth: boolean): void {
    this.cells[this.active]?.classList.remove("active");
    this.active = index;
    const cell = this.cells[index];
    if (!cell) return;
    cell.classList.add("active");
    const left = cell.offsetLeft - (this.root.clientWidth - cell.offsetWidth) / 2;
    this.root.scrollTo({ left, behavior: smooth ? "smooth" : "auto" });
  }
}
