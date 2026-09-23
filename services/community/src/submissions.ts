/**
 * Sending a pack for review, from the app:
 *
 *   POST   /v1/submissions                       what the pack is: its words, licence, source and
 *                                                every picture's sha256, size and dimensions
 *                                                → { submission_id, need, sheets }
 *   PUT    /v1/submissions/<id>/items/<sha256>   each picture the answer still needs
 *   PUT    /v1/submissions/<id>/sheets/<n>       the contact sheets, 16 pictures to a sheet
 *   POST   /v1/submissions/<id>/finalize         the checks, then into the review queue
 *   GET    /v1/submissions                       the author's packs, with status and reasons
 *   DELETE /v1/packs/<id>                        the author withdraws one
 *
 * Every picture is checked by its bytes before it is stored: the hash the pack declared, the format
 * its name promises, its dimensions from its header and no animation. Nothing is decoded here.
 */
import { requireAccount, verifySigned, type Account } from "./auth";
import { dayOf, now, randomId, sha256Hex } from "./bytes";
import type { Env } from "./env";
import { fail, json, parseJson } from "./http";
import { CONTENT_TYPES, formatOf, inspect, PictureError } from "./images";
import { networkHash } from "./ip";
import {
  MAX_JSON_BYTES,
  MAX_NOTES_CHARS,
  MAX_PICTURE_BYTES,
  MAX_PICTURE_SIDE,
  MAX_SHEET_BYTES,
  MAX_SHEET_SIDE,
  MIN_PICTURE_SIDE,
  MIN_SHEET_SIDE,
  PER_NETWORK,
  PICTURES_PER_SHEET,
  SOURCES,
  TERMS_VERSION,
  TIERS,
} from "./limits";
import { makeLink } from "./links";
import { alert, record } from "./notify";
import { giveBack, globalDailyPictures, maxWaiting, queueFull, requireAccepting, takeAll } from "./quota";
import { authorView, loadItems, loadSubmission, removeAll, SHA256, withdraw, type Submission } from "./store";
import { hasText, isLicense, readManifest, textFlags, type Flag } from "./text";
import { triage } from "./triage";

type DeclaredItem = { file: string; sha256: string; bytes: number; width: number; height: number };

function readItems(value: unknown, files: string[]): DeclaredItem[] {
  const list = Array.isArray(value) ? value : [];
  const bad = (why: string) => fail(400, "bad_pack", `The pack's pictures don't add up: ${why}.`);
  if (list.length !== files.length) throw bad("there should be one for each skin");
  const seen = new Set<string>();
  return list.map((raw, i) => {
    const item = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
    const { file, sha256, bytes, width, height } = item;
    if (file !== files[i]) throw bad(`picture ${i + 1} isn't the file its skin names`);
    if (typeof sha256 !== "string" || !SHA256.test(sha256)) throw bad(`picture ${i + 1} has no sha256`);
    if (seen.has(sha256)) throw fail(400, "duplicate_picture", "The same picture is in the pack twice. Take one of them out.");
    seen.add(sha256);
    if (!Number.isInteger(bytes) || (bytes as number) < 1 || (bytes as number) > MAX_PICTURE_BYTES) {
      throw fail(400, "too_large", `${file} is over the ${MAX_PICTURE_BYTES / 1024 / 1024} MB a picture can be.`);
    }
    for (const side of [width, height]) {
      if (!Number.isInteger(side) || (side as number) < MIN_PICTURE_SIDE || (side as number) > MAX_PICTURE_SIDE) {
        throw fail(400, "bad_size", `${file} has to be ${MIN_PICTURE_SIDE} to ${MAX_PICTURE_SIDE} px on each side.`);
      }
    }
    return { file, sha256, bytes: bytes as number, width: width as number, height: height as number };
  });
}

/** Opens a submission, once the pack, the quotas and the review queue all allow it. */
export async function create(request: Request, env: Env): Promise<Response> {
  const signed = await verifySigned(request, env, MAX_JSON_BYTES);
  const account = await requireAccount(env, signed.key);
  await requireAccepting(env);
  const body = parseJson(signed.body);

  if (body.terms_version !== TERMS_VERSION) {
    throw fail(400, "terms", "The pack terms have changed. Update FolderSkin, read them and send the pack again.");
  }
  const read = readManifest(body.manifest);
  if ("problems" in read) throw fail(400, "bad_pack", `The pack isn't valid: ${read.problems.join("; ")}.`);
  const { manifest } = read;
  if (!isLicense(body.license)) throw fail(400, "bad_license", "Choose one of the licences FolderSkin offers.");
  if (typeof body.source !== "string" || !(SOURCES as readonly string[]).includes(body.source)) {
    throw fail(400, "bad_source", "Say where the pictures came from.");
  }
  const notes = typeof body.notes === "string" ? body.notes.trim() : "";
  if (notes && !hasText(notes, MAX_NOTES_CHARS)) throw fail(400, "bad_notes", `Keep the credits to ${MAX_NOTES_CHARS} characters of plain text.`);
  const items = readItems(
    body.items,
    manifest.skins.map((s) => s.file),
  );
  for (const item of items) {
    if (formatOf(item.file) === null) throw fail(400, "bad_pack", `${item.file} isn't a PNG, JPEG or WebP file name.`);
  }

  // Pictures turned down before for what they show don't come back under another name.
  const blocked = await env.DB.prepare(
    `SELECT sha256 FROM blocked WHERE sha256 IN (${items.map((_, i) => `?${i + 1}`).join(", ")}) LIMIT 1`,
  )
    .bind(...items.map((i) => i.sha256))
    .first();
  if (blocked) throw fail(400, "blocked", "One of these pictures was turned down before for what it shows, so it can't be shared.");

  // Sending the same pack again, after a dropped connection or a closed laptop, picks up where it
  // stopped rather than starting over and counting against the day twice.
  const fingerprint = await sha256Hex(JSON.stringify([manifest, body.license, body.source, notes, items]));
  const unfinished = await env.DB.prepare("SELECT id, sheets FROM submissions WHERE key = ?1 AND status = 'open' AND fingerprint = ?2")
    .bind(account.key, fingerprint)
    .first<{ id: string; sheets: number }>();
  if (unfinished) {
    const { results } = await env.DB.prepare("SELECT sha256 FROM items WHERE submission = ?1 AND received = 0 ORDER BY rowid")
      .bind(unfinished.id)
      .all<{ sha256: string }>();
    return json({ submission_id: unfinished.id, need: results.map((r) => r.sha256), sheets: unfinished.sheets });
  }
  // Only one pack is on its way at a time: a different one gives up whatever was left unfinished.
  await giveUpUnfinished(env, account.key);

  const limits = TIERS[account.tier];
  const waiting = await env.DB.prepare("SELECT COUNT(*) AS n FROM submissions WHERE key = ?1 AND status IN ('pending', 'flagged')")
    .bind(account.key)
    .first<{ n: number }>();
  if ((waiting?.n ?? 0) >= limits.waiting) {
    const packs = limits.waiting === 1 ? "a pack" : `${limits.waiting} packs`;
    const once = limits.waiting === 1 ? "it's" : "one has";
    throw fail(429, "waiting", `You have ${packs} waiting for review already. Once ${once} been looked at, you can send another.`);
  }
  const queue = await env.DB.prepare("SELECT COUNT(*) AS n FROM submissions WHERE status IN ('pending', 'flagged')").first<{ n: number }>();
  if ((queue?.n ?? 0) >= maxWaiting(env)) throw queueFull();

  const at = now();
  const network = await networkHash(env, request, at);
  const pictures = items.length;
  const overToday = (what: string) => fail(429, "quota", `You've shared as ${what} as you can today. Please try again tomorrow.`);
  await takeAll(
    env,
    [
      { scope: "key:submissions", id: account.key, amount: 1, limit: limits.submissions, error: overToday("many packs") },
      { scope: "key:pictures", id: account.key, amount: pictures, limit: limits.pictures, error: overToday("many pictures") },
      {
        scope: "net:submissions",
        id: network,
        amount: 1,
        limit: PER_NETWORK.submissions,
        error: fail(429, "quota", "Your network has sent as many packs as it can today. Please try again tomorrow."),
      },
      {
        scope: "net:pictures",
        id: network,
        amount: pictures,
        limit: PER_NETWORK.pictures,
        error: fail(429, "quota", "Your network has sent as many pictures as it can today. Please try again tomorrow."),
      },
      { scope: "global", id: "pictures", amount: pictures, limit: globalDailyPictures(env), error: queueFull() },
    ],
    at,
  );

  const id = randomId("sub_", 12);
  const sheets = Math.ceil(pictures / PICTURES_PER_SHEET);
  await env.DB.batch([
    env.DB.prepare(
      `INSERT INTO submissions (id, key, status, name, license, source, terms_version, manifest, notes, items, sheets, ip_hash, fingerprint, created_at)
       VALUES (?1, ?2, 'open', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)`,
    ).bind(id, account.key, manifest.name, body.license, body.source, TERMS_VERSION, JSON.stringify(manifest), notes, pictures, sheets, network, fingerprint, at),
    ...items.map((i) =>
      env.DB.prepare("INSERT INTO items (submission, sha256, file, bytes, width, height) VALUES (?1, ?2, ?3, ?4, ?5, ?6)").bind(
        id,
        i.sha256,
        i.file,
        i.bytes,
        i.width,
        i.height,
      ),
    ),
  ]);
  return json({ submission_id: id, need: items.map((i) => i.sha256), sheets }, 201);
}

/**
 * Gives up a key's unfinished uploads when it starts a different pack. The pictures that never
 * arrived go back to the day's quotas; the ones that did arrive still count, so starting over and
 * over can't upload without limit.
 */
async function giveUpUnfinished(env: Env, key: string): Promise<void> {
  const { results } = await env.DB.prepare(
    `SELECT s.id, s.ip_hash, s.created_at, COUNT(i.sha256) - COALESCE(SUM(i.received), 0) AS missing
     FROM submissions s LEFT JOIN items i ON i.submission = s.id
     WHERE s.key = ?1 AND s.status = 'open' GROUP BY s.id`,
  )
    .bind(key)
    .all<{ id: string; ip_hash: string; created_at: number; missing: number }>();
  const today = dayOf(now());
  for (const s of results) {
    const done = await env.DB.prepare("UPDATE submissions SET status = 'expired' WHERE id = ?1 AND status = 'open'").bind(s.id).run();
    if (done.meta.changes !== 1) continue;
    await removeAll(env.HOLD, `hold/${s.id}/`);
    const day = dayOf(s.created_at);
    if (day === today && s.missing > 0) {
      await giveBack(env, day, "key:pictures", key, s.missing);
      await giveBack(env, day, "net:pictures", s.ip_hash, s.missing);
      await giveBack(env, day, "global", "pictures", s.missing);
    }
  }
}

/** The author's open submission `id`; anyone else's, or one that isn't open, is an error. */
async function ownOpen(env: Env, id: string, account: Account): Promise<Submission> {
  const s = await loadSubmission(env, id);
  // Someone else's submission looks exactly like one that doesn't exist.
  if (!s || s.key !== account.key) throw fail(404, "not_found", "There's no such pack.");
  if (s.status !== "open") throw fail(409, "closed", "That pack has already been sent for review.");
  return s;
}

/** Stores one picture of an open submission, once it is exactly the picture the pack declared. */
export async function putItem(request: Request, env: Env, id: string, sha: string): Promise<Response> {
  const signed = await verifySigned(request, env, MAX_PICTURE_BYTES);
  const account = await requireAccount(env, signed.key);
  await ownOpen(env, id, account);
  const item = await env.DB.prepare("SELECT file, bytes, width, height, received FROM items WHERE submission = ?1 AND sha256 = ?2")
    .bind(id, sha)
    .first<{ file: string; bytes: number; width: number; height: number; received: number }>();
  if (!item) throw fail(404, "not_found", "That picture isn't part of this pack.");
  if (item.received) return json({ received: true });
  if (signed.body.byteLength !== item.bytes || signed.digest !== sha) {
    throw fail(400, "mismatch", `The copy of ${item.file} that arrived isn't the one the pack described. Try sending it again.`);
  }
  let picture;
  try {
    picture = inspect(signed.body);
  } catch (e) {
    if (e instanceof PictureError) throw fail(400, "bad_picture", `${item.file} ${e.message}.`);
    throw e;
  }
  if (picture.format !== formatOf(item.file)) {
    throw fail(400, "bad_picture", `${item.file} isn't the kind of picture its name says it is.`);
  }
  if (picture.width !== item.width || picture.height !== item.height) {
    throw fail(400, "bad_picture", `${item.file} is ${picture.width}×${picture.height} px, not the size the pack described.`);
  }
  await env.HOLD.put(`hold/${id}/${sha}`, signed.body, {
    sha256: sha,
    httpMetadata: { contentType: CONTENT_TYPES[picture.format] },
    customMetadata: { file: item.file },
  });
  await env.DB.prepare("UPDATE items SET received = 1 WHERE submission = ?1 AND sha256 = ?2").bind(id, sha).run();
  return json({ received: true });
}

/** Stores contact sheet `n`: the pictures small, side by side, for the triage and for a review on a phone. */
export async function putSheet(request: Request, env: Env, id: string, n: number): Promise<Response> {
  const signed = await verifySigned(request, env, MAX_SHEET_BYTES);
  const account = await requireAccount(env, signed.key);
  const s = await ownOpen(env, id, account);
  if (!Number.isInteger(n) || n < 0 || n >= s.sheets) throw fail(404, "not_found", "This pack doesn't have that many sheets.");
  let sheet;
  try {
    sheet = inspect(signed.body);
  } catch (e) {
    if (e instanceof PictureError) throw fail(400, "bad_picture", `The contact sheet ${e.message}.`);
    throw e;
  }
  for (const side of [sheet.width, sheet.height]) {
    if (side < MIN_SHEET_SIDE || side > MAX_SHEET_SIDE) {
      throw fail(400, "bad_picture", `A contact sheet is ${MIN_SHEET_SIDE} to ${MAX_SHEET_SIDE} px on each side.`);
    }
  }
  await env.HOLD.put(`hold/${id}/sheet-${n}`, signed.body, {
    sha256: signed.digest,
    httpMetadata: { contentType: CONTENT_TYPES[sheet.format] },
  });
  await env.DB.prepare("UPDATE submissions SET sheets_received = sheets_received | ?2 WHERE id = ?1").bind(id, 1 << n).run();
  return json({ received: true });
}

/**
 * Closes the upload and puts the pack in the review queue: its words checked against the lists,
 * its contact sheets shown to the triage model when there is budget, and the maintainer told at
 * once if anything urgent turned up.
 */
export async function finalize(request: Request, env: Env, ctx: ExecutionContext, id: string): Promise<Response> {
  const signed = await verifySigned(request, env, MAX_JSON_BYTES);
  const account = await requireAccount(env, signed.key);
  const s = await ownOpen(env, id, account);
  const missing = (await loadItems(env, id)).filter((i) => !i.received);
  if (missing.length > 0) {
    const what = missing.length === 1 ? `${missing[0].file} hasn't` : `${missing.length} pictures haven't`;
    throw fail(409, "missing", `${what} arrived yet. Send ${missing.length === 1 ? "it" : "them"} and try again.`);
  }

  const manifest = JSON.parse(s.manifest) as { name: string; tags: string[]; skins: { name: string; tags: string[] }[] };
  const flags: Flag[] = textFlags(
    [
      { label: "the pack name", text: manifest.name },
      { label: "the pack tags", text: manifest.tags.join(", ") },
      { label: "the skin names", text: manifest.skins.map((k) => k.name).join(", ") },
      { label: "the skin tags", text: manifest.skins.flatMap((k) => k.tags).join(", ") },
      { label: "the credits", text: s.notes },
    ],
    env.EXTRA_BLOCKLIST,
  );
  const all = (1 << s.sheets) - 1;
  if ((s.sheets_received & all) !== all) flags.push({ code: "sheet:missing", severity: "normal", detail: "not every contact sheet arrived" });

  const sheets: Uint8Array[] = [];
  for (let n = 0; n < s.sheets; n++) {
    const object = await env.HOLD.get(`hold/${id}/sheet-${n}`);
    if (object) sheets.push(new Uint8Array(await object.arrayBuffer()));
  }
  const looked = await triage(env, sheets);
  flags.push(...looked.flags);
  if (!looked.looked && env.AI) flags.push({ code: "ai:skipped", severity: "normal", detail: "the triage model didn't look at every sheet" });

  // Only real findings make a pack "flagged"; a skipped look just leaves a note for the maintainer.
  const findings = flags.filter((f) => f.code !== "ai:skipped");
  const status = findings.length > 0 ? "flagged" : "pending";
  const done = await env.DB.prepare("UPDATE submissions SET status = ?2, flags = ?3, finalized_at = ?4 WHERE id = ?1 AND status = 'open'")
    .bind(id, status, JSON.stringify(flags), now())
    .run();
  if (done.meta.changes !== 1) throw fail(409, "closed", "That pack has already been sent for review.");

  const urgent = findings.some((f) => f.severity === "high");
  await record(env, "submitted", id, `${s.name} by ${account.handle}: ${s.items} pictures, ${flags.length} flags`, urgent ? "high" : "normal");
  // A flagged pack is worth a look before the digest: at once, and loudly when it's urgent.
  if (findings.length > 0) {
    const review = await makeLink(env, "review", id);
    ctx.waitUntil(
      alert(
        env,
        {
          title: `${urgent ? "Urgent" : "Flagged"}: "${s.name}" needs a look`,
          lines: [
            `${s.items} pictures by ${account.handle} (${account.tier}).`,
            ...findings.map((f) => `${f.code}: ${f.detail}`),
            "It is not public. Look before anyone else can.",
          ],
          links: review ? [{ label: "Review it", url: review }] : [],
        },
        urgent ? "urgent" : "flagged",
      ),
    );
  }
  return json({ status: "in_review" });
}

/** The author's own packs, newest first. */
export async function list(request: Request, env: Env): Promise<Response> {
  const signed = await verifySigned(request, env, 0);
  const account = await env.DB.prepare("SELECT key FROM keys WHERE key = ?1").bind(signed.key).first();
  if (!account) return json({ submissions: [] });
  const { results } = await env.DB.prepare("SELECT * FROM submissions WHERE key = ?1 ORDER BY created_at DESC LIMIT 100")
    .bind(signed.key)
    .all<Submission>();
  return json({ submissions: results.map(authorView) });
}

/** The author takes a pack back: out of the queue, or out of the public bucket if it was approved. */
export async function remove(request: Request, env: Env, id: string): Promise<Response> {
  const signed = await verifySigned(request, env, 0);
  // A banned key can still take its own packs down.
  const s = await loadSubmission(env, id);
  if (!s || s.key !== signed.key) throw fail(404, "not_found", "There's no such pack.");
  await withdraw(env, s);
  return json({ status: "withdrawn" });
}
