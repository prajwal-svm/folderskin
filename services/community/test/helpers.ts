/**
 * What the tests share: an environment with test secrets and no Workers AI, a computer's key that
 * signs requests the way the app does, pictures made of headers alone (the service never decodes
 * one), and the steps of a whole submission.
 */
import { createExecutionContext, waitOnExecutionContext } from "cloudflare:test";
import { env } from "cloudflare:workers";
import { vi } from "vitest";
import { b64url, sha256Hex } from "../src/bytes";
import type { Env } from "../src/env";
import worker from "../src/index";
import { TERMS_VERSION } from "../src/limits";

export const BASE = "https://community.test";

let networks = 0;
/** A network of its own (quotas count per /24), so one test's quotas never spill into another's. */
export const freshIp = () => `10.${Math.floor(++networks / 250)}.${networks % 250}.7`;

export function testEnv(overrides: Partial<Env> = {}): Env {
  return {
    ...env,
    AI: undefined,
    MAILER: undefined,
    BURST: undefined,
    IP_SALT: "test salt",
    LINK_SECRET: "test link secret",
    TURNSTILE_SECRET: "test turnstile secret",
    TURNSTILE_SITE_KEY: "1x00000000000000000000AA",
    PUBLIC_BASE_URL: BASE,
    NOTIFY_WEBHOOK_URL: undefined,
    ...overrides,
  } as Env;
}

/** Sends a request to the Worker and waits for everything it left running (notifications) to finish. */
export async function call(request: Request, overrides: Partial<Env> = {}): Promise<Response> {
  const ctx = createExecutionContext();
  const response = await worker.fetch(request as Request<unknown, IncomingRequestCfProperties>, testEnv(overrides), ctx);
  await waitOnExecutionContext(ctx);
  return response;
}

export type Device = { key: string; ip: string; sign(message: string): Promise<string> };

/** A computer's Ed25519 key, as the app keeps one. */
export async function device(ip = freshIp()): Promise<Device> {
  const pair = (await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"])) as CryptoKeyPair;
  const raw = new Uint8Array((await crypto.subtle.exportKey("raw", pair.publicKey)) as ArrayBuffer);
  return {
    key: b64url(raw),
    ip,
    async sign(message: string) {
      const sig = await crypto.subtle.sign({ name: "Ed25519" }, pair.privateKey, new TextEncoder().encode(message));
      return b64url(new Uint8Array(sig));
    },
  };
}

type Body = Uint8Array | Record<string, unknown> | undefined;

function bytesOf(body: Body): Uint8Array {
  if (body === undefined) return new Uint8Array(0);
  if (body instanceof Uint8Array) return body;
  return new TextEncoder().encode(JSON.stringify(body));
}

/** A request signed the way the app signs one: METHOD|path?query|ts|hex(sha256(body)). */
export async function signed(who: Device, method: string, path: string, body?: Body, { ts = Math.floor(Date.now() / 1000) } = {}) {
  const bytes = bytesOf(body);
  const message = `${method}|${path}|${ts}|${await sha256Hex(bytes)}`;
  return new Request(`${BASE}${path}`, {
    method,
    headers: {
      "X-FS-Key": who.key,
      "X-FS-Ts": String(ts),
      "X-FS-Sig": await who.sign(message),
      "CF-Connecting-IP": who.ip,
      ...(body === undefined ? {} : { "Content-Type": body instanceof Uint8Array ? "application/octet-stream" : "application/json" }),
    },
    body: method === "GET" || body === undefined ? undefined : bytes,
  });
}

/** Stubs Cloudflare's siteverify: a token of "pass" passes for whatever nonce the page was given. */
export function stubTurnstile(result: Partial<{ success: boolean; action: string; hostname: string; cdata: string }> = {}) {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    if (!url.startsWith("https://challenges.cloudflare.com/")) return new Response("unexpected", { status: 599 });
    const sent = JSON.parse(String(init?.body ?? "{}")) as { response?: string };
    const nonce = sent.response?.startsWith("pass:") ? sent.response.slice(5) : "";
    return Response.json({
      success: sent.response?.startsWith("pass:") ?? false,
      action: "verify",
      hostname: "community.test",
      cdata: nonce,
      ...result,
    });
  });
}

/** The verification link's fields, signed by `who` as the app signs them. */
export async function verifyLink(who: Device, handle: string, { ts = Math.floor(Date.now() / 1000), nonce = b64url(crypto.getRandomValues(new Uint8Array(16))) } = {}) {
  const t = String(ts);
  return { k: who.key, n: nonce, t, h: handle, s: await who.sign(`verify|${who.key}|${nonce}|${t}|${handle}`) };
}

export function postJson(path: string, body: unknown, ip = freshIp()): Request {
  return new Request(`${BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "CF-Connecting-IP": ip },
    body: JSON.stringify(body),
  });
}

/** Verifies `who` under `handle`, stubbing the challenge, and returns the handle it got. */
export async function verify(who: Device, handle: string): Promise<string> {
  const stub = stubTurnstile();
  try {
    const link = await verifyLink(who, handle);
    const response = await call(postJson("/v1/keys/verify", { ...link, token: `pass:${link.n}` }, who.ip));
    if (!response.ok) throw new Error(`verify failed: ${response.status} ${await response.text()}`);
    return ((await response.json()) as { handle: string }).handle;
  } finally {
    stub.mockRestore();
  }
}

// ---- pictures made of headers ----

const u32 = (n: number) => [(n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255];
const u32le = (n: number) => [n & 255, (n >>> 8) & 255, (n >>> 16) & 255, (n >>> 24) & 255];
const ascii = (s: string) => [...s].map((c) => c.charCodeAt(0));

function chunk(type: string, data: number[]): number[] {
  return [...u32(data.length), ...ascii(type), ...data, 0, 0, 0, 0];
}

/** A PNG's signature and chunks; `salt` makes pictures of the same size different files. */
export function png(width: number, height: number, { animated = false, salt = 0 } = {}): Uint8Array {
  return new Uint8Array([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ...chunk("IHDR", [...u32(width), ...u32(height), 8, 6, 0, 0, 0]),
    ...(animated ? chunk("acTL", [...u32(2), ...u32(0)]) : []),
    ...chunk("tEXt", ascii(`salt ${salt}`)),
    ...chunk("IDAT", [1, 2, 3, 4]),
    ...chunk("IEND", []),
  ]);
}

export function jpeg(width: number, height: number): Uint8Array {
  return new Uint8Array([
    0xff, 0xd8,
    0xff, 0xe0, 0, 16, ...ascii("JFIF"), 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
    0xff, 0xc0, 0, 17, 8, (height >> 8) & 255, height & 255, (width >> 8) & 255, width & 255, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1,
    0xff, 0xda, 0, 12, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 0x3f, 0,
    0x00, 0xff, 0xd9,
  ]);
}

/** A RIFF chunk as WebP has them: its type, its length (little-endian), its data and a byte to make it even. */
function riff(type: string, data: number[]): number[] {
  return [...ascii(type), ...u32le(data.length), ...data, ...(data.length & 1 ? [0] : [])];
}

const webpFile = (chunks: number[]) => new Uint8Array([...ascii("RIFF"), ...u32le(4 + chunks.length), ...ascii("WEBP"), ...chunks]);

/** A lossless picture's data: the signature byte, then width - 1 and height - 1 in 14 bits each. */
const vp8l = (width: number, height: number) => [0x2f, ...u32le((width - 1) | ((height - 1) << 14)), 0, 0, 0];

/** A lossy frame's data: a frame tag, the start code, then the width and height in 14 bits each. */
const vp8 = (width: number, height: number) => [0x10, 0x02, 0x00, 0x9d, 0x01, 0x2a, width & 255, (width >> 8) & 0x3f, height & 255, (height >> 8) & 0x3f, 0, 0];

/**
 * An extended WebP (VP8X): the canvas size, then the picture as `image` has it: lossless (a VP8L
 * chunk), lossy with an alpha channel (ALPH and "VP8 " chunks) or missing.
 */
export function webpExtended(width: number, height: number, { animated = false, image = "VP8L" as "VP8L" | "VP8 " | "none" } = {}): Uint8Array {
  const w = width - 1;
  const h = height - 1;
  const vp8x = riff("VP8X", [animated ? 0x02 : 0, 0, 0, 0, w & 255, (w >> 8) & 255, (w >> 16) & 255, h & 255, (h >> 8) & 255, (h >> 16) & 255]);
  const anim = animated ? riff("ANIM", [0, 0, 0, 0, 0, 0]) : [];
  const picture = image === "VP8L" ? riff("VP8L", vp8l(width, height)) : image === "VP8 " ? [...riff("ALPH", [0, 1, 2]), ...riff("VP8 ", vp8(width, height))] : [];
  return webpFile([...vp8x, ...anim, ...picture]);
}

/** A simple lossless WebP: one VP8L chunk. */
export function webpLossless(width: number, height: number): Uint8Array {
  return webpFile(riff("VP8L", vp8l(width, height)));
}

/** A simple lossy WebP: one "VP8 " chunk. */
export function webpLossy(width: number, height: number): Uint8Array {
  return webpFile(riff("VP8 ", vp8(width, height)));
}

// ---- a whole submission ----

export type Picture = { file: string; bytes: Uint8Array };

export function pictures(n: number, salt = 0): Picture[] {
  return Array.from({ length: n }, (_, i) => ({ file: `skin-${i + 1}.png`, bytes: png(512, 512, { salt: salt * 1000 + i }) }));
}

export async function describe(list: Picture[], manifest: Partial<{ name: string; tags: string[] }> = {}) {
  return {
    manifest: {
      name: manifest.name ?? "Night prints",
      tags: manifest.tags ?? ["woodblock"],
      skins: list.map((p, i) => ({ file: p.file, name: `Skin ${i + 1}`, tags: [] })),
    },
    license: "CC0-1.0",
    source: "own",
    notes: "Drawn by me.",
    terms_version: TERMS_VERSION,
    items: await Promise.all(list.map(async (p) => ({ file: p.file, sha256: await sha256Hex(p.bytes), bytes: p.bytes.length, width: 512, height: 512 }))),
  };
}

/** Opens, uploads and finalises a pack, returning its submission id. */
export async function submit(who: Device, list: Picture[], manifest: Partial<{ name: string; tags: string[] }> = {}, overrides: Partial<Env> = {}) {
  const created = await call(await signed(who, "POST", "/v1/submissions", await describe(list, manifest)), overrides);
  if (created.status !== 201) throw new Error(`create failed: ${created.status} ${await created.text()}`);
  const { submission_id: id, sheets } = (await created.json()) as { submission_id: string; sheets: number };
  for (const p of list) {
    const sha = await sha256Hex(p.bytes);
    const put = await call(await signed(who, "PUT", `/v1/submissions/${id}/items/${sha}`, p.bytes), overrides);
    if (!put.ok) throw new Error(`upload failed: ${put.status} ${await put.text()}`);
  }
  for (let n = 0; n < sheets; n++) {
    const put = await call(await signed(who, "PUT", `/v1/submissions/${id}/sheets/${n}`, jpeg(768, 768)), overrides);
    if (!put.ok) throw new Error(`sheet failed: ${put.status} ${await put.text()}`);
  }
  const done = await call(await signed(who, "POST", `/v1/submissions/${id}/finalize`, {}), overrides);
  if (!done.ok) throw new Error(`finalize failed: ${done.status} ${await done.text()}`);
  return id;
}

export async function errorOf(response: Response): Promise<{ code: string; message: string; retry_after?: number }> {
  return ((await response.json()) as { error: { code: string; message: string; retry_after?: number } }).error;
}

/**
 * Makes the next pack ids end in the characters these bytes stand for, one list of six for each
 * id newPackId draws, in turn. Only draws of six bytes are touched; every other use of the random
 * source, and every draw after the last list, gets real random bytes.
 */
export function drawSuffixes(...draws: number[][]) {
  const real = crypto.getRandomValues.bind(crypto);
  const left = [...draws];
  const draw: typeof crypto.getRandomValues = (array) => {
    if (array instanceof Uint8Array && array.length === 6 && left.length > 0) {
      array.set(left.shift()!);
      return array;
    }
    return real(array);
  };
  return vi.spyOn(crypto, "getRandomValues").mockImplementation(draw);
}

/** Records the SQL statements handed to D1, to count what a request costs. */
export function countingDb(db: D1Database): { db: D1Database; statements: string[] } {
  const statements: string[] = [];
  const counted = new Proxy(db, {
    get(target, property) {
      if (property === "prepare") {
        return (sql: string) => {
          statements.push(sql);
          return target.prepare(sql);
        };
      }
      const value = Reflect.get(target, property);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  return { db: counted, statements };
}
