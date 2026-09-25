import { env } from "cloudflare:workers";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { sha256Hex } from "../src/bytes";
import { daily, digest } from "../src/daily";
import { makeLink } from "../src/links";
import { forgetPublished } from "../src/published";
import {
  BASE,
  call,
  describe as describePack,
  device,
  errorOf,
  freshIp,
  pictures,
  postJson,
  signed,
  submit,
  testEnv,
  verify,
  verifyLink,
  type Device,
} from "./helpers";

beforeEach(() => forgetPublished());
afterEach(() => vi.restoreAllMocks());

/** A generated id for a pack of this name: its slug, a dash and six random characters. */
const idOf = (slug: string) => new RegExp(`^${slug}-[a-z2-7]{6}$`);

let maintainer: Device;
beforeAll(async () => {
  maintainer = await device();
});
const asAdmin = () => ({ ADMIN_KEYS: `someone-else,${maintainer.key}` });

async function author(handle: string): Promise<Device> {
  const who = await device();
  await verify(who, handle);
  return who;
}

async function admin(method: string, path: string, body?: Record<string, unknown>) {
  return call(await signed(maintainer, method, path, body), asAdmin());
}

async function mine(who: Device) {
  const response = await call(await signed(who, "GET", "/v1/submissions"));
  return ((await response.json()) as { submissions: Record<string, unknown>[] }).submissions;
}

describe("the maintainer's endpoints", () => {
  it("answer only the keys in ADMIN_KEYS, and look like nothing to anyone else", async () => {
    const who = await author("curious-cat");
    const response = await call(await signed(who, "GET", "/v1/admin/queue"), asAdmin());
    expect(response.status).toBe(404);
    expect((await admin("GET", "/v1/admin/queue")).status).toBe(200);
  });

  it("approve a pack into the public bucket, credited to the author's handle, ready to pull", async () => {
    const who = await author("night-painter");
    const list = pictures(2, 20);
    const id = await submit(who, list, { name: "Night prints" });

    const queue = (await (await admin("GET", "/v1/admin/queue")).json()) as { submissions: { id: string; handle: string }[] };
    expect(queue.submissions.find((s) => s.id === id)).toMatchObject({ handle: "night-painter" });

    const approved = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    const { status, pack_id: packId } = (await approved.json()) as { status: string; pack_id: string };
    expect(status).toBe("approved");
    expect(packId).toMatch(idOf("night-prints"));
    expect((await mine(who))[0]).toMatchObject({ id, status: "approved", pack_id: packId });
    expect((await env.PUBLIC.list({ prefix: `packs/${packId}/` })).objects).toHaveLength(3);
    // Moved, not copied: the private copies of the pictures are gone.
    expect((await env.HOLD.list({ prefix: `hold/${id}/` })).objects.map((o) => o.key)).toEqual([`hold/${id}/sheet-0`]);

    const exported = (await (await admin("GET", "/v1/admin/exports")).json()) as {
      packs: { id: string; pack_id: string; files: { file: string; sha256: string; bytes: number }[] }[];
    };
    const pack = exported.packs.find((p) => p.id === id)!;
    expect(pack.files).toEqual(await Promise.all(list.map(async (p) => ({ file: p.file, sha256: await sha256Hex(p.bytes), bytes: p.bytes.length }))));

    const manifest = await (await admin("GET", `/v1/admin/exports/${id}/pack.json`)).json();
    expect(manifest).toEqual({
      version: 1,
      name: "Night prints",
      author: "night-painter",
      license: "CC0-1.0",
      tags: ["woodblock"],
      skins: [
        { file: "skin-1.png", name: "Skin 1" },
        { file: "skin-2.png", name: "Skin 2" },
      ],
    });
    const file = await admin("GET", `/v1/admin/exports/${id}/files/skin-2.png`);
    expect(new Uint8Array(await file.arrayBuffer())).toEqual(list[1].bytes);
    expect((await admin("GET", `/v1/admin/exports/${id}/files/..%2Fpack.json`)).status).toBe(404);

    expect(await (await admin("POST", `/v1/admin/exports/${id}/done`, {})).json()).toEqual({ exported: true, folder: packId });
    const after = (await (await admin("GET", "/v1/admin/exports")).json()) as { packs: { id: string }[] };
    expect(after.packs.some((p) => p.id === id)).toBe(false);

    // The author is off probation once a pack has been approved.
    expect(await (await call(await signed(who, "GET", "/v1/me"))).json()).toMatchObject({ tier: "active" });
  });

  it("give packs of the same name ids of their own, so names can repeat", async () => {
    const one = await author("twin-one");
    const two = await author("twin-two");
    const first = await submit(one, pictures(1, 21), { name: "Twins" });
    const second = await submit(two, pictures(1, 22), { name: "Twins" });
    const approve = async (id: string) =>
      ((await (await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" })).json()) as { pack_id: string }).pack_id;
    const ids = [await approve(first), await approve(second)];
    for (const packId of ids) expect(packId).toMatch(idOf("twins"));
    expect(ids[0]).not.toBe(ids[1]);
  });

  it("turn a pack down with reasons the author reads, and keep its pictures from coming back", async () => {
    const who = await author("rule-breaker");
    const list = pictures(1, 23);
    const id = await submit(who, list);
    const rejected = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "reject", reasons: ["gore"], note: "Not for everyone." });
    expect(await rejected.json()).toEqual({ status: "rejected", bans: [] });
    expect((await mine(who))[0]).toMatchObject({
      status: "rejected",
      note: "Not for everyone.",
      reasons: [{ code: "gore", term: 10, message: "The pictures include gore or shocking violence." }],
    });
    expect((await env.HOLD.list({ prefix: `hold/${id}/` })).objects).toEqual([]);
    const later = { ts: Math.floor(Date.now() / 1000) + 1 };
    const again = await call(await signed(who, "POST", "/v1/submissions", await describePack(list), later));
    expect((await errorOf(again)).code).toBe("blocked");
  });

  it("ban the computer behind the worst", async () => {
    const who = await author("very-bad-actor");
    const id = await submit(who, pictures(1, 24));
    await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "reject", reasons: ["minor"] });
    const me = await call(await signed(who, "GET", "/v1/me"));
    expect(await me.json()).toMatchObject({ tier: "banned" });
    const blocked = await call(await signed(who, "POST", "/v1/submissions", { manifest: {} }));
    expect((await errorOf(blocked)).code).toBe("banned");
    // Verifying again doesn't help, and doesn't even get as far as asking Cloudflare.
    const link = await verifyLink(who, "very-bad-actor");
    expect((await errorOf(await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, who.ip)))).code).toBe("banned");
  });

  it("only take reasons that map to the pack terms", async () => {
    const who = await author("vague-reject");
    const id = await submit(who, pictures(1, 25));
    const response = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "reject", reasons: ["because"] });
    expect((await errorOf(response)).code).toBe("bad_reasons");
  });

  it("take an approved pack down from the public bucket", async () => {
    const who = await author("soon-gone");
    const id = await submit(who, pictures(1, 26), { name: "Soon gone" });
    const approved = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    const { pack_id: packId } = (await approved.json()) as { pack_id: string };
    expect((await env.PUBLIC.list({ prefix: `packs/${packId}/` })).objects).toHaveLength(2);
    const down = await admin("POST", `/v1/admin/submissions/${id}/takedown`, { reasons: ["brand"] });
    expect(await down.json()).toEqual({ status: "taken_down", pack_id: packId, exported: false, folder: null, bans: [] });
    expect((await env.PUBLIC.list({ prefix: `packs/${packId}/` })).objects).toEqual([]);
  });

  it("export a pack credited as it was approved, even after its author changes their name", async () => {
    const who = await author("first-name");
    const id = await submit(who, pictures(1, 50), { name: "Renamed later" });
    await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    const renamed = await call(await signed(who, "POST", "/v1/me", { handle: "second-name" }));
    expect(await renamed.json()).toEqual({ handle: "second-name" });

    const exported = (await (await admin("GET", "/v1/admin/exports")).json()) as { packs: { id: string; handle: string }[] };
    expect(exported.packs.find((p) => p.id === id)).toMatchObject({ handle: "first-name" });
    const manifest = (await (await admin("GET", `/v1/admin/exports/${id}/pack.json`)).json()) as { author: string };
    expect(manifest.author).toBe("first-name");
  });

  it("go by the folder a pack was pulled into when an older pull wrote it somewhere else", async () => {
    const who = await author("clash-author");
    const id = await submit(who, pictures(1, 51), { name: "Sky moods" });
    const approved = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    const { pack_id: packId } = (await approved.json()) as { pack_id: string };
    expect(packId).toMatch(idOf("sky-moods"));
    const bad = await admin("POST", `/v1/admin/exports/${id}/done`, { folder: "../sky-moods" });
    expect((await errorOf(bad)).code).toBe("bad_folder");
    // A folderskin-tools from before generated ids numbered the name it found taken, and says so.
    const done = await admin("POST", `/v1/admin/exports/${id}/done`, { folder: "sky-moods-2" });
    expect(await done.json()).toEqual({ exported: true, folder: "sky-moods-2" });
    expect((await mine(who))[0]).toMatchObject({ id, status: "approved", pack_id: "sky-moods-2", pulled: true });

    // A report about the pack from GitHub isn't about this one; one about its own folder is.
    const repo = "https://github.com/prajwal-svm/folderskin-community/tree/main/packs";
    const aboutOf = async (target: string) => {
      expect((await call(postJson("/v1/reports", { target, reason: "copyright" }))).status).toBe(201);
      return (await env.DB.prepare("SELECT submission FROM reports WHERE target = ?1").bind(target).first<{ submission: string | null }>())?.submission;
    };
    expect(await aboutOf(`${repo}/sky-moods`)).toBeNull();
    expect(await aboutOf(`${repo}/sky-moods-2/`)).toBe(id);

    const down = await admin("POST", `/v1/admin/submissions/${id}/takedown`, { reasons: ["brand"] });
    expect(await down.json()).toEqual({ status: "taken_down", pack_id: packId, exported: true, folder: "sky-moods-2", bans: [] });
  });

  it("tell the maintainer at once when a pack already in the repository is withdrawn", async () => {
    const who = await author("changed-mind");
    const id = await submit(who, pictures(1, 52), { name: "Second thoughts" });
    await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    await admin("POST", `/v1/admin/exports/${id}/done`, { folder: "second-thoughts" });

    const hook = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    const gone = await call(await signed(who, "DELETE", `/v1/packs/${id}`), { NOTIFY_WEBHOOK_URL: "https://ntfy.sh/folderskin-topic" });
    expect(await gone.json()).toEqual({ status: "withdrawn" });
    expect(hook).toHaveBeenCalledTimes(1);
    const text = String(hook.mock.calls[0][1]?.body);
    expect(text).toContain('Withdrawn: "Second thoughts" needs taking out of the repository');
    expect(text).toContain("folderskin-community as packs/second-thoughts");
    expect((await mine(who))[0]).toMatchObject({ id, status: "withdrawn", pulled: true });
  });

  it("pause and resume sharing with the kill switch", async () => {
    const who = await author("patient-sharer");
    await admin("POST", "/v1/admin/pause", { paused: true, message: "Back on Monday." });
    expect(await (await call(new Request(`${BASE}/v1/status`))).json()).toMatchObject({ accepting: false, message: "Back on Monday." });
    const paused = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 27))));
    expect(paused.status).toBe(503);
    expect((await errorOf(paused)).message).toBe("Sharing without GitHub is paused for now. Back on Monday. Please try again later.");
    await admin("POST", "/v1/admin/pause", { paused: false });
    expect(await (await call(new Request(`${BASE}/v1/status`))).json()).toMatchObject({ accepting: true });
    // Signed a second later, as the app signs every request after the one before.
    const later = { ts: Math.floor(Date.now() / 1000) + 1 };
    const resumed = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 27)), later));
    expect(resumed.status).toBe(201);
  });
});

describe("phone links", () => {
  it("show what they will do on GET, and do it once on POST", async () => {
    const who = await author("phone-approved");
    const id = await submit(who, pictures(1, 30), { name: "Phone pack" });
    const link = (await makeLink(testEnv(), "review", id))!;
    const path = new URL(link).pathname;

    const page = await call(new Request(`${BASE}${path}`));
    expect(page.status).toBe(200);
    const html = await page.text();
    expect(html).toContain("Phone pack");
    expect(html).toContain("phone-approved");
    expect(html).toContain(`src="${path}/sheets/0"`);
    // Looking changes nothing.
    expect((await mine(who))[0]).toMatchObject({ status: "in_review" });
    const sheet = await call(new Request(`${BASE}${path}/sheets/0`));
    expect(sheet.headers.get("Content-Type")).toBe("image/jpeg");

    const form = new FormData();
    form.set("decision", "approve");
    const done = await call(new Request(`${BASE}${path}`, { method: "POST", body: form }));
    const { pack_id: packId } = (await mine(who))[0] as { pack_id: string };
    expect(packId).toMatch(idOf("phone-pack"));
    expect(await done.text()).toContain(`approved as ${packId}.`);
    expect((await mine(who))[0]).toMatchObject({ status: "approved" });

    const again = await call(new Request(`${BASE}${path}`, { method: "POST", body: form }));
    expect(again.status).toBe(410);
    expect((await call(new Request(`${BASE}${path}`))).status).toBe(410);
  });

  it("do nothing when changed or run out", async () => {
    const who = await author("phone-tamper");
    const id = await submit(who, pictures(1, 31));
    const link = (await makeLink(testEnv(), "review", id))!;
    const path = new URL(link).pathname;
    const tampered = path.replace("review.", "takedown.");
    expect((await call(new Request(`${BASE}${tampered}`))).status).toBe(410);
    const old = (await makeLink(testEnv(), "review", id, Math.floor(Date.now() / 1000) - 30 * 86400))!;
    expect((await call(new Request(`${BASE}${new URL(old).pathname}`))).status).toBe(410);
    const otherSecret = (await makeLink(testEnv({ LINK_SECRET: "another secret" }), "review", id))!;
    expect((await call(new Request(`${BASE}${new URL(otherSecret).pathname}`))).status).toBe(410);
  });

  it("take a reported pack down without the maintainer's signing key", async () => {
    const who = await author("reported-one");
    const id = await submit(who, pictures(1, 32), { name: "Reported" });
    const approved = await admin("POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    const { pack_id: packId } = (await approved.json()) as { pack_id: string };

    const hook = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    const report = await call(postJson("/v1/reports", { target: `https://example.com/packs/${packId}`, reason: "csam", details: "Please look." }), {
      NOTIFY_WEBHOOK_URL: "https://ntfy.sh/folderskin-secret-topic",
    });
    expect(report.status).toBe(201);
    expect(hook).toHaveBeenCalledTimes(1);
    const [url, init] = hook.mock.calls[0];
    expect(url).toBe("https://ntfy.sh/folderskin-secret-topic");
    expect((init?.headers as Record<string, string>).Priority).toBe("5");
    const text = String(init?.body);
    expect(text).toContain("Urgent report: child sexual abuse material");
    expect(text).toContain(`That is submission ${id}.`);
    const takedown = text.match(/Take it down: (\S+)/)![1];
    hook.mockRestore();

    const form = new FormData();
    form.set("decision", "takedown");
    form.set("reason", "minor");
    const done = await call(new Request(`${BASE}${new URL(takedown).pathname}`, { method: "POST", body: form }));
    const page = await done.text();
    expect(page).toContain("gone from FolderSkin");
    // A child in the pictures is abuse, whether the box was ticked or not.
    expect(page).toContain("The computer that sent it is banned for good: its pack was turned down as abuse.");
    expect(page).toContain("Its network is banned for 30 days: a pack turned down as abuse came from it.");
    expect((await env.PUBLIC.list({ prefix: `packs/${packId}/` })).objects).toEqual([]);
    expect(await (await call(await signed(who, "GET", "/v1/me"))).json()).toMatchObject({ tier: "banned" });
  });

  it("can't be made to act on a decision they weren't made for", async () => {
    const link = (await makeLink(testEnv(), "pause", ""))!;
    const form = new FormData();
    form.set("decision", "approve");
    const response = await call(new Request(`${BASE}${new URL(link).pathname}`, { method: "POST", body: form }));
    expect(response.status).toBe(400);
  });
});

describe("reports", () => {
  it("need a target and a reason from the list, and are limited per network", async () => {
    const ip = freshIp();
    expect((await call(postJson("/v1/reports", { target: "", reason: "csam" }, ip))).status).toBe(400);
    expect((await call(postJson("/v1/reports", { target: "some pack", reason: "vibes" }, ip))).status).toBe(400);
    const statuses = [];
    for (let i = 0; i < 11; i++) statuses.push((await call(postJson("/v1/reports", { target: `pack ${i}`, reason: "copyright" }, ip))).status);
    expect(statuses.filter((s) => s === 201)).toHaveLength(10);
    expect(statuses.at(-1)).toBe(429);
  });

  it("list reports for the maintainer, with how to reach whoever sent each one", async () => {
    const report = { target: "The otters pack", reason: "copyright", details: "These are my photos.", contact: "reporter@example.org" };
    expect((await call(postJson("/v1/reports", report))).status).toBe(201);
    const listed = (await (await admin("GET", "/v1/admin/reports?days=1")).json()) as {
      reports: { target: string; submission: string | null; details: string; contact: string }[];
    };
    expect(listed.reports.find((r) => r.target === "The otters pack")).toMatchObject({
      submission: null,
      details: "These are my photos.",
      contact: "reporter@example.org",
    });
    expect((await errorOf(await admin("GET", "/v1/admin/reports?days=999"))).code).toBe("bad_days");
    const stranger = await author("nosy-neighbour");
    expect((await call(await signed(stranger, "GET", "/v1/admin/reports"), asAdmin())).status).toBe(404);
  });
});

describe("the daily run", () => {
  it("clears away uploads nobody finished", async () => {
    const who = await author("never-finished");
    const created = await call(await signed(who, "POST", "/v1/submissions", await describePack(pictures(1, 40))));
    const { submission_id: id } = (await created.json()) as { submission_id: string };
    await daily(testEnv(), Math.floor(Date.now() / 1000) + 2 * 86400);
    expect((await mine(who))[0]).toMatchObject({ id, status: "expired" });
  });

  it("lists each of the day's reports in the digest, even one about a pack the service doesn't hold", async () => {
    await call(postJson("/v1/reports", { target: "The otters pack on GitHub", reason: "copyright", contact: "someone@example.org" }));
    const notice = await digest(testEnv());
    expect(notice?.lines).toContain("Report, copyright: The otters pack on GitHub");
    // How to reach the reporter stays out of the chat channel.
    expect(notice?.lines.join("\n")).not.toContain("someone@example.org");
  });

  it("sends a digest with a review link for each pack waiting", async () => {
    const who = await author("digest-subject");
    await submit(who, pictures(1, 41), { name: "Digest pack" });
    const notice = await digest(testEnv());
    expect(notice?.title).toBe("FolderSkin community: the daily digest");
    expect(notice?.lines[0]).toMatch(/^Waiting for review: \d+/);
    expect(notice?.links?.some((l) => l.label.includes('"Digest pack" by digest-subject'))).toBe(true);
  });
});
