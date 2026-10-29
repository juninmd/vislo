// Pure helpers for the icon build (tested in ico.test.mjs).

/** Frames of the .ico; up to SMALL_MAX the simplified artwork is used. */
export const ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 256];
export const SMALL_MAX = 32;

/** PNG-in-ICO container: ICONDIR + one ICONDIRENTRY per frame, then the PNG payloads. */
export function packIco(frames) {
  const head = Buffer.alloc(6 + 16 * frames.length);
  head.writeUInt16LE(1, 2); // type: icon
  head.writeUInt16LE(frames.length, 4);
  let offset = head.length;
  frames.forEach(({ size, png }, i) => {
    const e = 6 + 16 * i;
    head[e] = size >= 256 ? 0 : size;
    head[e + 1] = size >= 256 ? 0 : size;
    head.writeUInt16LE(1, e + 4); // planes
    head.writeUInt16LE(32, e + 6); // bits per pixel
    head.writeUInt32LE(png.length, e + 8);
    head.writeUInt32LE(offset, e + 12);
    offset += png.length;
  });
  return Buffer.concat([head, ...frames.map((f) => f.png)]);
}
