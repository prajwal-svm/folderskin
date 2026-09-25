/**
 * Backing off and bans, on top of the daily quotas, the waiting cap and the burst limit: what a
 * device key or a network earns by being turned away. Each has a row in `penalties` under its
 * subject, "key:<device key>" or "net:<hash>" (penaltyNetwork in ip.ts).
 *
 * - Strikes. Every sharing request that is refused is a strike on its key and on its network: a
 *   quota spent, the waiting cap, the burst limit, or a request while cooling down. Sharing then
 *   waits min(60 s × 2^(strikes − 1), 24 h) from the latest strike, and every sharing request in
 *   that time is answered 429 cooling_down, and is a strike itself. Strikes start again from
 *   nothing after a day without one.
 * - Marks. Each pack the maintainer turns down or takes down is a mark on its key, kept 30 days.
 *   The third within 30 days bans the key for 30 days.
 * - Abuse. A pack turned down as abuse (the maintainer ticks "ban", or a reason is sexual, minor or
 *   hate) bans its key for good, with the tier, and the network it was sent from for 30 days.
 * - Networks. The second key banned from one network within 30 days bans the network for 30
 *   days. A banned network can't verify new keys, so a ban can't be dodged with a fresh key, and
 *   can't share.
 *
 * Checking a request takes one statement, the account's own, and refusing it writes at most one
 * more. The daily run deletes what has run out, and the maintainer can lift a key's or a network's
 * ban (admin.ts). Every number is in limits.ts.
 */
import { allowedAccount, type Account } from "./auth";
import { now } from "./bytes";
import type { Env } from "./env";
import { fail, type HttpError } from "./http";
import { penaltyNetwork } from "./ip";
import {
  BANNED_KEYS_SECONDS,
  BANNED_KEYS_TO_BAN_NETWORK,
  COOLDOWN_BASE_SECONDS,
  COOLDOWN_MAX_SECONDS,
  KEY_BAN_SECONDS,
  MARK_SECONDS,
  MARKS_TO_BAN,
  NETWORK_BAN_SECONDS,
  STRIKES_RESET_SECONDS,
  SUBMISSION_NETWORK_SECONDS,
  type Tier,
} from "./limits";
import { record } from "./notify";

const keySubject = (key: string) => `key:${key}`;
const netSubject = (network: string) => `net:${network}`;

/** A verified computer that may share right now, and the network it sends from, as penalties hash it. */
export type Sharer = Account & { network: string };

/**
 * The verified computer behind `key`, for every endpoint that shares a pack, once neither it nor
 * the network the request comes from is banned or cooling down. A request while cooling down is a
 * strike on both, which makes the wait longer.
 *
 * The account and whatever holds it or its network back are read in the one statement, so an
 * allowed request costs no more than the account alone did, and a refused one a strike besides.
 */
export async function requireSharer(env: Env, request: Request, key: string, at = now()): Promise<Sharer> {
  const network = await penaltyNetwork(env, request);
  const subjects = [keySubject(key), netSubject(network)];
  const { results } = await env.DB.prepare(
    `SELECT k.key, k.handle, k.tier, k.approved, k.rejected, p.subject, p.cool_until, p.banned_until
     FROM keys k LEFT JOIN penalties p ON p.subject IN (?2, ?3) AND (p.cool_until > ?4 OR p.banned_until > ?4)
     WHERE k.key = ?1`,
  )
    .bind(key, subjects[0], subjects[1], at)
    .all<Account & { subject: string | null; cool_until: number | null; banned_until: number | null }>();
  // One row for each penalty that holds, or one with no penalty at all; the account is in each.
  const [first] = results;
  const account = allowedAccount(
    first ? { key: first.key, handle: first.handle, tier: first.tier, approved: first.approved, rejected: first.rejected } : null,
  );
  const held = results.filter((r) => r.subject !== null);
  const bans = held.filter((r) => (r.banned_until ?? 0) > at).sort((a, b) => (b.banned_until ?? 0) - (a.banned_until ?? 0));
  if (bans.length > 0) throw banned(bans[0].subject ?? "", (bans[0].banned_until ?? 0) - at, "share");
  if (held.length > 0) {
    const until = Math.max(await strike(env, subjects, at), ...held.map((r) => r.cool_until ?? 0));
    throw coolingDown(until - at);
  }
  return { ...account, network };
}

/** Strikes the key and network behind a refused sharing request, and hands the refusal back to throw. */
export async function refused(env: Env, sharer: Sharer, error: HttpError, at = now()): Promise<HttpError> {
  await strike(env, [keySubject(sharer.key), netSubject(sharer.network)], at);
  return error;
}

/**
 * Strikes the network of a sharing request the burst limit turned away. Not its key: the limit is
 * checked before the request's signature, so the key it names can't be trusted yet. Nor a network
 * that is cooling down already, since in a flood that would be a database write for every one of
 * the requests the limit is there to keep off the database. Never throws.
 */
export async function burstStrike(env: Env, request: Request, at = now()): Promise<void> {
  try {
    await strike(env, [netSubject(await penaltyNetwork(env, request))], at, true);
  } catch {
    console.error("folderskin-community: couldn't strike a network for its burst");
  }
}

/**
 * Strikes `subjects` at `at`, and answers when the longest wait among the ones it wrote now ends
 * (0 when it wrote none). A subject's wait is min(60 s × 2^(strikes − 1), 24 h) from this strike,
 * and never ends sooner than the one it had; a strike a day or more after the one before counts
 * as the first. One statement, and it never throws: a refusal is answered whether the strike was
 * kept or not.
 *
 * Two kinds of strike aren't written, so a flood can't become a write for each request: one that
 * wouldn't lengthen a wait already as long as it gets by at least the first wait (a minute), and,
 * with `unlessCooling`, one on a subject that is cooling down already.
 */
async function strike(env: Env, subjects: string[], at: number, unlessCooling = false): Promise<number> {
  // ?1 now, ?2 the first wait, ?3 the longest, ?4 how long strikes last, ?5 unlessCooling, then the subjects.
  const rows = subjects.map((_, i) => `(?${i + 6}, 1, ?1, ?1 + ?2)`).join(", ");
  try {
    const { results } = await env.DB.prepare(
      `INSERT INTO penalties (subject, strikes, struck_at, cool_until) VALUES ${rows}
       ON CONFLICT (subject) DO UPDATE SET
         strikes = CASE WHEN struck_at > ?1 - ?4 THEN strikes + 1 ELSE 1 END,
         struck_at = ?1,
         cool_until = MAX(cool_until, ?1 + MIN(?2 << MIN(CASE WHEN struck_at > ?1 - ?4 THEN strikes ELSE 0 END, 30), ?3))
       WHERE cool_until <= ?1 + ?3 - ?2 AND (?5 = 0 OR cool_until <= ?1)
       RETURNING cool_until`,
    )
      .bind(at, COOLDOWN_BASE_SECONDS, COOLDOWN_MAX_SECONDS, STRIKES_RESET_SECONDS, unlessCooling ? 1 : 0, ...subjects)
      .all<{ cool_until: number }>();
    return Math.max(0, ...results.map((r) => r.cool_until));
  } catch {
    console.error("folderskin-community: couldn't record a strike");
    return 0;
  }
}

/**
 * Turns away the verification of `key` from a banned network, or of a key banned for a while: a
 * banned network verifies no new keys, so a ban can't be dodged with a fresh one. One statement.
 */
export async function requireVerifiable(env: Env, request: Request, key: string, at = now()): Promise<void> {
  const network = await penaltyNetwork(env, request);
  const row = await env.DB.prepare(
    "SELECT subject, banned_until FROM penalties WHERE subject IN (?1, ?2) AND banned_until > ?3 ORDER BY banned_until DESC LIMIT 1",
  )
    .bind(keySubject(key), netSubject(network), at)
    .first<{ subject: string; banned_until: number }>();
  if (row) throw banned(row.subject, row.banned_until - at, "verify");
}

/** A wait in words, rounded up so that trying again then works: "a minute", "5 minutes", "2 hours", "30 days". */
export function inWords(seconds: number): string {
  const minutes = Math.ceil(seconds / 60);
  if (minutes <= 1) return "a minute";
  if (minutes < 60) return `${minutes} minutes`;
  const hours = Math.ceil(seconds / 3600);
  if (hours <= 1) return "an hour";
  if (hours <= 48) return `${hours} hours`;
  return `${Math.ceil(seconds / 86400)} days`;
}

function coolingDown(seconds: number): HttpError {
  const wait = Math.max(1, Math.ceil(seconds));
  return fail(429, "cooling_down", `That's too many tries in a row. You can share again in ${inWords(wait)}.`, { "Retry-After": String(wait) }, wait);
}

function banned(subject: string, seconds: number, doing: "share" | "verify"): HttpError {
  const wait = Math.max(1, Math.ceil(seconds));
  const when = inWords(wait);
  let message = `This computer can't share packs for now, because too many of its packs were turned down. It can share again in ${when}.`;
  if (subject.startsWith("net:")) {
    message =
      doing === "verify"
        ? `Computers on your network can't be verified for now. You can try again in ${when}.`
        : `Packs can't be shared from your network for now. You can share again in ${when}.`;
  }
  return fail(403, "banned", message, {}, wait);
}

// ---- decisions ----

/**
 * A ban a decision made, or one in force. `until` is null for a key banned for good. `reason` is
 * "abuse", "terms" (a reason that bans, such as deceptive files), "marks" (the third pack turned
 * down in 30 days), "keys" (the second key banned from a network in 30 days) or, for a key banned
 * for good when it isn't known why any more, "tier".
 */
export type Ban = { kind: "key" | "network"; id: string; until: number | null; reason: string; handle?: string | null };

/**
 * What turning a pack down costs: a mark on its key; a ban on the key, for good (`forGood`, the
 * tier, which the caller sets) or, at the third mark in 30 days, for 30 days; and a ban on the
 * network it was sent from for 30 days, for abuse or once it is the second key banned from there
 * in 30 days. A submission from before networks were kept, or from over 30 days ago, has none,
 * and then only its key pays. Answers the bans it made.
 */
export async function turnedDown(
  env: Env,
  pack: { key: string; network: string | null; submission: string; abuse: boolean; forGood: boolean },
  at = now(),
): Promise<Ban[]> {
  const bans: Ban[] = [];
  const marks = await mark(env, keySubject(pack.key), pack.submission, at, MARK_SECONDS);
  if (pack.forGood) {
    bans.push({ kind: "key", id: pack.key, until: null, reason: pack.abuse ? "abuse" : "terms" });
  } else if (marks >= MARKS_TO_BAN) {
    await ban(env, keySubject(pack.key), at + KEY_BAN_SECONDS, "marks");
    bans.push({ kind: "key", id: pack.key, until: at + KEY_BAN_SECONDS, reason: "marks" });
  }
  if (pack.network) {
    // A banned key counts against the network its pack came from, once however often it is banned.
    const keys = bans.length > 0 ? await mark(env, netSubject(pack.network), pack.key, at, BANNED_KEYS_SECONDS) : 0;
    const why = pack.abuse ? "abuse" : keys >= BANNED_KEYS_TO_BAN_NETWORK ? "keys" : null;
    if (why) {
      await ban(env, netSubject(pack.network), at + NETWORK_BAN_SECONDS, why);
      bans.push({ kind: "network", id: pack.network, until: at + NETWORK_BAN_SECONDS, reason: why });
    }
  }
  for (const b of bans) await record(env, "ban", `${b.kind === "key" ? "key" : "net"}:${b.id}`, banWords(b, at));
  return bans;
}

/** A ban in words, for the events log and the phone page. */
export function banWords(b: Ban, at = now()): string {
  const how = b.until === null ? "for good" : `for ${inWords(b.until - at)}`;
  const why: Record<string, string> = {
    abuse: b.kind === "key" ? "its pack was turned down as abuse" : "a pack turned down as abuse came from it",
    terms: "its pack broke a rule that bans",
    marks: `${MARKS_TO_BAN} of its packs were turned down within ${inWords(MARK_SECONDS)}`,
    keys: `${BANNED_KEYS_TO_BAN_NETWORK} keys from it were banned within ${inWords(BANNED_KEYS_SECONDS)}`,
    tier: "its tier is banned",
  };
  return `banned ${how}: ${why[b.reason] ?? b.reason}`;
}

/** Puts a mark on `subject` for `cause` (again, if it had one), and answers how many it has from the last `window` seconds. */
async function mark(env: Env, subject: string, cause: string, at: number, window: number): Promise<number> {
  const [, counted] = await env.DB.batch<{ n: number }>([
    env.DB.prepare("INSERT INTO marks (subject, cause, at) VALUES (?1, ?2, ?3) ON CONFLICT (subject, cause) DO UPDATE SET at = excluded.at").bind(
      subject,
      cause,
      at,
    ),
    env.DB.prepare("SELECT COUNT(*) AS n FROM marks WHERE subject = ?1 AND at > ?2").bind(subject, at - window),
  ]);
  return counted.results[0]?.n ?? 0;
}

/** Bans `subject` until `until`, or keeps the ban it has if that one lasts longer. */
async function ban(env: Env, subject: string, until: number, reason: string): Promise<void> {
  await env.DB.prepare(
    `INSERT INTO penalties (subject, banned_until, ban_reason) VALUES (?1, ?2, ?3)
     ON CONFLICT (subject) DO UPDATE SET
       ban_reason = CASE WHEN COALESCE(banned_until, 0) < excluded.banned_until THEN excluded.ban_reason ELSE ban_reason END,
       banned_until = MAX(COALESCE(banned_until, 0), excluded.banned_until)`,
  )
    .bind(subject, until, reason)
    .run();
}

// ---- the maintainer ----

/** Every ban in force, for the maintainer's queue: bans for a while, longest first, then keys banned for good by their tier. */
export async function currentBans(env: Env, at = now()): Promise<Ban[]> {
  const [forAWhile, forGood] = await env.DB.batch<{ subject?: string; key?: string; banned_until?: number; ban_reason?: string; handle: string | null }>([
    env.DB.prepare(
      `SELECT p.subject, p.banned_until, p.ban_reason, k.handle FROM penalties p
       LEFT JOIN keys k ON p.subject LIKE 'key:%' AND k.key = substr(p.subject, 5)
       WHERE p.banned_until > ?1 ORDER BY p.banned_until DESC LIMIT 100`,
    ).bind(at),
    env.DB.prepare("SELECT key, handle FROM keys WHERE tier = 'banned' ORDER BY verified_at DESC LIMIT 100"),
  ]);
  return [
    ...forAWhile.results.map((r): Ban => {
      const subject = r.subject ?? "";
      const kind = subject.startsWith("net:") ? "network" : "key";
      return { kind, id: subject.slice(4), handle: kind === "key" ? r.handle : undefined, until: r.banned_until ?? null, reason: r.ban_reason ?? "" };
    }),
    ...forGood.results.map((r): Ban => ({ kind: "key", id: r.key ?? "", handle: r.handle, until: null, reason: "tier" })),
  ];
}

/**
 * Lifts every ban on `key`, and what led to it: its ban for a while, its marks and strikes, the
 * mark it left on the network it was banned from, and a ban for good, which puts it back on
 * probation. Answers its tier now, or null when the service doesn't know the key.
 */
export async function liftKeyBan(env: Env, key: string): Promise<Tier | null> {
  const account = await env.DB.prepare("SELECT tier FROM keys WHERE key = ?1").bind(key).first<{ tier: Tier }>();
  if (!account) return null;
  await env.DB.batch([
    env.DB.prepare("DELETE FROM penalties WHERE subject = ?1").bind(keySubject(key)),
    env.DB.prepare("DELETE FROM marks WHERE subject = ?1 OR (cause = ?2 AND subject LIKE 'net:%')").bind(keySubject(key), key),
    env.DB.prepare("UPDATE keys SET tier = 'probation' WHERE key = ?1 AND tier = 'banned'").bind(key),
  ]);
  await record(env, "unban", keySubject(key), "lifted by the maintainer");
  return account.tier === "banned" ? "probation" : account.tier;
}

/** Lifts a network's ban, with its strikes and the banned keys that counted against it. */
export async function liftNetworkBan(env: Env, network: string): Promise<void> {
  await env.DB.batch([
    env.DB.prepare("DELETE FROM penalties WHERE subject = ?1").bind(netSubject(network)),
    env.DB.prepare("DELETE FROM marks WHERE subject = ?1").bind(netSubject(network)),
  ]);
  await record(env, "unban", netSubject(network), "lifted by the maintainer");
}

/**
 * For the daily run: deletes the penalties that no longer count (strikes a day old, with any wait
 * and ban over), the marks from before the last 30 days, and the networks of submissions decided
 * over 30 days ago.
 */
export function clearExpired(env: Env, at: number): D1PreparedStatement[] {
  return [
    env.DB.prepare("DELETE FROM penalties WHERE struck_at <= ?1 AND cool_until <= ?2 AND COALESCE(banned_until, 0) <= ?2").bind(
      at - STRIKES_RESET_SECONDS,
      at,
    ),
    env.DB.prepare("DELETE FROM marks WHERE at <= ?1").bind(at - Math.max(MARK_SECONDS, BANNED_KEYS_SECONDS)),
    env.DB.prepare(
      `UPDATE submissions SET network = NULL
       WHERE network IS NOT NULL AND status NOT IN ('open', 'pending', 'flagged') AND COALESCE(decided_at, created_at) < ?1`,
    ).bind(at - SUBMISSION_NETWORK_SECONDS),
  ];
}
