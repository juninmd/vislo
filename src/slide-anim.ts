/**
 * Timing and keyframes of the picture transitions. Kept free of the DOM so the numbers can be
 * unit-tested; `anim.ts` only plays them.
 */

/** One step of travel: long enough to read as motion, short enough to feel instant. */
export const SLIDE_MS = 240;
/** How long a picture takes to settle when it opens. */
export const APPEAR_MS = 300;
/** Share of a step at which the incoming picture is fully opaque; the outgoing one only goes after that. */
export const SOLID_AT = 0.55;
/** Travel as a fraction of the stage, so a step reads the same at any window size. */
export const TRAVEL = 0.09;

/** Pixels a picture travels during one step (positive travels to the right). */
export const travel = (stageW: number): number => Math.round(stageW * TRAVEL);

/** Opacity of the incoming picture at progress `t`: full by `solidAt`, then held. */
export const incomingAlpha = (t: number, solidAt = SOLID_AT): number => Math.min(1, t / solidAt);

/** Opacity of the outgoing picture: it stays whole until the incoming one is solid, then goes. */
export const outgoingAlpha = (t: number, solidAt = SOLID_AT): number =>
  t <= solidAt ? 1 : Math.max(0, (1 - t) / (1 - solidAt));

/**
 * How much of the stage the two pictures cover together at progress `t`. Anything below 1 means the
 * background shows through, which reads as a dimming pulse halfway through the step. Keep this at 1.
 */
export const coverage = (t: number, solidAt = SOLID_AT): number => {
  const a = incomingAlpha(t, solidAt);
  return a + (1 - a) * outgoingAlpha(t, solidAt);
};

export interface SlideFrames {
  /** Travel of each picture; both keep moving to the end. */
  incomingX: Keyframe[];
  outgoingX: Keyframe[];
  /** Opacities sampled from `incomingAlpha` / `outgoingAlpha`, so the two never dim together. */
  incomingO: Keyframe[];
  outgoingO: Keyframe[];
}

/** Keyframes of one step, sampled exactly where the two opacity ramps change slope. */
export function slideFrames(dir: 1 | -1, stageW: number, solidAt = SOLID_AT): SlideFrames {
  const dx = dir * travel(stageW);
  return {
    incomingX: [{ transform: `translateX(${dx}px)` }, { transform: "translateX(0)" }],
    outgoingX: [{ transform: "translateX(0)" }, { transform: `translateX(${-dx}px)` }],
    incomingO: [
      { opacity: 0, offset: 0 },
      { opacity: 1, offset: solidAt },
      { opacity: 1, offset: 1 },
    ],
    outgoingO: [
      { opacity: 1, offset: 0 },
      { opacity: 1, offset: solidAt },
      { opacity: 0, offset: 1 },
    ],
  };
}

/** A crossfade only ever raises the incoming picture: the outgoing one stays whole underneath it. */
export const crossfadeFrames = (): Keyframe[] => [{ opacity: 0 }, { opacity: 1 }];

/** An opening picture settles from slightly smaller and is never dimmed, so it shows on the first frame. */
export const appearFrames = (): Keyframe[] => [{ transform: "scale(0.97)" }, { transform: "scale(1)" }];
