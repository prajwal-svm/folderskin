/**
 * The maintainer's side, every request signed by a key in ADMIN_KEYS (`folderskin-tools community`
 * makes them):
 *
 *   GET  /v1/admin/queue                              packs waiting, flagged first
 *   GET  /v1/admin/submissions/<id>                   one pack: flags, reports, pictures
 *   GET  /v1/admin/submissions/<id>/items/<sha256>    a picture
 *   GET  /v1/admin/submissions/<id>/sheets/<n>        a contact sheet
 *   POST /v1/admin/submissions/<id>/decision          {"decision": "approve" | "reject", "reasons", "note"}
 *   POST /v1/admin/submissions/<id>/takedown          {"reasons", "note"}
 *   POST /v1/admin/keys/<key>/tier                    {"tier"}
 *   POST /v1/admin/pause                              {"paused", "message"}: the kill switch
 *   GET  /v1/admin/exports                            approved packs not yet pulled into the repository
 *   GET  /v1/admin/exports/<id>/pack.json             …and each one's files, for `community pull`
 *   GET  /v1/admin/exports/<id>/files/<file>
 *   POST /v1/admin/exports/<id>/done                  {"folder"}: the folder it was written to
 *   GET  /v1/admin/reports?days=<n>                   reports from the last n days (7 unless said)
 */
import { isKey, requireAdmin, verifySigned } from "./auth";
import { now } from "./bytes";
import type { Env } from "./env";
import { fail, json, parseJson } from "./http";
import { isTier, MAX_JSON_BYTES } from "./limits";
import { record } from "./notify";
import { setPause } from "./quota";
import {
  approve,
  flagsOf,
  loadItems,
  loadSubmission,
  readReasons,
  reject,
  SHA256,
  takedown,
  type Submission,
} from "./store";
import { hasText, isPackId, isPictureFileName } from "./text";

async function admin(request: Request, env: Env, maxBody = 0) {
  return verifySigned(request, env, maxBody, (key) => requireAdmin(env, key));
}

async function needSubmission(env: Env, id: string): Promise<Submission> {
  const s = await loadSubmission(env, id);
  if (!s) throw fail(404, "not_found", "There's no such pack.");
  return s;
}

function readNote(value: unknown): string {
  if (value === undefined || value === "") return "";
  if (!hasText(value, 500)) throw fail(400, "bad_note", "Keep the note to 500 characters of plain text.");
  return value.trim();
}

function summary(s: Submission & { handle?: string; tier?: string; reports?: number }) {
  return {
    id: s.id,
    name: s.name,
    status: s.status,
    handle: s.handle ?? null,
    tier: s.tier ?? null,
    pictures: s.items,
    license: s.license,
    source: s.source,
    flags: flagsOf(s),
    reports: s.reports ?? 0,
    created_at: s.created_at,
    finalized_at: s.finalized_at,
    pack_id: s.pack_id,
    folder: s.folder,
  };
}

export async function queue(request: Request, env: Env): Promise<Response> {
  await admin(request, env);
  const which = new URL(request.url).searchParams.get("status") ?? "waiting";
  const statuses: Record<string, string[]> = {
    waiting: ["flagged", "pending"],
    flagged: ["flagged"],
    pending: ["pending"],
    approved: ["approved"],
    open: ["open"],
  };
  const wanted = statuses[which];
  if (!wanted) throw fail(400, "bad_status", `Ask for one of: ${Object.keys(statuses).join(", ")}.`);
  const { results } = await env.DB.prepare(
    `SELECT s.*, k.handle, k.tier, (SELECT COUNT(*) FROM reports r WHERE r.submission = s.id) AS reports
     FROM submissions s JOIN keys k ON k.key = s.key
     WHERE s.status IN (${wanted.map((_, i) => `?${i + 1}`).join(", ")})
     ORDER BY s.status = 'flagged' DESC, COALESCE(s.finalized_at, s.created_at) ASC
     LIMIT 100`,
  )
    .bind(...wanted)
    .all<Submission & { handle: string; tier: string; reports: number }>();
  return json({ submissions: results.map(summary) });
}

export async function detail(request: Request, env: Env, id: string): Promise<Response> {
  await admin(request, env);
  const s = await needSubmission(env, id);
  const account = await env.DB.prepare("SELECT handle, tier, approved, rejected FROM keys WHERE key = ?1").bind(s.key).first();
  const { results: reports } = await env.DB.prepare(
    "SELECT id, reason, target, details, contact, created_at FROM reports WHERE submission = ?1 ORDER BY created_at DESC",
  )
    .bind(id)
    .all();
  return json({
    ...summary(s),
    key: s.key,
    account,
    notes: s.notes,
    manifest: JSON.parse(s.manifest),
    items: await loadItems(env, id),
    sheets: s.sheets,
    reasons: JSON.parse(s.reasons),
    note: s.note,
    reports,
  });
}

async function stream(bucket: R2Bucket, key: string): Promise<Response> {
  const object = await bucket.get(key);
  if (!object) throw fail(404, "not_found", "That file isn't in storage any more.");
  return new Response(object.body, {
    headers: {
      "Content-Type": object.httpMetadata?.contentType ?? "application/octet-stream",
      "Content-Length": String(object.size),
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
      // Pictures under review are looked at, never run.
      "Content-Security-Policy": "default-src 'none'; sandbox",
    },
  });
}

export async function item(request: Request, env: Env, id: string, sha: string): Promise<Response> {
  await admin(request, env);
  if (!SHA256.test(sha)) throw fail(404, "not_found", "There's nothing here.");
  const s = await needSubmission(env, id);
  const known = (await loadItems(env, id)).find((i) => i.sha256 === sha);
  if (!known) throw fail(404, "not_found", "That picture isn't part of this pack.");
  if (s.status === "approved" && s.pack_id) return stream(env.PUBLIC, `packs/${s.pack_id}/${known.file}`);
  return stream(env.HOLD, `hold/${id}/${sha}`);
}

export async function sheet(request: Request, env: Env, id: string, n: number): Promise<Response> {
  await admin(request, env);
  const s = await needSubmission(env, id);
  if (!Number.isInteger(n) || n < 0 || n >= s.sheets) throw fail(404, "not_found", "This pack doesn't have that many sheets.");
  return stream(env.HOLD, `hold/${id}/sheet-${n}`);
}

export async function decision(request: Request, env: Env, id: string): Promise<Response> {
  const signed = await admin(request, env, MAX_JSON_BYTES);
  const body = parseJson(signed.body);
  const note = readNote(body.note);
  if (body.decision === "approve") return json({ status: "approved", ...(await approve(env, id, note)) });
  if (body.decision === "reject") {
    await reject(env, id, readReasons(body.reasons), note);
    return json({ status: "rejected" });
  }
  throw fail(400, "bad_decision", 'Decide "approve" or "reject".');
}

export async function takeDown(request: Request, env: Env, id: string): Promise<Response> {
  const signed = await admin(request, env, MAX_JSON_BYTES);
  const body = parseJson(signed.body);
  const result = await takedown(env, id, readReasons(body.reasons), readNote(body.note));
  return json({ status: "taken_down", ...result });
}

export async function setTier(request: Request, env: Env, key: string): Promise<Response> {
  const signed = await admin(request, env, MAX_JSON_BYTES);
  if (!isKey(key)) throw fail(404, "not_found", "There's no such key.");
  const { tier } = parseJson(signed.body);
  if (!isTier(tier)) throw fail(400, "bad_tier", "A tier is probation, active, trusted or banned.");
  const done = await env.DB.prepare("UPDATE keys SET tier = ?2 WHERE key = ?1").bind(key, tier).run();
  if (done.meta.changes !== 1) throw fail(404, "not_found", "There's no such key.");
  await record(env, "tier", key, tier);
  return json({ tier });
}

export async function pause(request: Request, env: Env): Promise<Response> {
  const signed = await admin(request, env, MAX_JSON_BYTES);
  const { paused, message } = parseJson(signed.body);
  if (typeof paused !== "boolean") throw fail(400, "bad_pause", 'Say {"paused": true} or {"paused": false}.');
  const words = message === undefined || message === "" ? "" : hasText(message, 200) ? message.trim() : null;
  if (words === null) throw fail(400, "bad_pause", "Keep the message to 200 characters of plain text.");
  await setPause(env, { paused, message: words });
  await record(env, paused ? "paused" : "resumed", "", words, paused ? "high" : "normal");
  return json({ paused, message: words });
}

// ---- export into the repository ----

export async function exportList(request: Request, env: Env): Promise<Response> {
  await admin(request, env);
  // The handle pack.json was written with at approval, not the author's name now: `pull` checks
  // that the two agree.
  const { results } = await env.DB.prepare(
    `SELECT id, pack_id, name, license, decided_at, author AS handle
     FROM submissions
     WHERE status = 'approved' AND exported_at IS NULL
     ORDER BY decided_at ASC LIMIT 50`,
  ).all<{ id: string; pack_id: string; name: string; license: string; decided_at: number; handle: string }>();
  const packs = [];
  for (const row of results) {
    const files = (await loadItems(env, row.id)).map((i) => ({ file: i.file, sha256: i.sha256, bytes: i.bytes }));
    packs.push({ ...row, files });
  }
  return json({ packs });
}

async function approvedPack(env: Env, id: string): Promise<Submission & { pack_id: string }> {
  const s = await needSubmission(env, id);
  if (s.status !== "approved" || !s.pack_id) throw fail(409, "not_approved", "That pack isn't approved.");
  return s as Submission & { pack_id: string };
}

export async function exportManifest(request: Request, env: Env, id: string): Promise<Response> {
  await admin(request, env);
  const s = await approvedPack(env, id);
  return stream(env.PUBLIC, `packs/${s.pack_id}/pack.json`);
}

export async function exportFile(request: Request, env: Env, id: string, file: string): Promise<Response> {
  await admin(request, env);
  if (!isPictureFileName(file)) throw fail(404, "not_found", "There's nothing here.");
  const s = await approvedPack(env, id);
  const known = (await loadItems(env, id)).some((i) => i.file === file);
  if (!known) throw fail(404, "not_found", "That file isn't part of this pack.");
  return stream(env.PUBLIC, `packs/${s.pack_id}/${file}`);
}

/**
 * Records that a pack is in folderskin-community now, and under which folder: `pull` numbers the name
 * when a pack from GitHub has it already, and reports, takedowns and the author all have to go by
 * the folder it is really in.
 */
export async function exportDone(request: Request, env: Env, id: string): Promise<Response> {
  const signed = await admin(request, env, MAX_JSON_BYTES);
  const { folder } = parseJson(signed.body);
  const s = await approvedPack(env, id);
  // An older folderskin-tools says nothing, and only ever wrote the pack under its own name.
  let written = s.pack_id;
  if (folder !== undefined) {
    if (!isPackId(folder)) throw fail(400, "bad_folder", "A folder name is lower-case letters and digits joined by single dashes.");
    written = folder;
  }
  await env.DB.prepare("UPDATE submissions SET exported_at = ?2, folder = ?3 WHERE id = ?1").bind(id, now(), written).run();
  await record(env, "exported", id, written);
  return json({ exported: true, folder: written });
}

/** Reports from the last few days, newest first, with how to reach whoever sent each one. */
export async function reports(request: Request, env: Env): Promise<Response> {
  await admin(request, env);
  const asked = new URL(request.url).searchParams.get("days");
  const days = asked === null ? 7 : Number(asked);
  if (!Number.isInteger(days) || days < 1 || days > 180) throw fail(400, "bad_days", "Ask for 1 to 180 days of reports.");
  const { results } = await env.DB.prepare(
    `SELECT r.id, r.reason, r.target, r.details, r.contact, r.created_at, r.submission, s.name, s.status
     FROM reports r LEFT JOIN submissions s ON s.id = r.submission
     WHERE r.created_at >= ?1 ORDER BY r.created_at DESC LIMIT 200`,
  )
    .bind(now() - days * 86400)
    .all();
  return json({ reports: results });
}
