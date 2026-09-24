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
 * The details behind the info button beside the machine: what the model runs with, how long a
 * picture took the last time one was painted on this machine (measured here, so another
 * machine says its own, once it has painted one), and where the files are kept. One per line.
 */
export function machineDetails(status: Pick<LocalStatus, "backend" | "seconds_per_image" | "home">): string {
  return [
    `Runs with ${status.backend}`,
    status.seconds_per_image === null
      ? "How long a picture takes shows here after the first one"
      : `The last picture here took about ${duration(status.seconds_per_image)}`,
    `Kept in ${status.home}`,
  ].join("\n");
}
