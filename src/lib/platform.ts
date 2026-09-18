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
