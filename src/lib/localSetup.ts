import type { LocalStatus } from "./tauri";
import { t } from "../i18n";
import { formatBytes, formatDuration } from "../i18n/format";

/** What pressing "Set up the local model" will take, in the line under the button. */
export function whatItTakes(status: Pick<LocalStatus, "download_bytes" | "installs">): string {
  const size = status.download_bytes > 0 ? formatBytes(status.download_bytes) : null;
  if (status.installs && size) return t("ai.local.takes.installAndDownload", { installs: status.installs, size });
  if (status.installs) return t("ai.local.takes.install", { installs: status.installs });
  if (size) return t("ai.local.takes.download", { size });
  return t("ai.local.takes.nothing");
}

/** "46 seconds", or "3 minutes" once it's a minute or more. */
export const duration = formatDuration;

/**
 * What to clear before setting up, when the disk has less free than setting up wants (half again
 * what it puts there); `null` when there's room, or the system didn't say.
 */
export function spaceShort(status: Pick<LocalStatus, "free_bytes" | "wanted_bytes">): string | null {
  if (status.free_bytes === null || status.wanted_bytes === 0 || status.free_bytes >= status.wanted_bytes) return null;
  return t("ai.local.spaceShort", { wanted: formatBytes(status.wanted_bytes), free: formatBytes(status.free_bytes) });
}
