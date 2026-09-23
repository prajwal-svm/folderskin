-- The community service's tables. Times are unix seconds. No table holds an IP address: where a
-- request's origin matters (quotas, reports), it is an HMAC of the network prefix under a key that
-- changes every day (src/ip.ts), so yesterday's rows can't be linked to today's.

-- A computer that has passed the person check. The key is the device's Ed25519 public key; the
-- handle is the name its packs are credited to, shaped like a GitHub user name so pack.json v1
-- accepts it.
CREATE TABLE keys (
  key TEXT PRIMARY KEY,
  handle TEXT NOT NULL UNIQUE COLLATE NOCASE,
  tier TEXT NOT NULL DEFAULT 'probation'
    CHECK (tier IN ('probation', 'active', 'trusted', 'banned')),
  verified_at INTEGER NOT NULL,
  approved INTEGER NOT NULL DEFAULT 0,
  rejected INTEGER NOT NULL DEFAULT 0
);

-- One pack sent for review. `open` while its pictures upload, `pending` or `flagged` once it is
-- finalised, then a decision. `manifest` is the pack without the fields the service decides
-- itself (version, author, licence).
CREATE TABLE submissions (
  id TEXT PRIMARY KEY,
  key TEXT NOT NULL REFERENCES keys(key),
  status TEXT NOT NULL
    CHECK (status IN ('open', 'pending', 'flagged', 'approved', 'rejected', 'withdrawn', 'taken_down', 'expired')),
  name TEXT NOT NULL,
  license TEXT NOT NULL,
  source TEXT NOT NULL,
  terms_version INTEGER NOT NULL,
  manifest TEXT NOT NULL,
  notes TEXT NOT NULL DEFAULT '',
  items INTEGER NOT NULL,
  sheets INTEGER NOT NULL,
  sheets_received INTEGER NOT NULL DEFAULT 0,
  flags TEXT NOT NULL DEFAULT '[]',
  reasons TEXT NOT NULL DEFAULT '[]',
  note TEXT NOT NULL DEFAULT '',
  pack_id TEXT,
  ip_hash TEXT NOT NULL DEFAULT '',
  -- A hash of what was declared, so sending the same pack again resumes this upload.
  fingerprint TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  finalized_at INTEGER,
  decided_at INTEGER,
  exported_at INTEGER
);
CREATE INDEX submissions_by_key ON submissions (key, created_at DESC);
CREATE INDEX submissions_by_status ON submissions (status, finalized_at);
-- Two approved packs never share a folder name.
CREATE UNIQUE INDEX packs_by_id ON submissions (pack_id) WHERE status = 'approved';

-- The pictures a submission declared, and whether each has arrived.
CREATE TABLE items (
  submission TEXT NOT NULL REFERENCES submissions(id),
  sha256 TEXT NOT NULL,
  file TEXT NOT NULL,
  bytes INTEGER NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  received INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (submission, sha256)
);

-- Daily counts behind every quota: per key, per hashed network prefix and for the whole service.
CREATE TABLE counters (
  day TEXT NOT NULL,
  scope TEXT NOT NULL,
  id TEXT NOT NULL,
  n INTEGER NOT NULL,
  PRIMARY KEY (day, scope, id)
);

-- Single-use values: request signatures (so a captured request can't be sent twice), verification
-- nonces and phone links. Kept until they could no longer be accepted anyway.
CREATE TABLE seen (
  id TEXT PRIMARY KEY,
  expires INTEGER NOT NULL
);
CREATE INDEX seen_by_expiry ON seen (expires);

-- Switches the maintainer flips at run time, such as pausing uploads.
CREATE TABLE settings (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Reports from anyone, no account needed. `target` is stored as text and never fetched.
CREATE TABLE reports (
  id TEXT PRIMARY KEY,
  target TEXT NOT NULL,
  submission TEXT,
  reason TEXT NOT NULL CHECK (reason IN ('csam', 'ncii', 'copyright', 'terms', 'other')),
  details TEXT NOT NULL DEFAULT '',
  contact TEXT NOT NULL DEFAULT '',
  ip_hash TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  handled_at INTEGER
);
CREATE INDEX reports_by_time ON reports (created_at);

-- Pictures a decision turned down for what they show, so the same file can't simply be sent again.
CREATE TABLE blocked (
  sha256 TEXT PRIMARY KEY,
  reason TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

-- What happened, for the daily digest and for looking back: decisions, takedowns, notifications.
CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  subject TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT '',
  severity TEXT NOT NULL DEFAULT 'normal' CHECK (severity IN ('normal', 'high'))
);
CREATE INDEX events_by_time ON events (at);
