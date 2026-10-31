import type { Dir } from "./nav";
import { APPEAR_MS, appearFrames, crossfadeFrames, SLIDE_MS, slideFrames } from "./slide-anim";

const EASE = "cubic-bezier(0.22, 0.8, 0.28, 1)";
const reduced = matchMedia("(prefers-reduced-motion: reduce)");

/** Slides the incoming slide in and the outgoing one out, in the direction of travel. */
export function slide(incoming: HTMLElement, outgoing: HTMLElement | null, dir: Dir, stageW: number): Animation[] {
  const ms = reduced.matches ? 0 : SLIDE_MS;
  const frames = slideFrames(dir, stageW);
  // Travel and opacity are separate animations: the travel keeps its easing, while the opacity ramps
  // stay piecewise linear exactly as `coverage` describes them.
  const move: KeyframeAnimationOptions = { duration: ms, easing: EASE, fill: "both" };
  const dim: KeyframeAnimationOptions = { duration: ms, easing: "linear", fill: "both" };
  const anims = [incoming.animate(frames.incomingX, move), incoming.animate(frames.incomingO, dim)];
  if (outgoing) {
    anims.push(outgoing.animate(frames.outgoingX, move), outgoing.animate(frames.outgoingO, dim));
  }
  return anims;
}

/**
 * A picture opening. Returned paused on its first frame so the caller can start it the moment the
 * window is shown; it is fully opaque from the start, so the first painted frame already shows it.
 */
export function appear(el: HTMLElement): Animation {
  const a = el.animate(appearFrames(), { duration: reduced.matches ? 0 : APPEAR_MS, easing: EASE, fill: "backwards" });
  a.pause();
  // Released when it lands: the picture is then rendered at its own size again instead of at the
  // scale it was animated from, which would leave it slightly resampled.
  void a.finished.then(() => a.cancel(), () => undefined);
  return a;
}

/** Rubber-band nudge when the user pushes past the first or last image. */
export function bump(el: HTMLElement, dir: Dir): Animation {
  const dx = dir * 18;
  return el.animate(
    [{ transform: "translateX(0)" }, { transform: `translateX(${dx}px)`, offset: 0.4 }, { transform: "translateX(0)" }],
    { duration: reduced.matches ? 0 : 260, easing: EASE },
  );
}

/**
 * Crossfade used by the slideshow (1 s by default). Only the incoming picture animates: the outgoing
 * one stays whole underneath, which keeps the stage covered and halves the compositor work.
 */
export function fade(incoming: HTMLElement, ms: number): Animation[] {
  const opts: KeyframeAnimationOptions = { duration: reduced.matches ? 0 : ms, easing: "ease-in-out", fill: "both" };
  return [incoming.animate(crossfadeFrames(), opts)];
}

/** Fades `el` out and removes it: a placeholder giving way to the real picture. */
export function dissolve(el: HTMLElement, ms = 160): void {
  const a = el.animate([{ opacity: 1 }, { opacity: 0 }], { duration: reduced.matches ? 0 : ms, easing: "ease-out", fill: "forwards" });
  const done = () => el.remove();
  a.finished.then(done, done);
}

/**
 * FLIP: after the stage resized, eases the picture from where it was (`dy` px higher or lower, `k` times
 * as large) to where it is now. Transform only, so nothing is laid out per frame.
 */
export function settle(el: HTMLElement, dy: number, k: number): Animation {
  return el.animate(
    [{ transform: `translateY(${dy}px) scale(${k})` }, { transform: "none" }],
    { duration: reduced.matches ? 0 : 220, easing: EASE },
  );
}
