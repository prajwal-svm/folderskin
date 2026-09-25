import { t } from "../i18n";

/** The three systems FolderSkin runs on, as the catalogs name them. */
export type Os = "macos" | "windows" | "linux";

/** The system `platform_info` named, as one of the three: anything that isn't macOS or Windows is Linux. */
export function osOf(os: string): Os {
  return os === "macos" || os === "windows" ? os : "linux";
}

/**
 * "your Mac", "your PC" or "your computer": where a folder is picked from, as a phrase sentences
 * put a `{{place}}` in ("Or click to pick one from your Mac.").
 */
export function browseLabel(os: string): string {
  return t(`common.place.${osOf(os)}`);
}

/**
 * The OS the app is drawn on, read from the web view itself, so shortcuts can be written the way
 * that OS writes them before platform_info has answered.
 */
export function localOs(): "macos" | "windows" | "linux" {
  const p = typeof navigator === "undefined" ? "" : `${navigator.platform} ${navigator.userAgent}`.toLowerCase();
  if (p.includes("mac")) return "macos";
  if (p.includes("win")) return "windows";
  return "linux";
}

/** A shortcut as this OS writes it: `keys("Z")` is ⌘Z on a Mac and Ctrl+Z elsewhere; `shift` adds ⇧ or Shift+. */
export function keys(key: string, opts: { shift?: boolean } = {}): string {
  if (localOs() === "macos") return `${opts.shift ? "⇧" : ""}⌘${key}`;
  return `Ctrl+${opts.shift ? "Shift+" : ""}${key}`;
}
