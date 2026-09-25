//! The published community tree, version 2: what a host serves and the app reads.
//!
//! ```text
//! head.json                          small and mutable: which catalog is current
//! catalog/<generation>.sqlite.gz     every pack and skin, searchable (see `build`)
//! strips/<pack hash>.webp            a pack's first four skins as folders, side by side
//! thumbs/<sha256>.webp               one skin as its folder, 256 px
//! pictures/<sha256>.<ext>            one skin's picture, as the pack has it
//! packs/<id>/<pack hash>.json        pack.json with each picture's size and SHA-256
//! ```
//!
//! Everything but `head.json` is named after what is in it, so a host can let every cache keep
//! it forever: a changed pack is new files under new names, never new bytes under an old one.
//! That is what lets the same tree sit on GitHub today and on a bucket behind a CDN later.

use folderskin_core::pack::{self, Pack, PackSkin};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// The version `head.json` declares, and the name of the folder the tree sits in.
pub const HEAD_VERSION: u32 = 2;
/// The one file that changes in place.
pub const HEAD_FILE: &str = "head.json";
/// The version a published `packs/<id>/<hash>.json` declares.
pub const MANIFEST_VERSION: u32 = 2;
/// Largest `head.json` the app reads.
pub const MAX_HEAD_BYTES: usize = 256 * 1024;
/// Largest gzipped catalog the app downloads, and the most it unpacks to. At 10,000 packs and
/// 100,000 skins the catalog is a few MB, so both leave room for a lot of growth.
pub const MAX_CATALOG_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CATALOG_UNPACKED: u64 = 512 * 1024 * 1024;
/// Largest strip or thumbnail the app accepts.
pub const MAX_PREVIEW_BYTES: usize = 1024 * 1024;
/// Largest published pack manifest: pack.json plus four fields a skin.
pub const MAX_MANIFEST_BYTES: usize = 2 * pack::MAX_MANIFEST_BYTES;

/// `head.json`: which catalog is current, and where else the tree is served.
///
/// Fields this version doesn't know are ignored, so a head can gain fields without breaking the
/// apps already installed: add new ones with `#[serde(default)]`, and never rename or remove one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Head {
    pub version: u32,
    /// Names this catalog: sixteen hex digits of the SHA-256 of the unpacked database.
    pub generation: String,
    /// How many packs and skins the catalog lists.
    pub packs: usize,
    pub skins: usize,
    pub catalog: CatalogRef,
    /// Packs chosen to show first, in order.
    #[serde(default)]
    pub featured: Vec<String>,
    /// Packs the maintainer vouches for as official (`official.json` beside `packs/`), which the
    /// app marks as such. Empty in a head written before there were any.
    #[serde(default)]
    pub official: Vec<String>,
    /// Other places serving this same tree, tried in order before the one `head.json` came from.
    #[serde(default)]
    pub mirrors: Vec<String>,
}

/// Where the catalog is and how to know it arrived whole.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CatalogRef {
    /// Relative to the folder `head.json` is in, or a full `https://` URL.
    pub url: String,
    /// SHA-256 of the gzipped file, in hex.
    pub sha256: String,
    /// Size of the gzipped file.
    pub bytes: u64,
}

impl Head {
    /// Reads `head.json`. A head from a newer FolderSkin says so rather than being called damaged.
    pub fn parse(bytes: &[u8]) -> Result<Head, String> {
        let damaged = || "the list of community packs is damaged".to_string();
        if bytes.len() > MAX_HEAD_BYTES {
            return Err(damaged());
        }
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| damaged())?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version.is_some_and(|v| v > u64::from(HEAD_VERSION)) {
            return Err("the community packs need a newer FolderSkin".into());
        }
        let head: Head = serde_json::from_value(value).map_err(|_| damaged())?;
        let sane = head.version == HEAD_VERSION
            && is_hex(&head.generation, 16)
            && is_hex(&head.catalog.sha256, 64)
            && head.catalog.bytes > 0
            && head.catalog.bytes <= MAX_CATALOG_BYTES
            && is_catalog_url(&head.catalog.url)
            && head.mirrors.iter().all(|m| is_mirror(m));
        if !sane {
            return Err(damaged());
        }
        Ok(head)
    }

    /// The featured ids that are pack ids, in order, without repeats.
    pub fn featured_ids(&self) -> Vec<String> {
        pack_ids(&self.featured)
    }

    /// The official ids that are pack ids, in order, without repeats.
    pub fn official_ids(&self) -> Vec<String> {
        pack_ids(&self.official)
    }
}

/// The ids in `ids` that are pack ids, in order, without repeats.
fn pack_ids(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        if pack::is_pack_id(id) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// A catalog's address: `catalog/<16 hex>.sqlite.gz` beside `head.json`, or a full `https://`
/// URL, such as a release asset, for a catalog kept out of the tree.
fn is_catalog_url(url: &str) -> bool {
    if let Some(name) = url
        .strip_prefix("catalog/")
        .and_then(|n| n.strip_suffix(".sqlite.gz"))
    {
        return is_hex(name, 16);
    }
    url.starts_with("https://") && !url.chars().any(char::is_whitespace)
}

/// A mirror: an `https://` folder serving the tree, with no query or fragment to confuse the
/// paths added to it.
pub fn is_mirror(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() <= 512
        && !url.contains(['?', '#'])
        && !url.chars().any(char::is_whitespace)
}

/// `packs/<id>/<hash>.json`: a pack as it was published, with what the app needs to fetch and
/// check each picture by its content.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublishedPack {
    pub version: u32,
    pub id: String,
    /// [`pack::pack_hash`] of the pack's folder: the same version string `index.json` gives, so
    /// a pack added from either is recognised by the other.
    pub hash: String,
    pub name: String,
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub skins: Vec<PublishedSkin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublishedSkin {
    /// The picture's name in the pack's folder, which gives its extension.
    pub file: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// SHA-256 of the picture, in hex: its name under `pictures/` and `thumbs/`.
    pub sha256: String,
    pub bytes: u64,
    pub w: u32,
    pub h: u32,
}

impl PublishedPack {
    /// Reads a published pack and holds it to every rule a `pack.json` follows, plus its own:
    /// the id and hash it claims, and a SHA-256 and a size within the limits for each picture.
    pub fn parse(bytes: &[u8]) -> Result<PublishedPack, String> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err("that pack's list is too big".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "that pack's list is damaged".to_string())?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version.is_some_and(|v| v > u64::from(MANIFEST_VERSION)) {
            return Err("this pack needs a newer FolderSkin".into());
        }
        let published: PublishedPack =
            serde_json::from_value(value).map_err(|_| "that pack's list is damaged".to_string())?;
        let mut problems = published.to_pack().problems();
        if published.version != MANIFEST_VERSION {
            problems.push(format!("it needs \"version\": {MANIFEST_VERSION}"));
        }
        if !pack::is_pack_id(&published.id) || !is_hex(&published.hash, 16) {
            problems.push("its id or version isn't one FolderSkin can use".into());
        }
        for skin in &published.skins {
            let fits = skin.bytes > 0
                && skin.bytes <= pack::MAX_PICTURE_BYTES as u64
                && (pack::MIN_PICTURE_SIDE..=pack::MAX_PICTURE_SIDE).contains(&skin.w)
                && (pack::MIN_PICTURE_SIDE..=pack::MAX_PICTURE_SIDE).contains(&skin.h);
            if !is_hex(&skin.sha256, 64) || !fits {
                problems.push(format!("{} isn't described properly", skin.file));
            }
        }
        if problems.is_empty() {
            Ok(published)
        } else {
            Err(problems.join("; "))
        }
    }

    /// The pack as its `pack.json` has it, for the checks and for saving its skins.
    pub fn to_pack(&self) -> Pack {
        Pack {
            version: pack::PACK_VERSION,
            name: self.name.clone(),
            author: self.author.clone(),
            license: self.license.clone(),
            tags: self.tags.clone(),
            skins: self
                .skins
                .iter()
                .map(|s| PackSkin {
                    file: s.file.clone(),
                    name: s.name.clone(),
                    tags: s.tags.clone(),
                })
                .collect(),
        }
    }
}

impl PublishedSkin {
    /// The picture's extension, lower case: part of its name under `pictures/`.
    pub fn ext(&self) -> String {
        picture_ext(&self.file)
    }
}

/// A picture file's extension, lower case.
pub fn picture_ext(file: &str) -> String {
    file.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Where the catalog of `generation` is, beside `head.json`.
pub fn catalog_path(generation: &str) -> String {
    format!("catalog/{generation}.sqlite.gz")
}

pub fn strip_path(pack_hash: &str) -> String {
    format!("strips/{pack_hash}.webp")
}

pub fn thumb_path(sha256: &str) -> String {
    format!("thumbs/{sha256}.webp")
}

pub fn picture_path(sha256: &str, ext: &str) -> String {
    format!("pictures/{sha256}.{ext}")
}

pub fn manifest_path(id: &str, pack_hash: &str) -> String {
    format!("packs/{id}/{pack_hash}.json")
}

/// True when `s` is `len` lower-case hex digits.
pub fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// SHA-256 of `bytes`, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Gzips `bytes` the same way every time: best compression, and no name or time in the header,
/// so the same catalog always makes the same file.
pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = flate2::GzBuilder::new().write(Vec::new(), flate2::Compression::best());
    // Writing to a Vec cannot fail.
    out.write_all(bytes).expect("gzip into memory");
    out.finish().expect("gzip into memory")
}

/// Unpacks a gzipped file, refusing one that would unpack to more than `max` bytes.
pub fn gunzip(bytes: &[u8], max: u64) -> Result<Vec<u8>, String> {
    let damaged = || "the catalog of community packs is damaged".to_string();
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(max + 1)
        .read_to_end(&mut out)
        .map_err(|_| damaged())?;
    if out.len() as u64 > max {
        return Err(damaged());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(url: &str) -> String {
        format!(
            r#"{{ "version": 2, "generation": "0123456789abcdef", "packs": 1, "skins": 2,
  "catalog": {{ "url": "{url}", "sha256": "{}", "bytes": 10 }},
  "featured": ["colours", "../nope", "colours"],
  "official": ["classic-art", "Not An Id", "classic-art"], "mirrors": [] }}"#,
            "a".repeat(64)
        )
    }

    #[test]
    fn a_head_is_read_and_its_featured_and_official_packs_cleaned() {
        let h = Head::parse(head("catalog/0123456789abcdef.sqlite.gz").as_bytes()).unwrap();
        assert_eq!(h.featured_ids(), ["colours"]);
        assert_eq!(h.official_ids(), ["classic-art"]);
        // A head from before there were official packs, and one with a field from after this
        // version, both read.
        let older = head("catalog/0123456789abcdef.sqlite.gz").replace(
            r#""official": ["classic-art", "Not An Id", "classic-art"],"#,
            "",
        );
        assert!(Head::parse(older.as_bytes()).unwrap().official.is_empty());
        let later = head("catalog/0123456789abcdef.sqlite.gz").replace(
            r#""mirrors": []"#,
            r#""mirrors": [], "installs": {"colours": 3}"#,
        );
        assert_eq!(Head::parse(later.as_bytes()).unwrap(), h);
        assert!(Head::parse(head("https://example.com/c.sqlite.gz").as_bytes()).is_ok());
        for bad in [
            "../catalog.sqlite.gz",
            "catalog/nothex.sqlite.gz",
            "http://x/c.gz",
        ] {
            assert!(Head::parse(head(bad).as_bytes()).is_err(), "{bad}");
        }
        let newer = r#"{ "version": 3, "whatever": [] }"#;
        assert!(Head::parse(newer.as_bytes())
            .unwrap_err()
            .contains("newer FolderSkin"));
        assert!(Head::parse(b"<html>").unwrap_err().contains("damaged"));
    }

    fn published(sha: &str) -> String {
        format!(
            r#"{{ "version": 2, "id": "colours", "hash": "0123456789abcdef", "name": "Colours",
  "author": "prajwal-svm", "license": "CC0-1.0", "tags": ["colour"],
  "skins": [ {{ "file": "Blue.PNG", "name": "Blue", "sha256": "{sha}", "bytes": 4370, "w": 512, "h": 480 }} ] }}"#
        )
    }

    #[test]
    fn a_published_pack_is_held_to_the_pack_rules_and_its_own() {
        let p = PublishedPack::parse(published(&"b".repeat(64)).as_bytes()).unwrap();
        assert_eq!(p.skins[0].ext(), "png");
        assert_eq!(p.to_pack().tags_for(&p.to_pack().skins[0]), ["colour"]);
        assert!(PublishedPack::parse(published("short").as_bytes())
            .unwrap_err()
            .contains("Blue.PNG"));
        let bad_license = published(&"b".repeat(64)).replace("CC0-1.0", "GPL-3.0");
        assert!(PublishedPack::parse(bad_license.as_bytes())
            .unwrap_err()
            .contains("license"));
        let escape = published(&"b".repeat(64)).replace("\"colours\"", "\"../x\"");
        assert!(PublishedPack::parse(escape.as_bytes()).is_err());
    }

    #[test]
    fn gzip_is_the_same_every_time_and_unpacking_is_capped() {
        let data = b"the same bytes, the same file".repeat(100);
        let a = gzip(&data);
        assert_eq!(a, gzip(&data));
        assert_eq!(gunzip(&a, 10_000).unwrap(), data);
        assert!(gunzip(&a, 100).is_err(), "over the cap");
        assert!(gunzip(b"not gzip", 100).is_err());
    }

    #[test]
    fn paths_are_named_after_their_contents() {
        assert_eq!(
            catalog_path("0123456789abcdef"),
            "catalog/0123456789abcdef.sqlite.gz"
        );
        assert_eq!(strip_path("ab"), "strips/ab.webp");
        assert_eq!(picture_path("cd", "jpg"), "pictures/cd.jpg");
        assert_eq!(manifest_path("x", "ab"), "packs/x/ab.json");
        assert!(is_hex(&sha256_hex(b""), 64));
    }
}
