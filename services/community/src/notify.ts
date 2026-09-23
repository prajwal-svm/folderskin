/**
 * Telling the maintainer, by a webhook (Discord, Slack, Telegram or ntfy) and by email to their
 * own verified address:
 *
 * - urgent: a report of abuse, or a pack the checks flagged as sexual, hateful or involving a
 *   child. Both channels at once, at the webhook's highest priority.
 * - flagged: a pack whose words or pictures a check caught (profanity, say). The webhook at once,
 *   at normal priority, so it can be looked at before it waits a day.
 * - digest: everything else, once a day from the cron trigger, by both channels.
 *
 * Every notice is also written to `events`. The webhook's address is a secret (it carries a
 * token), so it is never logged, and nothing here fetches an address that came from a request.
 */
import { now } from "./bytes";
import type { Env } from "./env";

export type Notice = {
  title: string;
  lines: string[];
  links?: { label: string; url: string }[];
};

export type Severity = "normal" | "high";

/** Keeps a line in `events`. Never throws: losing a log line mustn't fail the request it describes. */
export async function record(env: Env, kind: string, subject: string, detail: string, severity: Severity = "normal"): Promise<void> {
  try {
    await env.DB.prepare("INSERT INTO events (at, kind, subject, detail, severity) VALUES (?1, ?2, ?3, ?4, ?5)")
      .bind(now(), kind, subject, detail.slice(0, 2000), severity)
      .run();
  } catch {
    console.error("folderskin-community: couldn't record an event");
  }
}

export type Level = "urgent" | "flagged" | "digest";

/** Sends a notice now, by the channels its level goes to (see the top of this file). Never throws. */
export async function alert(env: Env, notice: Notice, level: Level = "urgent"): Promise<void> {
  await Promise.all([sendWebhook(env, notice, level === "urgent"), level === "flagged" ? undefined : sendEmail(env, notice)]);
}

export function noticeText(notice: Notice): string {
  const links = (notice.links ?? []).map((l) => `${l.label}: ${l.url}`);
  return [notice.title, "", ...notice.lines, ...(links.length ? ["", ...links] : [])].join("\n");
}

type Kind = "discord" | "slack" | "telegram" | "ntfy";

function webhookKind(env: Env, url: URL): Kind {
  const set = env.NOTIFY_WEBHOOK_KIND?.trim().toLowerCase();
  if (set === "discord" || set === "slack" || set === "telegram" || set === "ntfy") return set;
  if (url.hostname === "discord.com" || url.hostname === "discordapp.com") return "discord";
  if (url.hostname === "hooks.slack.com") return "slack";
  if (url.hostname === "api.telegram.org") return "telegram";
  return "ntfy";
}

/** The request each kind of webhook expects. Telegram's chat id goes in the webhook address's query. */
export function webhookRequest(kind: Kind, notice: Notice, urgent: boolean): RequestInit {
  const text = noticeText(notice);
  switch (kind) {
    case "discord":
      return json({ content: text.slice(0, 1900), allowed_mentions: { parse: [] } });
    case "slack":
      // Slack reads <!channel> and <@someone> in a message as mentions, and a pack's name or a
      // report's details are anyone's words, so its three special characters are escaped.
      return json({ text: text.slice(0, 3900).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;") });
    case "telegram":
      return json({ text: text.slice(0, 4000), disable_web_page_preview: true });
    case "ntfy": {
      const headers: Record<string, string> = {
        "Content-Type": "text/plain; charset=utf-8",
        // Header values have to be plain ASCII.
        Title: notice.title.replace(/[^\x20-\x7e]/g, "").slice(0, 200),
        Priority: urgent ? "5" : "3",
      };
      const first = notice.links?.[0]?.url;
      if (first) headers.Click = first;
      return { method: "POST", headers, body: text.slice(0, 3900) };
    }
  }
}

const json = (body: unknown): RequestInit => ({
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body),
});

async function sendWebhook(env: Env, notice: Notice, urgent: boolean): Promise<void> {
  const address = env.NOTIFY_WEBHOOK_URL?.trim();
  if (!address) return;
  let url: URL;
  try {
    url = new URL(address);
  } catch {
    console.error("folderskin-community: NOTIFY_WEBHOOK_URL isn't an address");
    return;
  }
  if (url.protocol !== "https:") {
    console.error("folderskin-community: NOTIFY_WEBHOOK_URL has to be https");
    return;
  }
  try {
    const response = await fetch(url.toString(), {
      ...webhookRequest(webhookKind(env, url), notice, urgent),
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) console.error(`folderskin-community: the webhook answered ${response.status}`);
  } catch {
    // Not the error itself: a failed fetch's message can repeat the address, and its token.
    console.error("folderskin-community: the webhook couldn't be reached");
  }
}

async function sendEmail(env: Env, notice: Notice): Promise<void> {
  if (!env.MAILER || !env.MAIL_TO || !env.MAIL_FROM) return;
  try {
    await env.MAILER.send({
      from: { name: "FolderSkin community", email: env.MAIL_FROM },
      to: env.MAIL_TO,
      subject: notice.title.slice(0, 180),
      text: noticeText(notice),
    });
  } catch {
    console.error("folderskin-community: the email couldn't be sent");
  }
}
