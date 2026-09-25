import { env } from "cloudflare:workers";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dayOf, now } from "../src/bytes";
import { tidy } from "../src/daily";
import type { Env } from "../src/env";
import { count, counts, COUNTS_PATH, forgetKept, WEBSITE_ORIGINS } from "../src/installs";
import { PUBLISHED_INDEX } from "../src/published";
import { networkHash } from "../src/ip";
import { BASE, call, errorOf, freshIp, testEnv } from "./helpers";

/** Stubs GitHub's copy of folderskin-community's index.json: these packs and renames, or that error status. */
function stubIndex(packs: string[] | number, moved?: unknown) {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    if (url !== PUBLISHED_INDEX) return new Response("unexpected", { status: 599 });
    if (typeof packs === "number") return new Response("GitHub is having a moment", { status: packs });
    return Response.json({
      version: 1,
      packs: [
        ...packs.map((id) => ({ id, name: id, author: "prajwal-svm", license: "CC0-1.0", tags: ["test"], count: 1 })),
        // An entry no pack id could be, which is passed over.
        { id: "../../escape", name: "No" },
      ],
      ...(moved === undefined ? {} : { moved }),
    });
  });
}

/** What the app sends once it has added pack `id`: its id in the address, and nothing else. */
const added = (id: string, ip = freshIp()) => new Request(`${BASE}/v1/packs/${id}/installs`, { method: "POST", headers: { "CF-Connecting-IP": ip } });

function countsRequest(origin?: string, method = "GET"): Request {
  return new Request(`${BASE}${COUNTS_PATH}`, { method, headers: { "CF-Connecting-IP": freshIp(), ...(origin ? { Origin: origin } : {}) } });
}

async function countsNow(overrides: Partial<Env> = {}): Promise<Record<string, number>> {
  const response = await call(countsRequest(), overrides);
  expect(response.status).toBe(200);
  return ((await response.json()) as { installs: Record<string, number> }).installs;
}

const counted = async (response: Response) => ((await response.json()) as { counted: boolean }).counted;

/** A burst limit that has run out, whoever asks. */
const spent = { limit: async () => ({ success: false }) } as unknown as RateLimit;

beforeEach(() => forgetKept());
afterEach(() => vi.restoreAllMocks());

describe("counting installs", () => {
  it("counts an add once a day for each network and pack, and the counts list it", async () => {
    stubIndex(["night-prints", "koi-ponds"]);
    const ip = freshIp();
    expect(await counted(await call(added("night-prints", ip)))).toBe(true);
    expect(await counted(await call(added("night-prints", ip)))).toBe(false);
    // Another address on the same network (quotas count per /24) is the same network.
    expect(await counted(await call(added("night-prints", ip.replace(/\.7$/, ".200"))))).toBe(false);
    expect(await counted(await call(added("night-prints")))).toBe(true);
    expect(await counted(await call(added("koi-ponds", ip)))).toBe(true);
    const counts = await countsNow();
    expect([counts["night-prints"], counts["koi-ponds"]]).toEqual([2, 1]);
  });

  it("keeps nothing of a request but that day's hash of its network, under keys of its own", async () => {
    stubIndex(["quiet-pack"]);
    const ip = freshIp();
    await call(added("quiet-pack", ip));
    const { results } = await env.DB.prepare("SELECT * FROM installs_seen WHERE pack = 'quiet-pack'").all<Record<string, string>>();
    expect(results).toHaveLength(1);
    expect(Object.keys(results[0]).sort()).toEqual(["day", "network", "pack"]);
    expect(results[0].day).toBe(dayOf(now()));
    expect(results[0].network).toMatch(/^[0-9a-f]{32}$/);
    expect(JSON.stringify(results)).not.toContain(ip.split(".").slice(0, 3).join("."));
    // Not the hash quotas and reports keep, so the two can't be matched.
    expect(results[0].network).not.toBe(await networkHash(testEnv(), added("quiet-pack", ip)));
    expect(results[0].network).toBe(await networkHash(testEnv(), added("quiet-pack", ip), now(), "installs"));
  });

  it("forgets yesterday's networks in the daily clean-up, and keeps the counts", async () => {
    stubIndex(["tidy-pack"]);
    const at = now();
    await count(added("tidy-pack"), testEnv(), "tidy-pack", at - 86400);
    await count(added("tidy-pack"), testEnv(), "tidy-pack", at);
    const days = async () =>
      (await env.DB.prepare("SELECT day FROM installs_seen WHERE pack = 'tidy-pack' ORDER BY day").all<{ day: string }>()).results.map((r) => r.day);
    expect(await days()).toEqual([dayOf(at - 86400), dayOf(at)]);
    await tidy(testEnv(), at);
    expect(await days()).toEqual([dayOf(at)]);
    expect((await countsNow())["tidy-pack"]).toBe(2);
  });

  it("counts only packs that are published, by an id that is one", async () => {
    stubIndex(["real-pack"]);
    const bad = await call(added("Not_A_Pack"));
    expect([bad.status, (await errorOf(bad)).code]).toEqual([400, "bad_pack"]);
    const dots = await call(added("..%2F..%2Fadmin"));
    expect([dots.status, (await errorOf(dots)).code]).toEqual([400, "bad_pack"]);
    const unknown = await call(added("made-up-pack"));
    expect([unknown.status, (await errorOf(unknown)).code]).toEqual([404, "unknown_pack"]);
    expect((await call(added("x".repeat(101)))).status).toBe(404);
    expect((await call(new Request(`${BASE}/v1/packs/real-pack/installs`))).status).toBe(405);
    expect("made-up-pack" in (await countsNow())).toBe(false);
  });

  it("reads the published list once for a while, and leans on it for a day when GitHub is down", async () => {
    const at = now();
    let fetches = stubIndex(["steady-pack"]);
    for (let i = 0; i < 3; i++) await count(added("steady-pack"), testEnv(), "steady-pack", at + i);
    expect(fetches).toHaveBeenCalledTimes(1);
    fetches.mockRestore();

    // Five minutes on, GitHub answers with an error: the list read before stands in.
    fetches = stubIndex(503);
    const later = await count(added("steady-pack"), testEnv(), "steady-pack", at + 301);
    expect(later.status).toBe(200);
    expect(fetches).toHaveBeenCalledTimes(1);
    // A day on, it's too old to go by.
    await expect(count(added("steady-pack"), testEnv(), "steady-pack", at + 86401)).rejects.toMatchObject({ status: 503, code: "index_unavailable" });

    // With nothing read before at all, it says so too.
    forgetKept();
    const down = await call(added("steady-pack"));
    expect([down.status, (await errorOf(down)).code]).toEqual([503, "index_unavailable"]);
  });

  it("needs IP_SALT to count, as the rest of the service does", async () => {
    const fetches = stubIndex(["salted-pack"]);
    const response = await call(added("salted-pack"), { IP_SALT: undefined });
    expect([response.status, (await errorOf(response)).code]).toEqual([503, "not_configured"]);
    const limited = await call(added("salted-pack"), { IP_SALT: undefined, BURST: spent });
    expect([limited.status, (await errorOf(limited)).code]).toEqual([503, "not_configured"]);
    expect(fetches).not.toHaveBeenCalled();
  });

  it("counts an add under a renamed pack's old id toward its new one, and an add under each as one", async () => {
    stubIndex(["classic-art-k7q2mx"], { "classic-art": "classic-art-k7q2mx" });
    const ip = freshIp();
    // FolderSkin 0.1.4 to 0.1.6 know the pack by the id it had when they added it.
    expect(await counted(await call(added("classic-art", ip)))).toBe(true);
    expect(await counted(await call(added("classic-art-k7q2mx", ip)))).toBe(false);
    expect(await counted(await call(added("classic-art-k7q2mx")))).toBe(true);
    const counts = await countsNow();
    expect(counts["classic-art-k7q2mx"]).toBe(2);
    expect("classic-art" in counts).toBe(false);
  });

  it("counts an old id only toward a pack that is published, and passes over renames it can't read", async () => {
    stubIndex(["kept-pack-abcdef"], {
      "gone-pack": "gone-pack-abcdef",
      constructor: "kept-pack-abcdef",
      "Not An Id": "kept-pack-abcdef",
      "odd-pack": 5,
      "kept-pack-abcdef": "kept-pack-abcdef",
    });
    expect((await errorOf(await call(added("gone-pack")))).code).toBe("unknown_pack");
    expect((await errorOf(await call(added("odd-pack")))).code).toBe("unknown_pack");
    // Every rename that is a pack id to a published pack counts, "constructor" included.
    expect(await counted(await call(added("constructor")))).toBe(true);
    expect(await counted(await call(added("kept-pack-abcdef")))).toBe(true);
    expect((await countsNow())["kept-pack-abcdef"]).toBe(2);

    // An index whose renames aren't a map still lists its packs.
    forgetKept();
    vi.restoreAllMocks();
    stubIndex(["listed-pack"], ["not", "a", "map"]);
    expect(await counted(await call(added("listed-pack")))).toBe(true);
  });

  it("is held to the burst limit", async () => {
    stubIndex(["busy-pack"]);
    const response = await call(added("busy-pack"), { BURST: spent });
    expect([response.status, (await errorOf(response)).code]).toEqual([429, "slow_down"]);
    expect("busy-pack" in (await countsNow())).toBe(false);
  });
});

describe("the counts", () => {
  it("read with D1 alone, before any secret is set, with or without a burst limit", async () => {
    const bare = { IP_SALT: undefined, TURNSTILE_SECRET: undefined, LINK_SECRET: undefined, NOTIFY_WEBHOOK_URL: undefined };
    for (const overrides of [bare, { ...bare, BURST: spent }]) {
      const response = await call(countsRequest(), overrides);
      expect(response.status).toBe(200);
      expect(await response.json()).toMatchObject({ version: 1, installs: {} });
    }
    // Once IP_SALT is set, the limit can tell networks apart, and holds.
    expect((await call(countsRequest(), { BURST: spent })).status).toBe(429);
  });

  it("are cached publicly for five minutes, and the website may read them from its pages", async () => {
    for (const origin of WEBSITE_ORIGINS) {
      const response = await call(countsRequest(origin));
      expect(response.headers.get("Access-Control-Allow-Origin")).toBe(origin);
      expect(response.headers.get("Cache-Control")).toBe("public, max-age=300");
      expect(response.headers.get("Vary")).toBe("Origin");
      expect(response.headers.get("Content-Type")).toBe("application/json; charset=utf-8");
    }
    for (const origin of ["https://evil.example", "http://folderskin.app", "https://folderskin.app.evil.example", undefined]) {
      const response = await call(countsRequest(origin));
      expect(response.status).toBe(200);
      expect(response.headers.get("Access-Control-Allow-Origin"), origin).toBeNull();
      expect(response.headers.get("Vary")).toBe("Origin");
    }
  });

  it("answer a preflight, though a plain GET needs none", async () => {
    const allowed = await call(countsRequest("https://www.folderskin.app", "OPTIONS"), { IP_SALT: undefined });
    expect(allowed.status).toBe(204);
    expect(allowed.headers.get("Access-Control-Allow-Origin")).toBe("https://www.folderskin.app");
    expect(allowed.headers.get("Access-Control-Allow-Methods")).toBe("GET, OPTIONS");
    const other = await call(countsRequest("https://evil.example", "OPTIONS"));
    expect(other.status).toBe(204);
    expect(other.headers.get("Access-Control-Allow-Origin")).toBeNull();
    expect((await call(countsRequest(undefined, "POST"))).status).toBe(405);
  });

  it("are read from D1 once a minute at most, and afresh after a new count", async () => {
    stubIndex(["fresh-pack"]);
    const at = now();
    const read = async (when: number) =>
      ((await (await counts(countsRequest(), testEnv(), when)).json()) as { installs: Record<string, number> }).installs["fresh-pack"];
    expect(await read(at)).toBeUndefined();
    await env.DB.prepare("INSERT INTO installs (pack, n) VALUES ('fresh-pack', 5)").run();
    expect(await read(at + 30)).toBeUndefined();
    expect(await read(at + 60)).toBe(5);
    await count(added("fresh-pack"), testEnv(), "fresh-pack", at + 61);
    expect(await read(at + 62)).toBe(6);
  });
});
