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

/** Left inset for the tab bar so it clears the macOS traffic lights. */
export function tabBarInset(os: string): number {
  return os === "macos" ? 136 : 24;
}
