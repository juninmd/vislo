/** The part of an element this needs, so the logic runs without a DOM in tests. */
export interface ClassHost {
  classList: { add(...names: string[]): void; remove(...names: string[]): void; contains(name: string): boolean };
}

const RELEASE_MS = 600;

/** After a native splash the page's dim and controls wait for it to hand over (see src/style.css `handoff`). */
export function beginHandoff(root: ClassHost, splash: boolean): void {
  if (splash) root.classList.add("handoff");
}

/**
 * The real window has just been shown. Transitions may run from now on, and after a native splash the page's
 * dim and controls are released so they fade in while the splash's dim fades out.
 */
export function windowShown(
  root: ClassHost = document.documentElement,
  later: (fn: () => void, ms: number) => unknown = setTimeout,
  paintedNext: (fn: () => void) => void = afterNextPaint,
  release: () => void = () => {},
): void {
  const afterSplash = root.classList.contains("handoff");
  if (afterSplash) root.classList.add("handoff-out");
  root.classList.remove("booting", "handoff");
  if (!afterSplash) return;
  // The splash starts fading its dim only once the page has a frame with its own dim starting, so the two never
  // overlap or leave a gap however long the page took to paint.
  paintedNext(release);
  later(() => root.classList.remove("handoff-out"), RELEASE_MS);
}

/** Runs `fn` after the browser has produced the next frame. */
function afterNextPaint(fn: () => void): void {
  requestAnimationFrame(() => requestAnimationFrame(fn));
}
