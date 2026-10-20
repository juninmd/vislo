/** Formats the webview decodes itself. Keep in sync with NATIVE_EXTENSIONS in src-tauri/src/album.rs. */
export const NATIVE_EXTENSIONS = ["jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "bmp", "avif", "ico"] as const;

const native = new Set<string>(NATIVE_EXTENSIONS);
const JPEG = new Set(["jpg", "jpeg", "jpe", "jfif"]);

export function extOf(path: string): string {
  const dot = path.lastIndexOf(".");
  const sep = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return dot > sep ? path.slice(dot + 1).toLowerCase() : "";
}

export const isNative = (path: string): boolean => native.has(extOf(path));
export const isJpeg = (path: string): boolean => JPEG.has(extOf(path));
