import type { LocalStatus } from "./tauri";
import { formatBytes } from "./tree";

/** What pressing "Set up the local model" will take, in the line under the button. */
export function whatItTakes(status: Pick<LocalStatus, "download_bytes" | "installs">): string {
  const size = status.download_bytes > 0 ? formatBytes(status.download_bytes) : null;
  if (status.installs && size) return `Installs ${status.installs} and downloads ${size} once, then works offline.`;
  if (status.installs) return `Installs ${status.installs}; the model is already here.`;
  if (size) return `Downloads ${size} once, then works offline.`;
  return "Nothing left to download; setting up checks what's here.";
}

/** "46 seconds", or "3 minutes" once it's a minute or more. */
export function duration(seconds: number): string {
  return seconds < 60 ? `${Math.round(seconds)} seconds` : `${Math.round(seconds / 60)} minutes`;
}

/**
 * What to clear before setting up, when the disk has less free than setting up wants (half again
 * what it puts there); `null` when there's room, or the system didn't say.
 */
export function spaceShort(status: Pick<LocalStatus, "free_bytes" | "wanted_bytes">): string | null {
  if (status.free_bytes === null || status.wanted_bytes === 0 || status.free_bytes >= status.wanted_bytes) return null;
  return `Clear some space first: setting it up wants ${formatBytes(status.wanted_bytes)} free, and this disk has ${formatBytes(status.free_bytes)}.`;
}
