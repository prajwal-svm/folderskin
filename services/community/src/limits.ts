/**
 * Every limit the service holds people to, in one place. The pack limits are the ones
 * folderskin_core::pack sets (and docs/PACKS.md describes), so a pack that is approved here is a
 * pack `folderskin-tools packs check` accepts. The burst limit is the one number that lives
 * elsewhere: it is the BURST binding's, in wrangler.toml (120 requests a minute per network).
 */

/** What a device key may do in a day, and how many of its packs can wait for review at once. */
export const TIERS = {
  /** A newly verified computer: one pack waiting at a time, with room in the day to try again. */
  probation: { submissions: 2, pictures: 100, waiting: 1 },
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

// ---- backing off and bans (penalties.ts) ----
/** The wait after a first refused sharing request. Each strike after it doubles the wait. */
export const COOLDOWN_BASE_SECONDS = 60;
/** The longest a wait gets, however many strikes there have been. */
export const COOLDOWN_MAX_SECONDS = 24 * 3600;
/** Strikes start again from nothing after this long without a new one. */
export const STRIKES_RESET_SECONDS = 24 * 3600;
/** How long a mark for a pack the maintainer turned down counts against its key. */
export const MARK_SECONDS = 30 * 86400;
/** Marks within MARK_SECONDS that ban a key, for KEY_BAN_SECONDS. */
export const MARKS_TO_BAN = 3;
export const KEY_BAN_SECONDS = 30 * 86400;
/** Keys banned from one network within BANNED_KEYS_SECONDS that ban the network, for NETWORK_BAN_SECONDS. */
export const BANNED_KEYS_TO_BAN_NETWORK = 2;
export const BANNED_KEYS_SECONDS = 30 * 86400;
/** How long a network is banned: after a pack from it is turned down as abuse, or once enough of its keys are. */
export const NETWORK_BAN_SECONDS = 30 * 86400;
/**
 * How long a submission keeps the network it was sent from (as penalties hash it) after its
 * decision, so that turning it down, or taking it down in that time, can ban the network too.
 */
export const SUBMISSION_NETWORK_SECONDS = 30 * 86400;

// ---- the pack contract (folderskin_core::pack) ----
export const PACK_VERSION = 1;
export const MAX_SKINS = 50;
/** The most bytes a picture can have, 1.5 MB. */
export const MAX_PICTURE_BYTES = 1_572_864;
/**
 * The most a pack's pictures can come to together, 64 MB: a full pack of 50 detailed pictures,
 * saved losslessly, is about 1.2 MB a picture.
 */
export const MAX_PACK_BYTES = 64 * 1024 * 1024;
export const MIN_PICTURE_SIDE = 256;
export const MAX_PICTURE_SIDE = 1024;
export const MAX_PACK_NAME_CHARS = 40;
export const MAX_SKIN_NAME_CHARS = 60;
export const MAX_PACK_TAGS = 5;
export const MAX_SKIN_TAGS = 3;
export const MAX_TAG_CHARS = 24;
export const LICENSES = ["CC0-1.0", "CC-BY-4.0", "MIT"] as const;
/**
 * The file names a pack's pictures can have. A pack shared here now has to be lossless, so PNG or
 * lossless WebP (images.ts checks the bytes); `.jpg` and `.jpeg` stay valid names for the packs
 * already published with them.
 */
export const PICTURE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"] as const;
/** A pack id is at most 40 characters; a generated one is its name's slug cut to 33, a dash and 6 random characters. */
export const MAX_PACK_ID_CHARS = 40;
export const ID_BASE_CHARS = 33;
export const ID_SUFFIX_CHARS = 6;

// ---- what the service adds ----
/** Where the pictures came from, as the app asks it. */
export const SOURCES = ["own", "ai", "mixed", "licensed"] as const;
/**
 * The version of docs/PACK-TERMS.md the app shows. A submission agreeing to another is turned away.
 * Version 2 is the terms for sharing through this service alone, with no GitHub; its rules keep
 * version 1's numbers, which terms.ts's reasons point at.
 */
export const TERMS_VERSION = 2;
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
