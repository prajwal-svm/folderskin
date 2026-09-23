import { afterEach, describe, expect, it, vi } from "vitest";
import { BASE, call, device, errorOf, freshIp, postJson, signed, stubTurnstile, verify, verifyLink } from "./helpers";

afterEach(() => vi.restoreAllMocks());

describe("signed requests", () => {
  it("are taken from the key that signed them", async () => {
    const me = await device();
    const handle = await verify(me, "sunny-otter");
    const response = await call(await signed(me, "GET", "/v1/me"));
    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({ verified: true, handle, tier: "probation", accepting: true });
  });

  it("aren't taken unsigned, or signed over anything but what was sent", async () => {
    const me = await device();
    await verify(me, "tidy-heron");
    const unsigned = await call(new Request(`${BASE}/v1/me`));
    expect(unsigned.status).toBe(401);
    expect((await errorOf(unsigned)).code).toBe("unsigned");

    // Signed for one body, sent with another.
    const good = await signed(me, "POST", "/v1/me", { handle: "tidy-heron-2" });
    const swapped = new Request(good.url, { method: "POST", headers: good.headers, body: JSON.stringify({ handle: "someone-else" }) });
    const answer = await call(swapped);
    expect(answer.status).toBe(401);
    expect((await errorOf(answer)).code).toBe("bad_signature");

    // Signed for one path, sent to another.
    const other = await signed(me, "GET", "/v1/me");
    const moved = await call(new Request(`${BASE}/v1/submissions`, { headers: other.headers }));
    expect(moved.status).toBe(401);
  });

  it("aren't taken twice", async () => {
    const me = await device();
    await verify(me, "quiet-lark");
    const request = await signed(me, "POST", "/v1/me", { handle: "quiet-lark-renamed" });
    expect((await call(request.clone())).status).toBe(200);
    const again = await call(request);
    expect(again.status).toBe(409);
    expect((await errorOf(again)).code).toBe("replayed");
  });

  it("say when the computer's clock is out, with the time to set it by", async () => {
    const me = await device();
    const late = Math.floor(Date.now() / 1000) - 3600;
    const response = await call(await signed(me, "GET", "/v1/me", undefined, { ts: late }));
    expect(response.status).toBe(401);
    expect((await errorOf(response)).code).toBe("clock");
    expect(Number(response.headers.get("X-FS-Time"))).toBeGreaterThan(late + 3000);
  });

  it("from a computer that hasn't been verified can't send a pack", async () => {
    const stranger = await device();
    const me = await call(await signed(stranger, "GET", "/v1/me"));
    expect(await me.json()).toMatchObject({ verified: false });
    const response = await call(await signed(stranger, "POST", "/v1/submissions", { manifest: {} }));
    expect(response.status).toBe(403);
    expect((await errorOf(response)).code).toBe("not_verified");
  });
});

describe("verifying a computer", () => {
  it("serves the challenge page under a policy that allows only Turnstile's script", async () => {
    const page = await call(new Request(`${BASE}/verify?k=a&n=b`));
    expect(page.status).toBe(200);
    const csp = page.headers.get("Content-Security-Policy") ?? "";
    expect(csp).toContain("script-src 'self' https://challenges.cloudflare.com");
    expect(csp).toContain("frame-ancestors 'none'");
    const html = await page.text();
    expect(html).toContain('data-sitekey="1x00000000000000000000AA"');
    expect(html).not.toMatch(/<script>(?!<\/script>)/);
    const script = await call(new Request(`${BASE}/verify.js`));
    expect(script.headers.get("Content-Type")).toContain("javascript");
  });

  it("needs the challenge passed for the very nonce the app signed", async () => {
    const me = await device();
    stubTurnstile();
    const link = await verifyLink(me, "brave-finch");
    const wrong = await call(postJson("/v1/keys/verify", { ...link, token: "pass:another-nonce-entirely" }, me.ip));
    expect(wrong.status).toBe(403);
    expect((await errorOf(wrong)).code).toBe("challenge_failed");
    const right = await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, me.ip));
    expect(right.status).toBe(201);
    expect(await right.json()).toEqual({ handle: "brave-finch" });
  });

  it("uses each link once", async () => {
    const me = await device();
    stubTurnstile();
    const link = await verifyLink(me, "swift-wren");
    expect((await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, me.ip))).status).toBe(201);
    const again = await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, me.ip));
    expect(again.status).toBe(409);
    expect((await errorOf(again)).code).toBe("link_used");
  });

  it("won't take a link someone else signed, or one that has run out", async () => {
    const me = await device();
    const mallory = await device();
    stubTurnstile();
    const link = await verifyLink(mallory, "copy-cat");
    const forged = await call(postJson("/v1/keys/verify", { ...link, k: me.key, token: `pass:${link.n}` }, me.ip));
    expect((await errorOf(forged)).code).toBe("bad_link");
    const old = await verifyLink(me, "slow-snail", { ts: Math.floor(Date.now() / 1000) - 7200 });
    const stale = await call(postJson("/v1/keys/verify", { ...old, token: `pass:${old.n}` }, me.ip));
    expect((await errorOf(stale)).code).toBe("link_expired");
  });

  it("gives a taken name a number rather than turning the person away", async () => {
    const first = await device();
    const second = await device();
    expect(await verify(first, "moss-garden")).toBe("moss-garden");
    expect(await verify(second, "Moss-Garden")).toBe("Moss-Garden-2");
  });

  it("turns away names nobody may use", async () => {
    const me = await device();
    stubTurnstile();
    const link = await verifyLink(me, "folderskin");
    const response = await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, me.ip));
    expect(response.status).toBe(400);
    expect((await errorOf(response)).message).toMatch(/kept for FolderSkin/);
  });

  it("verifies only a few computers a day from one network", async () => {
    const ip = freshIp();
    stubTurnstile();
    const statuses: number[] = [];
    for (let i = 0; i < 6; i++) {
      const who = await device(ip);
      const link = await verifyLink(who, `crowd-${i}`);
      statuses.push((await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, ip))).status);
    }
    expect(statuses).toEqual([201, 201, 201, 201, 201, 429]);
  });

  it("never stores the address a request came from", async () => {
    const me = await device("203.0.113.77");
    await verify(me, "private-owl");
    const { env } = await import("cloudflare:workers");
    const dump = JSON.stringify(await env.DB.prepare("SELECT * FROM counters").all());
    expect(dump).not.toContain("203.0.113");
  });
});
