import { srcFor, thumbSrc } from "./api";
import { type Lane, lane2d, lane3d, PITCH, visibleRadius } from "./filmstrip-geometry";
import { WheelAccumulator } from "./nav";

const TAU_MS = 110; // smoothing time constant of the continuous position
const THUMB_PX = 192;

export interface StripEvents {
  select(index: number): void;
  step(delta: number): void;
  hover(index: number | null, rect?: DOMRect): void;
}

/**
 * Bottom thumbnail strip. Its position is a continuous float that eases toward the active
 * image (constant-time easing), only on-screen cells exist in the DOM, and cells are laid out in 3D.
 */
export class Filmstrip {
  private files: string[] = [];
  private cells = new Map<number, HTMLButtonElement>();
  private pos = 0;
  private target = 0;
  private raf = 0;
  private last = 0;
  private lane: (d: number) => Lane = lane3d;
  private wheel = new WheelAccumulator(100);

  constructor(
    private readonly root: HTMLElement,
    events: StripEvents,
  ) {
    root.addEventListener("pointerdown", (e) => {
      const i = this.indexOf(e.target);
      if (i !== null) events.select(i);
    });
    root.addEventListener("pointerover", (e) => {
      const i = this.indexOf(e.target);
      if (i !== null && i !== this.target) events.hover(i, this.cells.get(i)?.getBoundingClientRect());
      else events.hover(null);
    });
    root.addEventListener("pointerleave", () => events.hover(null));
    root.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const steps = this.wheel.push(e.deltaMode === 1 ? e.deltaY * 40 : e.deltaY || e.deltaX);
        if (steps) events.step(Math.sign(steps));
      },
      { passive: false },
    );
    new ResizeObserver(() => this.render()).observe(root);
  }

  private indexOf(t: EventTarget | null): number | null {
    const cell = (t as HTMLElement | null)?.closest<HTMLElement>(".cell");
    return cell ? Number(cell.dataset.i) : null;
  }

  build(files: string[], active: number): void {
    this.files = files;
    for (const c of this.cells.values()) c.remove();
    this.cells.clear();
    this.pos = this.target = active;
    this.render();
  }

  set3d(on: boolean): void {
    this.lane = on ? lane3d : lane2d;
    this.render();
  }

  setActive(index: number, animate: boolean): void {
    this.target = index;
    if (!animate) this.pos = index;
    this.render();
    if (this.pos !== this.target && !this.raf) {
      this.last = performance.now();
      this.raf = requestAnimationFrame((t) => this.tick(t));
    }
  }

  private tick(now: number): void {
    const dt = Math.min(50, Math.max(1, now - this.last));
    this.last = now;
    this.pos += (this.target - this.pos) * (1 - Math.exp(-dt / TAU_MS));
    if (Math.abs(this.target - this.pos) < 0.002) this.pos = this.target;
    this.render();
    this.raf = this.pos === this.target ? 0 : requestAnimationFrame((t) => this.tick(t));
  }

  private cell(i: number): HTMLButtonElement {
    let cell = this.cells.get(i);
    if (!cell) {
      const path = this.files[i];
      cell = document.createElement("button");
      cell.className = "cell";
      cell.dataset.i = String(i);
      cell.tabIndex = -1;
      const img = new Image();
      img.decoding = "async";
      img.draggable = false;
      img.alt = "";
      img.src = thumbSrc(path, THUMB_PX);
      img.onerror = () => {
        img.onerror = null;
        img.src = srcFor(path); // formats the thumbnailer cannot decode fall back to the picture itself
      };
      cell.append(img);
      this.root.append(cell);
      this.cells.set(i, cell);
    }
    return cell;
  }

  private render(): void {
    const width = this.root.clientWidth;
    if (!width || !this.files.length) return;
    const radius = visibleRadius(width, this.lane);
    const lo = Math.max(0, Math.floor(this.pos) - radius);
    const hi = Math.min(this.files.length - 1, Math.ceil(this.pos) + radius);
    for (const [i, cell] of this.cells) {
      if (i < lo || i > hi) {
        cell.remove();
        this.cells.delete(i);
      }
    }
    for (let i = lo; i <= hi; i++) {
      const d = i - this.pos;
      const l = this.lane(d);
      const cell = this.cell(i);
      cell.style.transform = `translate3d(${l.x - PITCH / 2 + 4}px, 0, 0) rotateY(${l.rotateY}deg) scale(${l.scale})`;
      cell.style.opacity = String(l.opacity);
      cell.style.zIndex = String(1000 - Math.round(Math.abs(d) * 10));
      cell.classList.toggle("active", i === this.target);
    }
  }
}
