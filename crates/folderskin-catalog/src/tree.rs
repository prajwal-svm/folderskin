//! The published community tree, version 2: what a host serves and the app reads.
//!
//! ```text
//! head.json                          small and mutable: which catalog is current
//! catalog/<generation>.sqlite.gz     every pack and skin, searchable (see `build`)
//! strips/<pack hash>.webp            a pack's first four skins as folders, side by side
//! thumbs/<sha256>.webp               one skin as its folder, 256 px
//! drive-thumbs/<sha256>.webp         one skin of a pack of drives as its drive, 256 px
//! pictures/<sha256>.<ext>            one skin's picture, as the pack has it
//! packs/<id>/<pack hash>.json        pack.json with each picture's size and SHA-256
//! collection/<hash>.json             the official collection, every skin, for the website
//! ```
//!
//! Packs of drives are in a catalog of their own making, beside the other: `with_drives` in
//! `head.json` names a catalog of every pack, drives and all, and `catalog` goes on naming one of
//! the packs of folders alone. A FolderSkin from before drives reads `catalog`, so it never lists
//! a pack it can't add, and one that takes drives reads `with_drives`.
//!
//! The official collection's pictures and thumbnails sit in `pictures/` and `thumbs/` beside the
//! packs' skins', and both catalogs list its skins (`build`), each with its picture's SHA-256.
//! `head.json` says how many there are, and what licence they have. The whole collection is also
//! one JSON file ([`PublishedCollection`]), which `head.json` names as `collection_manifest`: the
//! website lists the skins from it, having no catalog to search. The app never reads it.
//!
//! Everything but `head.json` is named after what is in it, so a host can let every cache keep
//! it forever: a changed pack is new files under new names, never new bytes under an old one.
//! That is what lets the same tree sit on GitHub today and on a bucket behind a CDN later.

use folderskin_core::pack::{self, Pack, PackShape, PackSkin};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

/// The version `head.json` declares, and the name of the folder the tree sits in.
pub const HEAD_VERSION: u32 = 2;
/// The one file that changes in place.
pub const HEAD_FILE: &str = "head.json";
/// The version a published `packs/<id>/<hash>.json` of folders declares.
pub const MANIFEST_VERSION: u32 = 2;
/// The version a published pack of drives declares, with `"shape": "drive"`: one a FolderSkin from
/// before drives says is for a newer FolderSkin.
pub const DRIVE_MANIFEST_VERSION: u32 = 3;
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
    /// Packs whose ids changed (`moved.json` beside `packs/`), each old id to the one it has now,
    /// so an app can follow a pack it added under an old id. Left out when none has.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub moved: BTreeMap<String, String>,
    /// The catalog with the packs of drives in it as well, for the apps that take them. Left out
    /// when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_drives: Option<WithDrives>,
    /// How many skins the official collection has, which both catalogs list. Left out when it
    /// has none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub collection: usize,
    /// The licence of every skin in the official collection (`collection.json`'s), such as
    /// "MIT". Left out when there is no collection.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub collection_license: String,
    /// Where the whole official collection is listed for the website ([`PublishedCollection`]),
    /// as [`collection_manifest_path`] names it. Left out when there is no collection.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub collection_manifest: String,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// A catalog of every pack, the packs of drives with the others, and which of them are drives.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WithDrives {
    /// Names it, as [`Head::generation`] names the other.
    pub generation: String,
    pub packs: usize,
    pub skins: usize,
    pub catalog: CatalogRef,
    /// The ids of the packs of drives in it.
    pub drives: Vec<String>,
}

impl WithDrives {
    fn is_sane(&self) -> bool {
        is_hex(&self.generation, 16)
            && is_hex(&self.catalog.sha256, 64)
            && self.catalog.bytes > 0
            && self.catalog.bytes <= MAX_CATALOG_BYTES
            && is_catalog_url(&self.catalog.url)
    }
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
        let mut head: Head = serde_json::from_value(value).map_err(|_| damaged())?;
        // The packs of folders don't need the other catalog: one that isn't right is left out.
        if head.with_drives.as_ref().is_some_and(|w| !w.is_sane()) {
            head.with_drives = None;
        }
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

    /// The catalog an app that takes packs of drives reads: its generation, where it is, and the
    /// ids of the packs of drives in it (pack ids only, without repeats). The one of every pack
    /// when there is one, and the packs of folders otherwise.
    pub fn catalog_with_drives(&self) -> (&str, &CatalogRef, Vec<String>) {
        match &self.with_drives {
            Some(w) => (&w.generation, &w.catalog, pack_ids(&w.drives)),
            None => (&self.generation, &self.catalog, Vec::new()),
        }
    }

    /// The id pack `id` has now: where `moved` says it went, or `id` itself. An entry that
    /// isn't two pack ids is passed over rather than followed.
    pub fn current_id<'a>(&'a self, id: &'a str) -> &'a str {
        match self.moved.get(id) {
            Some(to) if pack::is_pack_id(id) && pack::is_pack_id(to) => to,
            _ => id,
        }
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
    /// `"drive"` for a pack of drives, which is version 3; left out of a pack of folders, which
    /// is published exactly as it always was.
    #[serde(default, skip_serializing_if = "PackShape::is_folder")]
    pub shape: PackShape,
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
    /// the id and hash it claims, a SHA-256 and a size within the limits for each picture, and
    /// pictures that come to [`pack::MAX_PACK_BYTES`] at most.
    pub fn parse(bytes: &[u8]) -> Result<PublishedPack, String> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err("that pack's list is too big".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "that pack's list is damaged".to_string())?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version.is_some_and(|v| v > u64::from(DRIVE_MANIFEST_VERSION)) {
            return Err("this pack needs a newer FolderSkin".into());
        }
        let published: PublishedPack =
            serde_json::from_value(value).map_err(|_| "that pack's list is damaged".to_string())?;
        let mut problems = published.to_pack().problems();
        let version = manifest_version(published.shape);
        if published.version != version {
            problems.push(format!("it needs \"version\": {version}"));
        }
        if !pack::is_pack_id(&published.id) || !is_hex(&published.hash, 16) {
            problems.push("its id or version isn't one FolderSkin can use".into());
        }
        for skin in &published.skins {
            let fits = skin.bytes > 0
                && skin.bytes <= pack::MAX_READ_PICTURE_BYTES as u64
                && (pack::MIN_PICTURE_SIDE..=pack::MAX_PICTURE_SIDE).contains(&skin.w)
                && (pack::MIN_PICTURE_SIDE..=pack::MAX_PICTURE_SIDE).contains(&skin.h);
            if !is_hex(&skin.sha256, 64) || !fits {
                problems.push(format!("{} isn't described properly", skin.file));
            }
        }
        let total = published
            .skins
            .iter()
            .fold(0u64, |sum, skin| sum.saturating_add(skin.bytes));
        if total > pack::MAX_PACK_BYTES as u64 {
            problems.push(format!(
                "its pictures come to more than the {} MB a pack can be",
                pack::MAX_PACK_BYTES / (1024 * 1024)
            ));
        }
        if problems.is_empty() {
            Ok(published)
        } else {
            Err(problems.join(". "))
        }
    }

    /// The pack as its `pack.json` has it, for the checks and for saving its skins.
    pub fn to_pack(&self) -> Pack {
        Pack {
            version: self.shape.version(),
            shape: self.shape.declared(),
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

/// The version a published collection declares.
pub const COLLECTION_MANIFEST_VERSION: u32 = 1;

/// The official collection as the website reads it: every skin, in `collection.json`'s order,
/// under the licence they all have.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublishedCollection {
    pub version: u32,
    pub license: String,
    pub skins: Vec<PublishedOfficialSkin>,
}

/// One skin of the published collection.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublishedOfficialSkin {
    pub name: String,
    pub tags: Vec<String>,
    /// SHA-256 of the picture, in hex: its name under `pictures/` and `thumbs/`.
    pub sha256: String,
    /// The picture's extension, part of its name under `pictures/`.
    pub ext: String,
    pub bytes: u64,
    pub w: u32,
    pub h: u32,
    /// The day it joined the collection, "YYYY-MM-DD".
    pub added: String,
}

impl PublishedCollection {
    /// The file's bytes, the same for the same collection every time, and the path it is
    /// published at, named after them.
    pub fn to_file(&self) -> Result<(String, Vec<u8>), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())? + "\n";
        let path = collection_manifest_path(&sha256_hex(json.as_bytes())[..16]);
        Ok((path, json.into_bytes()))
    }
}

/// The version a published pack of `shape` declares.
pub fn manifest_version(shape: PackShape) -> u32 {
    match shape {
        PackShape::Folder => MANIFEST_VERSION,
        PackShape::Drive => DRIVE_MANIFEST_VERSION,
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

/// A skin's thumbnail as a pack of drives shows it: on a drive, which the same picture in a pack
/// of folders isn't.
pub fn drive_thumb_path(sha256: &str) -> String {
    format!("drive-thumbs/{sha256}.webp")
}

/// [`thumb_path`] or [`drive_thumb_path`], for a skin of a pack of `shape`.
pub fn thumb_path_for(shape: PackShape, sha256: &str) -> String {
    match shape {
        PackShape::Folder => thumb_path(sha256),
        PackShape::Drive => drive_thumb_path(sha256),
    }
}

pub fn picture_path(sha256: &str, ext: &str) -> String {
    format!("pictures/{sha256}.{ext}")
}

pub fn manifest_path(id: &str, pack_hash: &str) -> String {
    format!("packs/{id}/{pack_hash}.json")
}

/// Where the published collection is: named after sixteen hex digits of its file's SHA-256.
pub fn collection_manifest_path(hash: &str) -> String {
    format!("collection/{hash}.json")
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
    fn a_head_follows_packs_that_moved_and_leaves_the_field_out_when_none_did() {
        let with = head("catalog/0123456789abcdef.sqlite.gz").replace(
            r#""mirrors": []"#,
            r#""mirrors": [], "moved": { "classic-art": "classic-art-k7q2mx", "Bad Id": "colours-k7q2mx", "colours": "../up" }"#,
        );
        let h = Head::parse(with.as_bytes()).unwrap();
        assert_eq!(h.current_id("classic-art"), "classic-art-k7q2mx");
        assert_eq!(h.current_id("greek-art"), "greek-art");
        // Entries that aren't two pack ids are passed over, not followed.
        assert_eq!(h.current_id("Bad Id"), "Bad Id");
        assert_eq!(h.current_id("colours"), "colours");
        // A head written with nothing moved has no "moved" at all, as before there was one.
        let none = Head::parse(head("catalog/0123456789abcdef.sqlite.gz").as_bytes()).unwrap();
        assert!(none.moved.is_empty());
        assert!(!serde_json::to_string(&none).unwrap().contains("moved"));
    }

    #[test]
    fn a_head_says_how_big_the_collection_is_and_leaves_it_out_when_there_is_none() {
        let none = Head::parse(head("catalog/0123456789abcdef.sqlite.gz").as_bytes()).unwrap();
        assert_eq!(
            (none.collection, none.collection_license.as_str()),
            (0, ""),
            "a head from before the collection"
        );
        assert!(!serde_json::to_string(&none).unwrap().contains("collection"));
        let some = head("catalog/0123456789abcdef.sqlite.gz").replace(
            r#""mirrors": []"#,
            r#""mirrors": [], "collection": 587, "collection_license": "MIT",
  "collection_manifest": "collection/0123456789abcdef.json""#,
        );
        let h = Head::parse(some.as_bytes()).unwrap();
        assert_eq!((h.collection, h.collection_license.as_str()), (587, "MIT"));
        assert_eq!(h.collection_manifest, "collection/0123456789abcdef.json");
        assert!(serde_json::to_string(&h).unwrap().ends_with(
            r#""collection":587,"collection_license":"MIT","collection_manifest":"collection/0123456789abcdef.json"}"#
        ));
    }

    #[test]
    fn a_published_collection_is_named_after_its_bytes_which_are_the_same_every_time() {
        let collection = PublishedCollection {
            version: COLLECTION_MANIFEST_VERSION,
            license: "MIT".into(),
            skins: vec![PublishedOfficialSkin {
                name: "Giraffe cola".into(),
                tags: vec!["pop art".into()],
                sha256: "b".repeat(64),
                ext: "webp".into(),
                bytes: 1234,
                w: 1024,
                h: 1024,
                added: "2026-09-30".into(),
            }],
        };
        let (path, bytes) = collection.to_file().unwrap();
        assert_eq!(collection.to_file().unwrap(), (path.clone(), bytes.clone()));
        assert_eq!(
            path,
            collection_manifest_path(&sha256_hex(&bytes)[..16]),
            "{path}"
        );
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "version": 1,
                "license": "MIT",
                "skins": [{
                    "name": "Giraffe cola", "tags": ["pop art"], "sha256": "b".repeat(64),
                    "ext": "webp", "bytes": 1234, "w": 1024, "h": 1024, "added": "2026-09-30"
                }]
            })
        );
        let read: PublishedCollection = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(read, collection);
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

        // Pictures within 2 MB each, and 64 MB together at most.
        let skin = |n: usize, bytes: u64| {
            format!(
                r#"{{ "file": "p{n}.png", "name": "P{n}", "sha256": "{}", "bytes": {bytes}, "w": 512, "h": 480 }}"#,
                "b".repeat(64)
            )
        };
        let with = |skins: Vec<String>| {
            let one = format!(
                r#"{{ "file": "Blue.PNG", "name": "Blue", "sha256": "{}", "bytes": 4370, "w": 512, "h": 480 }}"#,
                "b".repeat(64)
            );
            published(&"b".repeat(64)).replace(&one, &skins.join(", "))
        };
        let two_mb = 2 * 1024 * 1024;
        let fits: Vec<String> = (0..32).map(|n| skin(n, two_mb)).collect();
        assert!(PublishedPack::parse(with(fits).as_bytes()).is_ok(), "64 MB");
        let over: Vec<String> = (0..33).map(|n| skin(n, two_mb)).collect();
        assert_eq!(
            PublishedPack::parse(with(over).as_bytes()).unwrap_err(),
            "its pictures come to more than the 64 MB a pack can be"
        );
        let big = with(vec![skin(0, two_mb + 1)]);
        assert!(PublishedPack::parse(big.as_bytes())
            .unwrap_err()
            .contains("p0.png isn't described properly"));
    }

    #[test]
    fn a_published_pack_of_drives_is_version_3_and_one_of_folders_is_as_it_was() {
        let folders = PublishedPack::parse(published(&"b".repeat(64)).as_bytes()).unwrap();
        assert_eq!(folders.shape, PackShape::Folder);
        assert!(!serde_json::to_string(&folders).unwrap().contains("shape"));
        let drives = published(&"b".repeat(64))
            .replace(r#""version": 2,"#, r#""version": 3, "shape": "drive","#);
        let p = PublishedPack::parse(drives.as_bytes()).unwrap();
        assert_eq!(p.shape, PackShape::Drive);
        assert_eq!(p.to_pack().shape(), PackShape::Drive);
        assert_eq!(p.to_pack().version, pack::DRIVE_PACK_VERSION);
        // Each says its own version, and a newer one says so.
        let mismatched = published(&"b".repeat(64))
            .replace(r#""version": 2,"#, r#""version": 2, "shape": "drive","#);
        assert!(PublishedPack::parse(mismatched.as_bytes())
            .unwrap_err()
            .contains("\"version\": 3"));
        let newer = published(&"b".repeat(64)).replace(r#""version": 2,"#, r#""version": 4,"#);
        assert!(PublishedPack::parse(newer.as_bytes())
            .unwrap_err()
            .contains("newer FolderSkin"));
        assert_eq!(
            thumb_path_for(PackShape::Drive, "ab"),
            "drive-thumbs/ab.webp"
        );
        assert_eq!(thumb_path_for(PackShape::Folder, "ab"), "thumbs/ab.webp");
    }

    #[test]
    fn a_head_names_a_catalog_with_the_packs_of_drives_for_the_apps_that_take_them() {
        let plain = Head::parse(head("catalog/0123456789abcdef.sqlite.gz").as_bytes()).unwrap();
        assert_eq!(plain.with_drives, None);
        let (generation, catalog, drives) = plain.catalog_with_drives();
        assert_eq!((generation, catalog), (&*plain.generation, &plain.catalog));
        assert!(drives.is_empty());

        let with = |url: &str| {
            head("catalog/0123456789abcdef.sqlite.gz").replace(
                r#""mirrors": []"#,
                &format!(
                    r#""mirrors": [], "with_drives": {{ "generation": "fedcba9876543210", "packs": 2,
  "skins": 3, "catalog": {{ "url": "{url}", "sha256": "{}", "bytes": 12 }},
  "drives": ["plain-drives", "Not An Id", "plain-drives"] }}"#,
                    "c".repeat(64)
                ),
            )
        };
        let h = Head::parse(with("catalog/fedcba9876543210.sqlite.gz").as_bytes()).unwrap();
        let (generation, catalog, drives) = h.catalog_with_drives();
        assert_eq!(generation, "fedcba9876543210");
        assert_eq!(catalog.url, "catalog/fedcba9876543210.sqlite.gz");
        assert_eq!(drives, ["plain-drives"]);
        // The packs of folders don't need it: one that isn't right is left out, not the head.
        let wrong = Head::parse(with("../elsewhere.sqlite.gz").as_bytes()).unwrap();
        assert_eq!(wrong.with_drives, None);
        assert_eq!(wrong.catalog_with_drives().0, "0123456789abcdef");
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
        assert_eq!(
            collection_manifest_path("0123456789abcdef"),
            "collection/0123456789abcdef.json"
        );
        assert!(is_hex(&sha256_hex(b""), 64));
    }
}
