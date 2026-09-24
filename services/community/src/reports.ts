/**
 * Reports from anyone about a pack, no account needed: POST /v1/reports with what it is about, why,
 * and optionally more detail and a way to reach the reporter. A report of child sexual abuse
 * material or of intimate pictures shared without consent reaches the maintainer at once, with a
 * link that takes the pack down from a phone; the rest go in the daily digest.
 *
 * `target` is kept as text and never fetched, whatever it looks like.
 */
import { now, randomId } from "./bytes";
import type { Env } from "./env";
import { fail, json, parseJson, readBody } from "./http";
import { networkHash } from "./ip";
import { PER_NETWORK } from "./limits";
import { makeLink } from "./links";
import { alert, record } from "./notify";
import { takeAll } from "./quota";
import { SUBMISSION_ID } from "./store";
import { hasText, isPackId, trimTrailingSlashes } from "./text";

export const REPORT_REASONS = ["csam", "ncii", "copyright", "terms", "other"] as const;
type ReportReason = (typeof REPORT_REASONS)[number];
const URGENT: ReportReason[] = ["csam", "ncii"];

const WORDS: Record<ReportReason, string> = {
  csam: "child sexual abuse material",
  ncii: "intimate pictures shared without consent",
  copyright: "copyright",
  terms: "breaking the pack terms",
  other: "something else",
};

/**
 * The submission a report is about, when `target` names one: its id, or an approved pack's folder
 * name. A pack pulled into community/packs goes by the folder it was written to there, which is
 * the name people see; a folder of that name that isn't one of these belongs to a pack from GitHub.
 */
async function resolve(env: Env, target: string): Promise<string | null> {
  const id = target.match(/sub_[a-z2-7]{20}/)?.[0];
  if (id && SUBMISSION_ID.test(id)) {
    const row = await env.DB.prepare("SELECT id FROM submissions WHERE id = ?1").bind(id).first<{ id: string }>();
    if (row) return row.id;
  }
  const folder = trimTrailingSlashes(target.trim().toLowerCase()).replace(/^.*\//, "");
  if (isPackId(folder)) {
    const row = await env.DB.prepare(
      `SELECT id FROM submissions WHERE status = 'approved' AND (folder = ?1 OR (folder IS NULL AND pack_id = ?1))
       ORDER BY folder IS NULL LIMIT 1`,
    )
      .bind(folder)
      .first<{ id: string }>();
    if (row) return row.id;
  }
  return null;
}

export async function report(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  const body = parseJson(await readBody(request, 8 * 1024));
  const { target, reason, details, contact } = body;
  if (!hasText(target, 300)) throw fail(400, "bad_report", "Say which pack this is about: its name, its folder name or a link to it.");
  if (typeof reason !== "string" || !(REPORT_REASONS as readonly string[]).includes(reason)) {
    throw fail(400, "bad_report", `Choose what the report is about: ${REPORT_REASONS.join(", ")}.`);
  }
  const text = typeof details === "string" ? details.trim() : "";
  if (text.length > 2000) throw fail(400, "bad_report", "Keep the details to 2,000 characters.");
  const reach = typeof contact === "string" ? contact.trim() : "";
  if (reach.length > 200) throw fail(400, "bad_report", "Keep the contact details to 200 characters.");

  const at = now();
  const network = await networkHash(env, request, at);
  await takeAll(
    env,
    [
      {
        scope: "net:reports",
        id: network,
        amount: 1,
        limit: PER_NETWORK.reports,
        error: fail(429, "quota", "Your network has sent as many reports as it can today. For anything urgent, report it to GitHub as well."),
      },
    ],
    at,
  );
  const submission = await resolve(env, target);
  const id = randomId("rep_", 10);
  await env.DB.prepare(
    `INSERT INTO reports (id, target, submission, reason, details, contact, ip_hash, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)`,
  )
    .bind(id, target.trim(), submission, reason, text, reach, network, at)
    .run();

  const why = reason as ReportReason;
  const urgent = URGENT.includes(why);
  await record(env, "report", submission ?? target.trim().slice(0, 300), `${why}: ${text.slice(0, 300)}`, urgent ? "high" : "normal");
  if (urgent) {
    const takedown = submission ? await makeLink(env, "takedown", submission) : null;
    const pause = await makeLink(env, "pause", "");
    ctx.waitUntil(
      alert(env, {
        title: `Urgent report: ${WORDS[why]}`,
        lines: [
          `About: ${target.trim().slice(0, 300)}`,
          submission ? `That is submission ${submission}.` : "It doesn't match a pack the service holds; it may be one on GitHub.",
          text ? `Details: ${text.slice(0, 1000)}` : "No details given.",
        ],
        links: [
          ...(takedown ? [{ label: "Take it down", url: takedown }] : []),
          ...(pause ? [{ label: "Pause all sharing", url: pause }] : []),
        ],
      }),
    );
  }
  return json({ received: true }, 201);
}
