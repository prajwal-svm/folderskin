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
