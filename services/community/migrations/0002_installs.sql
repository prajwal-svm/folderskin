-- How often each community pack has been added, for the counts on folderskin.app's gallery. The
-- app sends a pack's id once it has added the pack, and nothing else (src/installs.ts).

-- One row per pack that has been counted.
CREATE TABLE installs (
  pack TEXT PRIMARY KEY,
  n INTEGER NOT NULL DEFAULT 0
);

-- The adds counted today, so a network adding the same pack again that day isn't counted twice.
-- `network` is the HMAC of the network prefix under a key made for installs and for that UTC day
-- (src/ip.ts), never an address. The daily clean-up deletes every row from before today.
CREATE TABLE installs_seen (
  day TEXT NOT NULL,
  network TEXT NOT NULL,
  pack TEXT NOT NULL,
  PRIMARY KEY (day, network, pack)
);
