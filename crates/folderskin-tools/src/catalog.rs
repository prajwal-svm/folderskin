//! `packs catalog`: the published tree the app reads, version 2 (the layout is in
//! `folderskin_catalog::tree`), built from the packs that pass `packs check`.
//!
//! Every file but `head.json` is named after what is in it, so a file that is already there is
//! already right: a second run only writes what changed, and renders a thumbnail only for a
//! picture it hasn't seen. A name, once published, never holds anything else, since caches and
//! mirrors keep it for a year: when this build would write a catalog or a manifest that says the
//! same in other bytes (a newer gzip, say), the one already there stays and is the one vouched
//! for. `head.json` is written last, so a host serving the folder while it is being built never
//! names a catalog that isn't there yet. Files that neither this generation
//! nor the one before it use are removed after that: an app that read the old `head.json` a
//! moment ago can still fetch what it names.
//!
//! `index.json` and `previews/` are left as `packs index` writes them, for the versions of the
//! app that read those.
//!
//! Packs of drives are published as the others are, with their thumbnails drawn on a drive in
//! `drive-thumbs/`, and listed in a catalog of their own making: one of every pack, which
//! `head.json` names as `with_drives`, while `catalog` goes on naming one of the packs of folders
//! alone, for the versions of the app from before drives.
//!
//! The official collection (`collection/`, collection.rs) is published beside the packs: each
//! picture in `pictures/` and its thumbnail in `thumbs/`, drawn as a pack skin's is, and its skins
//! in both catalogs, in `collection.json`'s order. `head.json` says how many there are and what
//! licence they have. The whole collection is also written as one file for the website,
//! `collection/<hash>.json` (`tree::PublishedCollection`), which `head.json` names as
//! `collection_manifest` and which is kept and cleaned up like a pack's manifest. A collection with
//! a problem stops the build as a pack with one does.

use crate::packs::{self, Changes, Report, PACKS_DIR};
use crate::{git, make};
use folderskin_catalog::tree::{
    self, CatalogRef, Head, PublishedCollection, PublishedOfficialSkin, PublishedPack,
    PublishedSkin, WithDrives, COLLECTION_MANIFEST_VERSION, HEAD_FILE, HEAD_VERSION,
};
use folderskin_catalog::{build, Catalog, CollectionRecord, PackRecord};
use folderskin_core::collection::{date_of, COLLECTION_DIR};
use folderskin_core::pack::{self, IndexEntry, Moved, Pack, PackShape, MANIFEST_FILE};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The packs to show first, in order: a JSON list of ids beside `packs/`. Optional.
pub const FEATURED_FILE: &str = "featured.json";
/// The side of a skin's thumbnail, the size the app's pack viewer draws.
pub const THUMB_SIDE: u32 = 256;
/// Thumbnails and strips are small pictures seen small: cwebp's quality 80 is plenty.
const WEBP_QUALITY: u8 = 80;

/// Everything [`write_catalog`] needs besides the packs.
#[derive(Debug, Clone, Default)]
pub struct CatalogOptions {
    /// Where the tree goes, such as `community/v2`.
    pub out: PathBuf,
    /// Other `https://` folders serving the same tree, for `head.json`.
    pub mirrors: Vec<String>,
    /// The `cwebp` program. Without it thumbnails and strips are lossless WebP, several times
    /// bigger.
    pub cwebp: Option<PathBuf>,
    /// When each pack was first published, in Unix seconds, by id ([`git_dates`]). A pack
    /// missing from it is dated 0.
    pub dates: HashMap<String, i64>,
}

/// What [`write_catalog`] made.
#[derive(Debug)]
pub struct Built {
    pub head: Head,
    pub changes: Changes,
}

/// Writes the published tree for every pack in `report` (the packs in `dir`) into `opts.out`.
/// Writes nothing when a pack has a problem.
pub fn write_catalog(dir: &Path, report: &Report, opts: &CatalogOptions) -> Result<Built, String> {
    if !report.problems.is_empty() {
        return Err(format!("{}; nothing was written", report.summary()));
    }
    let out = &opts.out;
    if same_folder(out, dir) || same_folder(&out.join(PACKS_DIR), &dir.join(PACKS_DIR)) {
        return Err(format!(
            "{} is the packs folder itself; write the tree into a folder of its own, such as {}",
            out.display(),
            dir.join("v2").display()
        ));
    }
    if let Some(bad) = opts.mirrors.iter().find(|m| !tree::is_mirror(m)) {
        return Err(format!(
            "{bad} can't be a mirror: it has to be an https:// address with no ? or #"
        ));
    }
    let featured = packs::read_pack_list(dir, report, FEATURED_FILE)?;
    let official = packs::read_pack_list(dir, report, packs::OFFICIAL_FILE)?;
    // Before anything is written: the generation the old head.json names.
    let previous = previous_files(out);

    let mut packs: Vec<&(String, Pack)> = report.packs.iter().collect();
    packs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut changes = Changes::default();
    let mut keep: HashSet<String> = HashSet::new();
    let mut renders: Vec<Render> = Vec::new();
    // Every pack's record, each with whether it is a pack of drives.
    let mut records = Vec::new();
    for (id, pack) in packs {
        let folder = dir.join(PACKS_DIR).join(id);
        let (record, published) = publish_pack(id, pack, &folder, opts, out, &mut changes)
            .map_err(|e| format!("{id}: {e}"))?;
        for (skin, source) in published.skins.iter().zip(&pack.skins) {
            keep.insert(tree::picture_path(&skin.sha256, &skin.ext()));
            let thumb = tree::thumb_path_for(published.shape, &skin.sha256);
            if !out.join(&thumb).is_file() && keep.insert(thumb.clone()) {
                renders.push(Render::Thumb {
                    source: folder.join(&source.file),
                    shape: published.shape,
                    to: out.join(&thumb),
                });
            }
            keep.insert(thumb);
        }
        let strip = tree::strip_path(&published.hash);
        if !out.join(&strip).is_file() {
            renders.push(Render::Strip {
                folder: folder.clone(),
                pack: pack.clone(),
                to: out.join(&strip),
            });
        }
        keep.insert(strip);
        keep.insert(tree::manifest_path(id, &published.hash));
        records.push((published.shape, record));
    }
    // The official collection: each picture and its thumbnail beside the packs' skins'.
    let collection_dir = dir.join(COLLECTION_DIR);
    let mut collection = Vec::with_capacity(report.collection.len());
    for checked in &report.collection {
        let source = collection_dir.join(&checked.skin.file);
        let ext = tree::picture_ext(&checked.skin.file);
        let picture = tree::picture_path(&checked.sha256, &ext);
        copy_new(&source, &out.join(&picture), checked, &mut changes)?;
        keep.insert(picture);
        let thumb = tree::thumb_path(&checked.sha256);
        if !out.join(&thumb).is_file() && keep.insert(thumb.clone()) {
            renders.push(Render::Thumb {
                source,
                shape: PackShape::Folder,
                to: out.join(&thumb),
            });
        }
        keep.insert(thumb);
        collection.push(CollectionRecord {
            name: checked.skin.name.trim().to_string(),
            tags: checked.skin.tags.clone(),
            sha256: checked.sha256.clone(),
            ext,
            bytes: checked.bytes,
            w: checked.w,
            h: checked.h,
            added: checked.skin.added_at(),
        });
    }
    render_all(&renders, opts.cwebp.as_deref(), &mut changes)?;
    // The whole collection in one file, for the website, which has no catalog to search.
    let collection_manifest = if collection.is_empty() {
        String::new()
    } else {
        let (path, bytes) =
            published_collection(&report.collection_license, &collection).to_file()?;
        write_new(&out.join(&path), &bytes, &mut changes)?;
        keep.insert(path.clone());
        path
    };

    // The packs of folders alone, for every version of the app.
    let folders: Vec<PackRecord> = records
        .iter()
        .filter(|(shape, _)| *shape == PackShape::Folder)
        .map(|(_, record)| record.clone())
        .collect();
    let (generation, catalog) = write_database(out, &folders, &collection, &mut changes)?;
    keep.insert(catalog.url.clone());
    // Every pack, drives and all, for the versions that take packs of drives.
    let drives: Vec<String> = records
        .iter()
        .filter(|(shape, _)| *shape == PackShape::Drive)
        .map(|(_, record)| record.id.clone())
        .collect();
    let with_drives = if drives.is_empty() {
        None
    } else {
        let every: Vec<PackRecord> = records.iter().map(|(_, record)| record.clone()).collect();
        let (generation, catalog) = write_database(out, &every, &collection, &mut changes)?;
        keep.insert(catalog.url.clone());
        Some(WithDrives {
            generation,
            packs: every.len(),
            skins: every.iter().map(|r| r.count).sum(),
            catalog,
            drives,
        })
    };

    let head = Head {
        version: HEAD_VERSION,
        generation,
        packs: folders.len(),
        skins: folders.iter().map(|r| r.count).sum(),
        catalog,
        featured,
        official,
        mirrors: opts.mirrors.clone(),
        // Apps that know it follow a pack added under an old id to the one it has now.
        moved: report.moved.moved.clone(),
        with_drives,
        collection: collection.len(),
        collection_license: if collection.is_empty() {
            String::new()
        } else {
            report.collection_license.clone()
        },
        collection_manifest,
    };
    let json = serde_json::to_string_pretty(&head).map_err(|e| e.to_string())? + "\n";
    write_changed(&out.join(HEAD_FILE), json.as_bytes(), &mut changes)?;

    keep.extend(previous);
    prune(out, &keep, &mut changes)?;
    Ok(Built { head, changes })
}

/// The official collection under `license`, as the website reads it: every skin in `collection`
/// in its order, each dated by its day.
fn published_collection(license: &str, collection: &[CollectionRecord]) -> PublishedCollection {
    PublishedCollection {
        version: COLLECTION_MANIFEST_VERSION,
        license: license.to_string(),
        skins: collection
            .iter()
            .map(|record| PublishedOfficialSkin {
                name: record.name.clone(),
                tags: record.tags.clone(),
                sha256: record.sha256.clone(),
                ext: record.ext.clone(),
                bytes: record.bytes,
                w: record.w,
                h: record.h,
                added: date_of(record.added),
            })
            .collect(),
    }
}

/// Writes the catalog of `records` and the official `collection` (unless it is there already,
/// whatever gzip made it) and returns its generation and where it is.
fn write_database(
    out: &Path,
    records: &[PackRecord],
    collection: &[CollectionRecord],
    changes: &mut Changes,
) -> Result<(String, CatalogRef), String> {
    let bytes = build::to_bytes(records, collection)?;
    let generation = tree::sha256_hex(&bytes)[..16].to_string();
    let url = tree::catalog_path(&generation);
    let path = out.join(&url);
    let gz = match kept_catalog(&path, &bytes) {
        Some(gz) => gz,
        None => {
            let gz = tree::gzip(&bytes);
            write_file(&path, &gz)?;
            changes.written.push(path);
            gz
        }
    };
    Ok((
        generation,
        CatalogRef {
            url,
            sha256: tree::sha256_hex(&gz),
            bytes: gz.len() as u64,
        },
    ))
}

/// Reads one pack's files, copies its pictures in under their SHA-256, and writes its manifest.
/// Returns what the catalog lists of it and the manifest.
fn publish_pack(
    id: &str,
    pack: &Pack,
    folder: &Path,
    opts: &CatalogOptions,
    out: &Path,
    changes: &mut Changes,
) -> Result<(PackRecord, PublishedPack), String> {
    let read = |file: &str| {
        std::fs::read(folder.join(file)).map_err(|e| format!("{file} couldn't be read: {e}"))
    };
    let manifest = read(MANIFEST_FILE)?;
    // One pack at a time is in memory: its pictures come to 64 MB at most, as `packs check` holds.
    let pictures = pack
        .skins
        .iter()
        .map(|s| read(&s.file))
        .collect::<Result<Vec<_>, String>>()?;
    let hash = pack::pack_hash(
        &manifest,
        pack.skins
            .iter()
            .zip(&pictures)
            .map(|(s, bytes)| (s.file.as_str(), bytes.as_slice())),
    );
    let mut skins = Vec::new();
    for (skin, bytes) in pack.skins.iter().zip(&pictures) {
        let (w, h) = pack::check_picture(bytes).map_err(|e| format!("{} {e}", skin.file))?;
        let sha256 = tree::sha256_hex(bytes);
        let ext = tree::picture_ext(&skin.file);
        write_new(&out.join(tree::picture_path(&sha256, &ext)), bytes, changes)?;
        skins.push(PublishedSkin {
            file: skin.file.clone(),
            name: skin.name.trim().to_string(),
            tags: skin.tags.clone(),
            sha256,
            bytes: bytes.len() as u64,
            w,
            h,
        });
    }
    let bytes_total = pictures.iter().map(|p| p.len() as u64).sum();
    let published = PublishedPack {
        version: tree::manifest_version(pack.shape()),
        shape: pack.shape(),
        id: id.to_string(),
        hash: hash.clone(),
        name: pack.name.trim().to_string(),
        author: pack.author.clone(),
        license: pack.license.clone(),
        tags: pack.tags.clone(),
        skins,
    };
    let json = serde_json::to_string_pretty(&published).map_err(|e| e.to_string())? + "\n";
    // The app reads it back with the same checks, so a tree that builds is a tree it takes.
    PublishedPack::parse(json.as_bytes())?;
    let path = out.join(tree::manifest_path(id, &hash));
    let listed = match std::fs::read(&path)
        .ok()
        .filter(|kept| PublishedPack::parse(kept).is_ok_and(|p| p == published))
    {
        Some(kept) => kept,
        None => {
            write_changed(&path, json.as_bytes(), changes)?;
            json.into_bytes()
        }
    };

    let entry = IndexEntry::new(id, pack, hash.clone());
    let record = PackRecord {
        id: entry.id,
        name: entry.name,
        author: entry.author,
        license: entry.license,
        tags: entry.tags,
        hash,
        manifest: tree::sha256_hex(&listed),
        added: opts.dates.get(id).copied().unwrap_or(0),
        count: entry.count,
        bytes: bytes_total,
        skin_names: published.skins.iter().map(|s| s.name.clone()).collect(),
    };
    Ok((record, published))
}

/// A picture to draw for the tree.
enum Render {
    /// One skin as its folder, or a pack of drives' as its drive, [`THUMB_SIDE`] px.
    Thumb {
        source: PathBuf,
        shape: PackShape,
        to: PathBuf,
    },
    /// A pack's first skins side by side, as `packs index` draws its preview.
    Strip {
        folder: PathBuf,
        pack: Pack,
        to: PathBuf,
    },
}

/// Draws every picture in `renders` on all cores, each written once it is encoded.
fn render_all(
    renders: &[Render],
    cwebp: Option<&Path>,
    changes: &mut Changes,
) -> Result<(), String> {
    let next = AtomicUsize::new(0);
    let written = Mutex::new(Vec::new());
    let failed = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(renders.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = renders.get(i) else { break };
                match render(job, cwebp) {
                    Ok(path) => lock(&written).push(path),
                    Err(e) => lock(&failed).push(e),
                }
            });
        }
    });
    let mut failed = failed.into_inner().unwrap_or_default();
    failed.sort();
    if let Some(first) = failed.first() {
        return Err(first.clone());
    }
    let mut written = written.into_inner().unwrap_or_default();
    written.sort();
    changes.written.extend(written);
    Ok(())
}

fn render(job: &Render, cwebp: Option<&Path>) -> Result<PathBuf, String> {
    let (img, to) = match job {
        Render::Thumb { source, shape, to } => {
            let name = source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let bytes =
                std::fs::read(source).map_err(|e| format!("{name} couldn't be read: {e}"))?;
            let rgba = pack::decode_picture(&bytes).map_err(|e| format!("{name} {e}"))?;
            let icon = packs::render_skin_for(rgba, THUMB_SIDE, *shape)
                .map_err(|e| format!("{name} {e}"))?;
            (icon, to)
        }
        Render::Strip { folder, pack, to } => (packs::preview_strip(folder, pack)?, to),
    };
    let webp = encode_webp(&img, cwebp)?;
    write_file(to, &webp)?;
    Ok(to.clone())
}

/// `img` as a WebP: lossy colour with lossless alpha from cwebp when there is one, lossless
/// otherwise.
fn encode_webp(img: &RgbaImage, cwebp: Option<&Path>) -> Result<Vec<u8>, String> {
    if let Some(cwebp) = cwebp {
        return make::run_cwebp(cwebp, img, WEBP_QUALITY);
    }
    let mut out = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut out)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("couldn't be saved as WebP: {e}"))?;
    Ok(out)
}

/// The files the generation named by `<out>/head.json` uses, as paths inside `out`; none when
/// there is no such head or its catalog can't be read.
fn previous_files(out: &Path) -> HashSet<String> {
    let mut files = HashSet::new();
    let Some(head) = std::fs::read(out.join(HEAD_FILE))
        .ok()
        .and_then(|bytes| Head::parse(&bytes).ok())
    else {
        return files;
    };
    // The catalog of every pack names them all, drives and folders, and the collection's skins.
    let (_, every, _) = head.catalog_with_drives();
    let Some(catalog) = std::fs::read(out.join(&every.url))
        .ok()
        .and_then(|gz| tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED).ok())
        .and_then(|bytes| Catalog::from_bytes(&bytes).ok())
    else {
        return files;
    };
    let Ok(versions) = catalog.versions() else {
        return files;
    };
    files.insert(head.catalog.url.clone());
    files.insert(every.url.clone());
    if !head.collection_manifest.is_empty() {
        files.insert(head.collection_manifest.clone());
    }
    for (sha256, ext) in catalog.collection_pictures().unwrap_or_default() {
        files.insert(tree::picture_path(&sha256, &ext));
        files.insert(tree::thumb_path(&sha256));
    }
    for (id, hash) in versions {
        let manifest = tree::manifest_path(&id, &hash);
        if let Some(published) = std::fs::read(out.join(&manifest))
            .ok()
            .and_then(|bytes| PublishedPack::parse(&bytes).ok())
        {
            for skin in &published.skins {
                files.insert(tree::picture_path(&skin.sha256, &skin.ext()));
                files.insert(tree::thumb_path_for(published.shape, &skin.sha256));
            }
        }
        files.insert(manifest);
        files.insert(tree::strip_path(&hash));
    }
    files
}

/// Removes the tree's files that aren't in `keep`. Only names the tree itself uses are touched
/// (content hashes with the right extension), so nothing else put in these folders is lost.
fn prune(out: &Path, keep: &HashSet<String>, changes: &mut Changes) -> Result<(), String> {
    let mut remove = |relative: String| -> Result<(), String> {
        if keep.contains(&relative) {
            return Ok(());
        }
        let path = out.join(&relative);
        std::fs::remove_file(&path)
            .map_err(|e| format!("couldn't remove {}: {e}", path.display()))?;
        changes.removed.push(path);
        Ok(())
    };
    for (folder, is_ours) in [
        ("catalog", is_catalog_file as fn(&str) -> bool),
        ("strips", |f| hex_named(f, 16, &["webp"])),
        ("thumbs", |f| hex_named(f, 64, &["webp"])),
        ("drive-thumbs", |f| hex_named(f, 64, &["webp"])),
        ("pictures", |f| hex_named(f, 64, pack::PICTURE_EXTENSIONS)),
        ("collection", |f| hex_named(f, 16, &["json"])),
    ] {
        for file in names(&out.join(folder))? {
            if is_ours(&file) {
                remove(format!("{folder}/{file}"))?;
            }
        }
    }
    for id in names(&out.join(PACKS_DIR))? {
        if !pack::is_pack_id(&id) {
            continue;
        }
        let pack_dir = out.join(PACKS_DIR).join(&id);
        for file in names(&pack_dir)? {
            if hex_named(&file, 16, &["json"]) {
                remove(format!("{PACKS_DIR}/{id}/{file}"))?;
            }
        }
        // Gone when it's empty; left when something else is in it.
        let _ = std::fs::remove_dir(&pack_dir);
    }
    changes.removed.sort();
    Ok(())
}

fn is_catalog_file(file: &str) -> bool {
    file.strip_suffix(".sqlite.gz")
        .is_some_and(|stem| tree::is_hex(stem, 16))
}

/// `<len hex digits>.<one of exts>`.
fn hex_named(file: &str, len: usize, exts: &[&str]) -> bool {
    file.split_once('.')
        .is_some_and(|(stem, ext)| tree::is_hex(stem, len) && exts.contains(&ext))
}

/// The names in a folder; none when it isn't there.
fn names(dir: &Path) -> Result<Vec<String>, String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            Ok(names)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("couldn't read {}: {e}", dir.display())),
    }
}

/// The catalog file at `path`, when it is there and unpacks to `database`: this generation,
/// already published, whatever gzip made it.
fn kept_catalog(path: &Path, database: &[u8]) -> Option<Vec<u8>> {
    let gz = std::fs::read(path).ok()?;
    let unpacked = tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED).ok()?;
    (unpacked == database).then_some(gz)
}

/// Writes a file named after its contents, unless it is already there: then it already holds
/// these bytes, and reading it back to make sure would cost as much as the whole build.
fn write_new(path: &Path, bytes: &[u8], changes: &mut Changes) -> Result<(), String> {
    if std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == bytes.len() as u64) {
        return Ok(());
    }
    write_file(path, bytes)?;
    changes.written.push(path.to_path_buf());
    Ok(())
}

/// Copies the collection's picture at `from` to `path`, the name its contents give it, unless it
/// is there already. It is read only to be copied, and has to be the picture `checked` says it
/// is, which it was a moment ago.
fn copy_new(
    from: &Path,
    path: &Path,
    checked: &crate::collection::CheckedSkin,
    changes: &mut Changes,
) -> Result<(), String> {
    if std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == checked.bytes) {
        return Ok(());
    }
    let file = &checked.skin.file;
    let bytes = std::fs::read(from).map_err(|e| format!("{file} couldn't be read: {e}"))?;
    if tree::sha256_hex(&bytes) != checked.sha256 {
        return Err(format!(
            "{file} changed while the catalog was being written; run it again"
        ));
    }
    write_file(path, &bytes)?;
    changes.written.push(path.to_path_buf());
    Ok(())
}

/// Writes a file whose name doesn't say what is in it, unless it holds `bytes` already.
fn write_changed(path: &Path, bytes: &[u8], changes: &mut Changes) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
    }
    packs::write_if_changed(path, bytes, changes)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("couldn't write {}: {e}", path.display()))
}

/// True when `a` and `b` are the same folder, however they are written.
fn same_folder(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// When each pack's `pack.json` first appeared in the git history of `dir`, in Unix seconds, by
/// pack id. Empty when git isn't installed or `dir` isn't in a repository, and a pack not yet
/// committed has no date: those sort last under Newest. A shallow clone dates everything to its
/// one commit, so CI fetches the whole history.
///
/// A pack keeps its date when its id changes: it is dated by the earliest of its own id and every
/// old id `moved` (`moved.json`) sends to it, so `packs rename` never makes a pack new again.
pub fn git_dates(dir: &Path, moved: &Moved) -> HashMap<String, i64> {
    // Without rename detection, the commit that moved a folder counts as adding the new one, so
    // the log reads the same whatever git's `diff.renames` is set to; the old id still dates the
    // pack from before.
    let mut command = git::git(dir);
    command.args([
        "log",
        "--no-renames",
        "--format=%x00%ct",
        "--name-only",
        "--relative",
        "--diff-filter=A",
        "--",
        "packs/*/pack.json",
    ]);
    let Ok(log) = git::run(command, "read the history") else {
        return HashMap::new();
    };
    follow_moves(parse_git_dates(&log), moved)
}

/// `dates`, with every pack that moved dated by the earliest of its ids.
fn follow_moves(mut dates: HashMap<String, i64>, moved: &Moved) -> HashMap<String, i64> {
    for (old, new) in &moved.moved {
        if let Some(&then) = dates.get(old) {
            let date = dates.entry(new.clone()).or_insert(then);
            *date = (*date).min(then);
        }
    }
    dates
}

/// Reads `git log --format=%x00%ct --name-only` over `packs/*/pack.json`: each commit's time on
/// a line after a NUL, then the files it added. A pack added twice keeps the earlier date.
fn parse_git_dates(log: &str) -> HashMap<String, i64> {
    let mut dates: HashMap<String, i64> = HashMap::new();
    let mut when = 0;
    for line in log.lines() {
        if let Some(time) = line.strip_prefix('\0') {
            when = time.trim().parse().unwrap_or(0);
        } else if let Some(id) = line
            .trim()
            .replace('\\', "/")
            .strip_prefix("packs/")
            .and_then(|rest| rest.strip_suffix("/pack.json"))
        {
            let date = dates.entry(id.to_string()).or_insert(when);
            *date = (*date).min(when);
        }
    }
    dates
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_catalog::Query;
    use folderskin_core::raster;
    use image::Rgba;

    /// A community folder in the system temp folder, removed when the test ends.
    struct Community(PathBuf);

    impl Community {
        fn new(test: &str) -> Community {
            let dir = std::env::temp_dir()
                .join(format!("folderskin-catalog-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(PACKS_DIR)).unwrap();
            Community(dir)
        }

        /// Writes a pack whose skins are (file, name, colour).
        fn pack(&self, id: &str, name: &str, skins: &[(&str, &str, [u8; 3])]) {
            self.pack_of(PackShape::Folder, id, name, skins);
        }

        /// Writes a pack of `shape` whose skins are (file, name, colour).
        fn pack_of(&self, shape: PackShape, id: &str, name: &str, skins: &[(&str, &str, [u8; 3])]) {
            let folder = self.0.join(PACKS_DIR).join(id);
            std::fs::create_dir_all(&folder).unwrap();
            let listed: Vec<String> = skins
                .iter()
                .map(|(file, name, _)| format!(r#"{{ "file": "{file}", "name": "{name}" }}"#))
                .collect();
            let version = match shape {
                PackShape::Folder => r#""version": 1"#,
                PackShape::Drive => r#""version": 2, "shape": "drive""#,
            };
            let json = format!(
                r#"{{ {version}, "name": "{name}", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["test"], "skins": [{}] }}"#,
                listed.join(", ")
            );
            std::fs::write(folder.join(MANIFEST_FILE), json).unwrap();
            for (file, _, [r, g, b]) in skins {
                let art = RgbaImage::from_pixel(320, 300, Rgba([*r, *g, *b, 255]));
                std::fs::write(folder.join(file), raster::encode_png(&art)).unwrap();
            }
        }

        /// Writes the official collection: each (file, name, colour), added on `day`.
        fn collection(&self, skins: &[(&str, &str, [u8; 3])], day: &str) {
            let folder = self.0.join(COLLECTION_DIR);
            std::fs::create_dir_all(&folder).unwrap();
            let listed: Vec<String> = skins
                .iter()
                .map(|(file, name, _)| {
                    format!(
                        r#"{{ "file": "{file}", "name": "{name}", "tags": ["official"], "added": "{day}" }}"#
                    )
                })
                .collect();
            let json = format!(
                r#"{{ "version": 1, "license": "MIT", "skins": [{}] }}"#,
                listed.join(", ")
            );
            std::fs::write(folder.join("collection.json"), json).unwrap();
            for (file, _, [r, g, b]) in skins {
                let art = RgbaImage::from_pixel(320, 300, Rgba([*r, *g, *b, 255]));
                std::fs::write(folder.join(file), raster::encode_png(&art)).unwrap();
            }
        }

        fn out(&self) -> PathBuf {
            self.0.join("v2")
        }

        fn build(&self) -> Result<Built, String> {
            let report = packs::check(&self.0).unwrap();
            let opts = CatalogOptions {
                out: self.out(),
                mirrors: vec!["https://mirror.example/v2".into()],
                cwebp: None,
                dates: HashMap::from([("reds".to_string(), 200), ("blues".to_string(), 100)]),
            };
            write_catalog(&self.0, &report, &opts)
        }

        /// Every file in the tree, as paths inside it.
        fn files(&self) -> Vec<String> {
            fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
                for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, base, out);
                    } else {
                        let rel = path.strip_prefix(base).unwrap().to_string_lossy();
                        out.push(rel.replace('\\', "/"));
                    }
                }
            }
            let mut files = Vec::new();
            walk(&self.out(), &self.out(), &mut files);
            files.sort();
            files
        }
    }

    impl Drop for Community {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn two_packs(c: &Community) {
        c.pack(
            "reds",
            "Reds",
            &[
                ("a.png", "Ruby", [200, 30, 30]),
                ("b.png", "Brick", [150, 60, 40]),
            ],
        );
        c.pack("blues", "Blues", &[("c.png", "Navy", [20, 30, 120])]);
    }

    fn open_catalog(c: &Community, head: &Head) -> Catalog {
        let gz = std::fs::read(c.out().join(&head.catalog.url)).unwrap();
        assert_eq!(tree::sha256_hex(&gz), head.catalog.sha256);
        assert_eq!(gz.len() as u64, head.catalog.bytes);
        let bytes = tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED).unwrap();
        assert_eq!(tree::sha256_hex(&bytes)[..16], head.generation);
        Catalog::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn the_tree_holds_everything_the_app_needs_named_after_its_contents() {
        let c = Community::new("tree");
        two_packs(&c);
        let built = c.build().unwrap();
        let head = Head::parse(&std::fs::read(c.out().join(HEAD_FILE)).unwrap()).unwrap();
        assert_eq!(head, built.head);
        assert_eq!((head.packs, head.skins), (2, 3));
        assert_eq!(head.mirrors, ["https://mirror.example/v2"]);
        assert!(head.featured.is_empty(), "no featured.json");

        let catalog = open_catalog(&c, &head);
        let found = catalog
            .search(&Query {
                q: "nav",
                limit: 10,
                ..Query::default()
            })
            .unwrap();
        assert_eq!(found.packs[0].id, "blues");
        assert_eq!(found.skins[0].name, "Navy");
        let newest = catalog
            .search(&Query {
                sort: folderskin_catalog::Sort::Newest,
                limit: 10,
                ..Query::default()
            })
            .unwrap();
        let order: Vec<&str> = newest.packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(order, ["reds", "blues"], "dated from git");

        // The hash index.json gives, so a pack added from either is one pack.
        let reds = &catalog.packs(&["reds".into()]).unwrap()[0];
        let report = packs::check(&c.0).unwrap();
        let pack = &report.packs.iter().find(|(id, _)| id == "reds").unwrap().1;
        assert_eq!(
            reds.hash,
            packs::hash_pack(&c.0.join("packs/reds"), pack).unwrap()
        );
        let manifest =
            std::fs::read(c.out().join(tree::manifest_path("reds", &reds.hash))).unwrap();
        assert_eq!(
            reds.manifest,
            tree::sha256_hex(&manifest),
            "the catalog vouches for it"
        );
        let published = PublishedPack::parse(&manifest).unwrap();
        assert_eq!(published.skins[1].name, "Brick");
        for skin in &published.skins {
            let picture =
                std::fs::read(c.out().join(tree::picture_path(&skin.sha256, "png"))).unwrap();
            assert_eq!(tree::sha256_hex(&picture), skin.sha256);
            assert_eq!(
                (skin.w, skin.h, skin.bytes),
                (320, 300, picture.len() as u64)
            );
            let thumb = image::open(c.out().join(tree::thumb_path(&skin.sha256))).unwrap();
            assert_eq!((thumb.width(), thumb.height()), (THUMB_SIDE, THUMB_SIDE));
        }
        let strip = image::open(c.out().join(tree::strip_path(&reds.hash))).unwrap();
        assert_eq!(
            (strip.width(), strip.height()),
            (2 * packs::PREVIEW_SIDE, packs::PREVIEW_SIDE)
        );
        assert_eq!(
            c.files().len(),
            1 + 1 + 2 + 3 + 3 + 2,
            "head, catalog, manifests, pictures, thumbs, strips"
        );
    }

    #[test]
    fn a_pack_of_drives_is_published_where_only_the_apps_that_take_drives_look() {
        let c = Community::new("drives");
        two_packs(&c);
        c.pack_of(
            PackShape::Drive,
            "plain-drives",
            "Plain drives",
            &[("d.png", "Teal", [20, 150, 150])],
        );
        let head = c.build().unwrap().head;

        // The catalog every version of the app reads lists the packs of folders alone.
        let folders = open_catalog(&c, &head);
        assert_eq!((head.packs, head.skins), (2, 3));
        assert_eq!(folders.counts().0, 2);
        assert!(folders.packs(&["plain-drives".into()]).unwrap().is_empty());

        // The one the apps that take drives read has every pack, and says which are drives.
        let with = head.with_drives.clone().expect("a catalog with the drives");
        assert_eq!(with.drives, ["plain-drives"]);
        assert_eq!((with.packs, with.skins), (3, 4));
        let gz = std::fs::read(c.out().join(&with.catalog.url)).unwrap();
        assert_eq!(tree::sha256_hex(&gz), with.catalog.sha256);
        let bytes = tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED).unwrap();
        assert_eq!(tree::sha256_hex(&bytes)[..16], with.generation);
        let every = Catalog::from_bytes(&bytes).unwrap();
        let row = &every.packs(&["plain-drives".into()]).unwrap()[0];

        // Its manifest is one a FolderSkin from before drives says is newer, and its thumbnails
        // are drawn on a drive: a drive stands narrower than a folder in the square.
        let manifest =
            std::fs::read(c.out().join(tree::manifest_path("plain-drives", &row.hash))).unwrap();
        let published = PublishedPack::parse(&manifest).unwrap();
        assert_eq!(published.shape, PackShape::Drive);
        assert_eq!(published.version, tree::DRIVE_MANIFEST_VERSION);
        let sha = &published.skins[0].sha256;
        assert!(!c.out().join(tree::thumb_path(sha)).exists());
        let thumb = image::open(c.out().join(tree::drive_thumb_path(sha)))
            .unwrap()
            .to_rgba8();
        assert_eq!(thumb.get_pixel(20, 128).0[3], 0, "beside the drive");
        assert_eq!(thumb.get_pixel(128, 128).0[3], 255, "its face");

        // Built again, nothing changes; and with no pack of drives, the head says nothing of them.
        assert!(c.build().unwrap().changes.is_empty());
        std::fs::remove_dir_all(c.0.join(PACKS_DIR).join("plain-drives")).unwrap();
        let head = c.build().unwrap().head;
        assert_eq!(head.with_drives, None);
        let json = std::fs::read_to_string(c.out().join(HEAD_FILE)).unwrap();
        assert!(!json.contains("with_drives"), "{json}");
    }

    #[test]
    fn the_collection_is_published_beside_the_packs_and_listed_in_both_catalogs() {
        let c = Community::new("collection");
        two_packs(&c);
        c.pack_of(
            PackShape::Drive,
            "plain-drives",
            "Plain drives",
            &[("d.png", "Teal", [20, 150, 150])],
        );
        c.collection(
            &[
                ("koi.png", "Koi", [230, 90, 20]),
                ("fox.png", "Fox", [90, 60, 20]),
            ],
            "2026-09-30",
        );
        let built = c.build().unwrap();
        let head = Head::parse(&std::fs::read(c.out().join(HEAD_FILE)).unwrap()).unwrap();
        assert_eq!(head, built.head);
        assert_eq!(
            (head.collection, head.collection_license.as_str()),
            (2, "MIT")
        );
        assert_eq!(
            (head.packs, head.skins),
            (2, 3),
            "the packs are counted as before"
        );

        // Both catalogs list the collection, in its own order, each skin with its picture.
        let with = head.with_drives.clone().unwrap();
        let gz = std::fs::read(c.out().join(&with.catalog.url)).unwrap();
        let every =
            Catalog::from_bytes(&tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED).unwrap()).unwrap();
        for catalog in [open_catalog(&c, &head), every] {
            assert_eq!(catalog.collection_count(), 2);
            let page = catalog
                .collection(&folderskin_catalog::CollectionQuery {
                    limit: 10,
                    ..Default::default()
                })
                .unwrap();
            let names: Vec<&str> = page.items.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(names, ["Koi", "Fox"], "a day's additions in their order");
            let koi = &page.items[0];
            assert_eq!((koi.w, koi.h, koi.ext.as_str()), (320, 300, "png"));
            assert_eq!(koi.tags, ["official"]);
            assert_eq!(koi.added, 1_790_726_400);
            let picture =
                std::fs::read(c.out().join(tree::picture_path(&koi.sha256, "png"))).unwrap();
            assert_eq!(tree::sha256_hex(&picture), koi.sha256);
            assert_eq!(picture.len() as u64, koi.bytes);
            let thumb = image::open(c.out().join(tree::thumb_path(&koi.sha256))).unwrap();
            assert_eq!(
                (thumb.width(), thumb.height()),
                (THUMB_SIDE, THUMB_SIDE),
                "drawn as its folder"
            );
        }
        // The website's list of the whole collection, named after its bytes, in the collection's
        // order, each skin dated by its day.
        let manifest_path = head.collection_manifest.clone();
        let bytes = std::fs::read(c.out().join(&manifest_path)).unwrap();
        assert_eq!(
            manifest_path,
            tree::collection_manifest_path(&tree::sha256_hex(&bytes)[..16])
        );
        let listed: tree::PublishedCollection = serde_json::from_slice(&bytes).unwrap();
        assert_eq!((listed.version, listed.license.as_str()), (1, "MIT"));
        let names: Vec<&str> = listed.skins.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Koi", "Fox"]);
        let koi = &listed.skins[0];
        assert_eq!(
            (koi.ext.as_str(), koi.w, koi.h, koi.added.as_str()),
            ("png", 320, 300, "2026-09-30")
        );
        assert_eq!(koi.tags, ["official"]);
        let picture = std::fs::read(c.out().join(tree::picture_path(&koi.sha256, "png"))).unwrap();
        assert_eq!(picture.len() as u64, koi.bytes);

        // Head, two catalogs, three manifests and strips, four pack pictures and thumbnails, the
        // collection's two pictures and thumbnails, and its list.
        assert_eq!(c.files().len(), 1 + 2 + 3 + 3 + 4 + 4 + 2 + 2 + 1);

        // Built again, nothing changes.
        assert!(c.build().unwrap().changes.is_empty());

        // A skin taken out of the collection keeps its files for one generation, as a pack's do.
        let fox_sha = tree::sha256_hex(&std::fs::read(c.0.join("collection/fox.png")).unwrap());
        std::fs::remove_file(c.0.join("collection/fox.png")).unwrap();
        c.collection(&[("koi.png", "Koi", [230, 90, 20])], "2026-09-30");
        let fewer = c.build().unwrap();
        assert_eq!(fewer.head.collection, 1);
        assert_ne!(fewer.head.collection_manifest, manifest_path);
        assert!(c.out().join(tree::picture_path(&fox_sha, "png")).is_file());
        assert!(c.out().join(&manifest_path).is_file(), "the list the old head names");
        c.build().unwrap();
        assert!(!c.out().join(tree::picture_path(&fox_sha, "png")).exists());
        assert!(!c.out().join(tree::thumb_path(&fox_sha)).exists());
        assert!(!c.out().join(&manifest_path).exists());
        assert!(c.out().join(&fewer.head.collection_manifest).is_file());

        // No collection at all: head.json says nothing of one.
        std::fs::remove_dir_all(c.0.join(COLLECTION_DIR)).unwrap();
        let head = c.build().unwrap().head;
        assert_eq!((head.collection, head.collection_license.as_str()), (0, ""));
        assert_eq!(head.collection_manifest, "");
        let json = std::fs::read_to_string(c.out().join(HEAD_FILE)).unwrap();
        assert!(!json.contains("collection"), "{json}");
    }

    #[test]
    fn a_collection_with_a_problem_writes_nothing() {
        let c = Community::new("collection-bad");
        two_packs(&c);
        c.collection(&[("koi.png", "Koi", [230, 90, 20])], "2026-02-30");
        let err = c.build().unwrap_err();
        assert_eq!(err, "1 problem in the collection; nothing was written");
        assert!(!c.out().exists());
    }

    #[test]
    fn building_again_changes_nothing_and_a_change_keeps_the_last_generation_for_a_while() {
        let c = Community::new("again");
        two_packs(&c);
        let first = c.build().unwrap();
        let first_files = c.files();
        let again = c.build().unwrap();
        assert!(again.changes.is_empty(), "rewrote {:?}", again.changes);
        assert_eq!(again.head, first.head);

        // Blues gets a new picture: a new generation, while the old one's files stay.
        c.pack("blues", "Blues", &[("c.png", "Navy", [20, 30, 160])]);
        let second = c.build().unwrap();
        assert_ne!(second.head.generation, first.head.generation);
        assert!(
            second.changes.removed.is_empty(),
            "{:?}",
            second.changes.removed
        );
        let second_files = c.files();
        assert!(first_files
            .iter()
            .all(|f| f == HEAD_FILE || second_files.contains(f)));

        // Once more with no change: the first generation's own files go, the second's stay.
        let third = c.build().unwrap();
        assert_eq!(third.head.generation, second.head.generation);
        let mut gone: Vec<String> = third
            .changes
            .removed
            .iter()
            .map(|p| {
                p.strip_prefix(c.out())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        gone.sort();
        let now = c.files();
        let only_first: Vec<&String> = first_files.iter().filter(|f| !now.contains(f)).collect();
        assert_eq!(gone.iter().collect::<Vec<_>>(), only_first);
        // The old catalog, and old Blues' manifest, strip, picture and thumbnail.
        assert_eq!(gone.len(), 5, "{gone:?}");
        assert!(gone.iter().any(|f| f.starts_with("catalog/")), "{gone:?}");
        assert!(gone.iter().any(|f| f.starts_with("pictures/")), "{gone:?}");
        assert!(gone.iter().any(|f| f.starts_with("thumbs/")), "{gone:?}");
        assert!(gone.iter().any(|f| f.starts_with("strips/")), "{gone:?}");
        assert!(
            gone.iter().any(|f| f.starts_with("packs/blues/")),
            "{gone:?}"
        );
        let head = Head::parse(&std::fs::read(c.out().join(HEAD_FILE)).unwrap()).unwrap();
        open_catalog(&c, &head);
    }

    #[test]
    fn a_published_name_keeps_its_bytes_and_head_json_describes_them() {
        let c = Community::new("same-name");
        two_packs(&c);
        let first = c.build().unwrap();
        let catalog = c.out().join(&first.head.catalog.url);
        let reds_hash = open_catalog(&c, &first.head)
            .packs(&["reds".into()])
            .unwrap()[0]
            .hash
            .clone();

        // The same catalog gzipped otherwise, as a newer flate2 might: another OS byte in the
        // header, the same database inside. It stays, and head.json describes it.
        let mut other_gzip = std::fs::read(&catalog).unwrap();
        other_gzip[9] ^= 1;
        std::fs::write(&catalog, &other_gzip).unwrap();
        let again = c.build().unwrap();
        assert_eq!(again.head.generation, first.head.generation);
        assert_eq!(
            std::fs::read(&catalog).unwrap(),
            other_gzip,
            "left as published"
        );
        assert_eq!(again.head.catalog.sha256, tree::sha256_hex(&other_gzip));
        let written: Vec<_> = again.changes.written.iter().collect();
        assert_eq!(written, [&c.out().join(HEAD_FILE)], "only head.json");
        open_catalog(&c, &again.head);

        // A manifest that says the same in other bytes stays too, and the catalog vouches for
        // it as it is (a new generation, since what it vouches for changed).
        let reds = c.out().join(tree::manifest_path("reds", &reds_hash));
        let parsed = PublishedPack::parse(&std::fs::read(&reds).unwrap()).unwrap();
        let compact = serde_json::to_vec(&parsed).unwrap();
        std::fs::write(&reds, &compact).unwrap();
        let vouched = c.build().unwrap();
        assert_eq!(std::fs::read(&reds).unwrap(), compact, "left as published");
        let listed = open_catalog(&c, &vouched.head)
            .packs(&["reds".into()])
            .unwrap();
        assert_eq!(listed[0].manifest, tree::sha256_hex(&compact));

        // A damaged file under a name is replaced: it was never right.
        let catalog = c.out().join(&vouched.head.catalog.url);
        std::fs::write(&catalog, b"not a catalog").unwrap();
        let mended = c.build().unwrap();
        assert_eq!(mended.head.generation, vouched.head.generation);
        open_catalog(&c, &mended.head);
    }

    #[test]
    fn featured_packs_have_to_be_packs() {
        let c = Community::new("featured");
        two_packs(&c);
        std::fs::write(c.0.join(FEATURED_FILE), r#"["reds", "blues", "reds"]"#).unwrap();
        assert_eq!(c.build().unwrap().head.featured, ["reds", "blues"]);
        std::fs::write(c.0.join(FEATURED_FILE), r#"["gone"]"#).unwrap();
        assert!(c
            .build()
            .unwrap_err()
            .contains("\"gone\", which isn't a pack"));
        std::fs::write(c.0.join(FEATURED_FILE), r#"{"reds": 1}"#).unwrap();
        assert!(c.build().unwrap_err().contains("a list of pack ids"));
    }

    #[test]
    fn official_packs_go_in_head_json_and_have_to_be_packs() {
        let c = Community::new("official");
        two_packs(&c);
        let official = c.0.join(packs::OFFICIAL_FILE);
        assert!(
            c.build().unwrap().head.official.is_empty(),
            "no official.json"
        );

        std::fs::write(&official, r#"["blues", "reds", "blues"]"#).unwrap();
        let built = c.build().unwrap();
        assert_eq!(
            built.head.official,
            ["blues", "reds"],
            "in order, once each"
        );
        let head = Head::parse(&std::fs::read(c.out().join(HEAD_FILE)).unwrap()).unwrap();
        assert_eq!(head.official_ids(), ["blues", "reds"]);
        assert_eq!(
            built.changes.written,
            [c.out().join(HEAD_FILE)],
            "only head.json changes: the catalog doesn't say which packs are official"
        );

        // A bad official.json writes nothing, not even into an empty folder.
        let fresh = Community::new("official-bad");
        two_packs(&fresh);
        for (content, says) in [
            (r#"["reds", "gone"]"#, "names \"gone\", which isn't a pack"),
            (r#"["reds", 1]"#, "has to be a list of pack ids"),
        ] {
            std::fs::write(fresh.0.join(packs::OFFICIAL_FILE), content).unwrap();
            let err = fresh.build().unwrap_err();
            assert!(err.contains(says), "{err}");
            assert!(!fresh.out().exists());
        }
    }

    #[test]
    fn the_tree_is_never_written_over_the_packs() {
        let c = Community::new("refuse");
        two_packs(&c);
        let report = packs::check(&c.0).unwrap();
        for out in [c.0.clone(), c.0.join(".")] {
            let opts = CatalogOptions {
                out,
                ..CatalogOptions::default()
            };
            let err = write_catalog(&c.0, &report, &opts).unwrap_err();
            assert!(err.contains("packs folder itself"), "{err}");
        }
        let opts = CatalogOptions {
            out: c.out(),
            mirrors: vec!["http://insecure.example".into()],
            ..CatalogOptions::default()
        };
        assert!(write_catalog(&c.0, &report, &opts)
            .unwrap_err()
            .contains("https://"));
        c.pack("broken", "Broken", &[]);
        let report = packs::check(&c.0).unwrap();
        let opts = CatalogOptions {
            out: c.out(),
            ..CatalogOptions::default()
        };
        assert!(write_catalog(&c.0, &report, &opts)
            .unwrap_err()
            .contains("nothing was written"));
        assert!(!c.out().exists());
    }

    #[test]
    fn a_pack_is_dated_by_the_commit_that_added_it() {
        let log = [
            "\x00300",
            "",
            "packs/reds/pack.json",
            "\x00200",
            "",
            "packs\\blues\\pack.json",
            "\x00100",
            "",
            "packs/reds/pack.json",
            "other.txt",
        ]
        .join("\n");
        let dates = parse_git_dates(&log);
        assert_eq!(dates.get("reds"), Some(&100), "the first time it was added");
        assert_eq!(dates.get("blues"), Some(&200));
        assert_eq!(dates.len(), 2);
    }

    #[test]
    fn a_pack_that_moved_keeps_the_date_of_its_first_id() {
        let dates = HashMap::from([
            ("reds".to_string(), 100),
            ("reds-k7q2mx".to_string(), 300),
            ("rubies".to_string(), 50),
            ("blues".to_string(), 200),
            ("greens-a2b3c4".to_string(), 400),
        ]);
        let moved = Moved::parse(
            br#"{ "version": 1, "moved": {
                "reds": "reds-k7q2mx", "rubies": "reds-k7q2mx",
                "blues": "blues-q5r6s7", "never-added": "greens-a2b3c4" } }"#,
        )
        .unwrap();
        let dates = follow_moves(dates, &moved);
        assert_eq!(dates["reds-k7q2mx"], 50, "the earliest of all its ids");
        // Moved before its new folder was ever committed: dated from its old id alone.
        assert_eq!(dates["blues-q5r6s7"], 200);
        // An old id with no date of its own changes nothing.
        assert_eq!(dates["greens-a2b3c4"], 400);
    }

    #[test]
    fn head_json_carries_moved_json() {
        let c = Community::new("moved");
        two_packs(&c);
        let first = c.build().unwrap();
        assert!(first.head.moved.is_empty());
        let raw = std::fs::read_to_string(c.out().join(HEAD_FILE)).unwrap();
        assert!(!raw.contains("moved"), "left out while nothing moved");

        let moved = c.0.join(pack::MOVED_FILE);
        std::fs::write(&moved, r#"{ "version": 1, "moved": { "rubies": "reds" } }"#).unwrap();
        let built = c.build().unwrap();
        assert_eq!(built.head.current_id("rubies"), "reds");
        assert_eq!(built.head.generation, first.head.generation);
        assert_eq!(
            built.changes.written,
            [c.out().join(HEAD_FILE)],
            "only head.json: the catalog doesn't say what moved"
        );
        let head = Head::parse(&std::fs::read(c.out().join(HEAD_FILE)).unwrap()).unwrap();
        assert_eq!(head.moved, built.head.moved);

        // A moved.json that breaks its rules writes nothing.
        std::fs::write(&moved, r#"{ "version": 1, "moved": { "rubies": "gone" } }"#).unwrap();
        let err = c.build().unwrap_err();
        assert_eq!(err, "1 problem in moved.json; nothing was written");
    }
}
