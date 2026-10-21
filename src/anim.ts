import type { Dir } from "./nav";

const DURATION = 240;
const EASE = "cubic-bezier(0.22, 0.8, 0.28, 1)";
const reduced = matchMedia("(prefers-reduced-motion: reduce)");

/** Travel is a fraction of the stage so it reads as a flick at any window size. */
const travel = (stageW: number) => Math.round(stageW * 0.09);

/** Slides the incoming slide in and the outgoing one out, in the direction of travel. */
export function slide(incoming: HTMLElement, outgoing: HTMLElement | null, dir: Dir, stageW: number): Animation[] {
  const ms = reduced.matches ? 0 : DURATION;
  const dx = dir * travel(stageW);
  const opts: KeyframeAnimationOptions = { duration: ms, easing: EASE, fill: "both" };
  const anims = [
    incoming.animate(
      [{ transform: `translateX(${dx}px)`, opacity: 0 }, { transform: "translateX(0)", opacity: 1 }],
      opts,
    ),
  ];
  if (outgoing) {
    anims.push(
      outgoing.animate(
        [{ transform: "translateX(0)", opacity: 1 }, { transform: `translateX(${-dx}px)`, opacity: 0 }],
        opts,
      ),
    );
  }
  return anims;
}

const APPEAR_MS = 320;

/**
 * A picture opening: it grows from slightly smaller while fading in, like Picasa's load effect.
 * Returned paused on its first frame so the caller can start it the moment the window is shown;
 * it starts fairly opaque so the first painted frame already shows the picture.
 */
export function appear(el: HTMLElement): Animation {
  const a = el.animate(
    [{ transform: "scale(0.94)", opacity: 0.4 }, { transform: "scale(1)", opacity: 1 }],
    { duration: reduced.matches ? 0 : APPEAR_MS, easing: EASE, fill: "backwards" },
  );
  a.pause();
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

/** Crossfade used by the slideshow (1 s by default). */
export function fade(incoming: HTMLElement, outgoing: HTMLElement | null, ms: number): Animation[] {
  const opts: KeyframeAnimationOptions = { duration: reduced.matches ? 0 : ms, easing: "ease-in-out", fill: "both" };
  const anims = [incoming.animate([{ opacity: 0 }, { opacity: 1 }], opts)];
  if (outgoing) anims.push(outgoing.animate([{ opacity: 1 }, { opacity: 0 }], opts));
  return anims;
}
