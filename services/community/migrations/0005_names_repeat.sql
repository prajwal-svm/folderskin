-- Names repeat as often as people like, the way pack names do: two computers can credit their
-- packs to the same name, and the key, which nobody sees, is what tells them apart.
--
-- SQLite can't drop a column's UNIQUE, so the table is made again. Submissions point at their key,
-- and D1 always checks that, so the check waits for the end of the migration, and the rows go back
-- into the new table only after the old one is dropped: each one found again there takes back the
-- miss that dropping it counted.
PRAGMA defer_foreign_keys = true;

CREATE TABLE keys_before (key TEXT, handle TEXT, tier TEXT, verified_at INTEGER, approved INTEGER, rejected INTEGER);
INSERT INTO keys_before SELECT key, handle, tier, verified_at, approved, rejected FROM keys;
DROP TABLE keys;

CREATE TABLE keys (
  key TEXT PRIMARY KEY,
  handle TEXT NOT NULL,
  tier TEXT NOT NULL DEFAULT 'probation'
    CHECK (tier IN ('probation', 'active', 'trusted', 'banned')),
  verified_at INTEGER NOT NULL,
  approved INTEGER NOT NULL DEFAULT 0,
  rejected INTEGER NOT NULL DEFAULT 0
);
INSERT INTO keys (key, handle, tier, verified_at, approved, rejected)
  SELECT key, handle, tier, verified_at, approved, rejected FROM keys_before;
DROP TABLE keys_before;

PRAGMA defer_foreign_keys = false;
