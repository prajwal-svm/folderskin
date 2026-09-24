/**
 * Submissions as they are kept, and the decisions on them. The admin endpoints and the phone
 * links both act through here, so a pack approved from a phone is approved exactly the way one
 * approved from `folderskin-tools community decide` is.
 *
 * Storage layout:
 *
 *   HOLD    hold/<submission>/<sha256>     a picture waiting for review
 *           hold/<submission>/sheet-<n>    a contact sheet
 *   PUBLIC  packs/<pack id>/pack.json      an approved pack, as folderskin-community's packs/ holds it
 *           packs/<pack id>/<file>
 */
import { now } from "./bytes";
import type { Env } from "./env";
import { fail } from "./http";
import { PACK_VERSION } from "./limits";
import { alert, record } from "./notify";
import { REASONS, explain, isReason } from "./terms";
import { slug, type Flag, type Manifest } from "./text";

export type Status = "open" | "pending" | "flagged" | "approved" | "rejected" | "withdrawn" | "taken_down" | "expired";

export type Submission = {
  id: string;
  key: string;
  status: Status;
  name: string;
  license: string;
  source: string;
  terms_version: number;
  manifest: string;
  notes: string;
  items: number;
  sheets: number;
  sheets_received: number;
  flags: string;
  reasons: string;
  note: string;
  pack_id: string | null;
  /** The handle pack.json credits, as it was when the pack was approved. */
  author: string | null;
  /** The folder in folderskin-community's packs/ it was pulled into, once it has been. */
  folder: string | null;
  ip_hash: string;
  created_at: number;
  finalized_at: number | null;
  decided_at: number | null;
  exported_at: number | null;
};

export type Item = { sha256: string; file: string; bytes: number; width: number; height: number; received: number };

export const SUBMISSION_ID = /^sub_[a-z2-7]{20}$/;
export const SHA256 = /^[0-9a-f]{64}$/;

export const WAITING: Status[] = ["pending", "flagged"];

export async function loadSubmission(env: Env, id: string): Promise<Submission | null> {
  if (!SUBMISSION_ID.test(id)) return null;
  return env.DB.prepare("SELECT * FROM submissions WHERE id = ?1").bind(id).first<Submission>();
}

export async function loadItems(env: Env, id: string): Promise<Item[]> {
  const { results } = await env.DB.prepare(
    "SELECT sha256, file, bytes, width, height, received FROM items WHERE submission = ?1 ORDER BY rowid",
  )
    .bind(id)
    .all<Item>();
  return results;
}

export const flagsOf = (s: Submission): Flag[] => parse<Flag[]>(s.flags, []);
export const reasonsOf = (s: Submission): string[] => parse<string[]>(s.reasons, []);
export const manifestOf = (s: Submission): Manifest => parse<Manifest>(s.manifest, { name: s.name, tags: [], skins: [] });

function parse<T>(text: string, fallback: T): T {
  try {
    return JSON.parse(text) as T;
  } catch {
    return fallback;
  }
}

/** What the author sees of their submission: flags stay with the maintainer, so the filters can't be probed. */
export function authorView(s: Submission) {
  const status: Record<Status, string> = {
    open: "uploading",
    pending: "in_review",
    flagged: "in_review",
    approved: "approved",
    rejected: "rejected",
    withdrawn: "withdrawn",
    taken_down: "taken_down",
    expired: "expired",
  };
  return {
    id: s.id,
    name: s.name,
    status: status[s.status],
    pictures: s.items,
    license: s.license,
    created_at: s.created_at,
    decided_at: s.decided_at,
    // The name it has in the community once it is there, which is the folder it was pulled into.
    pack_id: s.status === "approved" ? (s.folder ?? s.pack_id) : null,
    // In folderskin-community already, where only the maintainer can take it out again.
    pulled: s.exported_at !== null,
    reasons: explain(reasonsOf(s)),
    note: s.note,
  };
}

/** Removes every object under `prefix`, a page at a time. */
export async function removeAll(bucket: R2Bucket, prefix: string): Promise<void> {
  let cursor: string | undefined;
  do {
    const page = await bucket.list({ prefix, cursor, limit: 1000 });
    if (page.objects.length > 0) await bucket.delete(page.objects.map((o) => o.key));
    cursor = page.truncated ? page.cursor : undefined;
  } while (cursor);
}

/** The pictures waiting for review; the contact sheets stay until the nightly clean-up, for a later look. */
async function removeHeldPictures(env: Env, id: string): Promise<void> {
  const items = await loadItems(env, id);
  if (items.length > 0) await env.HOLD.delete(items.map((i) => `hold/${id}/${i.sha256}`));
}

/** pack.json as the approved pack is published: the author is the handle on record, not anything the app said. */
export function packJson(s: Submission, handle: string): string {
  const manifest = manifestOf(s);
  const pack = {
    version: PACK_VERSION,
    name: manifest.name,
    author: handle,
    license: s.license,
    tags: manifest.tags,
    skins: manifest.skins.map((skin) => (skin.tags.length > 0 ? skin : { file: skin.file, name: skin.name })),
  };
  return `${JSON.stringify(pack, null, 2)}\n`;
}

/**
 * A folder name for an approved pack: its name as a slug, numbered when another approved pack has
 * it, here or as the folder it was pulled into.
 */
async function freePackId(env: Env, name: string): Promise<string> {
  const base = slug(name) || "pack";
  for (let n = 1; n <= 99; n++) {
    const suffix = n === 1 ? "" : `-${n}`;
    const id = `${base.slice(0, 40 - suffix.length).replace(/-+$/, "")}${suffix}`;
    const taken = await env.DB.prepare("SELECT 1 FROM submissions WHERE status = 'approved' AND (pack_id = ?1 OR folder = ?1)")
      .bind(id)
      .first();
    if (!taken) return id;
  }
  throw fail(409, "name_taken", "Every folder name for that pack is taken. Rename it and approve it again.");
}

const notWaiting = () => fail(409, "not_waiting", "That pack isn't waiting for a decision.");
const notFound = () => fail(404, "not_found", "There's no such pack.");

/**
 * Approves a pack: its pictures and pack.json go to the public bucket under a folder name of its
 * own, and the author's key moves off probation.
 *
 * The folder name is claimed in the database first (an index keeps two approved packs from
 * sharing one), and only then are files written under it, so two approvals at the same moment can
 * never write into the same folder.
 */
export async function approve(env: Env, id: string, note: string): Promise<{ pack_id: string }> {
  const s = await loadSubmission(env, id);
  if (!s) throw notFound();
  if (!WAITING.includes(s.status)) throw notWaiting();
  const author = await env.DB.prepare("SELECT handle FROM keys WHERE key = ?1").bind(s.key).first<{ handle: string }>();
  if (!author) throw notFound();
  const packId = await claimPackId(env, s, author.handle, note);
  try {
    for (const item of await loadItems(env, id)) {
      const held = await env.HOLD.get(`hold/${id}/${item.sha256}`);
      if (!held) throw fail(409, "missing", `${item.file} is missing from storage, so the pack can't be published.`);
      // Streamed from one bucket to the other, so the pictures never sit in this Worker's memory.
      // R2 only takes a stream whose length it knows, which a FixedLengthStream promises.
      const { readable, writable } = new FixedLengthStream(held.size);
      const piped = held.body.pipeTo(writable);
      await env.PUBLIC.put(`packs/${packId}/${item.file}`, readable, { httpMetadata: held.httpMetadata, sha256: item.sha256 });
      await piped;
    }
    await env.PUBLIC.put(`packs/${packId}/pack.json`, packJson(s, author.handle), {
      httpMetadata: { contentType: "application/json" },
    });
  } catch (e) {
    // Back to waiting, with nothing left half-published.
    await env.DB.prepare("UPDATE submissions SET status = ?2, pack_id = NULL, author = NULL, decided_at = NULL WHERE id = ?1")
      .bind(id, s.status)
      .run();
    await removeAll(env.PUBLIC, `packs/${packId}/`);
    throw e;
  }
  await env.DB.prepare(
    "UPDATE keys SET approved = approved + 1, tier = CASE WHEN tier = 'probation' THEN 'active' ELSE tier END WHERE key = ?1",
  )
    .bind(s.key)
    .run();
  await removeHeldPictures(env, id);
  await record(env, "approved", id, packId);
  return { pack_id: packId };
}

/**
 * Marks the pack approved under a free folder name, trying the next name if another approval took
 * this one. The handle pack.json credits is kept with it, so the pack is exported under the name it
 * was published with even if the author changes theirs later.
 */
async function claimPackId(env: Env, s: Submission, handle: string, note: string): Promise<string> {
  for (let attempt = 0; attempt < 3; attempt++) {
    const packId = await freePackId(env, s.name);
    try {
      const done = await env.DB.prepare(
        `UPDATE submissions SET status = 'approved', pack_id = ?2, author = ?3, decided_at = ?4, reasons = '[]', note = ?5
         WHERE id = ?1 AND status IN ('pending', 'flagged')`,
      )
        .bind(s.id, packId, handle, now(), note)
        .run();
      if (done.meta.changes !== 1) throw notWaiting();
      return packId;
    } catch (e) {
      if (!(e instanceof Error) || !/UNIQUE/i.test(e.message)) throw e;
    }
  }
  throw fail(409, "name_taken", "Another pack took that folder name at the same moment. Approve it again.");
}

/** Checks decision reasons: at least one, every one known. */
export function readReasons(value: unknown): string[] {
  const codes = Array.isArray(value) ? value : [];
  if (codes.length === 0 || codes.length > 8 || !codes.every(isReason)) {
    throw fail(400, "bad_reasons", `Give one or more reasons from: ${Object.keys(REASONS).join(", ")}.`);
  }
  return [...new Set(codes as string[])];
}

/** Keeps pictures turned down for what they show from being sent again, and bans the key for the worst. */
async function consequences(env: Env, s: Submission, reasons: string[]): Promise<void> {
  const at = now();
  if (reasons.some((r) => REASONS[r].blocks)) {
    const items = await loadItems(env, s.id);
    const reason = reasons.join(",");
    await env.DB.batch(
      items.map((i) =>
        env.DB.prepare("INSERT INTO blocked (sha256, reason, created_at) VALUES (?1, ?2, ?3) ON CONFLICT (sha256) DO NOTHING").bind(
          i.sha256,
          reason,
          at,
        ),
      ),
    );
  }
  const bans = reasons.some((r) => REASONS[r].bans);
  await env.DB.prepare(
    "UPDATE keys SET rejected = rejected + 1, tier = CASE WHEN ?2 THEN 'banned' ELSE tier END WHERE key = ?1",
  )
    .bind(s.key, bans ? 1 : 0)
    .run();
}

/** Turns a waiting pack down, saying why with codes from terms.ts. Its pictures are deleted. */
export async function reject(env: Env, id: string, reasons: string[], note: string): Promise<void> {
  const s = await loadSubmission(env, id);
  if (!s) throw notFound();
  const done = await env.DB.prepare(
    `UPDATE submissions SET status = 'rejected', reasons = ?2, note = ?3, decided_at = ?4
     WHERE id = ?1 AND status IN ('pending', 'flagged')`,
  )
    .bind(id, JSON.stringify(reasons), note, now())
    .run();
  if (done.meta.changes !== 1) throw notWaiting();
  await consequences(env, s, reasons);
  await removeAll(env.HOLD, `hold/${id}/`);
  await record(env, "rejected", id, reasons.join(","));
}

/**
 * Takes a pack down: out of the public bucket at once, whether it was approved or still waiting.
 * A pack already pulled into the repository has to be removed there too, from the folder the
 * answer names.
 */
export async function takedown(
  env: Env,
  id: string,
  reasons: string[],
  note: string,
): Promise<{ pack_id: string | null; exported: boolean; folder: string | null }> {
  const s = await loadSubmission(env, id);
  if (!s) throw notFound();
  const done = await env.DB.prepare(
    `UPDATE submissions SET status = 'taken_down', reasons = ?2, note = ?3, decided_at = ?4
     WHERE id = ?1 AND status IN ('pending', 'flagged', 'approved')`,
  )
    .bind(id, JSON.stringify(reasons), note, now())
    .run();
  if (done.meta.changes !== 1) throw fail(409, "not_live", "That pack isn't published or waiting, so there's nothing to take down.");
  if (s.pack_id && s.status === "approved") await removeAll(env.PUBLIC, `packs/${s.pack_id}/`);
  await removeAll(env.HOLD, `hold/${id}/`);
  await consequences(env, s, reasons);
  const folder = s.exported_at ? repoFolder(s) : null;
  await record(env, "takedown", id, `${reasons.join(",")}${folder ? ` (remove packs/${folder} from folderskin-community too)` : ""}`, "high");
  return { pack_id: s.status === "approved" ? s.pack_id : null, exported: folder !== null, folder };
}

/** The folder a pulled pack has in folderskin-community's packs/. */
const repoFolder = (s: Submission) => s.folder ?? s.pack_id ?? "";

/**
 * The author's own withdrawal. A pack pulled into the repository already stays in the app until
 * the maintainer takes it out there, so they are told at once rather than in the digest.
 */
export async function withdraw(env: Env, s: Submission, ctx: ExecutionContext): Promise<void> {
  const done = await env.DB.prepare(
    `UPDATE submissions SET status = 'withdrawn', decided_at = ?2
     WHERE id = ?1 AND status IN ('open', 'pending', 'flagged', 'approved')`,
  )
    .bind(s.id, now())
    .run();
  if (done.meta.changes !== 1) throw fail(409, "not_live", "That pack can't be withdrawn now.");
  if (s.pack_id && s.status === "approved") await removeAll(env.PUBLIC, `packs/${s.pack_id}/`);
  await removeAll(env.HOLD, `hold/${s.id}/`);
  const folder = s.exported_at ? repoFolder(s) : null;
  await record(env, "withdrawn", s.id, `withdrawn by its author${folder ? `; remove packs/${folder} from folderskin-community too` : ""}`);
  if (folder) {
    ctx.waitUntil(
      alert(
        env,
        {
          title: `Withdrawn: "${s.name}" needs taking out of the repository`,
          lines: [
            `Its author withdrew it. It was pulled into folderskin-community as packs/${folder}, so it stays in the app until that folder is removed.`,
            "Remove the folder there and commit; its workflow rebuilds the index.",
          ],
        },
        "flagged",
      ),
    );
  }
}
