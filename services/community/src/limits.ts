/**
 * Every limit the service holds people to, in one place. The pack limits are the ones
 * folderskin_core::pack sets (and docs/PACKS.md describes), so a pack that is approved here is a
 * pack `folderskin-tools packs check` accepts.
 */

/** What a device key may do in a day, and how many of its packs can wait for review at once. */
export const TIERS = {
  /** A newly verified computer: one full pack a day, and one waiting at a time. */
  probation: { submissions: 1, pictures: 50, waiting: 1 },
  /** Once a pack of theirs has been approved. */
  active: { submissions: 3, pictures: 150, waiting: 3 },
  /** Set by the maintainer for people they know. */
  trusted: { submissions: 10, pictures: 500, waiting: 10 },
  banned: { submissions: 0, pictures: 0, waiting: 0 },
} as const;
export type Tier = keyof typeof TIERS;
export const isTier = (value: unknown): value is Tier => typeof value === "string" && value in TIERS;

/** What one network (an IPv4 /24 or IPv6 /48) may do in a day, whatever keys it uses. */
export const PER_NETWORK = { verifications: 5, submissions: 6, pictures: 300, reports: 10 } as const;

/** Name changes a key gets in a day. */
export const HANDLE_CHANGES_PER_DAY = 3;

/** Pictures the whole service takes in a day unless GLOBAL_DAILY_PICTURES says otherwise. */
export const DEFAULT_GLOBAL_DAILY_PICTURES = 1500;
/** Submissions waiting for a decision at once unless MAX_WAITING says otherwise. */
export const DEFAULT_MAX_WAITING = 150;

// ---- the pack contract (folderskin_core::pack) ----
export const PACK_VERSION = 1;
export const MAX_SKINS = 50;
export const MAX_PICTURE_BYTES = 2 * 1024 * 1024;
export const MIN_PICTURE_SIDE = 256;
export const MAX_PICTURE_SIDE = 1024;
export const MAX_PACK_NAME_CHARS = 40;
export const MAX_SKIN_NAME_CHARS = 60;
export const MAX_PACK_TAGS = 5;
export const MAX_SKIN_TAGS = 3;
export const MAX_TAG_CHARS = 24;
export const LICENSES = ["CC0-1.0", "CC-BY-4.0", "MIT"] as const;
export const PICTURE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"] as const;

// ---- what the service adds ----
/** Where the pictures came from, as the app asks it. */
export const SOURCES = ["own", "ai", "mixed", "licensed"] as const;
/** The version of docs/PACK-TERMS.md the app shows. A submission agreeing to another is turned away. */
export const TERMS_VERSION = 1;
export const MAX_NOTES_CHARS = 400;
/** A contact sheet: the pack's pictures small, side by side, for triage and for review on a phone. */
export const PICTURES_PER_SHEET = 16;
export const MAX_SHEET_BYTES = 300 * 1024;
export const MIN_SHEET_SIDE = 128;
export const MAX_SHEET_SIDE = 2048;
/** The largest JSON body anything takes. */
export const MAX_JSON_BYTES = 64 * 1024;
/** How long an upload that was never finished is kept before it is cleared away. */
export const OPEN_FOR_SECONDS = 24 * 3600;
/** How far a request's time may be from ours. */
export const CLOCK_SKEW_SECONDS = 300;
/** How long a verification link from the app stays usable: long enough to solve a challenge. */
export const VERIFY_LINK_SECONDS = 30 * 60;
