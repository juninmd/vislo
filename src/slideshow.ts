export interface SlideshowHost {
  displayMs(): number;
  fadeMs(): number;
  /** Starts the transition to the next picture; false when there is nowhere to go. */
  advance(fadeMs: number): boolean;
  changed(running: boolean): void;
}

/** Timer loop only: transitions, fullscreen and captions are the host's business. */
export class Slideshow {
  private timer = 0;
  running = false;

  constructor(private readonly host: SlideshowHost) {}

  start(): void {
    if (this.running) return;
    this.running = true;
    this.host.changed(true);
    this.schedule(this.host.displayMs());
  }

  stop(): void {
    if (!this.running) return;
    this.running = false;
    clearTimeout(this.timer);
    this.host.changed(false);
  }

  toggle(): void {
    if (this.running) this.stop();
    else this.start();
  }

  /** After a manual jump the picture gets its full display time again. */
  restart(): void {
    if (this.running) this.schedule(this.host.displayMs());
  }

  private schedule(ms: number): void {
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      if (!this.running) return;
      const fade = this.host.fadeMs();
      if (!this.host.advance(fade)) {
        this.stop();
        return;
      }
      this.schedule(this.host.displayMs() + fade);
    }, ms);
  }
}
