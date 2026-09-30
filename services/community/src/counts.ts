/**
 * How often each skin is downloaded on its own, and each pack and skin viewed on folderskin.app,
 * beside the install counts (installs.ts), for the website's sorts: trending, most viewed and most
 * downloaded.
 *
 *   POST    /v1/skins/<sha256>/downloads   the app, once it has used one skin, and the website's
 *                                          Download button. No body; `?pack=<id>` when the skin
 *                                          came from a pack.
 *   POST    /v1/views                      the website, when a pack's or a skin's dialog opens:
 *                                          {"pack": "<id>"} or {"skin": "<sha256>"}.
 *   GET     /v1/stats                      the website: every count, all told and for the last
 *                                          seven days.
 *   OPTIONS each of the three              a preflight.
 *
 * Each counts once per network, target and UTC day, the way an install does: the network is hashed
 * under a key of its own for each kind and each day (src/ip.ts), and the rows that remember it are
 * deleted by the daily clean-up (daily.ts). Nothing else about a request is kept. Every count,
 * installs too, also goes into its day's row in `counts_daily`, which keeps 30 days and gives the
 * last seven days' sums.
 *
 * Only what is published counts, so the numbers can't fill up with made-up targets: a pack in
 * folderskin-community's index, under the id it has now (published.ts), and a skin whose thumbnail
 * is in the catalog's bucket (publishedSkin). A download of a pack's skin is the skin's alone and
 * never adds to the pack's installs; its `pack` is checked to be a pack's id, and nothing is kept
 * of it.
 *
 * Views come from folderskin.app's pages and nowhere else: one sent with any other Origin, or with
 * none, as the app would, is turned away. The website's pages may read every answer here (CORS for
 * folderskin.app and www, errors included). The stats need nothing but D1, like the install counts;
 * counting needs IP_SALT, and without it answers 503 not_configured.
 */
import { dayOf, now } from "./bytes";
import type { Env } from "./env";
import { fail, HttpError, json, parseJson, readBody } from "./http";
import { cors, forgetKept, WEBSITE_ORIGINS } from "./installs";
import { networkHash } from "./ip";
import { currentPackId } from "./published";
import { SHA256 } from "./store";
import { isPackId } from "./text";

/** What is counted, as `counts_daily` names it. Installs are counted in installs.ts. */
export type CountKind = "install" | "skin_download" | "pack_view" | "skin_view";

/** Where the stats are read. */
export const STATS_PATH = "/v1/stats";
/** How long browsers and caches may keep the stats, as they may the install counts. */
const STATS_MAX_AGE = 300;
/**
 * How long this isolate answers with the stats it read last. Longer than the install counts'
 * minute, since the stats read a row for every pack and skin ever counted and for each of the
 * week's days: a flood of page views then reads D1 once every five minutes.
 */
const STATS_KEPT_SECONDS = 300;
/** The days the stats' `_7d` numbers add up: today and the six before it. */
export const WEEK_DAYS = 7;
/** How long a day's counts are kept, for the week and for a look further back. */
export const DAILY_KEPT_DAYS = 30;

/** Where the catalog's tree sits in the PACKS bucket. */
const TREE = "v2/";
/** How long this isolate goes by a skin it found published without asking the bucket again. */
const SKIN_KNOWN_SECONDS = 86400;
/** The most skins an isolate remembers: as many as the official collection can hold. */
const MAX_KNOWN_SKINS = 20_000;
/** The largest view: `{"skin": "<64 hex>"}` is 76 bytes. */
const MAX_VIEW_BYTES = 1024;
const DAY = 86400;

type Stats = {
  version: 1;
  packs: Record<string, Record<string, number>>;
  skins: Record<string, Record<string, number>>;
};

/** The stats this isolate read last. */
let keptStats: { stats: Stats; at: number } | null = null;
/** The skins this isolate found published, and when. */
let knownSkins = new Map<string, number>();

/** Forgets what this isolate keeps, the install counts and the published index included, so each test starts from nothing. */
export function forgetCounts(): void {
  forgetKept();
  keptStats = null;
  knownSkins = new Map();
}

/**
 * POST /v1/skins/<sha256>/downloads: one download of skin `sha`, counted unless its network was
 * counted for it today.
 */
export async function download(request: Request, env: Env, sha: string, at = now()): Promise<Response> {
  return forWebsite(request, async () => {
    const network = await networkHash(env, request, at, "skin_download");
    const pack = new URL(request.url).searchParams.get("pack");
    if (pack !== null && !isPackId(pack)) throw fail(400, "bad_pack", "That isn't a pack's id.");
    const skin = await publishedSkin(env, sha, at);
    return json({ counted: await countOnce(env, network, "skin_download", skin, at) }, 200, cors(request));
  });
}

/** POST /v1/views: one view of a pack or a skin on folderskin.app, counted unless its network was counted for it today. */
export async function view(request: Request, env: Env, at = now()): Promise<Response> {
  return forWebsite(request, async () => {
    if (!WEBSITE_ORIGINS.includes(request.headers.get("Origin") ?? "")) {
      throw fail(403, "not_the_website", "Views are counted from folderskin.app's pages alone.");
    }
    const body = parseJson(await readBody(request, MAX_VIEW_BYTES));
    const isPack = Object.hasOwn(body, "pack");
    if (isPack === Object.hasOwn(body, "skin")) {
      throw fail(400, "bad_view", 'Say what was viewed: {"pack": "<id>"} or {"skin": "<sha256>"}, one of them.');
    }
    const kind = isPack ? "pack_view" : "skin_view";
    const network = await networkHash(env, request, at, kind);
    const target = isPack ? await currentPackId(body.pack, at) : await publishedSkin(env, body.skin, at);
    return json({ counted: await countOnce(env, network, kind, target, at) }, 200, cors(request));
  });
}

/**
 * GET /v1/stats: every pack's installs and views, and every skin's downloads and views, all told
 * and for the last seven days. A number that is 0 is left out, and so is a pack or a skin with
 * none at all.
 */
export async function stats(request: Request, env: Env, at = now()): Promise<Response> {
  if (!keptStats || at - keptStats.at >= STATS_KEPT_SECONDS) keptStats = { stats: await readStats(env, at), at };
  return json(keptStats.stats, 200, { ...cors(request), "Cache-Control": `public, max-age=${STATS_MAX_AGE}` });
}

/** OPTIONS for any of the three: the website's pages may send `methods`, with a Content-Type. */
export function preflight(request: Request, methods: string): Response {
  return new Response(null, {
    status: 204,
    headers: {
      ...cors(request),
      "Access-Control-Allow-Methods": methods,
      "Access-Control-Allow-Headers": "Content-Type",
      "Access-Control-Max-Age": "86400",
    },
  });
}

/**
 * Skin `sha`, when it is published: when its thumbnail is in the catalog's bucket, drawn as a
 * folder (a skin of a pack of folders, or of the official collection) or, failing that, as a
 * drive. Every published skin has one of the two, named after its SHA-256 alone, where its
 * picture's name needs its extension too, so one `head` answers for most skins and two for the
 * rest. The mirror uploads a skin's files before the head.json that lists it, and deletes none, so
 * every skin packs.folderskin.app has ever listed is there. A skin found is remembered for a day.
 */
async function publishedSkin(env: Env, sha: unknown, at: number): Promise<string> {
  if (typeof sha !== "string" || !SHA256.test(sha)) throw fail(400, "bad_skin", "That isn't a skin's SHA-256: 64 lower-case hex digits.");
  const found = knownSkins.get(sha);
  if (found !== undefined && at - found < SKIN_KNOWN_SECONDS) return sha;
  for (const folder of ["thumbs", "drive-thumbs"]) {
    if (await env.PACKS.head(`${TREE}${folder}/${sha}.webp`)) {
      if (knownSkins.size >= MAX_KNOWN_SKINS) knownSkins = new Map();
      knownSkins.set(sha, at);
      return sha;
    }
  }
  throw fail(404, "unknown_skin", "No published skin has that picture.");
}

/**
 * Counts one `kind` of `target` from `network`, unless that network was counted for it today: into
 * the total, and into the day's count. Whether it counted.
 */
async function countOnce(env: Env, network: string, kind: Exclude<CountKind, "install">, target: string, at: number): Promise<boolean> {
  const day = dayOf(at);
  const seen = await env.DB.prepare("INSERT OR IGNORE INTO counts_seen (day, kind, network, target) VALUES (?1, ?2, ?3, ?4)")
    .bind(day, kind, network, target)
    .run();
  if (seen.meta.changes !== 1) return false;
  await env.DB.batch([
    env.DB.prepare("INSERT INTO counts_total (kind, target, n) VALUES (?1, ?2, 1) ON CONFLICT (kind, target) DO UPDATE SET n = n + 1").bind(kind, target),
    env.DB.prepare(
      "INSERT INTO counts_daily (kind, target, day, n) VALUES (?1, ?2, ?3, 1) ON CONFLICT (day, kind, target) DO UPDATE SET n = n + 1",
    ).bind(kind, target, day),
  ]);
  return true;
}

/** Where each kind of count goes in the stats: under packs or skins, and the number's name. */
const FIELDS: Record<CountKind, [side: "packs" | "skins", name: string]> = {
  install: ["packs", "installs"],
  pack_view: ["packs", "views"],
  skin_download: ["skins", "downloads"],
  skin_view: ["skins", "views"],
};

/** Reads the stats from D1: every total, then the week's sums, in one round trip. */
async function readStats(env: Env, at: number): Promise<Stats> {
  type Row = { kind: CountKind; target: string; n: number };
  const [installs, totals, week] = await env.DB.batch<Row>([
    env.DB.prepare("SELECT 'install' AS kind, pack AS target, n FROM installs WHERE n > 0"),
    env.DB.prepare("SELECT kind, target, n FROM counts_total WHERE n > 0 ORDER BY kind"),
    env.DB.prepare(
      "SELECT kind, target, SUM(n) AS n FROM counts_daily WHERE day >= ?1 GROUP BY kind, target HAVING SUM(n) > 0 ORDER BY kind",
    ).bind(dayOf(at - (WEEK_DAYS - 1) * DAY)),
  ]);
  // Maps rather than objects while they fill: "constructor" is a pack id, and an object has one of those.
  const sides = { packs: new Map<string, Record<string, number>>(), skins: new Map<string, Record<string, number>>() };
  const add = (rows: Row[], suffix: string) => {
    for (const { kind, target, n } of rows) {
      const [side, name] = FIELDS[kind];
      const numbers = sides[side].get(target) ?? {};
      numbers[`${name}${suffix}`] = n;
      sides[side].set(target, numbers);
    }
  };
  add(installs.results, "");
  add(totals.results, "");
  add(week.results, "_7d");
  const sorted = (side: Map<string, Record<string, number>>) => Object.fromEntries([...side].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  return { version: 1, packs: sorted(sides.packs), skins: sorted(sides.skins) };
}

/** Runs `answer`, giving an error it throws the website's CORS headers too, so a page can read why. */
async function forWebsite(request: Request, answer: () => Promise<Response>): Promise<Response> {
  try {
    return await answer();
  } catch (e) {
    if (e instanceof HttpError) throw fail(e.status, e.code, e.message, { ...e.headers, ...cors(request) }, e.retryAfter);
    throw e;
  }
}
