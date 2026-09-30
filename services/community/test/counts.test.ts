import { env } from "cloudflare:workers";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dayOf, now, sha256Hex } from "../src/bytes";
import { download, forgetCounts, stats, STATS_PATH } from "../src/counts";
import { tidy } from "../src/daily";
import { count, WEBSITE_ORIGINS } from "../src/installs";
import { networkHash } from "../src/ip";
import { PUBLISHED_INDEX } from "../src/published";
import { BASE, call, errorOf, freshIp, testEnv } from "./helpers";

const SITE = WEBSITE_ORIGINS[0];
const DAY = 86400;

/** Stubs GitHub's copy of folderskin-community's index.json: these packs and renames. */
function stubIndex(packs: string[], moved?: Record<string, string>) {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    if (url !== PUBLISHED_INDEX) return new Response("unexpected", { status: 599 });
    return Response.json({ version: 1, packs: packs.map((id) => ({ id, name: id })), ...(moved ? { moved } : {}) });
  });
}

/** A skin nobody else in these tests has. */
const newSha = () => sha256Hex(crypto.randomUUID());

/** A published skin: its thumbnail in the catalog's bucket, as a folder or as a drive. */
async function published(shape: "folder" | "drive" = "folder"): Promise<string> {
  const sha = await newSha();
  await env.PACKS.put(`v2/${shape === "folder" ? "thumbs" : "drive-thumbs"}/${sha}.webp`, new Uint8Array([1, 2, 3]));
  return sha;
}

/** What the app, or the website's Download button, sends once a skin is downloaded. */
const downloaded = (sha: string, { ip = freshIp(), query = "", origin }: { ip?: string; query?: string; origin?: string } = {}) =>
  new Request(`${BASE}/v1/skins/${sha}/downloads${query}`, {
    method: "POST",
    headers: { "CF-Connecting-IP": ip, ...(origin ? { Origin: origin } : {}) },
  });

/** What a page on folderskin.app sends when a pack's or a skin's dialog opens. */
const viewed = (body: unknown, { ip = freshIp(), origin = SITE as string | null, type = "application/json" } = {}) =>
  new Request(`${BASE}/v1/views`, {
    method: "POST",
    headers: { "CF-Connecting-IP": ip, "Content-Type": type, ...(origin ? { Origin: origin } : {}) },
    body: typeof body === "string" ? body : JSON.stringify(body),
  });

/** A browser's preflight for `path`, from a page at `origin`. */
const preflightFor = (path: string, origin: string) => new Request(`${BASE}${path}`, { method: "OPTIONS", headers: { Origin: origin, "CF-Connecting-IP": freshIp() } });

const added = (id: string, ip = freshIp()) => new Request(`${BASE}/v1/packs/${id}/installs`, { method: "POST", headers: { "CF-Connecting-IP": ip } });

const counted = async (response: Response) => {
  expect(response.status).toBe(200);
  return ((await response.json()) as { counted: boolean }).counted;
};

type Stats = { version: number; packs: Record<string, Record<string, number>>; skins: Record<string, Record<string, number>> };

const statsRequest = (origin?: string, method = "GET") =>
  new Request(`${BASE}${STATS_PATH}`, { method, headers: { "CF-Connecting-IP": freshIp(), ...(origin ? { Origin: origin } : {}) } });

async function statsNow(at = now()): Promise<Stats> {
  forgetCounts();
  const response = await stats(statsRequest(), testEnv(), at);
  expect(response.status).toBe(200);
  return (await response.json()) as Stats;
}

/** A burst limit that has run out, whoever asks. */
const spent = { limit: async () => ({ success: false }) } as unknown as RateLimit;

beforeEach(() => forgetCounts());
afterEach(() => vi.restoreAllMocks());

describe("counting downloads", () => {
  it("counts a download once a day for each network and skin, and never as an install", async () => {
    const fetches = stubIndex(["night-prints"]);
    const sha = await published();
    const ip = freshIp();
    expect(await counted(await call(downloaded(sha, { ip })))).toBe(true);
    expect(await counted(await call(downloaded(sha, { ip })))).toBe(false);
    // Another address on the same network is the same network, whichever pack it names.
    expect(await counted(await call(downloaded(sha, { ip: ip.replace(/\.7$/, ".200"), query: "?pack=night-prints" })))).toBe(false);
    expect(await counted(await call(downloaded(sha, { query: "?pack=night-prints" })))).toBe(true);
    expect((await statsNow()).skins[sha]).toEqual({ downloads: 2, downloads_7d: 2 });
    // The pack a skin came from is checked for its shape alone: the index isn't read, and its installs aren't touched.
    expect(fetches).not.toHaveBeenCalled();
    expect(await env.DB.prepare("SELECT n FROM installs WHERE pack = 'night-prints'").first()).toBeNull();
    expect((await statsNow()).packs["night-prints"]).toBeUndefined();
  });

  it("keeps nothing of a request but that day's hash of its network, under keys of its own", async () => {
    const sha = await published();
    const ip = freshIp();
    await call(downloaded(sha, { ip }));
    const { results } = await env.DB.prepare("SELECT * FROM counts_seen WHERE target = ?1").bind(sha).all<Record<string, string>>();
    expect(results).toHaveLength(1);
    expect(Object.keys(results[0]).sort()).toEqual(["day", "kind", "network", "target"]);
    expect(results[0]).toMatchObject({ day: dayOf(now()), kind: "skin_download" });
    expect(JSON.stringify(results)).not.toContain(ip.split(".").slice(0, 3).join("."));
    const request = downloaded(sha, { ip });
    expect(results[0].network).toBe(await networkHash(testEnv(), request, now(), "skin_download"));
    for (const other of ["network", "installs", "skin_view", "pack_view"]) {
      expect(results[0].network).not.toBe(await networkHash(testEnv(), request, now(), other));
    }
  });

  it("counts only a published skin, by a SHA-256 that is one", async () => {
    const drive = await published("drive");
    expect(await counted(await call(downloaded(drive, { query: "?pack=cosy-drives" })))).toBe(true);
    const sha = await published();
    const refusals: [Request, number, string][] = [
      [downloaded(sha.toUpperCase()), 400, "bad_skin"],
      [downloaded(sha.slice(1)), 400, "bad_skin"],
      [downloaded("..%2F..%2Fadmin"), 400, "bad_skin"],
      [downloaded(await newSha()), 404, "unknown_skin"],
      [downloaded(sha, { query: "?pack=Not_A_Pack" }), 400, "bad_pack"],
      [downloaded(sha, { query: "?pack=" }), 400, "bad_pack"],
    ];
    for (const [request, status, code] of refusals) {
      const response = await call(request);
      expect([response.status, (await errorOf(response)).code], request.url).toEqual([status, code]);
    }
    expect((await call(new Request(`${BASE}/v1/skins/${sha}/downloads`))).status).toBe(405);
    expect((await statsNow()).skins[sha]).toBeUndefined();
  });

  it("asks the bucket about a skin once a day, and a drive's thumbnail only when there is no folder's", async () => {
    const sha = await published();
    const drive = await published("drive");
    const asked: string[] = [];
    const bucket = { head: async (key: string) => (asked.push(key), env.PACKS.head(key)) } as unknown as R2Bucket;
    const at = now();
    for (let i = 0; i < 3; i++) await download(downloaded(sha), testEnv({ PACKS: bucket }), sha, at + i);
    await download(downloaded(drive), testEnv({ PACKS: bucket }), drive, at);
    expect(asked).toEqual([`v2/thumbs/${sha}.webp`, `v2/thumbs/${drive}.webp`, `v2/drive-thumbs/${drive}.webp`]);
    await download(downloaded(sha), testEnv({ PACKS: bucket }), sha, at + DAY);
    expect(asked).toHaveLength(4);
  });

  it("lets the website read the answer, and its errors, and answers a preflight", async () => {
    const sha = await published();
    const ok = await call(downloaded(sha, { origin: SITE }));
    expect(ok.headers.get("Access-Control-Allow-Origin")).toBe(SITE);
    expect(ok.headers.get("Vary")).toBe("Origin");
    const missing = await call(downloaded(await newSha(), { origin: SITE }));
    expect(missing.status).toBe(404);
    expect(missing.headers.get("Access-Control-Allow-Origin")).toBe(SITE);
    // The app sends no Origin, and a page elsewhere may count one too, but can't read the answer.
    expect(await counted(await call(downloaded(sha, { origin: "https://evil.example" })))).toBe(true);
    const preflight = await call(preflightFor(`/v1/skins/${sha}/downloads`, SITE));
    expect(preflight.status).toBe(204);
    expect(preflight.headers.get("Access-Control-Allow-Origin")).toBe(SITE);
    expect(preflight.headers.get("Access-Control-Allow-Methods")).toBe("POST, OPTIONS");
  });

  it("needs IP_SALT, and is held to the burst limit", async () => {
    const sha = await published();
    const unsalted = await call(downloaded(sha), { IP_SALT: undefined });
    expect([unsalted.status, (await errorOf(unsalted)).code]).toEqual([503, "not_configured"]);
    const limited = await call(downloaded(sha), { BURST: spent });
    expect([limited.status, (await errorOf(limited)).code]).toEqual([429, "slow_down"]);
    expect((await statsNow()).skins[sha]).toBeUndefined();
  });
});

describe("counting views", () => {
  it("counts a view of a pack or a skin once a day for each network, a renamed pack's under its new id", async () => {
    stubIndex(["koi-ponds-ab2cd3"], { "koi-ponds": "koi-ponds-ab2cd3" });
    const sha = await published();
    const ip = freshIp();
    expect(await counted(await call(viewed({ pack: "koi-ponds-ab2cd3" }, { ip })))).toBe(true);
    expect(await counted(await call(viewed({ pack: "koi-ponds" }, { ip })))).toBe(false);
    expect(await counted(await call(viewed({ pack: "koi-ponds" })))).toBe(true);
    expect(await counted(await call(viewed({ skin: sha }, { ip })))).toBe(true);
    expect(await counted(await call(viewed({ skin: sha }, { ip, origin: WEBSITE_ORIGINS[1] })))).toBe(false);
    // Sent as text, so the page needs no preflight, it counts all the same.
    expect(await counted(await call(viewed({ skin: sha }, { type: "text/plain;charset=UTF-8" })))).toBe(true);
    const counts = await statsNow();
    expect(counts.packs["koi-ponds-ab2cd3"]).toEqual({ views: 2, views_7d: 2 });
    expect(counts.packs["koi-ponds"]).toBeUndefined();
    expect(counts.skins[sha]).toEqual({ views: 2, views_7d: 2 });
  });

  it("keeps a pack's views and a skin's under keys of their own", async () => {
    stubIndex(["own-keys"]);
    const sha = await published();
    const ip = freshIp();
    await call(viewed({ pack: "own-keys" }, { ip }));
    await call(viewed({ skin: sha }, { ip }));
    const { results } = await env.DB.prepare("SELECT kind, network FROM counts_seen WHERE target IN ('own-keys', ?1) ORDER BY kind")
      .bind(sha)
      .all<{ kind: string; network: string }>();
    expect(results.map((r) => r.kind)).toEqual(["pack_view", "skin_view"]);
    expect(results[0].network).not.toBe(results[1].network);
  });

  it("are taken from folderskin.app's pages alone", async () => {
    stubIndex(["site-only"]);
    for (const origin of [null, "https://evil.example", "http://folderskin.app", "https://folderskin.app.evil.example", "null"]) {
      const response = await call(viewed({ pack: "site-only" }, { origin }));
      expect([response.status, (await errorOf(response)).code], String(origin)).toEqual([403, "not_the_website"]);
      expect(response.headers.get("Access-Control-Allow-Origin")).toBeNull();
    }
    expect((await statsNow()).packs["site-only"]).toBeUndefined();
  });

  it("name one published pack or one published skin, in a small JSON body", async () => {
    stubIndex(["real-pack"]);
    const sha = await published();
    const refusals: [Request, number, string][] = [
      [viewed({}), 400, "bad_view"],
      [viewed({ pack: "real-pack", skin: sha }), 400, "bad_view"],
      [viewed({ skins: sha }), 400, "bad_view"],
      [viewed({ pack: "Not_A_Pack" }), 400, "bad_pack"],
      [viewed({ pack: 5 }), 400, "bad_pack"],
      [viewed({ pack: "made-up-pack" }), 404, "unknown_pack"],
      [viewed({ skin: sha.toUpperCase() }), 400, "bad_skin"],
      [viewed({ skin: null }), 400, "bad_skin"],
      [viewed({ skin: await newSha() }), 404, "unknown_skin"],
      [viewed("not json"), 400, "bad_json"],
      [viewed(["pack", "real-pack"]), 400, "bad_json"],
      [viewed({ pack: "real-pack", padding: "x".repeat(2000) }), 413, "too_large"],
    ];
    for (const [request, status, code] of refusals) {
      const response = await call(request);
      const error = await errorOf(response);
      expect([response.status, error.code], error.message).toEqual([status, code]);
      // The page can read why.
      expect(response.headers.get("Access-Control-Allow-Origin")).toBe(SITE);
    }
    expect((await call(new Request(`${BASE}/v1/views`, { headers: { Origin: SITE } }))).status).toBe(405);
    const counts = await statsNow();
    expect([counts.packs["real-pack"], counts.skins[sha]]).toEqual([undefined, undefined]);
  });

  it("answer a preflight for a JSON body", async () => {
    const response = await call(preflightFor("/v1/views", SITE));
    expect(response.status).toBe(204);
    expect(response.headers.get("Access-Control-Allow-Origin")).toBe(SITE);
    expect(response.headers.get("Access-Control-Allow-Methods")).toBe("POST, OPTIONS");
    expect(response.headers.get("Access-Control-Allow-Headers")).toBe("Content-Type");
    const other = await call(preflightFor("/v1/views", "https://evil.example"));
    expect(other.headers.get("Access-Control-Allow-Origin")).toBeNull();
  });

  it("need IP_SALT", async () => {
    const fetches = stubIndex(["salted-view"]);
    const response = await call(viewed({ pack: "salted-view" }), { IP_SALT: undefined });
    expect([response.status, (await errorOf(response)).code]).toEqual([503, "not_configured"]);
    expect(fetches).not.toHaveBeenCalled();
  });
});

describe("the stats", () => {
  it("add up every count all told and over today and the six days before, and leave out every 0", async () => {
    const at = now();
    const [a, b] = [await newSha(), await newSha()];
    await env.DB.batch([
      env.DB.prepare("INSERT INTO installs (pack, n) VALUES ('week-pack', 40), ('quiet-pack', 0), ('constructor', 3)"),
      env.DB.prepare(
        "INSERT INTO counts_total (kind, target, n) VALUES ('pack_view', 'week-pack', 90), ('skin_download', ?1, 12), ('skin_view', ?1, 30), ('skin_view', ?2, 0)",
      ).bind(a, b),
      env.DB.prepare(
        `INSERT INTO counts_daily (kind, target, day, n) VALUES
         ('install', 'week-pack', ?1, 2), ('install', 'week-pack', ?2, 3), ('install', 'week-pack', ?3, 100),
         ('pack_view', 'week-pack', ?1, 7), ('skin_download', ?4, ?2, 4), ('skin_view', ?4, ?3, 9), ('skin_view', ?5, ?3, 1)`,
      ).bind(dayOf(at), dayOf(at - 6 * DAY), dayOf(at - 7 * DAY), a, b),
    ]);
    const response = await stats(statsRequest(), testEnv(), at);
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=300");
    const body = (await response.json()) as Stats;
    expect(body.version).toBe(1);
    expect(body.packs["week-pack"]).toEqual({ installs: 40, views: 90, installs_7d: 5, views_7d: 7 });
    expect(Object.keys(body.packs["week-pack"])).toEqual(["installs", "views", "installs_7d", "views_7d"]);
    expect(body.packs["quiet-pack"]).toBeUndefined();
    expect(Object.hasOwn(body.packs, "constructor") && body.packs["constructor"]).toEqual({ installs: 3 });
    expect(body.skins[a]).toEqual({ downloads: 12, views: 30, downloads_7d: 4 });
    // A week-old view alone leaves nothing to say about a skin.
    expect(body.skins[b]).toBeUndefined();
    const ids = Object.keys(body.packs);
    expect(ids).toEqual([...ids].sort());
  });

  it("count the installs the app sends into the week too", async () => {
    stubIndex(["weekly-pack"]);
    await call(added("weekly-pack"));
    await call(added("weekly-pack"));
    expect((await statsNow()).packs["weekly-pack"]).toEqual({ installs: 2, installs_7d: 2 });
  });

  it("read with D1 alone, before any secret is set, and the website may read them from its pages", async () => {
    const bare = { IP_SALT: undefined, TURNSTILE_SECRET: undefined, LINK_SECRET: undefined, NOTIFY_WEBHOOK_URL: undefined };
    for (const overrides of [bare, { ...bare, BURST: spent }]) {
      const response = await call(statsRequest(SITE), overrides);
      expect(response.status).toBe(200);
      expect(await response.json()).toMatchObject({ version: 1, packs: {}, skins: {} });
      expect(response.headers.get("Access-Control-Allow-Origin")).toBe(SITE);
      expect(response.headers.get("Vary")).toBe("Origin");
    }
    expect((await call(statsRequest(SITE), { BURST: spent })).status).toBe(429);
    expect((await call(statsRequest("https://evil.example"))).headers.get("Access-Control-Allow-Origin")).toBeNull();
    const preflight = await call(statsRequest(SITE, "OPTIONS"), { IP_SALT: undefined });
    expect([preflight.status, preflight.headers.get("Access-Control-Allow-Methods")]).toEqual([204, "GET, OPTIONS"]);
    expect((await call(statsRequest(SITE, "POST"))).status).toBe(405);
  });

  it("are read from D1 once every five minutes at most, however much is counted meanwhile", async () => {
    const sha = await published();
    const at = now();
    const read = async (when: number) => ((await (await stats(statsRequest(), testEnv(), when)).json()) as Stats).skins[sha];
    expect(await read(at)).toBeUndefined();
    await download(downloaded(sha), testEnv(), sha, at + 1);
    expect(await read(at + 299)).toBeUndefined();
    expect(await read(at + 300)).toEqual({ downloads: 1, downloads_7d: 1 });
  });
});

describe("the daily clean-up", () => {
  it("forgets yesterday's networks and each day's counts after 30 days, and keeps the totals", async () => {
    const sha = await published();
    const at = now();
    for (const days of [31, 30, 1, 0]) await download(downloaded(sha), testEnv(), sha, at - days * DAY);
    const rows = async (table: string) =>
      (await env.DB.prepare(`SELECT day FROM ${table} WHERE target = ?1 ORDER BY day`).bind(sha).all<{ day: string }>()).results.map((r) => r.day);
    expect(await rows("counts_daily")).toHaveLength(4);
    await tidy(testEnv(), at);
    expect(await rows("counts_seen")).toEqual([dayOf(at)]);
    expect(await rows("counts_daily")).toEqual([dayOf(at - 30 * DAY), dayOf(at - DAY), dayOf(at)]);
    expect((await statsNow(at)).skins[sha]).toEqual({ downloads: 4, downloads_7d: 2 });
  });
});

describe("counting an install", () => {
  it("still answers as it did, and the install counts are what they were", async () => {
    stubIndex(["same-as-ever"]);
    const response = await count(added("same-as-ever"), testEnv(), "same-as-ever");
    expect(await response.json()).toEqual({ counted: true });
    const installs = await call(new Request(`${BASE}/v1/packs/installs`, { headers: { "CF-Connecting-IP": freshIp() } }));
    expect(((await installs.json()) as { installs: Record<string, number> }).installs["same-as-ever"]).toBe(1);
  });
});
