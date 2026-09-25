import { env } from "cloudflare:workers";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { b64url, now } from "../src/bytes";
import { digest, tidy } from "../src/daily";
import { fail } from "../src/http";
import { networkHash, penaltyNetwork } from "../src/ip";
import { makeLink } from "../src/links";
import { inWords, refused, requireSharer, requireVerifiable, type Sharer } from "../src/penalties";
import {
  BASE,
  call,
  countingDb,
  describe as describePack,
  device,
  errorOf,
  freshIp,
  pictures,
  postJson,
  signed,
  stubTurnstile,
  submit,
  testEnv,
  verify,
  verifyLink,
  type Device,
} from "./helpers";

afterEach(() => vi.restoreAllMocks());

const DAY = 86400;

let maintainer: Device;
beforeAll(async () => {
  maintainer = await device();
});
const asAdmin = () => ({ ADMIN_KEYS: maintainer.key });

async function admin(method: string, path: string, body?: Record<string, unknown>) {
  return call(await signed(maintainer, method, path, body), asAdmin());
}

async function author(handle: string, ip = freshIp()): Promise<Device> {
  const who = await device(ip);
  await verify(who, handle);
  return who;
}

/** Another address on the same /24, which penalties count as the same network. */
const sameNetwork = (ip: string, last: number) => ip.replace(/\.\d+$/, `.${last}`);

/** The id penalties and the queue know a network by. */
const networkOf = (ip: string) => penaltyNetwork(testEnv(), new Request(BASE, { headers: { "CF-Connecting-IP": ip } }));

async function penalty(subject: string) {
  return env.DB.prepare("SELECT strikes, struck_at, cool_until, banned_until, ban_reason FROM penalties WHERE subject = ?1").bind(subject).first<{
    strikes: number;
    struck_at: number;
    cool_until: number;
    banned_until: number | null;
    ban_reason: string;
  }>();
}

/** Signed a few seconds on, so the same request can be sent again without being a replay. */
const later = (seconds: number) => ({ ts: now() + seconds });

async function share(who: Device, salt: number, seconds = 0) {
  return call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, salt)), later(seconds)));
}

async function reject(id: string, body: Record<string, unknown>) {
  const response = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "reject", ...body });
  expect(response.status).toBe(200);
  return ((await response.json()) as { bans: Record<string, unknown>[] }).bans;
}

/** Tries to verify a new computer on `ip`, and says whether Cloudflare was asked. */
async function verifyNew(ip: string, handle: string) {
  const turnstile = stubTurnstile();
  const who = await device(ip);
  const link = await verifyLink(who, handle);
  const response = await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, ip));
  const asked = turnstile.mock.calls.length > 0;
  turnstile.mockRestore();
  return { response, asked };
}

describe("backing off", () => {
  it("counts a refused request as a strike, and doubles the wait with each request while cooling down", async () => {
    const who = await author("eager-sharer");
    await submit(who, pictures(1, 100));
    // On probation a computer has one pack waiting at a time, so the second is refused: a strike.
    const second = await share(who, 101, 1);
    expect((await errorOf(second)).code).toBe("waiting");
    const third = await share(who, 101, 2);
    expect(third.status).toBe(429);
    expect(third.headers.get("Retry-After")).toBe("120");
    expect(await errorOf(third)).toEqual({ code: "cooling_down", message: "That's too many tries in a row. You can share again in 2 minutes.", retry_after: 120 });
    const fourth = await share(who, 101, 3);
    expect(fourth.headers.get("Retry-After")).toBe("240");
    expect((await errorOf(fourth)).message).toBe("That's too many tries in a row. You can share again in 4 minutes.");
    // The network behind it is struck as often.
    expect((await penalty(`net:${await networkOf(who.ip)}`))?.strikes).toBe(3);
  });

  it("holds back every request that sends a pack, and nothing else", async () => {
    const who = await author("mid-upload");
    const list = pictures(1, 102);
    const created = await call(await signed(who, "POST", "/v1/submissions", await describePack(list)));
    const { submission_id: id, need } = (await created.json()) as { submission_id: string; need: string[] };
    await env.DB.prepare("INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES (?1, 1, ?2, ?2 + 600)").bind(`key:${who.key}`, now()).run();

    for (const [method, path, body] of [
      ["PUT", `/v1/submissions/${id}/items/${need[0]}`, list[0].bytes],
      ["PUT", `/v1/submissions/${id}/sheets/0`, list[0].bytes],
      ["POST", `/v1/submissions/${id}/finalize`, {}],
      ["POST", "/v1/submissions", await describePack(pictures(1, 103))],
    ] as const) {
      const response = await call(await signed(who, method, path, body));
      expect(response.status, path).toBe(429);
      const error = await errorOf(response);
      expect(error.code, path).toBe("cooling_down");
      expect(error.retry_after, path).toBeGreaterThan(590);
    }
    expect((await call(await signed(who, "GET", "/v1/submissions"))).status).toBe(200);
    expect((await call(await signed(who, "GET", "/v1/me"))).status).toBe(200);
    expect((await call(await signed(who, "POST", "/v1/me", { handle: "mid-upload-2" }))).status).toBe(200);
    expect((await call(await signed(who, "DELETE", `/v1/packs/${id}`))).status).toBe(200);
  });

  it("holds back everyone on a network that is cooling down", async () => {
    const ip = freshIp();
    const first = await author("net-first", ip);
    const second = await author("net-second", sameNetwork(ip, 9));
    await env.DB.prepare("INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES (?1, 1, ?2, ?2 + 60)")
      .bind(`net:${await networkOf(ip)}`, now())
      .run();
    expect((await errorOf(await share(second, 104))).code).toBe("cooling_down");
    // Which was a strike on the second computer's key too.
    expect((await penalty(`key:${second.key}`))?.strikes).toBe(1);
    expect(await penalty(`key:${first.key}`)).toBeNull();
  });

  it("waits at most a day, and starts over after a day without a strike", async () => {
    const sharer = { key: "doubling-key", network: "doubling-network", handle: "x", tier: "active", approved: 0, rejected: 0 } as Sharer;
    const at = 1_800_000_000;
    const waits: number[] = [];
    for (let i = 0; i < 13; i++) {
      await refused(testEnv(), sharer, fail(429, "quota", "Spent."), at);
      waits.push((await penalty("key:doubling-key"))!.cool_until - at);
    }
    expect(waits).toEqual([60, 120, 240, 480, 960, 1920, 3840, 7680, 15360, 30720, 61440, DAY, DAY]);
    // The thirteenth couldn't make the wait any longer, so it wasn't even written.
    expect((await penalty("key:doubling-key"))?.strikes).toBe(12);
    // A minute on, one is again: the day runs from it.
    await refused(testEnv(), sharer, fail(429, "quota", "Spent."), at + 60);
    expect(await penalty("key:doubling-key")).toMatchObject({ strikes: 13, cool_until: at + 60 + DAY });
    // A day after the last strike, the next is the first.
    await refused(testEnv(), sharer, fail(429, "quota", "Spent."), at + 60 + DAY);
    expect(await penalty("key:doubling-key")).toMatchObject({ strikes: 1, cool_until: at + 60 + DAY + 60 });
    expect(await penalty("net:doubling-network")).toMatchObject({ strikes: 1 });
  });

  it("strikes the network, not the key, when the burst limit turns a sharing request away, once while it cools down", async () => {
    const who = await author("bursty-one");
    const spent = { BURST: { limit: async () => ({ success: false }) } as unknown as RateLimit };
    const net = `net:${await networkOf(who.ip)}`;
    for (let i = 0; i < 3; i++) {
      const response = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 105)), later(i)), spent);
      expect((await errorOf(response)).code).toBe("slow_down");
    }
    expect(await penalty(net)).toMatchObject({ strikes: 1 });
    // The key the request named can't be trusted before its signature is checked.
    expect(await penalty(`key:${who.key}`)).toBeNull();
    // A request that doesn't send a pack is no strike.
    const reader = await author("bursty-reader");
    expect((await call(await signed(reader, "GET", "/v1/me"), spent)).status).toBe(429);
    expect(await penalty(`net:${await networkOf(reader.ip)}`)).toBeNull();
    // Past the burst, the network's wait holds, and sending during it is a strike on the key too.
    expect((await errorOf(await share(who, 105, 5))).code).toBe("cooling_down");
    expect(await penalty(net)).toMatchObject({ strikes: 2 });
    expect(await penalty(`key:${who.key}`)).toMatchObject({ strikes: 1 });
  });

  it("says how long to wait when the burst leaves a network cooling down for longer than a minute", async () => {
    const who = await author("bursty-again");
    const spent = { BURST: { limit: async () => ({ success: false }) } as unknown as RateLimit };
    const net = `net:${await networkOf(who.ip)}`;
    const burst = async (seconds: number) => call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 107)), later(seconds)), spent);
    // Struck once already today, and that wait is over: this burst is the second strike, 120 s.
    await env.DB.prepare("INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES (?1, 1, ?2 - 100, ?2 - 40)").bind(net, now()).run();
    const second = await burst(0);
    expect(second.status).toBe(429);
    expect(second.headers.get("Retry-After")).toBe("120");
    expect(await errorOf(second)).toMatchObject({ code: "cooling_down", retry_after: 120 });
    // Already cooling down for an hour: the answer says so, and nothing more is written.
    await env.DB.prepare("UPDATE penalties SET cool_until = ?2 + 3600 WHERE subject = ?1").bind(net, now()).run();
    const hourLong = await burst(1);
    expect((await errorOf(hourLong)).message).toBe("That's too many tries in a row. You can share again in an hour.");
    expect(await penalty(net)).toMatchObject({ strikes: 2 });
  });

  it("isn't set off by a full queue or a pause, which are the service's doing", async () => {
    const who = await author("patient-one");
    const full = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 106))), { GLOBAL_DAILY_PICTURES: "0" });
    expect((await errorOf(full)).code).toBe("queue_full");
    const paused = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 106)), later(1)), { PAUSED: "1" });
    expect((await errorOf(paused)).code).toBe("paused");
    expect(await penalty(`key:${who.key}`)).toBeNull();
    expect(await penalty(`net:${await networkOf(who.ip)}`)).toBeNull();
  });

  it("checks a request in one statement, the account's own, and refuses it with one more", async () => {
    const who = await author("counted-one");
    const { db, statements } = countingDb(env.DB);
    const request = new Request(BASE, { headers: { "CF-Connecting-IP": who.ip } });
    await requireSharer(testEnv({ DB: db }), request, who.key);
    expect(statements).toHaveLength(1);
    statements.length = 0;
    await env.DB.prepare("INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES (?1, 1, ?2, ?2 + 60)").bind(`key:${who.key}`, now()).run();
    await expect(requireSharer(testEnv({ DB: db }), request, who.key)).rejects.toMatchObject({ code: "cooling_down" });
    expect(statements).toHaveLength(2);
    statements.length = 0;
    await requireVerifiable(testEnv({ DB: db }), request, who.key);
    expect(statements).toHaveLength(1);
  });

  it("says how long to wait in words, rounded up", () => {
    const said = [1, 60, 61, 3599, 3600, 3601, DAY, 2 * DAY, 2 * DAY + 1, 30 * DAY].map(inWords);
    expect(said).toEqual(["a minute", "a minute", "2 minutes", "an hour", "an hour", "2 hours", "24 hours", "48 hours", "3 days", "30 days"]);
  });
});

describe("turning packs down", () => {
  it("bans a key for 30 days at its third pack turned down in 30 days", async () => {
    const who = await author("thrice-turned");
    // Trusted, so three packs fit in one day.
    expect((await admin("POST", `/v1/admin/keys/${who.key}/tier`, { tier: "trusted" })).status).toBe(200);
    const bans = [];
    for (let i = 0; i < 3; i++) bans.push(await reject(await submit(who, pictures(1, 110 + i)), { reasons: ["quality"] }));
    expect(bans.slice(0, 2)).toEqual([[], []]);
    expect(bans[2]).toEqual([{ kind: "key", id: who.key, until: expect.any(Number), reason: "marks" }]);
    expect((bans[2][0].until as number) - now()).toBeGreaterThan(30 * DAY - 60);

    const response = await share(who, 113, 1);
    expect(response.status).toBe(403);
    expect(await errorOf(response)).toEqual({
      code: "banned",
      message: "This computer can't share packs for now, because too many of its packs were turned down. It can share again in 30 days.",
      retry_after: expect.any(Number),
    });
    // Verifying it again doesn't help, and Cloudflare isn't asked.
    const turnstile = stubTurnstile();
    const link = await verifyLink(who, "thrice-turned");
    const again = await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, who.ip));
    expect((await errorOf(again)).code).toBe("banned");
    expect(turnstile).not.toHaveBeenCalled();
  });

  it("counts only the marks from the last 30 days", async () => {
    const who = await author("long-ago");
    const old = now() - 31 * DAY;
    await env.DB.batch(
      ["sub_old1", "sub_old2"].map((cause) => env.DB.prepare("INSERT INTO marks (subject, cause, at) VALUES (?1, ?2, ?3)").bind(`key:${who.key}`, cause, old)),
    );
    expect(await reject(await submit(who, pictures(1, 114)), { reasons: ["quality"] })).toEqual([]);
  });

  it("bans the key for good and its network for 30 days when the maintainer ticks ban", async () => {
    const ip = freshIp();
    const who = await author("abusive-one", ip);
    const neighbour = await author("innocent-neighbour", sameNetwork(ip, 20));
    const net = await networkOf(ip);
    const bans = await reject(await submit(who, pictures(1, 120)), { reasons: ["quality"], ban: true });
    expect(bans).toEqual([
      { kind: "key", id: who.key, until: null, reason: "abuse" },
      { kind: "network", id: net, until: expect.any(Number), reason: "abuse" },
    ]);
    expect(await (await call(await signed(who, "GET", "/v1/me"))).json()).toMatchObject({ tier: "banned" });
    expect((await errorOf(await share(who, 121, 1))).message).toBe(
      "This computer can't share packs any more, because a pack from it broke the pack terms.",
    );

    // Everyone else on the network waits the month out.
    const blocked = await share(neighbour, 122);
    expect(blocked.status).toBe(403);
    const error = await errorOf(blocked);
    expect(error).toMatchObject({ code: "banned", message: "Packs can't be shared from your network for now. You can share again in 30 days." });
    expect(error.retry_after).toBeGreaterThan(30 * DAY - 60);
    // No new computer is verified there, and Cloudflare isn't asked.
    const newcomer = await verifyNew(sameNetwork(ip, 30), "fresh-face");
    expect(await errorOf(newcomer.response)).toMatchObject({
      code: "banned",
      message: "Computers on your network can't be verified for now. You can try again in 30 days.",
    });
    expect(newcomer.asked).toBe(false);
    // Elsewhere, nothing has changed.
    expect((await verifyNew(freshIp(), "far-away")).response.status).toBe(201);
  });

  it("bans for abuse by itself when the reason is sexual content, a child or hate", async () => {
    for (const [i, reason] of ["sexual", "minor", "hate"].entries()) {
      const who = await author(`abuse-reason-${i}`);
      const bans = await reject(await submit(who, pictures(1, 130 + i)), { reasons: [reason] });
      expect(bans.map((b) => [b.kind, b.until === null, b.reason]), reason).toEqual([
        ["key", true, "abuse"],
        ["network", false, "abuse"],
      ]);
    }
  });

  it("bans a network for 30 days once a second key from it is banned within 30 days", async () => {
    const ip = freshIp();
    const first = await author("churn-one", ip);
    const second = await author("churn-two", sameNetwork(ip, 40));
    // Deceptive files ban the key for good, without being abuse.
    expect(await reject(await submit(first, pictures(1, 140)), { reasons: ["deceptive"] })).toEqual([
      { kind: "key", id: first.key, until: null, reason: "terms" },
    ]);
    expect(await reject(await submit(second, pictures(1, 141)), { reasons: ["deceptive"] })).toEqual([
      { kind: "key", id: second.key, until: null, reason: "terms" },
      { kind: "network", id: await networkOf(ip), until: expect.any(Number), reason: "keys" },
    ]);
    expect((await verifyNew(sameNetwork(ip, 50), "third-try")).response.status).toBe(403);
  });

  it("can ban on a takedown too, and only on turning a pack down", async () => {
    const who = await author("taken-for-abuse");
    const id = await submit(who, pictures(1, 150));
    expect((await errorOf(await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve", ban: true }))).code).toBe("bad_ban");
    expect((await errorOf(await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "reject", reasons: ["fit"], ban: "yes" }))).code).toBe(
      "bad_ban",
    );
    const down = await admin("POST", `/v1/admin/submissions/${id}/takedown`, { reasons: ["brand"], ban: true });
    expect(((await down.json()) as { bans: { reason: string }[] }).bans.map((b) => b.reason)).toEqual(["abuse", "abuse"]);
  });

  it("makes the maintainer choose why before a phone link takes a pack down", async () => {
    const who = await author("chosen-reason");
    const id = await submit(who, pictures(1, 152), { name: "Chosen reason" });
    const path = new URL((await makeLink(testEnv(), "takedown", id))!).pathname;
    const page = await (await call(new Request(`${BASE}${path}`))).text();
    // No reason is picked for them, since some of them ban the author.
    expect(page).toContain('<select name="reason" aria-label="Reason" required><option value="" selected disabled>Choose a reason</option>');
    const form = new FormData();
    form.set("decision", "takedown");
    form.set("reason", "");
    expect((await call(new Request(`${BASE}${path}`, { method: "POST", body: form }))).status).toBe(400);
    // The link still works once a reason is chosen.
    form.set("reason", "brand");
    const done = await call(new Request(`${BASE}${path}`, { method: "POST", body: form }));
    expect(await done.text()).toContain("gone from FolderSkin");
    expect(await (await call(await signed(who, "GET", "/v1/me"))).json()).toMatchObject({ tier: "probation" });
  });

  it("turns a pack down as abuse from the phone page's box", async () => {
    const who = await author("phone-abuse");
    const id = await submit(who, pictures(1, 151), { name: "Phone abuse" });
    const path = new URL((await makeLink(testEnv(), "review", id))!).pathname;
    expect(await (await call(new Request(`${BASE}${path}`))).text()).toContain('<input type="checkbox" name="ban" value="1">');
    const form = new FormData();
    form.set("decision", "reject");
    form.set("reason", "quality");
    form.set("ban", "1");
    const page = await (await call(new Request(`${BASE}${path}`, { method: "POST", body: form }))).text();
    expect(page).toContain("The computer that sent it is banned for good: its pack was turned down as abuse.");
    expect(page).toContain("Its network is banned for 30 days: a pack turned down as abuse came from it.");
    expect(await (await call(await signed(who, "GET", "/v1/me"))).json()).toMatchObject({ tier: "banned" });
  });
});

describe("the maintainer", () => {
  it("sees the bans in the queue, and can lift them", async () => {
    const ip = freshIp();
    const who = await author("lifted-later", ip);
    const neighbour = await author("lifted-neighbour", sameNetwork(ip, 60));
    const net = await networkOf(ip);
    await reject(await submit(who, pictures(1, 160)), { reasons: ["quality"], ban: true });

    const queued = async () => ((await (await admin("GET", "/v1/admin/queue")).json()) as { bans: Record<string, unknown>[] }).bans;
    const bans = await queued();
    expect(bans).toContainEqual({ kind: "network", id: net, until: expect.any(Number), reason: "abuse" });
    expect(bans).toContainEqual({ kind: "key", id: who.key, handle: "lifted-later", until: null, reason: "tier" });

    const key = await admin("POST", `/v1/admin/keys/${who.key}/unban`, {});
    expect(await key.json()).toEqual({ key: who.key, tier: "probation", banned: false });
    expect(await (await call(await signed(who, "GET", "/v1/me"))).json()).toMatchObject({ tier: "probation" });
    // The network is still banned until that is lifted too.
    expect((await errorOf(await share(neighbour, 161))).code).toBe("banned");
    const network = await admin("POST", `/v1/admin/networks/${net}/unban`, {});
    expect(await network.json()).toEqual({ network: net, banned: false });
    expect((await share(neighbour, 161, 1)).status).toBe(201);
    expect((await share(who, 162, 2)).status).toBe(201);
    const after = await queued();
    expect(after.some((b) => b.id === net || b.id === who.key)).toBe(false);

    const unknown = b64url(crypto.getRandomValues(new Uint8Array(32)));
    expect((await admin("POST", `/v1/admin/keys/${unknown}/unban`, {})).status).toBe(404);
    expect((await call(await signed(who, "POST", `/v1/admin/networks/${net}/unban`, {}), asAdmin())).status).toBe(404);
  });

  it("gets the day's bans in the digest", async () => {
    // The digest lists 20 of the day's events; the tests before this one made plenty.
    await env.DB.prepare("DELETE FROM events").run();
    const who = await author("digest-banned");
    await reject(await submit(who, pictures(1, 170)), { reasons: ["hate"] });
    const lines = (await digest(testEnv()))?.lines.join("\n") ?? "";
    expect(lines).toContain(`ban: key:${who.key} banned for good: its pack was turned down as abuse`);
    expect(lines).toContain(`ban: net:${await networkOf(who.ip)} banned for 30 days: a pack turned down as abuse came from it`);
  });
});

describe("the daily run", () => {
  it("clears the penalties, marks and networks that no longer count", async () => {
    const at = now();
    await env.DB.batch([
      env.DB.prepare("INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES ('key:tidy-old', 3, ?1, ?1 + 240)").bind(at - DAY - 300),
      env.DB.prepare("INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES ('key:tidy-cooling', 1, ?1, ?1 + 60)").bind(at - 10),
      env.DB.prepare("INSERT INTO penalties (subject, banned_until, ban_reason) VALUES ('net:tidy-banned', ?1, 'abuse')").bind(at + 1000),
      env.DB.prepare("INSERT INTO penalties (subject, banned_until, ban_reason) VALUES ('net:tidy-was-banned', ?1, 'abuse')").bind(at - 1),
      env.DB.prepare("INSERT INTO marks (subject, cause, at) VALUES ('key:tidy-marked', 'sub_old', ?1)").bind(at - 31 * DAY),
      env.DB.prepare("INSERT INTO marks (subject, cause, at) VALUES ('key:tidy-marked', 'sub_new', ?1)").bind(at - DAY),
    ]);
    const who = await author("tidy-author");
    const old = await submit(who, pictures(1, 180));
    await reject(old, { reasons: ["quality"] });
    await env.DB.prepare("UPDATE submissions SET decided_at = ?2 WHERE id = ?1").bind(old, at - 31 * DAY).run();
    const recent = await submit(who, pictures(1, 181));
    await reject(recent, { reasons: ["quality"] });

    await tidy(testEnv(), at);
    const { results: left } = await env.DB.prepare("SELECT subject FROM penalties WHERE subject LIKE '%:tidy-%' ORDER BY subject").all<{ subject: string }>();
    expect(left.map((r) => r.subject)).toEqual(["key:tidy-cooling", "net:tidy-banned"]);
    const { results: marks } = await env.DB.prepare("SELECT cause FROM marks WHERE subject = 'key:tidy-marked'").all<{ cause: string }>();
    expect(marks.map((m) => m.cause)).toEqual(["sub_new"]);
    const network = async (id: string) => (await env.DB.prepare("SELECT network FROM submissions WHERE id = ?1").bind(id).first<{ network: string | null }>())?.network;
    expect(await network(old)).toBeNull();
    expect(await network(recent)).toBe(await networkOf(who.ip));
  });
});

describe("what is kept", () => {
  it("is a hash of the network under a key of its own, never an address, and never the quotas' hash", async () => {
    const request = (ip: string) => new Request(BASE, { headers: { "CF-Connecting-IP": ip } });
    const net = await networkOf("198.51.100.23");
    expect(net).toMatch(/^[0-9a-f]{32}$/);
    expect(net).toBe(await networkOf("198.51.100.200"));
    expect(net).not.toBe(await networkOf("198.51.101.23"));
    for (const day of [0, 1, 40]) {
      expect(net).not.toBe(await networkHash(testEnv(), request("198.51.100.23"), now() + day * DAY));
    }
    const who = await author("kept-private", "198.51.100.23");
    await submit(who, pictures(1, 190));
    const dump = JSON.stringify([
      await env.DB.prepare("SELECT * FROM submissions WHERE key = ?1").bind(who.key).all(),
      await env.DB.prepare("SELECT * FROM penalties").all(),
      await env.DB.prepare("SELECT * FROM marks").all(),
    ]);
    expect(dump).not.toContain("198.51.100");
  });
});
