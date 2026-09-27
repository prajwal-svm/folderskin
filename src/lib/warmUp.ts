/**
 * What the app gets ready once the library is on show, so the first visit to each view waits for
 * nothing:
 *
 * - the code of the views loaded when they're first opened: the AI view, "Design your own" and
 *   the subfolder chooser (App.tsx and FolderStage.tsx load them lazily, so launch doesn't wait
 *   for them);
 * - every shape's picture, which the AI view's and the composer's pickers show
 *   (src/lib/shapeList.ts);
 * - the template "Design your own" opens on, drawn by the app and kept (`composer_warm_up`);
 * - Community's first search, and the preview strips of the packs it shows first;
 * - the icons the composer's icon picker offers.
 *
 * One step after another, each when the window has a moment, so the library never waits for any
 * of it. Every step is kept for the rest of the session; one that fails is simply done again when
 * its view opens, as it would have been without the warm-up.
 */
import { api } from "./tauri";
import { loadShapes } from "./shapeList";
import { community } from "./communityStore";
import type { FolderStyle } from "../composer/parts";

/** How many packs' preview strips are fetched ahead: the ones the view shows without scrolling. */
const PREVIEWS_AHEAD = 12;

let started = false;

/** A moment when the window isn't busy (WebKit has no `requestIdleCallback`). */
const breather = () =>
  new Promise<void>((resolve) => {
    const idle = (window as { requestIdleCallback?: (cb: () => void, opts: { timeout: number }) => number }).requestIdleCallback;
    if (idle) idle(() => resolve(), { timeout: 400 });
    else setTimeout(resolve, 60);
  });

/**
 * Starts getting everything ready, once per session: `views` loads the code of each lazily loaded
 * view, and `look` says which folder "Design your own" opens on, asked when its turn comes.
 */
export function warmUp(look: () => FolderStyle, views: (() => Promise<unknown>)[]): void {
  if (started) return;
  started = true;
  const steps: (() => Promise<unknown>)[] = [
    loadShapes,
    ...views,
    () => api.composerWarmUp(look()),
    async () => {
      await community.warm();
      const packs = community.get().shown?.packs ?? [];
      for (const pack of packs.slice(0, PREVIEWS_AHEAD)) {
        if (pack?.preview) new Image().src = pack.preview;
      }
    },
    () => import("../composer/icons/load").then((m) => m.loadPack(m.BUILTIN_PACK)),
  ];
  void (async () => {
    for (const step of steps) {
      await breather();
      await step().catch(() => {
        // Done again when its view opens.
      });
    }
  })();
}
