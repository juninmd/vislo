/** Formats the webview decodes itself. Keep in sync with NATIVE_EXTENSIONS in src-tauri/src/album.rs. */
export const NATIVE_EXTENSIONS = ["jpg", "jpeg", "jpe", "jfif", "jif", "jfi", "pjpeg", "pjp", "png", "apng", "gif", "webp", "bmp", "dib", "avif", "ico"] as const;

const native = new Set<string>(NATIVE_EXTENSIONS);
const JPEG = new Set(["jpg", "jpeg", "jpe", "jfif", "jif", "jfi", "pjpeg", "pjp"]);

export function extOf(path: string): string {
  const dot = path.lastIndexOf(".");
  const sep = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return dot > sep ? path.slice(dot + 1).toLowerCase() : "";
}

export const isNative = (path: string): boolean => native.has(extOf(path));
// Animated formats keep their own frames; a re-rendered still would freeze them.
const ANIMATED = new Set(["gif", "webp", "avif", "ico", "apng"]);
export const isStill = (path: string): boolean => !ANIMATED.has(extOf(path));
export const isJpeg = (path: string): boolean => JPEG.has(extOf(path));
