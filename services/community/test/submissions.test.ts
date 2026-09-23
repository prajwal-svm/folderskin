import { env } from "cloudflare:workers";
import { afterEach, describe, expect, it, vi } from "vitest";
import { sha256Hex } from "../src/bytes";
import { call, describe as describePack, device, errorOf, jpeg, pictures, png, signed, submit, verify, webpExtended, type Device } from "./helpers";

afterEach(() => vi.restoreAllMocks());

async function author(handle: string): Promise<Device> {
  const who = await device();
  await verify(who, handle);
  return who;
}

async function open(who: Device, list = pictures(2)) {
  const response = await call(await signed(who, "POST", "/v1/submissions", await describePack(list)));
  expect(response.status).toBe(201);
  return (await response.json()) as { submission_id: string; need: string[]; sheets: number };
}

async function mine(who: Device) {
  const response = await call(await signed(who, "GET", "/v1/submissions"));
  return ((await response.json()) as { submissions: { id: string; status: string; reasons: unknown[] }[] }).submissions;
}

describe("sending a pack", () => {
  it("goes from open to in review, and the author sees it there", async () => {
    const who = await author("paper-crane");
    const list = pictures(3, 1);
    const opened = await open(who, list);
    expect(opened.need).toEqual(await Promise.all(list.map((p) => sha256Hex(p.bytes))));
    expect(opened.sheets).toBe(1);
    expect((await mine(who))[0]).toMatchObject({ id: opened.submission_id, status: "uploading" });

    for (const p of list) {
      const sha = await sha256Hex(p.bytes);
      const put = await call(await signed(who, "PUT", `/v1/submissions/${opened.submission_id}/items/${sha}`, p.bytes));
      expect(put.status).toBe(200);
    }
    // Early: the sheet hasn't arrived, but finalising still works and notes it for the maintainer.
    const done = await call(await signed(who, "POST", `/v1/submissions/${opened.submission_id}/finalize`, {}));
    expect(await done.json()).toEqual({ status: "in_review" });
    const row = await env.DB.prepare("SELECT status, flags FROM submissions WHERE id = ?1").bind(opened.submission_id).first<{ status: string; flags: string }>();
    expect(row?.status).toBe("flagged");
    expect(JSON.parse(row!.flags)).toEqual([{ code: "sheet:missing", severity: "normal", detail: "not every contact sheet arrived" }]);
    expect((await mine(who))[0]).toMatchObject({ status: "in_review", reasons: [] });
  });

  it("can't be finalised before every picture has arrived", async () => {
    const who = await author("late-post");
    const opened = await open(who);
    const done = await call(await signed(who, "POST", `/v1/submissions/${opened.submission_id}/finalize`, {}));
    expect(done.status).toBe(409);
    expect((await errorOf(done)).message).toBe("2 pictures haven't arrived yet. Send them and try again.");
  });

  it("only takes the exact picture the pack described", async () => {
    const who = await author("exact-match");
    const list = pictures(1, 2);
    const opened = await open(who, list);
    const sha = opened.need[0];
    const path = `/v1/submissions/${opened.submission_id}/items/${sha}`;

    const other = png(512, 512, { salt: 999 });
    const swapped = await call(await signed(who, "PUT", path, other));
    expect((await errorOf(swapped)).code).toBe("mismatch");

    const right = await call(await signed(who, "PUT", path, list[0].bytes));
    expect(right.status).toBe(200);
    expect(await env.HOLD.head(`hold/${opened.submission_id}/${sha}`)).not.toBeNull();
    // Sending it again, as the app does after a dropped connection, is harmless.
    const retry = await signed(who, "PUT", path, list[0].bytes, { ts: Math.floor(Date.now() / 1000) + 1 });
    expect((await call(retry)).status).toBe(200);
  });

  it("turns away a picture that isn't what its name and size say", async () => {
    const who = await author("honest-files");
    const cases: [string, Uint8Array, RegExp][] = [
      ["a.png", jpeg(512, 512), /isn't the kind of picture its name says/],
      ["b.png", png(512, 512, { animated: true }), /animated PNG/],
      ["c.webp", webpExtended(512, 512, { animated: true }), /animated WebP/],
      ["d.png", png(600, 512), /600×512 px, not the size the pack described/],
    ];
    const list = cases.map(([file, bytes]) => ({ file, bytes }));
    const body = await describePack(list);
    const opened = await call(await signed(who, "POST", "/v1/submissions", body));
    const { submission_id: id } = (await opened.json()) as { submission_id: string };
    for (const [file, bytes, why] of cases) {
      const sha = await sha256Hex(bytes);
      const response = await call(await signed(who, "PUT", `/v1/submissions/${id}/items/${sha}`, bytes));
      expect(response.status, file).toBe(400);
      expect((await errorOf(response)).message, file).toMatch(why);
    }
  });

  it("describes its pictures honestly or not at all", async () => {
    const who = await author("careful-counter");
    const list = pictures(2, 3);
    const body = await describePack(list);
    const twice = { ...body, items: [body.items[0], { ...body.items[0], file: list[1].file }] };
    const dup = await call(await signed(who, "POST", "/v1/submissions", twice));
    expect((await errorOf(dup)).code).toBe("duplicate_picture");

    const big = { ...body, items: [{ ...body.items[0], bytes: 3 * 1024 * 1024 }, body.items[1]] };
    expect((await errorOf(await call(await signed(who, "POST", "/v1/submissions", big)))).code).toBe("too_large");

    const tiny = { ...body, items: [{ ...body.items[0], width: 128 }, body.items[1]] };
    expect((await errorOf(await call(await signed(who, "POST", "/v1/submissions", tiny)))).code).toBe("bad_size");

    const oldTerms = { ...body, terms_version: 0 };
    expect((await errorOf(await call(await signed(who, "POST", "/v1/submissions", oldTerms)))).code).toBe("terms");

    const license = { ...body, license: "All rights reserved" };
    expect((await errorOf(await call(await signed(who, "POST", "/v1/submissions", license)))).code).toBe("bad_license");
  });

  it("keeps someone else's submission out of reach", async () => {
    const owner = await author("rightful-owner");
    const other = await author("nosy-neighbour");
    const list = pictures(1, 4);
    const opened = await open(owner, list);
    const put = await call(await signed(other, "PUT", `/v1/submissions/${opened.submission_id}/items/${opened.need[0]}`, list[0].bytes));
    expect(put.status).toBe(404);
    const withdraw = await call(await signed(other, "DELETE", `/v1/packs/${opened.submission_id}`));
    expect(withdraw.status).toBe(404);
    expect(await mine(other)).toEqual([]);
  });

  it("can be withdrawn by its author, pictures and all", async () => {
    const who = await author("second-thoughts");
    const id = await submit(who, pictures(2, 5));
    const gone = await call(await signed(who, "DELETE", `/v1/packs/${id}`));
    expect(await gone.json()).toEqual({ status: "withdrawn" });
    expect((await env.HOLD.list({ prefix: `hold/${id}/` })).objects).toEqual([]);
    expect((await mine(who))[0]).toMatchObject({ id, status: "withdrawn" });
  });
});

describe("quotas", () => {
  it("let a new computer have one pack waiting at a time", async () => {
    const who = await author("eager-beaver");
    await submit(who, pictures(1, 6));
    const second = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 7))));
    expect(second.status).toBe(429);
    expect((await errorOf(second)).message).toBe(
      "You have a pack waiting for review already. Once it's been looked at, you can send another.",
    );
  });

  it("say the review queue is full once the day's pictures run out", async () => {
    const who = await author("big-day");
    const response = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(3, 8))), {
      GLOBAL_DAILY_PICTURES: "2",
    });
    expect(response.status).toBe(503);
    expect(await errorOf(response)).toEqual({ code: "queue_full", message: "The review queue is full for today. Please try again tomorrow." });
    expect(response.headers.get("Retry-After")).toBe("3600");
  });

  it("stop everything new while sharing is paused", async () => {
    const who = await author("patient-person");
    const response = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 9))), { PAUSED: "1" });
    expect(response.status).toBe(503);
    expect((await errorOf(response)).code).toBe("paused");
    const status = await call(new Request("https://community.test/v1/status"), { PAUSED: "1" });
    expect(await status.json()).toMatchObject({ accepting: false });
  });

  it("don't take pictures that were turned down before for what they show", async () => {
    const who = await author("repeat-offender");
    const list = pictures(1, 10);
    await env.DB.prepare("INSERT INTO blocked (sha256, reason, created_at) VALUES (?1, 'sexual', 0)")
      .bind(await sha256Hex(list[0].bytes))
      .run();
    const response = await call(await signed(who, "POST", "/v1/submissions", await describePack(list)));
    expect((await errorOf(response)).code).toBe("blocked");
  });
});

describe("the words in a pack", () => {
  it("flag it for the maintainer at once when they hit the blocklist", async () => {
    const who = await author("edgy-name");
    const hook = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    const id = await submit(who, pictures(1, 11), { name: "Hentai dreams", tags: ["anime"] }, { NOTIFY_WEBHOOK_URL: "https://discord.com/api/webhooks/1/secret" });
    const row = await env.DB.prepare("SELECT status, flags FROM submissions WHERE id = ?1").bind(id).first<{ status: string; flags: string }>();
    expect(row?.status).toBe("flagged");
    expect(JSON.parse(row!.flags)).toContainEqual({ code: "text:blocklist", severity: "high", detail: '"hentai" in the pack name' });
    expect(hook).toHaveBeenCalledTimes(1);
    const [url, init] = hook.mock.calls[0];
    expect(url).toBe("https://discord.com/api/webhooks/1/secret");
    const sent = JSON.parse(String(init?.body)) as { content: string };
    expect(sent.content).toContain('Urgent: "Hentai dreams" was flagged');
    expect(sent.content).toMatch(/Review it: https:\/\/community\.test\/l\/review\./);
    // The author sees it waiting, like any other.
    expect((await mine(who))[0]).toMatchObject({ status: "in_review" });
  });
});
