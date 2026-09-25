/**
 * How often each community pack has been added, for the counts on folderskin.app's gallery.
 *
 *   POST    /v1/packs/<id>/installs   the app, once it has added a pack from Community. No body.
 *   GET     /v1/packs/installs        the website: {"version": 1, "installs": {"<id>": n}}.
 *   OPTIONS /v1/packs/installs        a preflight, which a plain GET never needs.
 *
 * An add counts once per network, pack and UTC day. The network is hashed the way quotas hash it
 * (src/ip.ts), under keys of its own that change every day, and the rows that remember it last
 * until the day is over: the daily clean-up deletes them (daily.ts). Nothing else about a request
 * is kept. Only packs in folderskin-community's published index count, so the numbers can't fill
 * up with made-up ids.
 *
 * Reading the counts needs nothing but D1: it works before any secret is set, and the website may
 * read it from a page (CORS for folderskin.app and www). Counting needs IP_SALT, and without it
 * answers 503 not_configured, as the rest of the service does.
 */
import { dayOf, now } from "./bytes";
import type { Env } from "./env";
import { fail, json, parseJson, readBody } from "./http";
import { networkHash } from "./ip";
import { isPackId } from "./text";

/** folderskin-community's list of published packs, as the app reads it. */
export const PUBLISHED_INDEX = "https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main/index.json";
/** The pages that may read the counts. */
export const WEBSITE_ORIGINS = ["https://folderskin.app", "https://www.folderskin.app"];
/** Where the counts are read. */
export const COUNTS_PATH = "/v1/packs/installs";
/** How long browsers and caches may keep the counts. */
const COUNTS_MAX_AGE = 300;
/** How long this isolate answers with the counts it read last: a flood of page views then reads D1 once a minute. */
const COUNTS_KEPT_SECONDS = 60;
/** How long a published index is used before it is fetched again, here and at Cloudflare's edge. */
const INDEX_FRESH_SECONDS = 300;
/** How long one still stands in when GitHub can't be reached for a newer one. */
const INDEX_STALE_SECONDS = 24 * 3600;
/** The largest index read: today's is a few KB. */
const MAX_INDEX_BYTES = 4 * 1024 * 1024;

/** What this isolate keeps between requests: the published pack ids, and the counts it last read. */
let published: { ids: Set<string>; at: number } | null = null;
let kept: { installs: Record<string, number>; at: number } | null = null;

/** Forgets what this isolate keeps, so each test starts from nothing. */
export function forgetKept(): void {
  published = null;
  kept = null;
}

/** POST /v1/packs/<id>/installs: one add of pack `id`, counted unless its network was counted for it today. */
export async function count(request: Request, env: Env, id: string, at = now()): Promise<Response> {
  const network = await networkHash(env, request, at, "installs");
  if (!isPackId(id)) throw fail(400, "bad_pack", "That isn't a pack's id.");
  const ids = await publishedIds(at);
  if (!ids) {
    throw fail(503, "index_unavailable", "The list of community packs can't be read right now. Please try again later.", {
      "Retry-After": "300",
    });
  }
  if (!ids.has(id)) throw fail(404, "unknown_pack", "No community pack has that id.");
  const seen = await env.DB.prepare("INSERT OR IGNORE INTO installs_seen (day, network, pack) VALUES (?1, ?2, ?3)")
    .bind(dayOf(at), network, id)
    .run();
  const counted = seen.meta.changes === 1;
  if (counted) {
    await env.DB.prepare("INSERT INTO installs (pack, n) VALUES (?1, 1) ON CONFLICT (pack) DO UPDATE SET n = n + 1").bind(id).run();
    kept = null;
  }
  return json({ counted });
}

/** GET /v1/packs/installs: every pack's count. */
export async function counts(request: Request, env: Env, at = now()): Promise<Response> {
  if (!kept || at - kept.at >= COUNTS_KEPT_SECONDS) {
    const { results } = await env.DB.prepare("SELECT pack, n FROM installs WHERE n > 0 ORDER BY pack").all<{ pack: string; n: number }>();
    const installs: Record<string, number> = {};
    for (const { pack, n } of results) installs[pack] = n;
    kept = { installs, at };
  }
  return json({ version: 1, installs: kept.installs }, 200, { ...cors(request), "Cache-Control": `public, max-age=${COUNTS_MAX_AGE}` });
}

/** OPTIONS /v1/packs/installs. */
export function preflight(request: Request): Response {
  return new Response(null, {
    status: 204,
    headers: { ...cors(request), "Access-Control-Allow-Methods": "GET, OPTIONS", "Access-Control-Max-Age": "86400" },
  });
}

/** The request's origin back when it is the website's; `Vary: Origin` either way, since the answer depends on it. */
function cors(request: Request): Record<string, string> {
  const origin = request.headers.get("Origin") ?? "";
  return WEBSITE_ORIGINS.includes(origin) ? { "Access-Control-Allow-Origin": origin, Vary: "Origin" } : { Vary: "Origin" };
}

/**
 * The ids of the packs published now: the index this isolate fetched in the last five minutes, or
 * a fresh one. When GitHub can't be reached, or answers with something that isn't the list, the
 * last one read stands in for a day; `null` when there is none that recent.
 */
async function publishedIds(at: number): Promise<Set<string> | null> {
  if (published && at - published.at < INDEX_FRESH_SECONDS) return published.ids;
  try {
    const response = await fetch(PUBLISHED_INDEX, { cf: { cacheTtl: INDEX_FRESH_SECONDS, cacheEverything: true } });
    if (!response.ok) throw new Error(`index.json answered ${response.status}`);
    const ids = packIds(parseJson(await readBody(response, MAX_INDEX_BYTES)));
    if (!ids) throw new Error("index.json isn't a list of packs");
    published = { ids, at };
    return ids;
  } catch {
    return published && at - published.at < INDEX_STALE_SECONDS ? published.ids : null;
  }
}

/** The pack ids an index.json lists, or `null` when it has no list of packs. */
function packIds(index: Record<string, unknown>): Set<string> | null {
  if (!Array.isArray(index.packs)) return null;
  const ids = new Set<string>();
  for (const entry of index.packs as unknown[]) {
    const id = entry && typeof entry === "object" ? (entry as { id?: unknown }).id : undefined;
    if (isPackId(id)) ids.add(id);
  }
  return ids;
}
