import type { Info } from "./api";
import type { Key } from "./i18n";

export type Translate = (key: Key) => string;

export interface InfoLine {
  label: string;
  value: string;
}

export interface InfoGroup {
  /** Stable id, also the i18n suffix of the heading. */
  id: string;
  title: string;
  lines: InfoLine[];
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[u]}`;
}

export function formatDimensions(w: number | null, h: number | null): string | null {
  if (!w || !h) return null;
  return `${w} × ${h} px (${((w * h) / 1e6).toFixed(1)} MP)`;
}

const gcd = (a: number, b: number): number => (b === 0 ? a : gcd(b, a % b));

/** `16:9` for the common shapes, otherwise the ratio to two decimals (`1.37:1`). */
export function formatAspect(w: number | null, h: number | null): string | null {
  if (!w || !h) return null;
  const d = gcd(w, h);
  const [a, b] = [w / d, h / d];
  if (a <= 32 && b <= 32) return `${a}:${b}`;
  return w >= h ? `${(w / h).toFixed(2)}:1` : `1:${(h / w).toFixed(2)}`;
}

/** Rotation values are phrased here; the EXIF library only has English wording for them. */
const ORIENTATION: Record<number, Key> = {
  3: "info.val.rot180",
  6: "info.val.rotCW",
  8: "info.val.rotCCW",
};

/** Enumerations whose numeric code has a translated wording; anything else keeps the backend's text. */
export function enumKey(key: string, code: number): Key | null {
  switch (key) {
    case "orientation":
      return ORIENTATION[code] ?? "info.val.mirrored";
    case "colorSpace":
      return code === 0xffff ? "info.val.uncalibrated" : null;
    case "program":
      return code >= 0 && code <= 8 ? (`info.val.program.${code}` as Key) : null;
    case "metering":
      return [0, 1, 2, 3, 4, 5, 6, 255].includes(code) ? (`info.val.metering.${code}` as Key) : null;
    case "whiteBalance":
      return code === 0 || code === 1 ? (`info.val.whiteBalance.${code}` as Key) : null;
    case "flash":
      return code === 0 || code === 1 ? (`info.val.flash.${code}` as Key) : null;
    default:
      return null;
  }
}

/** Row ids the backend emits; an id from a newer backend is shown as is rather than dropped. */
const LABELS: Record<string, Key> = {
  taken: "info.taken",
  resolution: "info.resolution",
  software: "info.software",
  camera: "info.camera",
  lens: "info.lens",
  exposure: "info.exposure",
  aperture: "info.aperture",
  iso: "info.iso",
  bias: "info.bias",
  focal: "info.focal",
  focal35: "info.focal35",
  artist: "info.artist",
  copyright: "info.copyright",
  description: "info.description",
  gps: "info.gps",
  altitude: "info.altitude",
  direction: "info.direction",
  orientation: "info.orientation",
  colorSpace: "info.colorSpace",
  program: "info.program",
  metering: "info.metering",
  whiteBalance: "info.whiteBalance",
  flash: "info.flash",
};

const GROUP_TITLES: Record<string, Key> = {
  file: "info.group.file",
  image: "info.group.image",
  camera: "info.group.camera",
  exposure: "info.group.exposure",
  authorship: "info.group.authorship",
  location: "info.group.location",
};

/** Groups the facts the file system gave with the EXIF rows; empty groups are left out. */
export function buildGroups(info: Info, tr: Translate, fmtDate: (ms: number) => string): InfoGroup[] {
  const groups = new Map<string, InfoLine[]>();
  const add = (group: string, label: string, value: string | null) => {
    if (value === null || value === "") return;
    groups.set(group, [...(groups.get(group) ?? []), { label, value }]);
  };

  add("file", tr("info.file"), info.name);
  add("file", tr("info.folder"), info.folder);
  add("file", tr("info.size"), formatBytes(info.size));
  add("file", tr("info.format"), info.format || null);
  add("file", tr("info.created"), info.created_ms === null ? null : fmtDate(info.created_ms));
  add("file", tr("info.modified"), info.modified_ms === null ? null : fmtDate(info.modified_ms));

  add("image", tr("info.dimensions"), formatDimensions(info.width, info.height));
  add("image", tr("info.aspect"), formatAspect(info.width, info.height));
  add("image", tr("info.color"), info.color);

  for (const r of info.rows) {
    const label = r.key in LABELS ? tr(LABELS[r.key]) : r.key;
    const wording = r.code === null ? null : enumKey(r.key, r.code);
    add(r.group, label, wording ? tr(wording) : r.value);
  }

  return Object.keys(GROUP_TITLES)
    .filter((id) => groups.has(id))
    .map((id) => ({ id, title: tr(GROUP_TITLES[id]), lines: groups.get(id)! }));
}
