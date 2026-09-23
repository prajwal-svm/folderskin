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
 * The request's signer, once its signature, its time and (for anything but a read) its freshness
 * have been checked. The body is read here, up to `maxBody` bytes, since the signature covers it.
 */
export async function verifySigned(request: Request, env: Env, maxBody: number): Promise<SignedRequest> {
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
  // Only once the headers are in order: an unsigned upload is turned away before it is read.
  const body = await readBody(request, maxBody);
  const url = new URL(request.url);
  const digest = await sha256Hex(body);
  const message = `${request.method}|${url.pathname}${url.search}|${ts}|${digest}`;
  if (!(await verifySignature(key, sig, message))) {
    throw fail(401, "bad_signature", "That request wasn't signed by this computer's key.");
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    // What was signed is remembered rather than the signature, so a second signature over the
    // same request (Ed25519 allows none, but nothing here depends on that) is still a repeat.
    // Kept until the request would fail the clock check anyway.
    const fresh = await env.DB.prepare("INSERT INTO seen (id, expires) VALUES (?1, ?2) ON CONFLICT (id) DO NOTHING")
      .bind(`req:${await sha256Hex(`${key}|${message}`)}`, ts + CLOCK_SKEW_SECONDS + 60)
      .run();
    if (fresh.meta.changes !== 1) throw fail(409, "replayed", "That request was already received.");
  }
  return { key, body, digest };
}

/** The verified computer behind `key`. Unknown and banned keys go no further. */
export async function requireAccount(env: Env, key: string): Promise<Account> {
  const account = await env.DB.prepare("SELECT key, handle, tier, approved, rejected FROM keys WHERE key = ?1")
    .bind(key)
    .first<Account>();
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

export function requireAdmin(env: Env, signed: SignedRequest): void {
  // The same answer as a route that doesn't exist: the admin surface isn't advertised.
  if (!isAdmin(env, signed.key)) throw fail(404, "not_found", "There's nothing here.");
}
