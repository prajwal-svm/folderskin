import { env } from "cloudflare:workers";
import { afterEach, describe, expect, it, vi } from "vitest";
import { sha256Hex } from "../src/bytes";
import {
  call,
  describe as describePack,
  device,
  errorOf,
  jpeg,
  pictures,
  png,
  signed,
  submit,
  verify,
  webpExtended,
  webpLossless,
  webpLossy,
  type Device,
} from "./helpers";

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
    // Early: the sheet hasn't arrived, but finalising still works and notes it for the maintainer,
    // without calling the pack flagged for it.
    const done = await call(await signed(who, "POST", `/v1/submissions/${opened.submission_id}/finalize`, {}));
    expect(await done.json()).toEqual({ status: "in_review" });
    const row = await env.DB.prepare("SELECT status, flags FROM submissions WHERE id = ?1").bind(opened.submission_id).first<{ status: string; flags: string }>();
    expect(row?.status).toBe("pending");
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
    // Version 1 were the terms for sharing through GitHub as well; the app shows whichever
    // version /v1/status names, and agrees to that one.
    expect(await (await call(new Request("https://community.test/v1/status"))).json()).toMatchObject({ terms_version: 2 });
    const firstTerms = { ...body, terms_version: 1 };
    expect(await errorOf(await call(await signed(who, "POST", "/v1/submissions", firstTerms)))).toEqual({
      code: "terms",
      message: "The pack terms have changed. Update FolderSkin, read them and send the pack again.",
    });

    const license = { ...body, license: "All rights reserved" };
    expect((await errorOf(await call(await signed(who, "POST", "/v1/submissions", license)))).code).toBe("bad_license");
  });

  it("takes only lossless pictures: PNG, and WebP whose picture is VP8L", async () => {
    const lossy = { code: "lossy_picture", message: "FolderSkin shares pictures without losing any quality. Update FolderSkin to share this pack." };
    const who = await author("lossless-only");
    // A JPEG never is, so a pack naming one is turned away before anything is sent.
    const withJpeg = await describePack([...pictures(1, 70), { file: "photo.jpg", bytes: jpeg(512, 512) }]);
    expect(await errorOf(await call(await signed(who, "POST", "/v1/submissions", withJpeg)))).toEqual(lossy);

    // A WebP could be either, which only its bytes tell.
    const list = [
      { file: "simple.webp", bytes: webpLossless(512, 512) },
      { file: "extended.webp", bytes: webpExtended(512, 512) },
      { file: "lossy.webp", bytes: webpLossy(512, 512) },
      { file: "lossy-alpha.webp", bytes: webpExtended(512, 512, { image: "VP8 " }) },
    ];
    const opened = await call(await signed(who, "POST", "/v1/submissions", await describePack(list)));
    expect(opened.status).toBe(201);
    const { submission_id: id } = (await opened.json()) as { submission_id: string };
    const put = async (p: (typeof list)[number]) => call(await signed(who, "PUT", `/v1/submissions/${id}/items/${await sha256Hex(p.bytes)}`, p.bytes));
    expect((await put(list[0])).status).toBe(200);
    expect((await put(list[1])).status).toBe(200);
    expect(await errorOf(await put(list[2]))).toEqual(lossy);
    expect(await errorOf(await put(list[3]))).toEqual(lossy);
  });

  it("takes pictures of up to 1.5 MB, and packs of up to 40 MB", async () => {
    const who = await author("heavy-lifter");
    const sized = async (count: number, bytes: number, salt: number) => {
      const body = await describePack(pictures(count, salt));
      return { ...body, items: body.items.map((item) => ({ ...item, bytes })) };
    };
    const tooBig = await call(await signed(who, "POST", "/v1/submissions", await sized(1, 1_572_865, 71)));
    expect(await errorOf(tooBig)).toEqual({ code: "too_large", message: "skin-1.png is over the 1.5 MB a picture can be." });
    // 27 pictures of 1.5 MB are 40.5 MB, over; 26 are 39 MB, and go through.
    const heavy = await call(await signed(who, "POST", "/v1/submissions", await sized(27, 1_572_864, 72)));
    expect(await errorOf(heavy)).toEqual({
      code: "pack_too_large",
      message: "The pack's pictures come to 40.5 MB, and a pack can be 40 MB at most. Take some out and try again.",
    });
    const fits = await call(await signed(who, "POST", "/v1/submissions", await sized(26, 1_572_864, 73)));
    expect(fits.status).toBe(201);
  });

  it("checks the pack's size again when it is sent for review", async () => {
    const who = await author("second-look");
    const list = pictures(1, 74);
    const created = await call(await signed(who, "POST", "/v1/submissions", await describePack(list)));
    const { submission_id: id, need } = (await created.json()) as { submission_id: string; need: string[] };
    expect((await call(await signed(who, "PUT", `/v1/submissions/${id}/items/${need[0]}`, list[0].bytes))).status).toBe(200);
    // As a pack opened before the limit came down would stand.
    await env.DB.prepare("UPDATE items SET bytes = ?2 WHERE submission = ?1").bind(id, 40 * 1024 * 1024 + 1).run();
    const done = await call(await signed(who, "POST", `/v1/submissions/${id}/finalize`, {}));
    expect((await errorOf(done)).code).toBe("pack_too_large");
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

  it("picks up where it stopped when the same pack is sent again", async () => {
    const who = await author("patchy-wifi");
    const list = pictures(3, 14);
    const first = await open(who, list);
    const sha = first.need[0];
    expect((await call(await signed(who, "PUT", `/v1/submissions/${first.submission_id}/items/${sha}`, list[0].bytes))).status).toBe(200);

    const later = { ts: Math.floor(Date.now() / 1000) + 1 };
    const again = await call(await signed(who, "POST", "/v1/submissions", await describePack(list), later));
    expect(again.status).toBe(200);
    expect(await again.json()).toEqual({ submission_id: first.submission_id, need: first.need.slice(1), sheets: 1 });
  });

  it("gives up an unfinished upload for a different pack, and gives back the pictures that never came", async () => {
    const who = await author("changed-mind");
    const first = await open(who, pictures(3, 15));
    const later = { ts: Math.floor(Date.now() / 1000) + 1 };
    const second = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(2, 16)), later));
    expect(second.status).toBe(201);
    const list = await mine(who);
    expect(list.find((s) => s.id === first.submission_id)).toMatchObject({ status: "expired" });
    // Three pictures were declared and none arrived, so only the second pack's two count today.
    const me = (await (await call(await signed(who, "GET", "/v1/me"))).json()) as { today: { pictures: number; submissions: number } };
    expect(me.today).toEqual({ pictures: 2, submissions: 2 });
  });

  it("keeps credits written over more than one line", async () => {
    const who = await author("two-liner");
    const body = { ...(await describePack(pictures(1, 60))), notes: "Base photo by Jane Doe, CC0\r\nFrame drawn by me\n\n\n\nThanks!" };
    const response = await call(await signed(who, "POST", "/v1/submissions", body));
    expect(response.status).toBe(201);
    const { submission_id: id } = (await response.json()) as { submission_id: string };
    const row = await env.DB.prepare("SELECT notes FROM submissions WHERE id = ?1").bind(id).first<{ notes: string }>();
    expect(row?.notes).toBe("Base photo by Jane Doe, CC0\nFrame drawn by me\n\nThanks!");
  });

  it("says what is wrong with credits it can't take", async () => {
    const who = await author("hidden-marks");
    const flipped = { ...(await describePack(pictures(1, 61))), notes: "Photo by \u202eeoD enaJ" };
    expect(await errorOf(await call(await signed(who, "POST", "/v1/submissions", flipped)))).toEqual({
      code: "bad_notes",
      message: "The credits have a character in them that doesn't show, such as a text direction mark. Take it out and try again.",
    });
    const long = { ...(await describePack(pictures(1, 61))), notes: "a".repeat(401) };
    expect(await errorOf(await call(await signed(who, "POST", "/v1/submissions", long)))).toEqual({
      code: "bad_notes",
      message: "Keep the credits to 400 characters.",
    });
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
  /** A stand-in for the send_email binding that keeps what it was given. */
  const mailer = () => {
    const sent: { subject: string; to: unknown }[] = [];
    return { sent, MAILER: { send: async (message: { subject: string; to: unknown }) => void sent.push(message) } as unknown as SendEmail };
  };
  const mail = { MAIL_TO: "maintainer@example.org", MAIL_FROM: "community@example.org" };

  it("flag it for the maintainer at once, by webhook and email, when they hit the blocklist", async () => {
    const who = await author("edgy-name");
    const hook = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    const { sent: emails, MAILER } = mailer();
    const id = await submit(who, pictures(1, 11), { name: "Hentai dreams", tags: ["anime"] }, {
      NOTIFY_WEBHOOK_URL: "https://discord.com/api/webhooks/1/secret",
      MAILER,
      ...mail,
    });
    const row = await env.DB.prepare("SELECT status, flags FROM submissions WHERE id = ?1").bind(id).first<{ status: string; flags: string }>();
    expect(row?.status).toBe("flagged");
    expect(JSON.parse(row!.flags)).toContainEqual({ code: "text:blocklist", severity: "high", detail: '"hentai" in the pack name' });
    expect(hook).toHaveBeenCalledTimes(1);
    const [url, init] = hook.mock.calls[0];
    expect(url).toBe("https://discord.com/api/webhooks/1/secret");
    const sent = JSON.parse(String(init?.body)) as { content: string };
    expect(sent.content).toContain('Urgent: "Hentai dreams" needs a look');
    expect(sent.content).toMatch(/Review it: https:\/\/community\.test\/l\/review\./);
    expect(emails).toMatchObject([{ subject: 'Urgent: "Hentai dreams" needs a look', to: "maintainer@example.org" }]);
    // The author sees it waiting, like any other.
    expect((await mine(who))[0]).toMatchObject({ status: "in_review" });
  });

  it("send profanity to the webhook at once, quietly, and leave the email for the digest", async () => {
    const who = await author("salty-sailor");
    const hook = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    const { sent: emails, MAILER } = mailer();
    await submit(who, pictures(1, 12), { name: "Damn fine shit", tags: ["boats"] }, { NOTIFY_WEBHOOK_URL: "https://ntfy.sh/folderskin-topic", MAILER, ...mail });
    expect(hook).toHaveBeenCalledTimes(1);
    const init = hook.mock.calls[0][1];
    expect((init?.headers as Record<string, string>).Priority).toBe("3");
    expect(String(init?.body)).toContain('Flagged: "Damn fine shit" needs a look');
    expect(emails).toEqual([]);
  });

  it("send nothing at once for a pack nothing caught", async () => {
    const who = await author("quiet-painter");
    const hook = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    await submit(who, pictures(1, 13), { name: "Harbour at dawn", tags: ["boats"] }, { NOTIFY_WEBHOOK_URL: "https://ntfy.sh/folderskin-topic" });
    expect(hook).not.toHaveBeenCalled();
  });
});
