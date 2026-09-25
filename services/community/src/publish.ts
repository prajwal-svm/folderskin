/**
 * Getting approved packs published: what folderskin-community's packs workflow needs from here.
 * The workflow pulls each approved pack with `folderskin-tools community pull`, checks it, commits
 * it, builds the catalog and uploads the catalog's v2/ tree into R2 through this Worker.
 *
 *   repository_dispatch pack-approved         sent the moment a pack is approved (askToPublish)
 *   GET /v1/exports/pending                   how many approved packs wait: the workflow asks
 *                                             every 15 minutes, and only does the work when some do
 *   PUT /v1/admin/tree/<path>                 one file of the catalog, into the PACKS bucket
 *
 * The dispatch is sent after the approval has been answered, from ctx.waitUntil, so no approval
 * waits on GitHub or fails because of it. Without GITHUB_DISPATCH_TOKEN it isn't sent at all, and
 * when GitHub turns it down or can't be reached, the workflow's next look at the pending count
 * picks the pack up instead; either way the attempt is written to `events`. Only this one fixed
 * address is ever fetched, and the token is never logged.
 */
import { requirePublisher, verifySignedDigest } from "./auth";
import { now } from "./bytes";
import type { Env } from "./env";
import { fail, json } from "./http";
import { record } from "./notify";

/** GitHub's endpoint for starting folderskin-community's workflows from outside. */
export const DISPATCH_URL = "https://api.github.com/repos/prajwal-svm/folderskin-community/dispatches";
/** What the workflow listens for (`repository_dispatch: {types: [pack-approved]}`). */
export const DISPATCH_EVENT = "pack-approved";
/** How long GitHub gets to answer before the pack is left to the workflow's own schedule. */
const TIMEOUT_MS = 10_000;

/** Asks folderskin-community to publish approved submission `submission`, whose id is `pack`. Never throws. */
export async function askToPublish(env: Env, submission: string, pack: string): Promise<void> {
  const token = env.GITHUB_DISPATCH_TOKEN?.trim();
  if (!token) {
    await record(env, "publish", submission, `not asked: GITHUB_DISPATCH_TOKEN isn't set, so the scheduled run publishes ${pack}`);
    return;
  }
  let status: number;
  try {
    const response = await fetch(DISPATCH_URL, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        // GitHub turns away API requests that don't name themselves.
        "User-Agent": "folderskin-community",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ event_type: DISPATCH_EVENT, client_payload: { submission, pack } }),
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    status = response.status;
    // Nothing in the answer is needed: GitHub says 204 with no body when it starts the workflow.
    await response.body?.cancel();
  } catch {
    // Not the error itself, which could repeat the request.
    await record(env, "publish_failed", submission, `GitHub couldn't be reached, so the scheduled run publishes ${pack}`);
    return;
  }
  if (status >= 200 && status < 300) await record(env, "publish", submission, `folderskin-community was asked to publish ${pack}`);
  else await record(env, "publish_failed", submission, `GitHub answered ${status}, so the scheduled run publishes ${pack}`);
}

/** How long this isolate answers with the pending count it read last. */
const PENDING_KEPT_SECONDS = 60;
let pendingKept: { n: number; at: number } | null = null;

/** Forgets the pending count this isolate read, so each test starts from nothing. */
export function forgetPending(): void {
  pendingKept = null;
}

/**
 * GET /v1/exports/pending: how many approved packs haven't been pulled into folderskin-community
 * yet, and nothing about them. Public, so the workflow can ask without a key, and cheap to ask
 * however often: the burst limit covers it as it covers everything, the count comes from an index
 * of its own (migrations/0004), and each isolate reads it at most once a minute. A Worker's own
 * answers aren't cached on Cloudflare's side, so that is what keeps a flood off the database.
 */
export async function pending(env: Env, at = now()): Promise<Response> {
  if (!pendingKept || at - pendingKept.at >= PENDING_KEPT_SECONDS) {
    const row = await env.DB.prepare("SELECT COUNT(*) AS n FROM submissions WHERE status = 'approved' AND exported_at IS NULL").first<{ n: number }>();
    pendingKept = { n: row?.n ?? 0, at };
  }
  return json({ pending: pendingKept.n }, 200, { "Cache-Control": `public, max-age=${PENDING_KEPT_SECONDS}` });
}

// ---- the catalog tree ----

/** The largest file the catalog has, with room to grow: the gzipped catalog is a few MB today. */
export const MAX_TREE_FILE_BYTES = 64 * 1024 * 1024;
/** The longest path a file in the tree can have, v2/ included. */
const MAX_TREE_PATH_CHARS = 200;
/** The tree's own name for head.json, the one file whose bytes change under the same name. */
const HEAD = "v2/head.json";

/** Each kind of file in the tree, by its extension. Anything else is stored as plain bytes. */
const CONTENT_TYPES: Record<string, string> = {
  json: "application/json",
  webp: "image/webp",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gz: "application/gzip",
};

/**
 * Whether `path` can be a file in the catalog's tree: v2/, then letters, digits, dots, dashes,
 * underscores and slashes, at most 200 characters in all, with no empty part, no `.` part and no
 * `..` anywhere. It becomes an R2 key as it is, and packs.folderskin.app serves it under that
 * name.
 */
export function isTreePath(path: string): boolean {
  if (path.length > MAX_TREE_PATH_CHARS || !path.startsWith("v2/") || !/^[A-Za-z0-9._/-]+$/.test(path) || path.includes("..")) return false;
  return path
    .slice(3)
    .split("/")
    .every((part) => part !== "" && part !== ".");
}

/**
 * How long browsers and Cloudflare may keep a file. head.json says which catalog is current, so it
 * is kept a minute at most; every other file is named after what is in it and never changes, so
 * it can be kept for good.
 */
export const cacheControlOf = (path: string) => (path === HEAD ? "public, max-age=60" : "public, max-age=31536000, immutable");

/** A file's type, by its extension. */
export const contentTypeOf = (path: string) => {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
  return CONTENT_TYPES[extension] ?? "application/octet-stream";
};

/**
 * PUT /v1/admin/tree/<path>: one file of the catalog, from folderskin-community's workflow,
 * signed by a key in PUBLISH_KEYS or ADMIN_KEYS. The body goes from the request to R2 as it arrives, never read
 * here, and R2 checks it against the SHA-256 the signature covers (verifySignedDigest), so the
 * Worker spends no time hashing a file of up to 64 MB.
 */
export async function putTree(request: Request, env: Env, path: string): Promise<Response> {
  // Who is asking first, so the path rules aren't told to anyone but the keys that may publish.
  const { digest } = await verifySignedDigest(request, env, (key) => requirePublisher(env, key));
  if (!isTreePath(path)) {
    throw fail(400, "bad_path", "A catalog path is v2/ and then letters, digits, dots, dashes, underscores and slashes, at most 200 characters.");
  }
  const declared = request.headers.get("Content-Length") ?? "";
  if (!/^\d{1,12}$/.test(declared)) throw fail(411, "length_required", "Say how big the file is with Content-Length.");
  if (Number(declared) > MAX_TREE_FILE_BYTES) {
    throw fail(413, "too_large", `That is bigger than the ${MAX_TREE_FILE_BYTES / 1024 / 1024} MB a catalog file can be.`);
  }

  // Straight from the request to R2, which knows the body's length from Content-Length (HTTP keeps
  // a body to it), and fails the upload, storing nothing, when the bytes don't have the SHA-256
  // the signature covers.
  try {
    await env.PACKS.put(path, request.body, {
      sha256: digest,
      httpMetadata: { contentType: contentTypeOf(path), cacheControl: cacheControlOf(path) },
    });
  } catch (e) {
    if (e instanceof Error && /checksum|did not match/i.test(e.message)) {
      throw fail(400, "mismatch", "The file that arrived isn't the one that was signed for. Send it again.");
    }
    throw e;
  }
  return json({ stored: true });
}
