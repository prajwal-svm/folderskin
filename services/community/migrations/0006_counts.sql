-- Downloads of one skin at a time and views on folderskin.app, beside the install counts, and each
-- day's counts of all four for the last 30 days, so the website can rank packs and skins by the
-- last seven (src/counts.ts). A skin is its picture's SHA-256, a pack its id.

-- How often each skin has been downloaded on its own, and each pack and skin viewed, all told.
-- Installs keep the table they have (0002), which goes on counting them.
CREATE TABLE counts_total (
  kind TEXT NOT NULL CHECK (kind IN ('skin_download', 'pack_view', 'skin_view')),
  target TEXT NOT NULL,
  n INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (kind, target)
);

-- Each UTC day's count of each kind for each pack or skin, installs included. The day leads the
-- key, so a week is read without reading the month around it, and the daily clean-up deletes the
-- days more than 30 days old.
CREATE TABLE counts_daily (
  kind TEXT NOT NULL CHECK (kind IN ('install', 'skin_download', 'pack_view', 'skin_view')),
  target TEXT NOT NULL,
  day TEXT NOT NULL,
  n INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (day, kind, target)
);

-- The downloads and views counted today, so a network doing the same again that day isn't counted
-- twice, as installs_seen does for installs. `network` is the HMAC of the network prefix under a
-- key made for that kind and that UTC day (src/ip.ts), never an address. The daily clean-up deletes
-- every row from before today.
CREATE TABLE counts_seen (
  day TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('skin_download', 'pack_view', 'skin_view')),
  network TEXT NOT NULL,
  target TEXT NOT NULL,
  PRIMARY KEY (day, kind, network, target)
);
