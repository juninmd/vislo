/** Formats the webview decodes itself. Keep in sync with NATIVE_EXTENSIONS in src-tauri/src/album.rs. */
export const NATIVE_EXTENSIONS = ["jpg", "jpeg", "jpe", "jfif", "jif", "jfi", "pjpeg", "pjp", "png", "apng", "gif", "webp", "bmp", "dib", "avif", "avifs", "ico", "cur", "svg"] as const;

const native = new Set<string>(NATIVE_EXTENSIONS);
const JPEG = new Set(["jpg", "jpeg", "jpe", "jfif", "jif", "jfi", "pjpeg", "pjp"]);

export function extOf(path: string): string {
  const dot = path.lastIndexOf(".");
  const sep = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return dot > sep ? path.slice(dot + 1).toLowerCase() : "";
}

export const isNative = (path: string): boolean => native.has(extOf(path));
// Native formats the backend has no decoder for (its image crate lacks AVIF and SVG): Copy draws them from the page.
const PAGE_COPY = new Set(["avif", "avifs", "svg"]);
export const isPageCopy = (path: string): boolean => PAGE_COPY.has(extOf(path));
// Animated formats keep their own frames (a re-rendered still would freeze them) and vectors are already
// rendered sharply at any size, so neither is handed to the sharp downscaler.
const ANIMATED = new Set(["gif", "webp", "avif", "avifs", "ico", "cur", "apng", "svg"]);
export const isStill = (path: string): boolean => !ANIMATED.has(extOf(path));
export const isJpeg = (path: string): boolean => JPEG.has(extOf(path));
export const isVector = (path: string): boolean => extOf(path) === "svg";
