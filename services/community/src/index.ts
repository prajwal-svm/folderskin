/**
 * FolderSkin's community service: sharing a pack without a GitHub account.
 *
 * The app verifies the computer once (a Turnstile check in the browser, bound to the computer's
 * Ed25519 key), then sends packs here, signed with that key. Every pack waits in a private bucket
 * until the maintainer approves it; approved packs are pulled into folderskin-community's packs/ with
 * `folderskin-tools community pull` and published the same way every other pack is.
 *
 * Built for the Workers free plan: no picture is ever decoded here (10 ms of CPU per request), the
 * quotas and the global daily cap keep D1 and R2 inside their free allowances, and a burst limit
 * sits in front of all of it. Cloudflare's own network absorbs floods before they get this far.
 */
import * as account from "./account";
import * as admin from "./admin";
import { daily } from "./daily";
import type { Env } from "./env";
import { errorResponse, fail, html, HttpError } from "./http";
import { networkHash } from "./ip";
import { verifyPage, VERIFY_SCRIPT } from "./pages";
import { actOnLink, linkSheet, showLink } from "./phone";
import { report } from "./reports";
import * as submissions from "./submissions";

const ID = "(sub_[a-z2-7]{20})";

type Handler = (request: Request, env: Env, ctx: ExecutionContext, params: string[]) => Promise<Response> | Response;
type Route = [method: string, pattern: RegExp, handler: Handler];

const ROUTES: Route[] = [
  ["GET", /^\/verify$/, (_, env) => html(verifyPage(env.TURNSTILE_SITE_KEY ?? ""), 200, { turnstile: true })],
  ["GET", /^\/verify\.js$/, () => script(VERIFY_SCRIPT)],
  ["GET", /^\/v1\/status$/, (_, env) => account.status(env)],
  ["POST", /^\/v1\/keys\/verify$/, (req, env) => account.verifyKey(req, env)],
  ["GET", /^\/v1\/me$/, (req, env) => account.me(req, env)],
  ["POST", /^\/v1\/me$/, (req, env) => account.rename(req, env)],
  ["POST", /^\/v1\/submissions$/, (req, env) => submissions.create(req, env)],
  ["GET", /^\/v1\/submissions$/, (req, env) => submissions.list(req, env)],
  ["PUT", new RegExp(`^/v1/submissions/${ID}/items/([0-9a-f]{64})$`), (req, env, _, [id, sha]) => submissions.putItem(req, env, id, sha)],
  ["PUT", new RegExp(`^/v1/submissions/${ID}/sheets/([0-3])$`), (req, env, _, [id, n]) => submissions.putSheet(req, env, id, Number(n))],
  ["POST", new RegExp(`^/v1/submissions/${ID}/finalize$`), (req, env, ctx, [id]) => submissions.finalize(req, env, ctx, id)],
  ["DELETE", new RegExp(`^/v1/packs/${ID}$`), (req, env, ctx, [id]) => submissions.remove(req, env, ctx, id)],
  ["POST", /^\/v1\/reports$/, (req, env, ctx) => report(req, env, ctx)],

  ["GET", /^\/v1\/admin\/queue$/, (req, env) => admin.queue(req, env)],
  ["GET", new RegExp(`^/v1/admin/submissions/${ID}$`), (req, env, _, [id]) => admin.detail(req, env, id)],
  ["GET", new RegExp(`^/v1/admin/submissions/${ID}/items/([0-9a-f]{64})$`), (req, env, _, [id, sha]) => admin.item(req, env, id, sha)],
  ["GET", new RegExp(`^/v1/admin/submissions/${ID}/sheets/([0-3])$`), (req, env, _, [id, n]) => admin.sheet(req, env, id, Number(n))],
  ["POST", new RegExp(`^/v1/admin/submissions/${ID}/decision$`), (req, env, _, [id]) => admin.decision(req, env, id)],
  ["POST", new RegExp(`^/v1/admin/submissions/${ID}/takedown$`), (req, env, _, [id]) => admin.takeDown(req, env, id)],
  ["POST", /^\/v1\/admin\/keys\/([A-Za-z0-9_-]{43})\/tier$/, (req, env, _, [key]) => admin.setTier(req, env, key)],
  ["POST", /^\/v1\/admin\/pause$/, (req, env) => admin.pause(req, env)],
  ["GET", /^\/v1\/admin\/exports$/, (req, env) => admin.exportList(req, env)],
  ["GET", new RegExp(`^/v1/admin/exports/${ID}/pack\\.json$`), (req, env, _, [id]) => admin.exportManifest(req, env, id)],
  ["GET", new RegExp(`^/v1/admin/exports/${ID}/files/([A-Za-z0-9._-]{1,64})$`), (req, env, _, [id, file]) => admin.exportFile(req, env, id, file)],
  ["POST", new RegExp(`^/v1/admin/exports/${ID}/done$`), (req, env, _, [id]) => admin.exportDone(req, env, id)],
  ["GET", /^\/v1\/admin\/reports$/, (req, env) => admin.reports(req, env)],

  ["GET", /^\/l\/([A-Za-z0-9._-]{1,300})\/sheets\/([0-3])$/, (_, env, __, [token, n]) => linkSheet(env, token, Number(n))],
  ["GET", /^\/l\/([A-Za-z0-9._-]{1,300})$/, (_, env, __, [token]) => showLink(env, token)],
  ["POST", /^\/l\/([A-Za-z0-9._-]{1,300})$/, (req, env, __, [token]) => actOnLink(req, env, token)],
];

function script(source: string): Response {
  return new Response(source, {
    headers: {
      "Content-Type": "text/javascript; charset=utf-8",
      "Cache-Control": "public, max-age=300",
      "X-Content-Type-Options": "nosniff",
    },
  });
}

async function route(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  const { pathname } = new URL(request.url);
  // A burst limit per network before anything else, the database included.
  if (env.BURST && pathname !== "/") {
    const { success } = await env.BURST.limit({ key: await networkHash(env, request) });
    if (!success) throw fail(429, "slow_down", "Too many requests from your network. Wait a minute and try again.", { "Retry-After": "60" });
  }
  let allowed = false;
  for (const [method, pattern, handler] of ROUTES) {
    const match = pattern.exec(pathname);
    if (!match) continue;
    if (method !== request.method) {
      allowed = true;
      continue;
    }
    return handler(request, env, ctx, match.slice(1));
  }
  if (pathname === "/") return new Response("FolderSkin community service\n", { headers: { "Content-Type": "text/plain" } });
  if (allowed) throw fail(405, "method", "That isn't something this address does.");
  throw fail(404, "not_found", "There's nothing here.");
}

export default {
  async fetch(request, env, ctx): Promise<Response> {
    try {
      return await route(request, env, ctx);
    } catch (e) {
      if (e instanceof HttpError) return errorResponse(e);
      // The error's kind and message only: never the request, whose headers and body are the user's.
      console.error("folderskin-community: unexpected error", e instanceof Error ? `${e.name}: ${e.message}` : typeof e);
      return errorResponse(fail(500, "server", "Something went wrong on FolderSkin's side. Please try again in a while."));
    }
  },
  async scheduled(_controller, env, ctx): Promise<void> {
    ctx.waitUntil(daily(env));
  },
} satisfies ExportedHandler<Env>;
