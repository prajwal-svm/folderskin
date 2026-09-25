/**
 * folderskin-community's published index.json, as the app and the website read it: the packs that
 * are published, and `moved`, the ids renamed packs had before with the ids they have now. Install
 * counts only count published packs, under their new ids (installs.ts), and a pack approved here
 * never gets an id that either one already uses (store.ts).
 *
 * Each Worker isolate reads it at most every five minutes (and Cloudflare caches the fetch as
 * long), and goes on with the last copy for up to a day while GitHub can't be reached. Only this
 * one fixed address is ever fetched.
 */
import { now } from "./bytes";
import { parseJson, readBody } from "./http";
import { isPackId } from "./text";

/** folderskin-community's list of published packs, as the app reads it. */
export const PUBLISHED_INDEX = "https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main/index.json";
/** How long a published index is used before it is fetched again, here and at Cloudflare's edge. */
const FRESH_SECONDS = 300;
/** How long one still stands in when GitHub can't be reached for a newer one. */
const STALE_SECONDS = 24 * 3600;
/** The largest index read: today's is a few KB. */
const MAX_INDEX_BYTES = 4 * 1024 * 1024;
/** How long GitHub gets to answer, so an approval never hangs on it. */
const FETCH_TIMEOUT_MS = 5000;

export type Published = {
  /** The ids of the packs published now. */
  ids: Set<string>;
  /** Each renamed pack's old id, with the id it has now. */
  moved: Map<string, string>;
};

/** The index this isolate read last, and when. */
let kept: (Published & { at: number }) | null = null;

/** Forgets the index this isolate read, so each test starts from nothing. */
export function forgetPublished(): void {
  kept = null;
}

/**
 * The index as it is now: the one this isolate read in the last five minutes, or a fresh one.
 * When GitHub can't be reached, or answers with something that isn't the list, the last one read
 * stands in for a day; `null` when there is none that recent.
 */
export async function publishedIndex(at = now()): Promise<Published | null> {
  if (kept && at - kept.at < FRESH_SECONDS) return kept;
  try {
    const response = await fetch(PUBLISHED_INDEX, {
      cf: { cacheTtl: FRESH_SECONDS, cacheEverything: true },
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    });
    if (!response.ok) throw new Error(`index.json answered ${response.status}`);
    const index = readIndex(parseJson(await readBody(response, MAX_INDEX_BYTES)));
    if (!index) throw new Error("index.json isn't a list of packs");
    kept = { ...index, at };
    return kept;
  } catch {
    return kept && at - kept.at < STALE_SECONDS ? kept : null;
  }
}

/**
 * The packs an index.json lists and the renames it carries, or `null` when it has no list of
 * packs. Anything that isn't a pack id is passed over, and so is a rename to itself. `moved` is
 * optional: an index from before any pack was renamed has none.
 */
function readIndex(index: Record<string, unknown>): Published | null {
  if (!Array.isArray(index.packs)) return null;
  const ids = new Set<string>();
  for (const entry of index.packs as unknown[]) {
    const id = entry && typeof entry === "object" ? (entry as { id?: unknown }).id : undefined;
    if (isPackId(id)) ids.add(id);
  }
  // A Map rather than the object itself: "constructor" is a pack id, and an object has one of those.
  const moved = new Map<string, string>();
  const renames = index.moved;
  if (renames && typeof renames === "object" && !Array.isArray(renames)) {
    for (const [from, to] of Object.entries(renames)) {
      if (isPackId(from) && isPackId(to) && from !== to) moved.set(from, to);
    }
  }
  return { ids, moved };
}
