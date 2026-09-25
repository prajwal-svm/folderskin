/**
 * The pages behind the maintainer's phone links (links.ts): GET shows what the link will do and the
 * pack's contact sheets; POST, from the page's own buttons, does it and spends the link. Turning a
 * pack down or taking it down can be marked as abuse, which bans as the admin API's `"ban": true`
 * does, and an approval asks folderskin-community to publish the pack as the admin API's does.
 */
import type { Env } from "./env";
import { fail, html, HttpError } from "./http";
import { linkUsed, readLink, useLink, type Link } from "./links";
import { messagePage, pausePage, reviewPage, takedownPage, type ReviewInfo } from "./pages";
import { banWords, type Ban } from "./penalties";
import { pauseState, setPause } from "./quota";
import { approve, flagsOf, loadSubmission, readReasons, reject, takedown } from "./store";
import { record } from "./notify";

const gone = () =>
  html(
    messagePage("This link doesn't work any more", "It has been used already, or it has run out. Use folderskin-tools community queue to act instead."),
    410,
  );

async function valid(env: Env, token: string): Promise<Link | null> {
  const link = await readLink(env, token);
  if (!link || (await linkUsed(env, link))) return null;
  return link;
}

async function info(env: Env, id: string): Promise<ReviewInfo | null> {
  const s = await loadSubmission(env, id);
  if (!s) return null;
  const account = await env.DB.prepare("SELECT handle, tier FROM keys WHERE key = ?1").bind(s.key).first<{ handle: string; tier: string }>();
  const { results } = await env.DB.prepare("SELECT reason, details FROM reports WHERE submission = ?1 ORDER BY created_at DESC LIMIT 10")
    .bind(id)
    .all<{ reason: string; details: string }>();
  return {
    name: s.name,
    handle: account?.handle ?? "unknown",
    tier: account?.tier ?? "unknown",
    status: s.status,
    pictures: s.items,
    license: s.license,
    source: s.source,
    notes: s.notes,
    folder: s.exported_at ? (s.folder ?? s.pack_id) : null,
    flags: flagsOf(s),
    sheets: s.sheets,
    reports: results,
  };
}

export async function showLink(env: Env, token: string): Promise<Response> {
  const link = await valid(env, token);
  if (!link) return gone();
  if (link.action === "pause") return html(pausePage((await pauseState(env)).paused));
  const about = await info(env, link.subject);
  if (!about) return html(messagePage("That pack is gone", "There's no such pack any more."), 404);
  return html(link.action === "review" ? reviewPage(about, token) : takedownPage(about, token));
}

/** A contact sheet, for the page's pictures: allowed while the link still works, and it isn't spent by looking. */
export async function linkSheet(env: Env, token: string, n: number): Promise<Response> {
  const link = await valid(env, token);
  if (!link || link.action === "pause" || !Number.isInteger(n) || n < 0 || n > 3) throw fail(404, "not_found", "There's nothing here.");
  const object = await env.HOLD.get(`hold/${link.subject}/sheet-${n}`);
  if (!object) throw fail(404, "not_found", "That sheet isn't in storage any more.");
  return new Response(object.body, {
    headers: {
      "Content-Type": object.httpMetadata?.contentType ?? "image/jpeg",
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
      "Content-Security-Policy": "default-src 'none'; sandbox",
    },
  });
}

/** What a decision banned, as a sentence for the page, or nothing when it banned nothing. */
function bansSaid(bans: Ban[]): string {
  if (bans.length === 0) return "";
  const who = (b: Ban) => (b.kind === "key" ? "The computer that sent it is" : "Its network is");
  return ` ${bans.map((b) => `${who(b)} ${banWords(b)}.`).join(" ")}`;
}

export async function actOnLink(request: Request, env: Env, ctx: ExecutionContext, token: string): Promise<Response> {
  const link = await valid(env, token);
  if (!link) return gone();
  const form = await request.formData().catch(() => null);
  const decision = form?.get("decision");
  const reason = form?.get("reason");
  // The page's "abuse" box, on the forms that turn a pack down or take it down.
  const ban = form?.get("ban") === "1";
  const allowed: Record<Link["action"], string[]> = {
    review: ["approve", "reject", "takedown"],
    takedown: ["takedown"],
    pause: ["pause", "resume"],
  };
  if (typeof decision !== "string" || !allowed[link.action].includes(decision)) {
    return html(messagePage("That didn't work", "This link can't do that."), 400);
  }
  const needsReason = decision === "reject" || decision === "takedown";
  let reasons: string[] = [];
  try {
    if (needsReason) reasons = readReasons([reason]);
  } catch (e) {
    if (e instanceof HttpError) return html(messagePage("That didn't work", e.message), e.status);
    throw e;
  }
  // Spent before acting, so two presses (or two phones) can't both act.
  if (!(await useLink(env, link))) return gone();
  try {
    switch (decision) {
      case "approve": {
        const { pack_id } = await approve(env, ctx, link.subject, "");
        return html(
          messagePage("Approved", `It's approved as ${pack_id}. folderskin-community's workflow brings it into the repository and publishes it.`),
        );
      }
      case "reject": {
        const bans = await reject(env, link.subject, reasons, "", ban);
        return html(messagePage("Turned down", `The author sees the reason in FolderSkin, and the pictures are deleted.${bansSaid(bans)}`));
      }
      case "takedown": {
        const { folder, bans } = await takedown(env, link.subject, reasons, "", ban);
        const repo = folder ? ` It was pulled into folderskin-community already, so remove packs/${folder} there too.` : "";
        return html(messagePage("Taken down", `It's gone from FolderSkin's storage.${repo}${bansSaid(bans)}`));
      }
      case "pause":
      case "resume": {
        const paused = decision === "pause";
        await setPause(env, { paused, message: "" });
        await record(env, paused ? "paused" : "resumed", "", "from a phone link", paused ? "high" : "normal");
        return html(messagePage(paused ? "Sharing is paused" : "Sharing has resumed", paused ? "No new packs or verifications are taken until you resume." : "New packs are taken again."));
      }
    }
  } catch (e) {
    if (e instanceof HttpError) return html(messagePage("That didn't work", e.message), e.status);
    throw e;
  }
  return html(messagePage("That didn't work", "This link can't do that."), 400);
}
