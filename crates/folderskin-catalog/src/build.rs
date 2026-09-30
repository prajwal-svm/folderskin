//! Writing the catalog: every pack and skin in one SQLite database, with FTS5 indexes over what
//! people search by.
//!
//! Two full-text tables do the searching. `packs_fts` holds one row per pack: its name, its
//! author, its tags and the names of all of its skins, so typing a skin's name finds its pack.
//! `skins_fts` holds one row per skin name, for the strip of skins that match. Both are
//! contentless (the words are indexed, not stored twice) and have prefix indexes, so each
//! keystroke is a prefix query that stays fast at a hundred thousand skins.
//!
//! The official collection (`folderskin_core::collection`) has a table of its own, `collection`,
//! one row per skin in `collection.json`'s order, with `collection_fts` over its names and tags.
//! Unlike a pack's skins, each row carries its picture's SHA-256, size and extension: the
//! collection's thumbnails are the main thing its view shows, a grid of hundreds of them, and
//! there is no pack manifest to name them, so the catalog does. The table is new since the first
//! catalogs, and [`CATALOG_VERSION`] stays 1 all the same: a FolderSkin from before it never
//! queries it and reads every other table as it always did, and this one reads a catalog without
//! it as an empty collection.
//!
//! The same packs always make the same bytes: rows go in in id order (the collection's in its own
//! order), the indexes are merged into one segment, and the database is vacuumed before it is
//! handed over. So the catalog's hash names its generation, and rebuilding unchanged packs
//! publishes nothing new.

use rusqlite::{params, Connection};

/// `PRAGMA user_version` of a catalog this crate writes and reads. A new layout gets a new
/// number, and a catalog with a higher one asks for a newer FolderSkin.
pub const CATALOG_VERSION: i32 = 1;
/// `PRAGMA application_id`: "FSKC", so a catalog is recognisable as one.
pub const APPLICATION_ID: i32 = 0x4653_4B43;

/// One pack as the catalog lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct PackRecord {
    pub id: String,
    pub name: String,
    pub author: String,
    pub license: String,
    /// The pack's tags, then any its skins add, as `index.json` has them.
    pub tags: Vec<String>,
    /// [`folderskin_core::pack::pack_hash`] of the pack.
    pub hash: String,
    /// SHA-256 of its published manifest, in hex; empty when it has none (a catalog made from
    /// `index.json`). With it, what the app fetches is checked all the way down from head.json:
    /// the catalog by head.json, the manifest by the catalog, each picture by the manifest.
    pub manifest: String,
    /// When the pack was first published, in Unix seconds; 0 when nobody knows.
    pub added: i64,
    /// How many skins it has. Kept apart from `skins` because a catalog made from `index.json`
    /// knows the count and none of the names.
    pub count: usize,
    /// What adding it downloads, in bytes; 0 when nobody knows.
    pub bytes: u64,
    /// Its skins' names, in the pack's order. A skin's picture and thumbnail are named in the
    /// pack's manifest rather than here: 32 bytes of SHA-256 a skin would be nearly half of a
    /// gzipped catalog, downloaded again with every generation, for thumbnails few people see.
    pub skin_names: Vec<String>,
}

/// One skin of the official collection as the catalog lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionRecord {
    pub name: String,
    pub tags: Vec<String>,
    /// SHA-256 of its picture, in hex: its name under `pictures/` and `thumbs/`.
    pub sha256: String,
    /// The picture's extension, lower case: the rest of its name under `pictures/`.
    pub ext: String,
    /// The picture's size, which it is checked against as it arrives.
    pub bytes: u64,
    pub w: u32,
    pub h: u32,
    /// When it was added: 00:00 UTC of its day, in Unix seconds.
    pub added: i64,
}

const SCHEMA: &str = "
CREATE TABLE packs (
    n INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    sort_name TEXT NOT NULL,
    author TEXT NOT NULL,
    license TEXT NOT NULL,
    tags TEXT NOT NULL,
    count INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    hash TEXT NOT NULL,
    manifest TEXT NOT NULL,
    added INTEGER NOT NULL
);
CREATE INDEX packs_by_name ON packs (sort_name, n);
CREATE INDEX packs_by_added ON packs (added DESC, n);
CREATE INDEX packs_by_count ON packs (count DESC, n);
CREATE TABLE pack_tags (
    tag TEXT NOT NULL,
    pack INTEGER NOT NULL,
    PRIMARY KEY (tag, pack)
) WITHOUT ROWID;
CREATE INDEX pack_tags_by_pack ON pack_tags (pack, tag);
CREATE TABLE tag_counts (
    tag TEXT PRIMARY KEY,
    packs INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE skins (
    n INTEGER PRIMARY KEY,
    pack INTEGER NOT NULL,
    position INTEGER NOT NULL,
    name TEXT NOT NULL
);
CREATE VIRTUAL TABLE packs_fts USING fts5(
    name, author, tags, skins,
    content = '', prefix = '1 2', tokenize = 'unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE skins_fts USING fts5(
    name,
    content = '', detail = none, prefix = '1 2', tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TABLE collection (
    n INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT NOT NULL,
    tags TEXT NOT NULL,
    sha256 TEXT NOT NULL UNIQUE,
    ext TEXT NOT NULL,
    bytes INTEGER NOT NULL,
    w INTEGER NOT NULL,
    h INTEGER NOT NULL,
    added INTEGER NOT NULL
);
CREATE INDEX collection_by_added ON collection (added DESC, n);
CREATE INDEX collection_by_name ON collection (sort_name, n);
CREATE VIRTUAL TABLE collection_fts USING fts5(
    name, tags,
    content = '', prefix = '1 2', tokenize = 'unicode61 remove_diacritics 2'
);
";

/// Writes `packs` and the official `collection` into `conn`, an empty database, in one
/// transaction. The packs go in in id order whatever order they come in, and the collection in the
/// order it is given (`collection.json`'s), so the same packs and collection make the same
/// database.
pub fn write(
    conn: &Connection,
    packs: &[PackRecord],
    collection: &[CollectionRecord],
) -> Result<(), String> {
    let mut order: Vec<&PackRecord> = packs.iter().collect();
    order.sort_by(|a, b| a.id.cmp(&b.id));
    if order.windows(2).any(|w| w[0].id == w[1].id) {
        return Err("two packs have the same id".into());
    }
    let mut pictures = std::collections::HashSet::new();
    if !collection.iter().all(|c| pictures.insert(&c.sha256)) {
        return Err("two skins in the collection have the same picture".into());
    }
    write_sorted(conn, &order, collection).map_err(|e| format!("couldn't write the catalog: {e}"))
}

fn write_sorted(
    conn: &Connection,
    packs: &[&PackRecord],
    collection: &[CollectionRecord],
) -> rusqlite::Result<()> {
    // Set before the first table, while the page size can still change.
    conn.execute_batch("PRAGMA page_size = 4096;")?;
    conn.pragma_update(None, "user_version", CATALOG_VERSION)?;
    conn.pragma_update(None, "application_id", APPLICATION_ID)?;
    conn.execute_batch("BEGIN;")?;
    conn.execute_batch(SCHEMA)?;
    {
        let mut pack = conn.prepare(
            "INSERT INTO packs (n, id, name, sort_name, author, license, tags, count, bytes, hash, manifest, added)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        let mut pack_fts = conn.prepare(
            "INSERT INTO packs_fts (rowid, name, author, tags, skins) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut tag =
            conn.prepare("INSERT OR IGNORE INTO pack_tags (tag, pack) VALUES (?1, ?2)")?;
        let mut skin =
            conn.prepare("INSERT INTO skins (n, pack, position, name) VALUES (?1, ?2, ?3, ?4)")?;
        let mut skin_fts = conn.prepare("INSERT INTO skins_fts (rowid, name) VALUES (?1, ?2)")?;
        let mut skin_n: i64 = 0;
        for (i, p) in packs.iter().enumerate() {
            let n = i as i64 + 1;
            let name = p.name.trim();
            let tags = serde_json::to_string(&p.tags).unwrap_or_else(|_| "[]".into());
            pack.execute(params![
                n,
                p.id,
                name,
                name.to_lowercase(),
                p.author,
                p.license,
                tags,
                p.count as i64,
                p.bytes as i64,
                p.hash,
                p.manifest,
                p.added,
            ])?;
            let skin_names: Vec<&str> = p.skin_names.iter().map(|s| s.trim()).collect();
            pack_fts.execute(params![
                n,
                name,
                p.author,
                p.tags.join(" "),
                skin_names.join(" "),
            ])?;
            for t in &p.tags {
                tag.execute(params![t, n])?;
            }
            for (position, skin_name) in skin_names.iter().enumerate() {
                skin_n += 1;
                skin.execute(params![skin_n, n, position as i64, skin_name])?;
                skin_fts.execute(params![skin_n, skin_name])?;
            }
        }
        let mut official = conn.prepare(
            "INSERT INTO collection (n, name, sort_name, tags, sha256, ext, bytes, w, h, added)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        let mut official_fts =
            conn.prepare("INSERT INTO collection_fts (rowid, name, tags) VALUES (?1, ?2, ?3)")?;
        for (i, c) in collection.iter().enumerate() {
            let n = i as i64 + 1;
            let name = c.name.trim();
            let tags = serde_json::to_string(&c.tags).unwrap_or_else(|_| "[]".into());
            official.execute(params![
                n,
                name,
                name.to_lowercase(),
                tags,
                c.sha256,
                c.ext,
                c.bytes as i64,
                c.w,
                c.h,
                c.added,
            ])?;
            official_fts.execute(params![n, name, c.tags.join(" ")])?;
        }
    }
    conn.execute_batch(
        "INSERT INTO tag_counts (tag, packs) SELECT tag, COUNT(*) FROM pack_tags GROUP BY tag;
         INSERT INTO packs_fts (packs_fts) VALUES ('optimize');
         INSERT INTO skins_fts (skins_fts) VALUES ('optimize');
         INSERT INTO collection_fts (collection_fts) VALUES ('optimize');
         COMMIT;
         ANALYZE;",
    )
}

/// A catalog of `packs` and the official `collection` in memory, ready to search: what the app
/// makes from `index.json` when no catalog is published, with an empty collection.
pub fn in_memory(
    packs: &[PackRecord],
    collection: &[CollectionRecord],
) -> Result<Connection, String> {
    let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    write(&conn, packs, collection)?;
    Ok(conn)
}

/// A catalog of `packs` and the official `collection` as the bytes of a SQLite file, compacted.
/// The same packs and collection make the same bytes every time.
pub fn to_bytes(packs: &[PackRecord], collection: &[CollectionRecord]) -> Result<Vec<u8>, String> {
    let conn = in_memory(packs, collection)?;
    conn.execute_batch("VACUUM;")
        .map_err(|e| format!("couldn't compact the catalog: {e}"))?;
    let data = conn
        .serialize("main")
        .map_err(|e| format!("couldn't save the catalog: {e}"))?;
    Ok(data.to_vec())
}
