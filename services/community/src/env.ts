/**
 * The bindings, variables and secrets the service runs with. wrangler.toml says where each one
 * comes from; the secrets are set with `wrangler secret put` and never appear in the repository.
 */
export interface Env {
  DB: D1Database;
  /** Private: pictures waiting for review, and the contact sheets the review looks at. */
  HOLD: R2Bucket;
  /** Public: approved packs, laid out the way community/packs is. */
  PUBLIC: R2Bucket;
  /** Workers AI, for a first look at contact sheets. Optional: without it every pack waits for a person. */
  AI?: Vision;
  /** A burst limit in front of everything, before the database is touched. Optional. */
  BURST?: RateLimit;
  /** Email to the maintainer's own verified address through Email Routing. Optional. */
  MAILER?: SendEmail;

  /** Public keys (base64url, comma separated) whose signatures open the admin endpoints. */
  ADMIN_KEYS?: string;
  /** Where this service is reached, for the links it puts in notifications. */
  PUBLIC_BASE_URL?: string;
  TURNSTILE_SITE_KEY?: string;
  /** "1" stops new submissions and verifications, whatever the database says. */
  PAUSED?: string;
  /** Most pictures the whole service takes in a day; the review queue is full after that. */
  GLOBAL_DAILY_PICTURES?: string;
  /** Most submissions waiting for a decision at once. */
  MAX_WAITING?: string;
  /** Workers AI: the vision model, the neurons one contact sheet is budgeted at, and the day's budget. */
  TRIAGE_MODEL?: string;
  AI_NEURONS_PER_SHEET?: string;
  AI_DAILY_NEURONS?: string;
  /** More words to flag, comma separated, on top of the built-in list. */
  EXTRA_BLOCKLIST?: string;
  /** The maintainer's verified address (the send_email binding's destination) and who mail comes from. */
  MAIL_TO?: string;
  MAIL_FROM?: string;
  /** "discord", "slack", "telegram" or "ntfy"; worked out from the webhook's address when unset. */
  NOTIFY_WEBHOOK_KIND?: string;

  // ---- secrets ----
  TURNSTILE_SECRET?: string;
  /** Keys the daily HMAC of a network prefix. */
  IP_SALT?: string;
  /** Signs the single-use links in notifications. */
  LINK_SECRET?: string;
  /** Discord, Slack, Telegram or ntfy webhook for anything that can't wait for the digest. */
  NOTIFY_WEBHOOK_URL?: string;
}

/** The one call the service makes to Workers AI. The real binding has this shape among others. */
export interface Vision {
  run(model: string, inputs: Record<string, unknown>): Promise<unknown>;
}
