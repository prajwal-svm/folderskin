//! Searching the catalog: the packs matching what was typed, a page at a time in the order
//! asked for, the skins whose names match, and how many of the matching packs carry each tag.
//!
//! Every word typed is a prefix, and a pack matches when each word starts a word of its name,
//! its author, its tags or one of its skins' names. So "star" finds The Starry Night, and
//! "van star" narrows it to packs that also say something starting with "van".

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
}

/// A catalog open for searching.
#[derive(Debug)]
pub struct Catalog {
    conn: Connection,
    packs: usize,
    skins: usize,
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
        Ok(Catalog { conn, packs, skins })
    }

    /// How many packs and skins it lists.
    pub fn counts(&self) -> (usize, usize) {
        (self.packs, self.skins)
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

        Ok(Results {
            total,
            all,
            packs,
            skins,
            facets,
        })
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
