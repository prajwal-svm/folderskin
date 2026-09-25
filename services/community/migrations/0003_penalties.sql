-- Backing off and bans, on top of the daily quotas (src/penalties.ts; the numbers are in
-- src/limits.ts).
--
-- A network here is an HMAC of its prefix under a key made from IP_SALT for penalties alone
-- (src/ip.ts). Unlike the quotas' keys it doesn't change every day, since a network banned for 30
-- days has to be the same network all month; so it is kept only while it counts, and the daily
-- clean-up deletes it after that.

-- What a device key ("key:<key>") or a network ("net:<hash>") has earned by being turned away.
CREATE TABLE penalties (
  subject TEXT PRIMARY KEY,
  -- Refused sharing requests in a row, when the latest was, and until when every sharing request
  -- is refused: 60 seconds after the first strike, doubling with each one after it, up to a day.
  strikes INTEGER NOT NULL DEFAULT 0,
  struck_at INTEGER NOT NULL DEFAULT 0,
  cool_until INTEGER NOT NULL DEFAULT 0,
  -- A ban for a while, NULL when there is none, and why: 'marks' (a key with too many packs turned
  -- down), 'abuse' (a network a pack turned down as abuse came from) or 'keys' (a network too many
  -- banned keys came from). A key banned for good has the tier 'banned' instead.
  banned_until INTEGER,
  ban_reason TEXT NOT NULL DEFAULT ''
);

-- What counts against a subject for 30 days: a pack the maintainer turned down or took down,
-- against its key (`cause` is the submission), and a key that was banned, against the network
-- its pack came from (`cause` is the key). Enough of either within 30 days bans the subject.
CREATE TABLE marks (
  subject TEXT NOT NULL,
  cause TEXT NOT NULL,
  at INTEGER NOT NULL,
  PRIMARY KEY (subject, cause)
);

-- The network a submission was sent from, hashed as penalties hash it, so turning the pack down
-- as abuse can ban the network behind it. Cleared 30 days after the decision.
ALTER TABLE submissions ADD COLUMN network TEXT;

-- Approved packs not pulled into folderskin-community yet, which GET /v1/exports/pending counts
-- for anyone who asks: from this index alone, rather than by reading every pack ever approved.
CREATE INDEX exports_pending ON submissions (status, exported_at);
