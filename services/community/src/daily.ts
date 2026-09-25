/**
 * The cron trigger, once a day: clear away what has run its course, then send the maintainer the
 * digest of everything that didn't need them at once. Each step is bounded, so a backlog is worked
 * off over a few days rather than in one run that runs out of time.
 */
import { dayOf, now } from "./bytes";
import type { Env } from "./env";
import { makeLink } from "./links";
import { alert, type Notice } from "./notify";
import { envNumber, pauseState } from "./quota";
import { OPEN_FOR_SECONDS } from "./limits";
import { clearExpired } from "./penalties";
import { removeAll } from "./store";

const DAY = 86400;

export async function daily(env: Env, at = now()): Promise<void> {
  await tidy(env, at);
  const notice = await digest(env, at);
  if (notice) await alert(env, notice, "digest");
}

/** Expires uploads nobody finished, and forgets what no longer needs remembering. */
export async function tidy(env: Env, at = now()): Promise<void> {
  const { results: stale } = await env.DB.prepare("SELECT id FROM submissions WHERE status = 'open' AND created_at < ?1 LIMIT 25")
    .bind(at - OPEN_FOR_SECONDS)
    .all<{ id: string }>();
  for (const { id } of stale) {
    await env.DB.prepare("UPDATE submissions SET status = 'expired' WHERE id = ?1 AND status = 'open'").bind(id).run();
    await removeAll(env.HOLD, `hold/${id}/`);
  }
  // Contact sheets are kept a month after a decision, for a takedown that needs another look.
  const { results: decided } = await env.DB.prepare(
    `SELECT id FROM submissions WHERE status NOT IN ('open', 'pending', 'flagged') AND sheets > 0
     AND COALESCE(decided_at, created_at) < ?1 LIMIT 25`,
  )
    .bind(at - 30 * DAY)
    .all<{ id: string }>();
  for (const { id } of decided) {
    await removeAll(env.HOLD, `hold/${id}/`);
    await env.DB.prepare("UPDATE submissions SET sheets = 0, sheets_received = 0 WHERE id = ?1").bind(id).run();
  }
  await env.DB.batch([
    env.DB.prepare("DELETE FROM seen WHERE expires < ?1").bind(at),
    env.DB.prepare("DELETE FROM counters WHERE day < ?1").bind(dayOf(at - 8 * DAY)),
    env.DB.prepare("DELETE FROM events WHERE at < ?1").bind(at - 90 * DAY),
    env.DB.prepare("DELETE FROM reports WHERE created_at < ?1").bind(at - 180 * DAY),
    // The hashed networks behind the install counts are only kept for the day they were counted.
    env.DB.prepare("DELETE FROM installs_seen WHERE day < ?1").bind(dayOf(at)),
    // Strikes, bans and marks that have run out, and the networks of packs decided a month ago.
    ...clearExpired(env, at),
  ]);
}

/** The day's digest, or null when there is nothing to say. */
export async function digest(env: Env, at = now()): Promise<Notice | null> {
  const count = async (sql: string, ...binds: unknown[]) =>
    (await env.DB.prepare(sql).bind(...binds).first<{ n: number }>())?.n ?? 0;
  const waiting = await count("SELECT COUNT(*) AS n FROM submissions WHERE status IN ('pending', 'flagged')");
  const flagged = await count("SELECT COUNT(*) AS n FROM submissions WHERE status = 'flagged'");
  const reports = await count("SELECT COUNT(*) AS n FROM reports WHERE created_at >= ?1", at - DAY);
  const unpulled = await count("SELECT COUNT(*) AS n FROM submissions WHERE status = 'approved' AND exported_at IS NULL");
  // Bans, since some happen by themselves (a third pack turned down, a second key banned from a
  // network), and requests to publish that GitHub turned down, which a token run out would cause.
  const { results: events } = await env.DB.prepare(
    `SELECT kind, subject, detail FROM events
     WHERE at >= ?1 AND kind IN ('withdrawn', 'takedown', 'paused', 'ban', 'publish_failed') ORDER BY at LIMIT 20`,
  )
    .bind(at - DAY)
    .all<{ kind: string; subject: string; detail: string }>();
  // Each of the day's reports by what it is about, since one about a pack on GitHub or one naming a
  // pack loosely matches nothing here and would otherwise only be counted. The details and how to
  // reach the reporter stay with `folderskin-tools community reports`, out of the chat channel.
  const { results: reported } = await env.DB.prepare(
    "SELECT reason, target, submission FROM reports WHERE created_at >= ?1 ORDER BY created_at DESC, rowid DESC LIMIT 10",
  )
    .bind(at - DAY)
    .all<{ reason: string; target: string; submission: string | null }>();
  const pause = await pauseState(env);
  if (waiting === 0 && reports === 0 && unpulled === 0 && events.length === 0 && !pause.paused) return null;

  const oldest = await env.DB.prepare(
    `SELECT s.id, s.name, s.items, s.status, s.finalized_at, k.handle FROM submissions s JOIN keys k ON k.key = s.key
     WHERE s.status IN ('pending', 'flagged') ORDER BY s.status = 'flagged' DESC, s.finalized_at ASC LIMIT 10`,
  ).all<{ id: string; name: string; items: number; status: string; finalized_at: number; handle: string }>();
  const links: Notice["links"] = [];
  for (const s of oldest.results) {
    const link = await makeLink(env, "review", s.id, at);
    if (link) links.push({ label: `${s.status === "flagged" ? "Flagged: " : ""}"${s.name}" by ${s.handle}, ${s.items} pictures`, url: link });
  }
  if (pause.paused) {
    const resume = await makeLink(env, "pause", "", at);
    if (resume) links.push({ label: "Sharing is paused; resume it", url: resume });
  }
  const neurons = (await env.DB.prepare("SELECT n FROM counters WHERE day = ?1 AND scope = 'global' AND id = 'neurons'")
    .bind(dayOf(at - DAY))
    .first<{ n: number }>())?.n;
  const since = await count("SELECT MIN(finalized_at) AS n FROM submissions WHERE status IN ('pending', 'flagged')");
  const waited = Math.max(0, Math.floor((at - since) / DAY));
  const lines = [
    `Waiting for review: ${waiting}${flagged ? ` (${flagged} flagged)` : ""}.`,
    ...(waiting > 0 ? [`The oldest has waited ${waited === 1 ? "a day" : `${waited} days`}.`] : []),
    `Reports in the last day: ${reports}.`,
    ...reported.map((r) => `Report, ${r.reason}: ${r.target.slice(0, 200)}${r.submission ? ` (${r.submission})` : ""}`),
    ...(reports > reported.length ? [`…and ${reports - reported.length} more reports.`] : []),
    ...(reports > 0 ? ["folderskin-tools community reports shows their details and how to reach whoever sent them."] : []),
    `Approved but not pulled into the repository yet: ${unpulled}.`,
    ...(env.AI ? [`Triage used ${neurons ?? 0} of ${envNumber(env.AI_DAILY_NEURONS, 9000)} neurons yesterday.`] : []),
    ...events.map((e) => `${e.kind}: ${e.subject} ${e.detail}`.trim()),
  ];
  return { title: "FolderSkin community: the daily digest", lines, links };
}
