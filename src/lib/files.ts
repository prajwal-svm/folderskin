import type { DragInfo } from "../state/dropzone";

/** Picture formats FolderSkin can read (HEIC goes through the system converter on macOS). */
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "heic", "heif"];

/** Last path component, without a trailing separator. */
export function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  return trimmed.split(/[\\/]/).pop() || trimmed;
}

export function isImagePath(path: string): boolean {
  const name = baseName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 && IMAGE_EXTENSIONS.includes(name.slice(dot + 1).toLowerCase());
}

/**
 * What a drag is carrying, judged from its first path before anything lands. Only a guess:
 * the drop handler still asks the backend what the path really is.
 */
export function dragInfoFor(paths: string[]): DragInfo | null {
  const first = paths[0];
  if (!first) return null;
  return { kind: isImagePath(first) ? "image" : "folder", name: baseName(first) };
}

/** A path for display: the home folder shown as ~ (macOS and Linux). */
export function prettyPath(path: string): string {
  return path.replace(/^\/(?:Users|home)\/[^/]+(?=\/|$)/, "~");
}
