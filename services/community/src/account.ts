/**
 * Verifying a computer, and what it can see about itself.
 *
 * Turnstile can't run inside the app (its pages come from tauri://, which Turnstile won't serve), so
 * the app signs a link with the computer's key and opens it in the browser:
 *
 *   /verify?k=<key>&n=<nonce>&t=<time>&h=<handle>&s=<signature over "verify|k|n|t|h">
 *
 * The page runs the challenge with the nonce as its cData and posts everything back. Here the
 * signature proves the link came from the holder of the key, the challenge's answer is checked
 * with Cloudflare and must carry the same nonce, the nonce is spent, and the key is recorded as
 * verified under the handle. The app meanwhile asks GET /v1/me until it says so.
 */
import { anyone, isKey, requireAccount, verifySignature, verifySigned } from "./auth";
import { dayOf, now } from "./bytes";
import type { Env } from "./env";
import { fail, json, parseJson, readBody } from "./http";
import { networkHash } from "./ip";
import {
  CLOCK_SKEW_SECONDS,
  HANDLE_CHANGES_PER_DAY,
  LICENSES,
  MAX_JSON_BYTES,
  MAX_SKINS,
  PER_NETWORK,
  TERMS_VERSION,
  TIERS,
  VERIFY_LINK_SECONDS,
} from "./limits";
import type { Tier } from "./limits";
import { giveBack, pauseState, requireAccepting, takeAll, used, type Take } from "./quota";
import { handleProblem } from "./text";

const NONCE = /^[A-Za-z0-9_-]{22,43}$/;
export const SITEVERIFY = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/** What the app needs to know before it offers sharing without GitHub at all. */
export async function status(env: Env): Promise<Response> {
  const pause = await pauseState(env);
  return json(
    {
      accepting: !pause.paused,
      message: pause.paused ? pause.message || "Sharing without GitHub is paused for now." : "",
      terms_version: TERMS_VERSION,
      licenses: LICENSES,
      max_pictures: MAX_SKINS,
    },
    200,
    // Asked when the share dialog opens; a minute old is fresh enough, and spares the database.
    { "Cache-Control": "public, max-age=60" },
  );
}

/** Checks the person check, and records the computer as verified. */
export async function verifyKey(request: Request, env: Env): Promise<Response> {
  const body = parseJson(await readBody(request, 4096));
  const { k, n, t, h, s, token } = body;
  if (!isKey(k) || typeof n !== "string" || !NONCE.test(n) || typeof t !== "string" || !/^\d{1,12}$/.test(t)) {
    throw fail(400, "bad_link", "This link is incomplete. Start again from FolderSkin.");
  }
  if (typeof s !== "string" || typeof token !== "string" || token.length === 0 || token.length > 4096) {
    throw fail(400, "bad_link", "This link is incomplete. Start again from FolderSkin.");
  }
  const handle = typeof h === "string" ? h : "";
  const problem = handleProblem(handle);
  if (problem) throw fail(400, "bad_handle", problem);
  const at = now();
  const made = Number(t);
  // The app sets its clock by the service's before it signs a link, so a link from the future
  // means the computer's clock is out and the app is too old to correct it.
  if (made > at + CLOCK_SKEW_SECONDS) {
    throw fail(400, "clock", "Your computer's clock is more than five minutes out. Set it to the right time and start again from FolderSkin.");
  }
  if (at - made > VERIFY_LINK_SECONDS) throw fail(400, "link_expired", "This link has run out. Start again from FolderSkin.");
  if (!(await verifySignature(k, s, `verify|${k}|${n}|${t}|${handle}`))) {
    throw fail(400, "bad_link", "This link wasn't made by FolderSkin on your computer. Start again from FolderSkin.");
  }
  await requireAccepting(env);
  const network = await networkHash(env, request, at);
  const quota: Take = {
    scope: "net:verify",
    id: network,
    amount: 1,
    limit: PER_NETWORK.verifications,
    error: fail(429, "quota", "Too many computers on your network were verified today. Please try again tomorrow."),
  };
  // Only a verification that goes through counts against the network, so a failed check, a
  // reloaded page or someone else on a shared network trying and failing can't use up the day.
  // A network that has had its fill is turned away before Cloudflare is asked, though.
  if ((await used(env, quota.scope, quota.id, at)) >= quota.limit) throw quota.error;
  await checkChallenge(env, request, token, n);
  await takeAll(env, [quota], at);
  // Spent only now, so a challenge that failed can be tried again from the same link.
  const spent = await env.DB.prepare("INSERT INTO seen (id, expires) VALUES (?1, ?2) ON CONFLICT (id) DO NOTHING")
    .bind(`verify:${n}`, made + VERIFY_LINK_SECONDS + 600)
    .run();
  if (spent.meta.changes !== 1) {
    await giveBack(env, dayOf(at), quota.scope, quota.id, quota.amount);
    throw fail(409, "link_used", "This link has been used already. Start again from FolderSkin.");
  }

  const existing = await env.DB.prepare("SELECT handle, tier FROM keys WHERE key = ?1").bind(k).first<{ handle: string; tier: Tier }>();
  if (existing) {
    if (existing.tier === "banned") throw fail(403, "banned", "This computer can't share packs any more.");
    await env.DB.prepare("UPDATE keys SET verified_at = ?2 WHERE key = ?1").bind(k, at).run();
    return json({ handle: existing.handle });
  }
  const given = await claimHandle(env, k, handle, at);
  return json({ handle: given }, 201);
}

/** Asks Cloudflare whether the challenge was passed on this page, for this nonce. */
async function checkChallenge(env: Env, request: Request, token: string, nonce: string): Promise<void> {
  if (!env.TURNSTILE_SECRET) throw fail(503, "not_configured", "FolderSkin's sharing service isn't set up yet. Please try again later.");
  let answer: { success?: boolean; action?: string; cdata?: string; hostname?: string };
  try {
    const response = await fetch(SITEVERIFY, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        secret: env.TURNSTILE_SECRET,
        response: token,
        remoteip: request.headers.get("CF-Connecting-IP") ?? undefined,
      }),
      signal: AbortSignal.timeout(8000),
    });
    answer = (await response.json()) as typeof answer;
  } catch {
    throw fail(503, "challenge_unavailable", "The check couldn't be confirmed just now. Reload the page and try again.");
  }
  const host = new URL(request.url).hostname;
  // Cloudflare's always-passes test secret answers for a made-up page, so with it (in `wrangler
  // dev`) only success is checked. It lets anyone through anyway, so this opens nothing new.
  const testing = env.TURNSTILE_SECRET === TEST_SECRET;
  const bound = answer.action === "verify" && answer.cdata === nonce && answer.hostname === host;
  if (answer.success !== true || (!bound && !testing)) {
    throw fail(403, "challenge_failed", "The check didn't go through. Reload the page and try again.");
  }
}

/** Turnstile's documented test secret that always passes. */
const TEST_SECRET = "1x0000000000000000000000000000000AA";

/**
 * Records a new key under `wanted`, or under the first free `wanted-2`, `wanted-3`… if someone
 * has it already. The app shows whichever it got.
 */
async function claimHandle(env: Env, key: string, wanted: string, at: number): Promise<string> {
  for (let n = 1; n <= 20; n++) {
    const suffix = n === 1 ? "" : `-${n}`;
    const handle = `${wanted.slice(0, 39 - suffix.length).replace(/-+$/, "")}${suffix}`;
    const added = await env.DB.prepare(
      "INSERT INTO keys (key, handle, tier, verified_at) VALUES (?1, ?2, 'probation', ?3) ON CONFLICT DO NOTHING",
    )
      .bind(key, handle, at)
      .run();
    if (added.meta.changes === 1) return handle;
    const mine = await env.DB.prepare("SELECT handle FROM keys WHERE key = ?1").bind(key).first<{ handle: string }>();
    if (mine) return mine.handle;
  }
  throw fail(409, "handle_taken", "That name and the ones like it are taken. Choose another in FolderSkin and start again.");
}

/** Who this computer is to the service, and what it has left today. Unverified keys get `verified: false`. */
export async function me(request: Request, env: Env): Promise<Response> {
  const { key } = await verifySigned(request, env, 0, anyone);
  const account = await env.DB.prepare("SELECT handle, tier FROM keys WHERE key = ?1").bind(key).first<{ handle: string; tier: Tier }>();
  const pause = await pauseState(env);
  if (!account) return json({ verified: false, accepting: !pause.paused });
  const limits = TIERS[account.tier];
  return json({
    verified: true,
    handle: account.handle,
    tier: account.tier,
    accepting: !pause.paused,
    today: {
      submissions: await used(env, "key:submissions", key),
      pictures: await used(env, "key:pictures", key),
    },
    limits,
  });
}

/** Changes the name this computer's packs are credited to. Packs already published keep the old one. */
export async function rename(request: Request, env: Env): Promise<Response> {
  const signed = await verifySigned(request, env, MAX_JSON_BYTES, (key) => requireAccount(env, key));
  const account = signed.signer;
  const { handle } = parseJson(signed.body);
  const problem = handleProblem(handle);
  if (problem) throw fail(400, "bad_handle", problem);
  if (typeof handle !== "string" || handle === account.handle) return json({ handle: account.handle });
  await takeAll(env, [
    {
      scope: "key:handles",
      id: signed.key,
      amount: 1,
      limit: HANDLE_CHANGES_PER_DAY,
      error: fail(429, "quota", "You've changed your name enough for one day. Try again tomorrow."),
    },
  ]);
  try {
    await env.DB.prepare("UPDATE keys SET handle = ?2 WHERE key = ?1").bind(signed.key, handle).run();
  } catch (e) {
    if (e instanceof Error && /UNIQUE/i.test(e.message)) throw fail(409, "handle_taken", "Someone already uses that name. Try another.");
    throw e;
  }
  return json({ handle });
}
