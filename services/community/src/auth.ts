/**
 * Signed requests. Every write, and every read of someone's own data, is signed by the Ed25519
 * key of the computer sending it:
 *
 *   X-FS-Key  the public key, 32 bytes as base64url
 *   X-FS-Ts   unix seconds when it was signed
 *   X-FS-Sig  the signature, 64 bytes as base64url, over
 *             METHOD|/path?query|ts|hex(sha256(body))
 *
 * The time keeps a captured request from being any use for more than five minutes either side,
 * and every request that changes something is remembered for as long as it could be accepted, so
 * one can't be sent twice. The verification link from the app is signed the same
 * way over a message that starts with "verify|", which no method name does, so neither signature
 * can stand in for the other. crates/folderskin-share makes these signatures in Rust.
 *
 * A body too large to read here (a catalog file, up to 64 MB) is signed the same way, with its
 * SHA-256 also sent as X-Content-SHA256: the signature is checked against that, and whatever
 * stores the body checks the bytes against it, so the Worker never reads or hashes them
 * (verifySignedDigest).
 */
import { fromB64url, sha256Hex, now } from "./bytes";
import type { Env } from "./env";
import { fail, readBody } from "./http";
import { CLOCK_SKEW_SECONDS } from "./limits";
import type { Tier } from "./limits";

export type SignedRequest = {
  /** The signer's public key, base64url. */
  key: string;
  body: Uint8Array;
  /** hex(sha256(body)). */
  digest: string;
};

export type Account = { key: string; handle: string; tier: Tier; approved: number; rejected: number };

const KEY = /^[A-Za-z0-9_-]{43}$/;
const SIG = /^[A-Za-z0-9_-]{86}$/;

export const isKey = (value: unknown): value is string => typeof value === "string" && KEY.test(value) && fromB64url(value)?.length === 32;

/** Checks an Ed25519 signature; false for a key or signature that doesn't even decode. */
export async function verifySignature(key: string, sig: string, message: string): Promise<boolean> {
  const raw = fromB64url(key);
  const signature = fromB64url(sig);
  if (!raw || raw.length !== 32 || !signature || signature.length !== 64) return false;
  try {
    const publicKey = await crypto.subtle.importKey("raw", raw, { name: "Ed25519" }, false, ["verify"]);
    return await crypto.subtle.verify({ name: "Ed25519" }, publicKey, signature, new TextEncoder().encode(message));
  } catch {
    return false;
  }
}

/**
 * The request's signer, once its signature, its time, who it is and (for anything but a read) its
 * freshness have been checked. The body is read here, up to `maxBody` bytes, since the signature
 * covers it.
 *
 * `allow` says whether the signer may use this endpoint at all (a verified computer, say, or the
 * maintainer), and hands back what it looked up. It runs before the request is remembered: anyone
 * can make a key and sign with it, so a key that is turned away anyway mustn't cost a database
 * write, or junk requests could spend the day's writes and stop sharing for everyone.
 */
export async function verifySigned<T>(
  request: Request,
  env: Env,
  maxBody: number,
  allow: (key: string) => Promise<T> | T,
): Promise<SignedRequest & { signer: T }> {
  const head = signedHeaders(request);
  // Only once the headers are in order: an unsigned upload is turned away before it is read.
  const body = await readBody(request, maxBody);
  const digest = await sha256Hex(body);
  const signer = await accept(request, env, head, digest, allow);
  return { key: head.key, body, digest, signer };
}

/**
 * Like `verifySigned`, for a body too large to read here: the signature is checked against the
 * SHA-256 that X-Content-SHA256 declares, and the body is left unread for the caller to stream to
 * storage that holds it to that digest (R2's `sha256`). A body that isn't the one signed for is
 * then turned away by the storage, as surely as by a hash worked out here.
 */
export async function verifySignedDigest<T>(
  request: Request,
  env: Env,
  allow: (key: string) => Promise<T> | T,
): Promise<{ key: string; digest: string; signer: T }> {
  const head = signedHeaders(request);
  const digest = request.headers.get("X-Content-SHA256") ?? "";
  if (!/^[0-9a-f]{64}$/.test(digest)) {
    throw fail(400, "bad_digest", "Say the body's SHA-256 in X-Content-SHA256, as 64 lower-case hex digits.");
  }
  const signer = await accept(request, env, head, digest, allow);
  return { key: head.key, digest, signer };
}

type SignedHeaders = { key: string; sig: string; ts: number };

/** The signing headers, once they are all there and the time in them is near enough to ours. */
function signedHeaders(request: Request): SignedHeaders {
  const key = request.headers.get("X-FS-Key") ?? "";
  const sig = request.headers.get("X-FS-Sig") ?? "";
  const tsText = request.headers.get("X-FS-Ts") ?? "";
  if (!KEY.test(key) || !SIG.test(sig) || !/^\d{1,12}$/.test(tsText)) {
    throw fail(401, "unsigned", "That request wasn't signed by FolderSkin. Update the app and try again.");
  }
  const ts = Number(tsText);
  const time = now();
  if (Math.abs(time - ts) > CLOCK_SKEW_SECONDS) {
    // The app sets its clock by this header and tries once more.
    throw fail(401, "clock", "Your computer's clock is more than five minutes out. Set it to the right time and try again.", {
      "X-FS-Time": String(time),
    });
  }
  return { key, sig, ts };
}

/** Checks the signature over the request and `digest`, asks `allow`, and remembers a request that changes something. */
async function accept<T>(request: Request, env: Env, { key, sig, ts }: SignedHeaders, digest: string, allow: (key: string) => Promise<T> | T): Promise<T> {
  const url = new URL(request.url);
  const message = `${request.method}|${url.pathname}${url.search}|${ts}|${digest}`;
  if (!(await verifySignature(key, sig, message))) {
    throw fail(401, "bad_signature", "That request wasn't signed by this computer's key.");
  }
  const signer = await allow(key);
  if (request.method !== "GET" && request.method !== "HEAD") {
    // What was signed is remembered rather than the signature, so a second signature over the
    // same request (Ed25519 allows none, but nothing here depends on that) is still a repeat.
    // Kept until the request would fail the clock check anyway.
    const fresh = await env.DB.prepare("INSERT INTO seen (id, expires) VALUES (?1, ?2) ON CONFLICT (id) DO NOTHING")
      .bind(`req:${await sha256Hex(`${key}|${message}`)}`, ts + CLOCK_SKEW_SECONDS + 60)
      .run();
    if (fresh.meta.changes !== 1) throw fail(409, "replayed", "That request was already received.");
  }
  return signer;
}

/** For `verifySigned` on reads of a key's own data, which any key may ask for. */
export const anyone = () => undefined;

/** The verified computer behind `key`. Unknown and banned keys go no further. */
export async function requireAccount(env: Env, key: string): Promise<Account> {
  const account = await env.DB.prepare("SELECT key, handle, tier, approved, rejected FROM keys WHERE key = ?1")
    .bind(key)
    .first<Account>();
  return allowedAccount(account);
}

/** `account` when it may act at all: a key that isn't verified, or is banned for good, is turned away. */
export function allowedAccount(account: Account | null): Account {
  if (!account) throw fail(403, "not_verified", "This computer hasn't been verified yet. Verify it in FolderSkin first.");
  if (account.tier === "banned") {
    throw fail(403, "banned", "This computer can't share packs any more, because a pack from it broke the pack terms.");
  }
  return account;
}

/** Whether `key` is one of the maintainer's, from ADMIN_KEYS. */
export function isAdmin(env: Env, key: string): boolean {
  return (env.ADMIN_KEYS ?? "")
    .split(",")
    .map((k) => k.trim())
    .some((k) => k.length > 0 && k === key);
}

export function requireAdmin(env: Env, key: string): void {
  // The same answer as a route that doesn't exist: the admin surface isn't advertised.
  if (!isAdmin(env, key)) throw fail(404, "not_found", "There's nothing here.");
}
