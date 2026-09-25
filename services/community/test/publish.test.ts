import { createExecutionContext, waitOnExecutionContext } from "cloudflare:test";
import { env } from "cloudflare:workers";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { sha256Hex } from "../src/bytes";
import worker from "../src/index";
import { makeLink } from "../src/links";
import { DISPATCH_URL, forgetPending, isTreePath, pending } from "../src/publish";
import { forgetPublished, PUBLISHED_INDEX } from "../src/published";
import { BASE, call, device, drawSuffixes, errorOf, pictures, signed, submit, testEnv, verify, type Device } from "./helpers";

beforeEach(() => forgetPublished());
afterEach(() => vi.restoreAllMocks());

const TOKEN = "github_pat_test_only";
const withToken = { GITHUB_DISPATCH_TOKEN: TOKEN };

let maintainer: Device;
beforeAll(async () => {
  maintainer = await device();
});
const asAdmin = () => ({ ADMIN_KEYS: maintainer.key });

async function author(handle: string): Promise<Device> {
  const who = await device();
  await verify(who, handle);
  return who;
}

async function decide(id: string, body: Record<string, unknown>, overrides = {}) {
  return call(await signed(maintainer, "POST", `/v1/admin/submissions/${id}/decision`, body), { ...asAdmin(), ...overrides });
}

type Sent = { url: string; init: RequestInit };

/**
 * Stands in for GitHub: raw.githubusercontent.com answers with `index` (an error when there is
 * none), and the dispatch endpoint with what `dispatch` says. Returns what was sent to the latter.
 */
function stubGitHub({ index, dispatch = () => new Response(null, { status: 204 }) }: { index?: unknown; dispatch?: () => Response | Promise<Response> }) {
  const sent: Sent[] = [];
  const fetches = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    if (url === PUBLISHED_INDEX) return index === undefined ? new Response("not here", { status: 404 }) : Response.json(index);
    if (url === DISPATCH_URL) {
      sent.push({ url, init: init ?? {} });
      return dispatch();
    }
    return new Response("unexpected", { status: 599 });
  });
  return { sent, fetches };
}

async function eventsFor(subject: string) {
  const { results } = await env.DB.prepare("SELECT kind, detail FROM events WHERE subject = ?1 AND kind LIKE 'publish%' ORDER BY id").bind(subject).all<{
    kind: string;
    detail: string;
  }>();
  return results;
}

describe("approving a pack", () => {
  it("asks folderskin-community's workflow to publish it, as GitHub's API wants", async () => {
    const who = await author("dispatch-author");
    const id = await submit(who, pictures(1, 70), { name: "Sent along" });
    const { sent } = stubGitHub({});
    const approved = await decide(id, { decision: "approve" }, withToken);
    expect(approved.status).toBe(200);
    const { pack_id: packId } = (await approved.json()) as { pack_id: string };

    expect(sent).toHaveLength(1);
    const { init } = sent[0];
    expect(init.method).toBe("POST");
    expect(init.headers).toMatchObject({
      Authorization: `Bearer ${TOKEN}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "folderskin-community",
    });
    expect(JSON.parse(String(init.body))).toEqual({ event_type: "pack-approved", client_payload: { submission: id, pack: packId } });
    expect(await eventsFor(id)).toEqual([{ kind: "publish", detail: `folderskin-community was asked to publish ${packId}` }]);
  });

  it("does it from a phone link too", async () => {
    const who = await author("phone-dispatch");
    const id = await submit(who, pictures(1, 71), { name: "From a phone" });
    const link = (await makeLink(testEnv(), "review", id))!;
    const { sent } = stubGitHub({});
    const form = new FormData();
    form.set("decision", "approve");
    const done = await call(new Request(`${BASE}${new URL(link).pathname}`, { method: "POST", body: form }), withToken);
    expect(done.status).toBe(200);
    expect(sent).toHaveLength(1);
    expect(JSON.parse(String(sent[0].init.body))).toMatchObject({ event_type: "pack-approved", client_payload: { submission: id } });
  });

  it("asks nothing of GitHub without the token, and says so in the events", async () => {
    const who = await author("no-token-author");
    const id = await submit(who, pictures(1, 72), { name: "Waits a day" });
    const { sent } = stubGitHub({});
    const approved = await decide(id, { decision: "approve" });
    expect(approved.status).toBe(200);
    const { pack_id: packId } = (await approved.json()) as { pack_id: string };
    expect(sent).toEqual([]);
    expect(await eventsFor(id)).toEqual([
      { kind: "publish", detail: `not asked: GITHUB_DISPATCH_TOKEN isn't set, so the scheduled run publishes ${packId}` },
    ]);
  });

  it("goes through when GitHub turns the request down or can't be reached, leaving the pack to the scheduled run", async () => {
    const who = await author("github-down");
    const first = await submit(who, pictures(1, 73), { name: "Turned away" });
    stubGitHub({ dispatch: () => new Response(JSON.stringify({ message: "Bad credentials" }), { status: 401 }) });
    const refused = await decide(first, { decision: "approve" }, withToken);
    expect(await refused.json()).toMatchObject({ status: "approved" });
    expect((await eventsFor(first))[0]).toMatchObject({ kind: "publish_failed", detail: expect.stringMatching(/^GitHub answered 401, so the scheduled run publishes /) });
    vi.restoreAllMocks();

    const second = await submit(who, pictures(1, 74), { name: "Unreachable" }, {});
    stubGitHub({
      dispatch: () => {
        throw new TypeError("Network connection lost.");
      },
    });
    const unreachable = await decide(second, { decision: "approve" }, withToken);
    expect(await unreachable.json()).toMatchObject({ status: "approved" });
    expect((await eventsFor(second))[0]).toMatchObject({ kind: "publish_failed", detail: expect.stringMatching(/^GitHub couldn't be reached/) });
    // The token never goes into the log.
    const { results } = await env.DB.prepare("SELECT detail FROM events WHERE detail LIKE ?1").bind(`%${TOKEN}%`).all();
    expect(results).toEqual([]);
  });

  it("answers before GitHub does", async () => {
    const who = await author("slow-github");
    const id = await submit(who, pictures(1, 75), { name: "Answered first" });
    let answer!: (response: Response) => void;
    const { sent } = stubGitHub({ dispatch: () => new Promise<Response>((resolve) => (answer = resolve)) });
    const ctx = createExecutionContext();
    const request = await signed(maintainer, "POST", `/v1/admin/submissions/${id}/decision`, { decision: "approve" });
    const response = await worker.fetch(request as Request<unknown, IncomingRequestCfProperties>, testEnv({ ...asAdmin(), ...withToken }), ctx);
    expect(response.status).toBe(200);
    // Approved, and GitHub hasn't answered yet.
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(await eventsFor(id)).toEqual([]);
    answer(new Response(null, { status: 204 }));
    await waitOnExecutionContext(ctx);
    expect((await eventsFor(id))[0]).toMatchObject({ kind: "publish" });
  });
});

describe("the pending count", () => {
  beforeEach(() => forgetPending());
  const pendingNow = async (overrides = {}) => call(new Request(`${BASE}/v1/exports/pending`, { headers: { "CF-Connecting-IP": "192.0.2.1" } }), overrides);
  const pendingAt = async (at: number) => ((await (await pending(testEnv(), at)).json()) as { pending: number }).pending;

  it("says how many approved packs wait to be pulled, and nothing about them", async () => {
    const response = await pendingNow();
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=60");
    const { pending: before } = (await response.json()) as { pending: number };
    expect(Object.keys(await (await pendingNow()).json())).toEqual(["pending"]);

    const who = await author("pending-author");
    const id = await submit(who, pictures(1, 90), { name: "Waiting to go" });
    stubGitHub({});
    await decide(id, { decision: "approve" });
    const at = Math.floor(Date.now() / 1000);
    forgetPending();
    expect(await pendingAt(at)).toBe(before + 1);
    await call(await signed(maintainer, "POST", `/v1/admin/exports/${id}/done`, {}), asAdmin());
    // Each isolate reads the database for it once a minute at most, however often it is asked.
    expect(await pendingAt(at + 30)).toBe(before + 1);
    expect(await pendingAt(at + 60)).toBe(before);
  });

  it("is held to the burst limit", async () => {
    const response = await pendingNow({ BURST: { limit: async () => ({ success: false }) } as unknown as RateLimit });
    expect((await errorOf(response)).code).toBe("slow_down");
  });
});

describe("the catalog tree", () => {
  /**
   * A PUT of `body` to the tree, signed by `who` for the SHA-256 of `signedFor` (the body itself
   * unless said), which X-Content-SHA256 declares.
   */
  async function put(path: string, body: Uint8Array, { who = maintainer, signedFor = body, headers = {} as Record<string, string>, ts = Math.floor(Date.now() / 1000) } = {}) {
    const request = await signed(who, "PUT", `/v1/admin/tree/${path}`, signedFor, { ts });
    const sent = new Headers(request.headers);
    sent.set("X-Content-SHA256", await sha256Hex(signedFor));
    sent.set("Content-Length", String(body.length));
    for (const [name, value] of Object.entries(headers)) {
      if (value === "") sent.delete(name);
      else sent.set(name, value);
    }
    return new Request(request.url, { method: "PUT", headers: sent, body });
  }
  const bytes = (text: string) => new TextEncoder().encode(text);

  it("stores a file under its path, typed by its extension, cached for good", async () => {
    const picture = bytes("a picture's bytes");
    const path = `v2/pictures/${await sha256Hex(picture)}.png`;
    const response = await call(await put(path, picture), asAdmin());
    expect(await response.json()).toEqual({ stored: true });
    const object = await env.PACKS.get(path);
    expect(new Uint8Array(await object!.arrayBuffer())).toEqual(picture);
    expect(object!.httpMetadata).toMatchObject({ contentType: "image/png", cacheControl: "public, max-age=31536000, immutable" });
  });

  it("types each kind of file, and lets head.json be cached for a minute only", async () => {
    const cases: [string, string, string][] = [
      ["v2/head.json", "application/json", "public, max-age=60"],
      ["v2/catalog/0123456789abcdef.sqlite.gz", "application/gzip", "public, max-age=31536000, immutable"],
      ["v2/strips/abc.webp", "image/webp", "public, max-age=31536000, immutable"],
      ["v2/pictures/abc.jpg", "image/jpeg", "public, max-age=31536000, immutable"],
      ["v2/pictures/abc.JPEG", "image/jpeg", "public, max-age=31536000, immutable"],
      ["v2/packs/night-prints-k7q2mx/abc.json", "application/json", "public, max-age=31536000, immutable"],
      ["v2/notes/readme", "application/octet-stream", "public, max-age=31536000, immutable"],
    ];
    for (const [i, [path, contentType, cacheControl]] of cases.entries()) {
      const response = await call(await put(path, bytes(`file ${i}`)), asAdmin());
      expect(response.status, path).toBe(200);
      expect((await env.PACKS.head(path))?.httpMetadata, path).toMatchObject({ contentType, cacheControl });
    }
  });

  it("takes files only from a key in ADMIN_KEYS, and looks like nothing to anyone else", async () => {
    const stranger = await author("tree-stranger");
    const path = "v2/packs/not-yours-abcdef/0f.json";
    const response = await call(await put(path, bytes("{}"), { who: stranger }), asAdmin());
    expect(response.status).toBe(404);
    const unsigned = await call(new Request(`${BASE}/v1/admin/tree/${path}`, { method: "PUT", body: "{}" }), asAdmin());
    expect((await errorOf(unsigned)).code).toBe("unsigned");
    const noDigest = await call(await put(path, bytes("{}"), { headers: { "X-Content-SHA256": "" } }), asAdmin());
    expect((await errorOf(noDigest)).code).toBe("bad_digest");
    expect(await env.PACKS.head(path)).toBeNull();
  });

  it("stores nothing that isn't exactly what was signed for", async () => {
    const mismatch = await call(await put("v2/strips/swapped.webp", bytes("other bytes"), { signedFor: bytes("signed bytes") }), asAdmin());
    expect([mismatch.status, (await errorOf(mismatch)).code]).toEqual([400, "mismatch"]);
    expect(await env.PACKS.head("v2/strips/swapped.webp")).toBeNull();
    // The same request sent twice is a replay.
    const ts = Math.floor(Date.now() / 1000);
    expect((await call(await put("v2/strips/once.webp", bytes("once"), { ts }), asAdmin())).status).toBe(200);
    expect((await errorOf(await call(await put("v2/strips/once.webp", bytes("once"), { ts }), asAdmin()))).code).toBe("replayed");
  });

  it("needs its size said, up to 64 MB", async () => {
    const unsaid = await call(await put("v2/strips/unsaid.webp", bytes("x"), { headers: { "Content-Length": "" } }), asAdmin());
    expect([unsaid.status, (await errorOf(unsaid)).code]).toEqual([411, "length_required"]);
    const huge = await call(await put("v2/strips/huge.webp", bytes("x"), { headers: { "Content-Length": String(64 * 1024 * 1024 + 1) } }), asAdmin());
    expect([huge.status, (await errorOf(huge)).code]).toEqual([413, "too_large"]);
  });

  it("keeps to plain paths under v2/", async () => {
    for (const good of ["v2/head.json", "v2/packs/koi-abcdef/0f.json", `v2/${"a".repeat(197)}`]) expect(isTreePath(good), good).toBe(true);
    for (const bad of [
      "v1/head.json",
      "head.json",
      "/v2/head.json",
      "v2/",
      "v2//head.json",
      "v2/head.json/",
      "v2/./head.json",
      "v2/../head.json",
      "v2/a..b.json",
      "v2/a b.json",
      "v2/a%2e%2e/b",
      "v2/~user",
      `v2/${"a".repeat(198)}`,
    ]) {
      expect(isTreePath(bad), bad).toBe(false);
    }
    const response = await call(await put("v2/a%20b.json", bytes("x")), asAdmin());
    expect([response.status, (await errorOf(response)).code]).toEqual([400, "bad_path"]);
  });
});

describe("a new pack's id", () => {
  async function approveWith(name: string, salt: number, index: unknown, ...draws: number[][]) {
    const who = await author(`taken-${salt}`);
    const id = await submit(who, pictures(1, salt), { name });
    stubGitHub({ index });
    drawSuffixes(...draws);
    const approved = await decide(id, { decision: "approve" });
    vi.restoreAllMocks();
    return { id, ...((await approved.json()) as { pack_id: string }) };
  }
  const a = [0, 0, 0, 0, 0, 0];
  const b = [1, 1, 1, 1, 1, 1];
  const c = [2, 2, 2, 2, 2, 2];

  it("is never one folderskin-community publishes already", async () => {
    const index = { version: 1, packs: [{ id: "harbour-aaaaaa" }] };
    expect((await approveWith("Harbour", 80, index, a, b)).pack_id).toBe("harbour-bbbbbb");
  });

  it("is never an old id a renamed pack had", async () => {
    const index = { version: 1, packs: [{ id: "lanterns-zzzzzz" }], moved: { "lanterns-aaaaaa": "lanterns-zzzzzz" } };
    expect((await approveWith("Lanterns", 81, index, a, b)).pack_id).toBe("lanterns-bbbbbb");
  });

  it("is never another approved pack's, nor the folder one was pulled into", async () => {
    const first = await approveWith("Meadow", 82, undefined, a);
    expect(first.pack_id).toBe("meadow-aaaaaa");
    expect((await approveWith("Meadow", 83, undefined, a, b)).pack_id).toBe("meadow-bbbbbb");
    // An older pull wrote the first one to a folder of another name.
    await call(await signed(maintainer, "POST", `/v1/admin/exports/${first.id}/done`, { folder: "meadow-cccccc" }), asAdmin());
    expect((await approveWith("Meadow", 84, undefined, c, a, b, [3, 3, 3, 3, 3, 3])).pack_id).toBe("meadow-dddddd");
  });

  it("is checked against the database alone while the published list can't be read", async () => {
    expect((await approveWith("Orchard", 85, undefined, a)).pack_id).toBe("orchard-aaaaaa");
  });
});
