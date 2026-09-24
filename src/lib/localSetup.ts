import type { LocalStatus } from "./tauri";
import { formatBytes } from "./tree";

/** What pressing "Set up this computer" will take, in the line under the button. */
export function whatItTakes(status: Pick<LocalStatus, "download_bytes" | "installs">): string {
  const size = status.download_bytes > 0 ? formatBytes(status.download_bytes) : null;
  if (status.installs && size) return `Installs ${status.installs} and downloads ${size} once, then works offline.`;
  if (status.installs) return `Installs ${status.installs}; the model is already here.`;
  if (size) return `Downloads ${size} once, then works offline.`;
  return "Nothing left to download; setting up checks what's here.";
}
