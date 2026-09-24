/** "Finder", "Explorer" or "Files": what people call the file browser on their OS. */
export function fileBrowser(os: string): string {
  if (os === "macos") return "Finder";
  if (os === "windows") return "Explorer";
  return "Files";
}

/** "or click to browse your Mac" — the noun changes per OS. */
export function browseLabel(os: string): string {
  switch (os) {
    case "macos":
      return "your Mac";
    case "windows":
      return "your PC";
    default:
      return "your computer";
  }
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
