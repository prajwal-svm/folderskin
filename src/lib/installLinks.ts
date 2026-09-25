/**
 * folderskin://install links, as the page hears of them. The app keeps the pack a link asked for
 * until the page takes it (src-tauri/src/deep_link.rs): a link can come before the window has
 * loaded, or during the first-launch welcome, so the page asks for one as it starts and again
 * each time the app says another has come.
 */
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "./devMock";
import { api } from "./tauri";

/** What the app emits when a link is waiting (deep_link.rs `EVENT`). */
export const INSTALL_LINK_EVENT = "install-link";

/**
 * Hands `open` the pack of every folderskin://install link: the one waiting now, if any, and each
 * that comes after. Returns what stops listening. A pack already taken from the app is handed
 * over even when that happens after the stop, since nothing else would ever see it (React mounts
 * everything twice in development, so a stop can come a moment after the start).
 */
export function watchInstallLinks(open: (packId: string) => void): () => void {
  const take = () => {
    api
      .takeInstallLink()
      .then((packId) => {
        if (packId) open(packId);
      })
      .catch(() => {});
  };
  // The browser preview has no links: `?install=` stands in for one (devMock.ts).
  if (!isTauri()) {
    take();
    return () => {};
  }
  let stopped = false;
  let unlisten: (() => void) | null = null;
  listen(INSTALL_LINK_EVENT, take)
    .then((stop) => {
      if (stopped) {
        stop();
        return;
      }
      unlisten = stop;
      // Listening first, then asking: a link that comes in between is heard one way or the other,
      // and the app hands it to whichever asks first.
      take();
    })
    .catch(() => {
      if (!stopped) take();
    });
  return () => {
    stopped = true;
    unlisten?.();
  };
}
