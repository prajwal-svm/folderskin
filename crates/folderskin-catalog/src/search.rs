//! Searching the catalog: the packs matching what was typed, a page at a time in the order
//! asked for, the skins whose names match, and how many of the matching packs carry each tag.
//!
//! Every word typed is a prefix, and a pack matches when each word starts a word of its name,
//! its author, its tags or one of its skins' names. So "star" finds The Starry Night, and
//! "van star" narrows it to packs that also say something starting with "van".
//!
//! The official collection is searched the same way, over each skin's name and tags, a page at a
//! time ([`Catalog::collection`]), and a pack search brings the collection's skins that match
//! along with it ([`Results::collection`]). A catalog with no collection table (one written before
//! there was a collection, or one made from `index.json`) has an empty collection.

use crate::build::{APPLICATION_ID, CATALOG_VERSION};
use regex::Regex;
use rusqlite::{Connection, OpenFlags, ToSql};
use serde::Serialize;
use std::path::Path;
use std::sync::LazyLock;

/// The most packs one page holds.
pub const MAX_PAGE: usize = 200;
/// How many matching skins a search returns.
pub const SKIN_HITS: usize = 12;
/// How many of the official collection's skins a pack search brings along when they match.
pub const COLLECTION_HITS: usize = 24;
/// How many tags a search counts, most used first.
pub const MAX_FACETS: usize = 200;
/// Words of a search past this many are ignored: nobody types more, and each is a join.
const MAX_WORDS: usize = 8;

/// The order packs are listed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    /// Best match for what was typed; with nothing typed, the featured packs and then the newest.
    #[default]
    Best,
    Newest,
    Name,
    /// Most skins first.
    Skins,
}

impl Sort {
    /// A sort by its name in the webview; anything unknown is [`Sort::Best`].
    pub fn parse(s: &str) -> Sort {
        match s {
            "newest" => Sort::Newest,
            "name" => Sort::Name,
            "skins" => Sort::Skins,
            _ => Sort::Best,
        }
    }
}

/// One search.
#[derive(Clone, Debug, Default)]
pub struct Query<'a> {
    /// What was typed, as typed.
    pub q: &'a str,
    /// Only packs with this tag; empty for all.
    pub tag: &'a str,
    pub sort: Sort,
    pub offset: usize,
    pub limit: usize,
    /// Pack ids to list first when nothing is typed and the sort is [`Sort::Best`].
    pub featured: &'a [String],
}

/// The order the official collection is listed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CollectionSort {
    /// The latest days' additions first, and a day's in the order they were added.
    #[default]
    Newest,
    /// By name, A to Z.
    Name,
}

impl CollectionSort {
    /// A sort by its name in the webview; anything unknown is [`CollectionSort::Newest`].
    pub fn parse(s: &str) -> CollectionSort {
        match s {
            "name" => CollectionSort::Name,
            _ => CollectionSort::Newest,
        }
    }
}

/// One page of the official collection.
#[derive(Clone, Debug, Default)]
pub struct CollectionQuery<'a> {
    /// What was typed, as typed; nothing lists every skin.
    pub q: &'a str,
    pub sort: CollectionSort,
    pub offset: usize,
    pub limit: usize,
}

/// One skin of the official collection.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CollectionItem {
    /// Where it is in `collection.json`, from 0.
    pub position: usize,
    pub name: String,
    pub tags: Vec<String>,
    /// SHA-256 of its picture, in hex: its name under `pictures/` and `thumbs/`.
    pub sha256: String,
    /// The picture's extension, lower case.
    pub ext: String,
    /// The picture's size.
    pub bytes: u64,
    pub w: u32,
    pub h: u32,
    /// When it was added: 00:00 UTC of its day, in Unix seconds.
    pub added: i64,
}

/// A page of the official collection, and how many skins there are to page through.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct CollectionPage {
    /// Skins matching the words: all of them when nothing is typed.
    pub total: usize,
    pub items: Vec<CollectionItem>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PackRow {
    pub id: String,
    pub name: String,
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub count: usize,
    pub bytes: u64,
    pub hash: String,
    pub added: i64,
    /// SHA-256 of its published manifest, in hex; empty when it has none.
    pub manifest: String,
}

/// A skin whose name matches, with the pack it is in.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SkinHit {
    pub pack: String,
    pub pack_name: String,
    pub pack_hash: String,
    pub name: String,
    /// Where it is in its pack, from 0: its place in the pack's manifest, which names its
    /// thumbnail.
    pub position: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Facet {
    pub tag: String,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Results {
    /// Packs matching the words and the tag.
    pub total: usize,
    /// Packs matching the words, whatever their tags: what "All" counts.
    pub all: usize,
    /// The page asked for.
    pub packs: Vec<PackRow>,
    /// Skins whose names match the words, in packs with the tag; none when nothing is typed.
    pub skins: Vec<SkinHit>,
    /// Tags of the packs matching the words, most used first, not narrowed by the tag, so
    /// another tag can be picked from them.
    pub facets: Vec<Facet>,
    /// Up to [`COLLECTION_HITS`] of the official collection's skins whose names or tags match
    /// the words, whatever the tag; none when nothing is typed.
    pub collection: Vec<CollectionItem>,
}

/// A catalog open for searching.
#[derive(Debug)]
pub struct Catalog {
    conn: Connection,
    packs: usize,
    skins: usize,
    /// How many skins the official collection has: 0 when the catalog has no collection table.
    collection: usize,
}

impl Catalog {
    /// Opens the catalog file at `path`, read-only.
    pub fn open(path: &Path) -> Result<Catalog, String> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| damaged())?;
        // A catalog is read over and over while someone types: keep it in memory once read.
        let _ = conn.execute_batch("PRAGMA cache_size = -32768; PRAGMA mmap_size = 268435456;");
        Catalog::from_connection(conn)
    }

    /// A catalog from the bytes of its file, held in memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<Catalog, String> {
        let mut conn = Connection::open_in_memory().map_err(|_| damaged())?;
        conn.deserialize_read_exact("main", bytes, bytes.len(), true)
            .map_err(|_| damaged())?;
        Catalog::from_connection(conn)
    }

    /// A catalog already open, such as one [`crate::build::in_memory`] made.
    pub fn from_connection(conn: Connection) -> Result<Catalog, String> {
        let pragma = |name: &str| -> Result<i32, String> {
            conn.pragma_query_value(None, name, |r| r.get(0))
                .map_err(|_| damaged())
        };
        if pragma("application_id")? != APPLICATION_ID {
            return Err(damaged());
        }
        let version = pragma("user_version")?;
        if version > CATALOG_VERSION {
            return Err("the community packs need a newer FolderSkin".into());
        }
        if version != CATALOG_VERSION {
            return Err(damaged());
        }
        let count = |sql: &str| -> Result<usize, String> {
            conn.query_row(sql, [], |r| r.get::<_, i64>(0))
                .map(|n| n as usize)
                .map_err(|_| damaged())
        };
        let packs = count("SELECT COUNT(*) FROM packs")?;
        let skins = count("SELECT COUNT(*) FROM skins")?;
        // A catalog from before the collection, or one made from index.json, has none: an empty
        // collection, which is never asked about.
        let has_collection = count(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'collection'",
        )? > 0;
        let collection = if has_collection {
            count("SELECT COUNT(*) FROM collection")?
        } else {
            0
        };
        Ok(Catalog {
            conn,
            packs,
            skins,
            collection,
        })
    }

    /// How many packs and skins it lists.
    pub fn counts(&self) -> (usize, usize) {
        (self.packs, self.skins)
    }

    /// How many skins the official collection has.
    pub fn collection_count(&self) -> usize {
        self.collection
    }

    /// One page of the official collection's skins matching `query`, in the order it asks for,
    /// with how many match in all.
    pub fn collection(&self, query: &CollectionQuery) -> Result<CollectionPage, String> {
        if self.collection == 0 {
            return Ok(CollectionPage::default());
        }
        self.collection_page(query).map_err(unreadable)
    }

    /// The official collection's skin whose picture has SHA-256 `sha256`; `None` when it has
    /// none.
    pub fn collection_item(&self, sha256: &str) -> Result<Option<CollectionItem>, String> {
        if self.collection == 0 {
            return Ok(None);
        }
        let sql = format!("SELECT {COLLECTION_COLUMNS} FROM collection c WHERE c.sha256 = :sha");
        self.rows(&sql, &[(":sha", &sha256)], collection_row)
            .map(|rows| rows.into_iter().next())
            .map_err(unreadable)
    }

    /// Every picture of the official collection, as its SHA-256 and extension, in the
    /// collection's order: the files it publishes.
    pub fn collection_pictures(&self) -> Result<Vec<(String, String)>, String> {
        if self.collection == 0 {
            return Ok(Vec::new());
        }
        self.rows("SELECT sha256, ext FROM collection ORDER BY n", &[], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(unreadable)
    }

    /// One page of packs matching `query`, with the skins and the tags that match too.
    pub fn search(&self, query: &Query) -> Result<Results, String> {
        self.run(query).map_err(unreadable)
    }

    /// Packs by id, in the order asked for; ids the catalog doesn't have are left out.
    pub fn packs(&self, ids: &[String]) -> Result<Vec<PackRow>, String> {
        self.packs_by_id(ids).map_err(unreadable)
    }

    /// Every pack's id and hash, in id order: the versions this catalog publishes.
    pub fn versions(&self) -> Result<Vec<(String, String)>, String> {
        self.rows("SELECT id, hash FROM packs ORDER BY id", &[], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(unreadable)
    }

    fn packs_by_id(&self, ids: &[String]) -> rusqlite::Result<Vec<PackRow>> {
        let json = serde_json::to_string(ids).unwrap_or_else(|_| "[]".into());
        let sql = format!(
            "SELECT {PACK_COLUMNS} FROM json_each(:ids) j JOIN packs p ON p.id = j.value ORDER BY j.key"
        );
        self.rows(&sql, &[(":ids", &json)], pack_row)
    }

    fn run(&self, query: &Query) -> rusqlite::Result<Results> {
        let words = match_expr(query.q);
        let tag = query.tag.trim();
        let limit = query.limit.clamp(1, MAX_PAGE) as i64;
        let offset = query.offset.min(i64::MAX as usize) as i64;
        let facets = MAX_FACETS as i64;
        let q = words.clone().unwrap_or_default();
        let bind: Binds = &[(":q", &q), (":tag", &tag), (":facets", &facets)];

        let (total, packs) = match &words {
            Some(_) => self.matching(&q, tag, query.sort, offset, limit)?,
            None => self.browsing(tag, query.sort, query.featured, offset, limit)?,
        };
        let all = match (&words, tag.is_empty()) {
            (_, true) => total,
            (Some(_), false) => self.count(&format!("{HITS}SELECT COUNT(*) FROM hits"), bind)?,
            (None, false) => self.packs,
        };

        let facets = if words.is_some() {
            self.rows(
                &format!(
                    "{HITS}SELECT t.tag, COUNT(*) AS c FROM hits h JOIN pack_tags t ON t.pack = h.n \
                     GROUP BY t.tag ORDER BY c DESC, t.tag LIMIT :facets"
                ),
                bind,
                facet_row,
            )?
        } else {
            self.rows(
                "SELECT tag, packs FROM tag_counts ORDER BY packs DESC, tag LIMIT :facets",
                bind,
                facet_row,
            )?
        };

        // The strip of skins is extra: should the skins index ever refuse what was typed, the
        // packs still come.
        let skins = if words.is_some() && self.skins > 0 {
            self.skin_hits(query.q, tag).unwrap_or_default()
        } else {
            Vec::new()
        };
        // So is the collection's.
        let collection = if words.is_some() && self.collection > 0 {
            self.collection_hits(query.q).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Results {
            total,
            all,
            packs,
            skins,
            facets,
            collection,
        })
    }

    /// [`Catalog::collection`], with a collection to page through.
    fn collection_page(&self, query: &CollectionQuery) -> rusqlite::Result<CollectionPage> {
        let limit = query.limit.clamp(1, MAX_PAGE) as i64;
        let offset = query.offset.min(i64::MAX as usize) as i64;
        let order = collection_order(query.sort);
        let Some(m) = match_expr(query.q) else {
            let sql = format!(
                "SELECT {COLLECTION_COLUMNS} FROM collection c ORDER BY {order} \
                 LIMIT :limit OFFSET :offset"
            );
            let bind: Binds = &[(":limit", &limit), (":offset", &offset)];
            return Ok(CollectionPage {
                total: self.collection,
                items: self.rows(&sql, bind, collection_row)?,
            });
        };
        let sql = format!(
            "SELECT {COLLECTION_COLUMNS}, COUNT(*) OVER () \
             FROM collection_fts JOIN collection c ON c.n = collection_fts.rowid \
             WHERE collection_fts MATCH :m ORDER BY {order} LIMIT :limit OFFSET :offset"
        );
        let bind: Binds = &[(":m", &m), (":limit", &limit), (":offset", &offset)];
        let mut total = None;
        let items = self.rows(&sql, bind, |r| {
            total = Some(r.get::<_, i64>(COLLECTION_COLUMN_COUNT)? as usize);
            collection_row(r)
        })?;
        let total = match total {
            Some(n) => n,
            // Past the end: nothing came back to carry the count.
            None => self.count(
                "SELECT COUNT(*) FROM collection_fts WHERE collection_fts MATCH :m",
                bind,
            )?,
        };
        Ok(CollectionPage { total, items })
    }

    /// Up to [`COLLECTION_HITS`] of the collection's skins whose names or tags match `q`, newest
    /// first: those with every word typed out whole, then those the words only start, as
    /// the strip of pack skins has them.
    fn collection_hits(&self, q: &str) -> rusqlite::Result<Vec<CollectionItem>> {
        let sql = format!(
            "SELECT {COLLECTION_COLUMNS} \
             FROM collection_fts JOIN collection c ON c.n = collection_fts.rowid \
             WHERE collection_fts MATCH :m ORDER BY {} LIMIT :hits",
            collection_order(CollectionSort::Newest)
        );
        let limit = COLLECTION_HITS as i64;
        let mut hits: Vec<CollectionItem> = Vec::new();
        for expr in [exact_expr(q), match_expr(q)].into_iter().flatten() {
            let bind: Binds = &[(":m", &expr), (":hits", &limit)];
            for hit in self.rows(&sql, bind, collection_row)? {
                if hits.len() < COLLECTION_HITS && !hits.iter().any(|h| h.sha256 == hit.sha256) {
                    hits.push(hit);
                }
            }
            if hits.len() == COLLECTION_HITS {
                break;
            }
        }
        Ok(hits)
    }

    /// How many packs match the words `m` (and `tag`), and the page of them. The count comes
    /// with the page, from the same pass over the index.
    fn matching(
        &self,
        m: &str,
        tag: &str,
        sort: Sort,
        offset: i64,
        limit: i64,
    ) -> rusqlite::Result<(usize, Vec<PackRow>)> {
        let order = match sort {
            Sort::Best => "h.score, p.n",
            other => order_by(other),
        };
        let sql = format!(
            "{HITS}SELECT {PACK_COLUMNS}, COUNT(*) OVER () FROM hits h JOIN packs p ON p.n = h.n \
             WHERE 1{} ORDER BY {order} LIMIT :limit OFFSET :offset",
            only_tag(tag)
        );
        let bind: Binds = &[
            (":q", &m),
            (":tag", &tag),
            (":limit", &limit),
            (":offset", &offset),
        ];
        let mut total = None;
        let packs = self.rows(&sql, bind, |r| {
            total = Some(r.get::<_, i64>(PACK_COLUMN_COUNT)? as usize);
            pack_row(r)
        })?;
        let total = match total {
            Some(n) => n,
            // Past the end: nothing came back to carry the count.
            None => self.count(
                &format!(
                    "{HITS}SELECT COUNT(*) FROM hits h JOIN packs p ON p.n = h.n WHERE 1{}",
                    only_tag(tag)
                ),
                bind,
            )?,
        };
        Ok((total, packs))
    }

    /// Every pack (with `tag`), and the page of them, when nothing is typed. The best order is
    /// the `featured` packs first, then the newest. Each list is walked along its own index, so
    /// a page costs the same at the start of ten thousand packs as at the end.
    fn browsing(
        &self,
        tag: &str,
        sort: Sort,
        featured: &[String],
        offset: i64,
        limit: i64,
    ) -> rusqlite::Result<(usize, Vec<PackRow>)> {
        let total = if tag.is_empty() {
            self.packs
        } else {
            self.count(
                "SELECT COUNT(*) FROM pack_tags WHERE tag = :tag",
                &[(":tag", &tag)],
            )?
        };
        let first: Vec<PackRow> = if sort == Sort::Best && !featured.is_empty() {
            self.packs_by_id(featured)?
                .into_iter()
                .filter(|p| tag.is_empty() || p.tags.iter().any(|t| t == tag))
                .collect()
        } else {
            Vec::new()
        };
        let shown_first = first.len() as i64;
        let mut page: Vec<PackRow> = first
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        let (rest_offset, rest_limit) = ((offset - shown_first).max(0), limit - page.len() as i64);
        if rest_limit > 0 {
            let ids = serde_json::to_string(featured).unwrap_or_else(|_| "[]".into());
            let not_first = if shown_first > 0 {
                " AND p.id NOT IN (SELECT value FROM json_each(:first))"
            } else {
                ""
            };
            let sql = format!(
                "SELECT {PACK_COLUMNS} FROM packs p WHERE 1{}{not_first} \
                 ORDER BY {} LIMIT :limit OFFSET :offset",
                only_tag(tag),
                order_by(sort)
            );
            let bind: Binds = &[
                (":tag", &tag),
                (":first", &ids),
                (":limit", &rest_limit),
                (":offset", &rest_offset),
            ];
            page.extend(self.rows(&sql, bind, pack_row)?);
        }
        Ok((total, page))
    }

    /// Up to [`SKIN_HITS`] skins whose names match `q`, in packs tagged `tag` when there is one.
    /// Names with every word typed out whole come first, then names the words only start. Each
    /// list is in the catalog's own order rather than by rank: ranking every match of a
    /// one-letter prefix across a hundred thousand names would cost more than the rest of the
    /// search, while in rowid order SQLite stops at the first dozen.
    fn skin_hits(&self, q: &str, tag: &str) -> rusqlite::Result<Vec<SkinHit>> {
        let only_tag = if tag.is_empty() {
            ""
        } else {
            " AND s.pack IN (SELECT pack FROM pack_tags WHERE tag = :tag)"
        };
        let sql = format!(
            "SELECT s.name, s.position, p.id, p.name, p.hash \
             FROM skins_fts JOIN skins s ON s.n = skins_fts.rowid JOIN packs p ON p.n = s.pack \
             WHERE skins_fts MATCH :m{only_tag} ORDER BY s.n LIMIT :skins"
        );
        let limit = SKIN_HITS as i64;
        let mut hits: Vec<SkinHit> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for expr in [exact_expr(q), match_expr(q)].into_iter().flatten() {
            let bind: Binds = &[(":m", &expr), (":tag", &tag), (":skins", &limit)];
            for hit in self.rows(&sql, bind, skin_row)? {
                if hits.len() < SKIN_HITS && seen.insert((hit.pack.clone(), hit.position)) {
                    hits.push(hit);
                }
            }
            if hits.len() == SKIN_HITS {
                break;
            }
        }
        Ok(hits)
    }

    fn count(&self, sql: &str, bind: Binds) -> rusqlite::Result<usize> {
        let mut stmt = self.conn.prepare(sql)?;
        let params = used(sql, bind);
        stmt.query_row(params.as_slice(), |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
    }

    fn rows<T>(
        &self,
        sql: &str,
        bind: Binds,
        f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<Vec<T>> {
        let mut stmt = self.conn.prepare(sql)?;
        let params = used(sql, bind);
        let rows = stmt.query_map(params.as_slice(), f)?;
        rows.collect()
    }
}

type Binds<'a> = &'a [(&'a str, &'a dyn ToSql)];

/// The named parameters `sql` mentions: SQLite refuses a statement bound to one it doesn't have.
fn used<'a>(sql: &str, bind: Binds<'a>) -> Vec<(&'a str, &'a dyn ToSql)> {
    bind.iter()
        .filter(|(name, _)| {
            sql.match_indices(name).any(|(at, _)| {
                // `:q` is not `:quick`.
                !sql[at + name.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
        })
        .copied()
        .collect()
}

const PACK_COLUMNS: &str =
    "p.id, p.name, p.author, p.license, p.tags, p.count, p.bytes, p.hash, p.added, p.manifest";
/// How many columns [`PACK_COLUMNS`] is, so a column after them can be read.
const PACK_COLUMN_COUNT: usize = 10;

const COLLECTION_COLUMNS: &str = "c.n, c.name, c.tags, c.sha256, c.ext, c.bytes, c.w, c.h, c.added";
/// How many columns [`COLLECTION_COLUMNS`] is.
const COLLECTION_COLUMN_COUNT: usize = 9;

/// The order of a sort of the collection, along one of its indexes. Ties go by where each skin
/// is in `collection.json`, so pages never overlap or skip, and a day's additions come in the
/// order they were added.
fn collection_order(sort: CollectionSort) -> &'static str {
    match sort {
        CollectionSort::Newest => "c.added DESC, c.n",
        CollectionSort::Name => "c.sort_name, c.n",
    }
}

/// The packs whose index rows match `:q`, each with how well: bm25 weighs a word in the name
/// most, then the tags, the author, and a skin's name.
const HITS: &str =
    "WITH hits AS (SELECT rowid AS n, bm25(packs_fts, 10.0, 3.0, 4.0, 1.0) AS score \
                    FROM packs_fts WHERE packs_fts MATCH :q) ";

/// The condition that keeps packs `p` to those tagged `:tag`, when there is a tag.
fn only_tag(tag: &str) -> &'static str {
    if tag.is_empty() {
        ""
    } else {
        " AND p.n IN (SELECT pack FROM pack_tags WHERE tag = :tag)"
    }
}

/// The order of a sort that doesn't depend on the words, along one of the catalog's indexes.
/// Ties go by id, so pages never overlap or skip.
fn order_by(sort: Sort) -> &'static str {
    match sort {
        Sort::Best | Sort::Newest => "p.added DESC, p.n",
        Sort::Name => "p.sort_name, p.n",
        Sort::Skins => "p.count DESC, p.n",
    }
}

fn pack_row(r: &rusqlite::Row) -> rusqlite::Result<PackRow> {
    let tags: String = r.get(4)?;
    Ok(PackRow {
        id: r.get(0)?,
        name: r.get(1)?,
        author: r.get(2)?,
        license: r.get(3)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        count: r.get::<_, i64>(5)? as usize,
        bytes: r.get::<_, i64>(6)? as u64,
        hash: r.get(7)?,
        added: r.get(8)?,
        manifest: r.get(9)?,
    })
}

fn facet_row(r: &rusqlite::Row) -> rusqlite::Result<Facet> {
    Ok(Facet {
        tag: r.get(0)?,
        count: r.get::<_, i64>(1)? as usize,
    })
}

fn collection_row(r: &rusqlite::Row) -> rusqlite::Result<CollectionItem> {
    let tags: String = r.get(2)?;
    Ok(CollectionItem {
        position: (r.get::<_, i64>(0)? - 1).max(0) as usize,
        name: r.get(1)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        sha256: r.get(3)?,
        ext: r.get(4)?,
        bytes: r.get::<_, i64>(5)? as u64,
        w: r.get(6)?,
        h: r.get(7)?,
        added: r.get(8)?,
    })
}

fn skin_row(r: &rusqlite::Row) -> rusqlite::Result<SkinHit> {
    Ok(SkinHit {
        name: r.get(0)?,
        position: r.get::<_, i64>(1)? as usize,
        pack: r.get(2)?,
        pack_name: r.get(3)?,
        pack_hash: r.get(4)?,
    })
}

fn damaged() -> String {
    "the catalog of community packs is damaged".into()
}

/// What a catalog that opened but can't be read says. SQLite's own words ("database disk image
/// is malformed") mean nothing to someone searching for skins; they only go to the log.
fn unreadable(e: rusqlite::Error) -> String {
    eprintln!("folderskin: the community catalog couldn't be read: {e}");
    "the list of community packs on this computer couldn't be read. Try Refresh".into()
}

/// A word as the index's tokenizer, unicode61, reads one: a letter, digit or private-use
/// character, then more of them and any of the combining accents it folds away (SQLite's list,
/// U+0300 to U+0331 with gaps). Every other mark ends a word, the vowel signs of Hindi, Tamil
/// and Thai and the points of Hebrew and Arabic among them. Rust's `is_alphanumeric` counts
/// those as letters, but a quoted word the index reads as two is a phrase, and the skins index,
/// which keeps no positions, refuses phrases.
static WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"[\p{L}\p{N}\p{Co}]",
        r"[\p{L}\p{N}\p{Co}\x{300}-\x{304}\x{306}-\x{30C}\x{30F}\x{311}\x{31B}",
        r"\x{323}-\x{328}\x{32D}\x{32E}\x{330}\x{331}]*",
    ))
    .expect("the pattern is valid")
});

/// The words of a search, split the way the index splits them: "हिन्दी" is three words to the
/// index (ह, न and द), so it is three here. The index folds case and accents itself.
fn words(q: &str) -> impl Iterator<Item = &str> {
    WORD.find_iter(q).take(MAX_WORDS).map(|m| m.as_str())
}

/// What was typed as an FTS5 query: every word a quoted prefix, all of them required. `None`
/// when there is nothing to search for. Quoting each word keeps anything typed from being read
/// as FTS5 syntax: only letters and digits get this far.
pub fn match_expr(q: &str) -> Option<String> {
    let terms: Vec<String> = words(q).map(|w| format!("\"{w}\"*")).collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// Like [`match_expr`], with every word matched whole rather than as a prefix.
fn exact_expr(q: &str) -> Option<String> {
    let terms: Vec<String> = words(q).map(|w| format!("\"{w}\"")).collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}
