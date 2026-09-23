//! `packs catalog`: the published tree the app reads, version 2 (the layout is in
//! `folderskin_catalog::tree`), built from the packs that pass `packs check`.
//!
//! Every file but `head.json` is named after what is in it, so a file that is already there is
//! already right: a second run only writes what changed, and renders a thumbnail only for a
//! picture it hasn't seen. `head.json` is written last, so a host serving the folder while it is
//! being built never names a catalog that isn't there yet. Files that neither this generation
//! nor the one before it use are removed after that: an app that read the old `head.json` a
//! moment ago can still fetch what it names.
//!
//! `index.json` and `previews/` are left as `packs index` writes them, for the versions of the
//! app that read those.

use crate::make;
use crate::packs::{self, Changes, Report, PACKS_DIR};
use folderskin_catalog::tree::{
    self, CatalogRef, Head, PublishedPack, PublishedSkin, HEAD_FILE, HEAD_VERSION, MANIFEST_VERSION,
};
use folderskin_catalog::{build, Catalog, PackRecord};
use folderskin_core::pack::{self, IndexEntry, Pack, MANIFEST_FILE};
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
            "{} is the community folder itself; write the tree into a folder of its own, such as {}",
            out.display(),
            dir.join("v2").display()
        ));
    }
    if let Some(bad) = opts.mirrors.iter().find(|m| !tree::is_mirror(m)) {
        return Err(format!(
            "{bad} can't be a mirror: it has to be an https:// address with no ? or #"
        ));
    }
    let featured = read_featured(dir, report)?;
    // Before anything is written: the generation the old head.json names.
    let previous = previous_files(out);

    let mut packs: Vec<&(String, Pack)> = report.packs.iter().collect();
    packs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut changes = Changes::default();
    let mut keep: HashSet<String> = HashSet::new();
    let mut renders: Vec<Render> = Vec::new();
    let mut records = Vec::new();
    for (id, pack) in packs {
        let folder = dir.join(PACKS_DIR).join(id);
        let (record, published) = publish_pack(id, pack, &folder, opts, out, &mut changes)
            .map_err(|e| format!("{id}: {e}"))?;
        for (skin, source) in published.skins.iter().zip(&pack.skins) {
            keep.insert(tree::picture_path(&skin.sha256, &skin.ext()));
            let thumb = tree::thumb_path(&skin.sha256);
            if !out.join(&thumb).is_file() && keep.insert(thumb.clone()) {
                renders.push(Render::Thumb {
                    source: folder.join(&source.file),
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
        records.push(record);
    }
    render_all(&renders, opts.cwebp.as_deref(), &mut changes)?;

    let bytes = build::to_bytes(&records)?;
    let generation = tree::sha256_hex(&bytes)[..16].to_string();
    let gz = tree::gzip(&bytes);
    let catalog = tree::catalog_path(&generation);
    write_new(&out.join(&catalog), &gz, &mut changes)?;
    keep.insert(catalog.clone());

    let head = Head {
        version: HEAD_VERSION,
        generation,
        packs: records.len(),
        skins: records.iter().map(|r| r.count).sum(),
        catalog: CatalogRef {
            url: catalog,
            sha256: tree::sha256_hex(&gz),
            bytes: gz.len() as u64,
        },
        featured,
        mirrors: opts.mirrors.clone(),
    };
    let json = serde_json::to_string_pretty(&head).map_err(|e| e.to_string())? + "\n";
    write_changed(&out.join(HEAD_FILE), json.as_bytes(), &mut changes)?;

    keep.extend(previous);
    prune(out, &keep, &mut changes)?;
    Ok(Built { head, changes })
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
    // One pack at a time is in memory: at most fifty pictures of 2 MB.
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
        version: MANIFEST_VERSION,
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
    write_changed(
        &out.join(tree::manifest_path(id, &hash)),
        json.as_bytes(),
        changes,
    )?;

    let entry = IndexEntry::new(id, pack, hash.clone());
    let record = PackRecord {
        id: entry.id,
        name: entry.name,
        author: entry.author,
        license: entry.license,
        tags: entry.tags,
        hash,
        manifest: tree::sha256_hex(json.as_bytes()),
        added: opts.dates.get(id).copied().unwrap_or(0),
        count: entry.count,
        bytes: bytes_total,
        skin_names: published.skins.iter().map(|s| s.name.clone()).collect(),
    };
    Ok((record, published))
}

/// A picture to draw for the tree.
enum Render {
    /// One skin as its folder, [`THUMB_SIDE`] px.
    Thumb { source: PathBuf, to: PathBuf },
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
        Render::Thumb { source, to } => {
            let name = source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let bytes =
                std::fs::read(source).map_err(|e| format!("{name} couldn't be read: {e}"))?;
            let rgba = pack::decode_picture(&bytes).map_err(|e| format!("{name} {e}"))?;
            let icon = packs::render_skin(rgba, THUMB_SIDE).map_err(|e| format!("{name} {e}"))?;
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

/// The featured packs from `<dir>/featured.json`, or none when there is no such file. Every id
/// in it has to be a pack that passed, so a pack that is renamed or removed can't leave a gap.
fn read_featured(dir: &Path, report: &Report) -> Result<Vec<String>, String> {
    let path = dir.join(FEATURED_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("couldn't read {}: {e}", path.display())),
    };
    let ids: Vec<String> = serde_json::from_slice(&bytes).map_err(|_| {
        format!(
            "{} has to be a list of pack ids, such as [\"classic-art\", \"colours\"]",
            path.display()
        )
    })?;
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        if !report.packs.iter().any(|(p, _)| *p == id) {
            return Err(format!(
                "{} names {id:?}, which isn't a pack",
                path.display()
            ));
        }
        if !out.contains(&id) {
            out.push(id);
        }
    }
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
    let catalog = std::fs::read(out.join(&head.catalog.url))
        .ok()
        .and_then(|gz| tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED).ok())
        .and_then(|bytes| Catalog::from_bytes(&bytes).ok());
    let Some(versions) = catalog.and_then(|c| c.versions().ok()) else {
        return files;
    };
    files.insert(head.catalog.url.clone());
    for (id, hash) in versions {
        let manifest = tree::manifest_path(&id, &hash);
        if let Some(published) = std::fs::read(out.join(&manifest))
            .ok()
            .and_then(|bytes| PublishedPack::parse(&bytes).ok())
        {
            for skin in &published.skins {
                files.insert(tree::picture_path(&skin.sha256, &skin.ext()));
                files.insert(tree::thumb_path(&skin.sha256));
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
        ("pictures", |f| hex_named(f, 64, pack::PICTURE_EXTENSIONS)),
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
pub fn git_dates(dir: &Path) -> HashMap<String, i64> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "log",
            "--format=%x00%ct",
            "--name-only",
            "--relative",
            "--diff-filter=A",
            "--",
            "packs/*/pack.json",
        ])
        .output();
    let Some(output) = output.ok().filter(|o| o.status.success()) else {
        return HashMap::new();
    };
    parse_git_dates(&String::from_utf8_lossy(&output.stdout))
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
            let folder = self.0.join(PACKS_DIR).join(id);
            std::fs::create_dir_all(&folder).unwrap();
            let listed: Vec<String> = skins
                .iter()
                .map(|(file, name, _)| format!(r#"{{ "file": "{file}", "name": "{name}" }}"#))
                .collect();
            let json = format!(
                r#"{{ "version": 1, "name": "{name}", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["test"], "skins": [{}] }}"#,
                listed.join(", ")
            );
            std::fs::write(folder.join(MANIFEST_FILE), json).unwrap();
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
            assert!(err.contains("community folder itself"), "{err}");
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
}
