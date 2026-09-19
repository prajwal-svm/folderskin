import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { isTauri, mockFindUpdate } from "./devMock";

/**
 * Updates come from the newest GitHub release: its latest.json names the new version, its notes
 * and a download for each platform (tauri.conf.json, plugins.updater). The download has to be
 * signed with the key whose public half is built into the app, or it isn't installed.
 */

/** A newer FolderSkin than this one. */
export type AvailableUpdate = {
  version: string;
  /** What changed: the version's section of CHANGELOG.md, in Markdown. */
  notes: string;
  /**
   * Downloads it, checks its signature and installs it over this one. `onProgress` gets how much
   * has arrived, from 0 to 1, or null while the size isn't known. On Windows the installer closes
   * FolderSkin itself, so this may not return there.
   */
  install: (onProgress: (fraction: number | null) => void) => Promise<void>;
};

/** How long a check may take before it counts as failed. */
const CHECK_TIMEOUT_MS = 15_000;

/** The newest version if it's newer than this one, null if this is it. Throws when it can't tell. */
export async function findUpdate(): Promise<AvailableUpdate | null> {
  if (import.meta.env.DEV && !isTauri()) return mockFindUpdate();
  const update = await check({ timeout: CHECK_TIMEOUT_MS });
  if (!update) return null;
  return {
    version: update.version,
    notes: update.body ?? "",
    install: async (onProgress) => {
      let size = 0;
      let arrived = 0;
      await update.downloadAndInstall((e: DownloadEvent) => {
        if (e.event === "Started") {
          size = e.data.contentLength ?? 0;
          onProgress(size ? 0 : null);
        } else if (e.event === "Progress") {
          arrived += e.data.chunkLength;
          if (size) onProgress(Math.min(1, arrived / size));
        } else {
          onProgress(1);
        }
      });
    },
  };
}

/** Starts the installed version in place of this one. */
export async function restartApp(): Promise<void> {
  if (isTauri()) await relaunch();
  else location.replace(location.pathname);
}

/**
 * Release builds look for an update a few seconds after they open. `pnpm tauri dev` doesn't: its
 * version is the one being made. The browser preview does only with `?update` in the address.
 */
export function checksOnLaunch(): boolean {
  return isTauri() ? import.meta.env.PROD : new URLSearchParams(location.search).has("update");
}
