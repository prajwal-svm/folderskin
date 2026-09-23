/**
 * Single-use links for acting from a phone: review a pack, take one down, pause uploads. They are
 * what the notifications carry, and the way to act in an emergency without the maintainer's
 * signing key, which stays on their own computer.
 *
 * A link is `/l/<action>.<subject>.<expires>.<nonce>.<tag>`, the tag an HMAC-SHA256 under
 * LINK_SECRET. Opening it (GET) only shows a page saying what it will do, so a chat app fetching
 * a preview of it does nothing; pressing the button (POST) does it, once. The nonce is remembered
 * when it is used, so the same link can't act twice, and every link runs out.
 */
import { b64url, fromB64url, hmac, hmacVerify, now } from "./bytes";
import type { Env } from "./env";

export type LinkAction = "review" | "takedown" | "pause";

/** How long each kind of link works. */
const LIFETIME: Record<LinkAction, number> = { review: 7 * 86400, takedown: 3 * 86400, pause: 3 * 86400 };

export type Link = { action: LinkAction; subject: string; expires: number; nonce: string; token: string };

const TOKEN = /^(review|takedown|pause)\.([a-z0-9_-]{1,64})\.(\d{1,12})\.([A-Za-z0-9_-]{22})\.([A-Za-z0-9_-]{43})$/;

const signed = (payload: string) => `link|${payload}`;

/** A link for `action` on `subject`, or null when the service has no LINK_SECRET or address to make one with. */
export async function makeLink(env: Env, action: LinkAction, subject: string, at = now()): Promise<string | null> {
  const base = env.PUBLIC_BASE_URL?.trim().replace(/\/+$/, "");
  if (!env.LINK_SECRET || !base) return null;
  const nonce = b64url(crypto.getRandomValues(new Uint8Array(16)));
  const payload = `${action}.${subject || "-"}.${at + LIFETIME[action]}.${nonce}`;
  const tag = b64url(await hmac(env.LINK_SECRET, signed(payload)));
  return `${base}/l/${payload}.${tag}`;
}

/** The link `token` stands for, if its tag is right and it hasn't run out. Whether it was used is `linkUsed`. */
export async function readLink(env: Env, token: string, at = now()): Promise<Link | null> {
  const m = TOKEN.exec(token);
  if (!m || !env.LINK_SECRET) return null;
  const [, action, subject, expires, nonce, tag] = m;
  const raw = fromB64url(tag);
  if (!raw || raw.length !== 32) return null;
  const payload = `${action}.${subject}.${expires}.${nonce}`;
  if (!(await hmacVerify(env.LINK_SECRET, signed(payload), raw))) return null;
  if (Number(expires) < at) return null;
  return { action: action as LinkAction, subject: subject === "-" ? "" : subject, expires: Number(expires), nonce, token };
}

export async function linkUsed(env: Env, link: Link): Promise<boolean> {
  const row = await env.DB.prepare("SELECT 1 AS used FROM seen WHERE id = ?1").bind(`link:${link.nonce}`).first();
  return row !== null;
}

/** Marks a link used. False when it already was, by this request or another at the same moment. */
export async function useLink(env: Env, link: Link): Promise<boolean> {
  const result = await env.DB.prepare("INSERT INTO seen (id, expires) VALUES (?1, ?2) ON CONFLICT (id) DO NOTHING")
    .bind(`link:${link.nonce}`, link.expires + 60)
    .run();
  return result.meta.changes === 1;
}
