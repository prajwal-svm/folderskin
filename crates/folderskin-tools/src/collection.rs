//! The official collection in a folderskin-community checkout: `collection check` and
//! `collection add`.
//!
//! `check` holds `collection/` to the contract in [`folderskin_core::collection`], and to what only
//! a folder on disk can get wrong: a picture that is missing or named with different capitals, one
//! that isn't a lossless picture within a pack picture's limits, two that are the same picture, and
//! files the list doesn't name. `packs check` runs it as well whenever there is a collection, so CI
//! that checks the packs checks the collection too, and `packs catalog` publishes only a
//! collection that passes.
//!
//! `add` makes pictures ready exactly as `packs make` makes a pack's ([`make::prepare_pictures`]):
//! finished folders cut out and given one shape, everything shrunk to 1024 px and saved as lossless
//! WebP. Each is saved under its file's name, made file-name-safe, and listed at the end of
//! `collection.json`, dated today. A picture the collection has already is skipped. The pictures
//! and the new list are written beside the collection first and moved in once all of them are
//! there, then the whole collection is checked again; when anything fails, `collection/` is left
//! exactly as it was.

use crate::make::{self, Made, Preparing};
use crate::{packs, parallel};
use folderskin_catalog::tree;
use folderskin_core::collection::{
    self, Collection, CollectionSkin, COLLECTION_DIR, COLLECTION_FILE,
};
use folderskin_core::matte;
use folderskin_core::pack::{self, PackShape, MAX_PICTURE_BYTES, MAX_SKIN_TAGS};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A skin of the collection, with what its picture is.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedSkin {
    pub skin: CollectionSkin,
    /// SHA-256 of its picture, in hex.
    pub sha256: String,
    pub bytes: u64,
    pub w: u32,
    pub h: u32,
}

/// What [`check`] found in a community folder.
#[derive(Debug, Default)]
pub struct Checked {
    /// The licence of every skin in it; empty when there's a problem, or no collection.
    pub license: String,
    /// Every skin, in the list's order; empty when there's a problem, or no collection.
    pub skins: Vec<CheckedSkin>,
    /// Every problem as `<dir>/collection: <problem>`.
    pub problems: Vec<String>,
}

impl Checked {
    /// "587 skins in the collection, all good", or "3 problems in the collection".
    pub fn summary(&self) -> String {
        if self.problems.is_empty() {
            format!(
                "{} in the collection, all good",
                packs::count(self.skins.len(), "skin")
            )
        } else {
            format!(
                "{} in the collection",
                packs::count(self.problems.len(), "problem")
            )
        }
    }
}

/// Checks `<dir>/collection` the way the app takes it: `collection.json`, every picture it lists,
/// that no two are the same picture, and that nothing else is in the folder. Dotfiles such as
/// `.DS_Store` are ignored. No `collection/` at all is an empty collection, and fine.
pub fn check(dir: &Path) -> Checked {
    let folder = dir.join(COLLECTION_DIR);
    let label = folder.display().to_string();
    let at = |problems: Vec<String>| -> Vec<String> {
        problems
            .into_iter()
            .map(|p| format!("{label}: {p}"))
            .collect()
    };
    match std::fs::symlink_metadata(&folder) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Checked::default(),
        Err(e) => return failed(at(vec![format!("couldn't be read: {e}")])),
        Ok(meta) if !meta.is_dir() => {
            return failed(at(vec![
                "isn't a folder; the collection is a folder beside packs/".into(),
            ]))
        }
        Ok(_) => {}
    }
    let files = match packs::list(&folder) {
        Ok(files) => files,
        Err(e) => return failed(at(vec![format!("couldn't be read: {e}")])),
    };
    let mut problems = Vec::new();
    let (license, skins) = read_listing(&folder, &files, &mut problems).unwrap_or_default();

    // Each picture read, checked and hashed on every core at once: a collection has hundreds.
    let pictures = parallel::map(&skins, |skin| {
        // A name the list's own check turned down is reported already, and could point outside
        // the folder.
        collection::is_collection_file_name(&skin.file)
            .then(|| check_picture(&folder, &files, &skin.file))
    });
    let mut checked = Vec::with_capacity(skins.len());
    let mut by_picture: HashMap<String, &str> = HashMap::new();
    for (skin, picture) in skins.iter().zip(pictures) {
        match picture {
            Some(Ok((sha256, bytes, (w, h)))) => {
                match by_picture.get(&sha256) {
                    Some(first) => problems.push(format!(
                        "{} is the same picture as {first}; the collection has each picture once",
                        skin.file
                    )),
                    None => {
                        by_picture.insert(sha256.clone(), &skin.file);
                    }
                }
                checked.push(CheckedSkin {
                    skin: skin.clone(),
                    sha256,
                    bytes,
                    w,
                    h,
                });
            }
            Some(Err(e)) => problems.push(e),
            None => {}
        }
    }

    let listed: Vec<&str> = skins
        .iter()
        .map(|s| s.file.as_str())
        .chain([COLLECTION_FILE])
        .collect();
    for (file, kind) in &files {
        // A file whose name differs from a missing listed one only in case was reported with it.
        let misnamed = || {
            listed
                .iter()
                .any(|l| l.eq_ignore_ascii_case(file) && !files.contains_key(*l))
        };
        if !listed.contains(&file.as_str()) && !misnamed() {
            let slash = if kind.is_dir() { "/" } else { "" };
            problems.push(format!(
                "{file}{slash} isn't listed in {COLLECTION_FILE}; the collection holds only \
                 {COLLECTION_FILE} and the pictures it lists"
            ));
        }
    }
    if problems.is_empty() {
        Checked {
            license,
            skins: checked,
            problems,
        }
    } else {
        failed(at(problems))
    }
}

fn failed(problems: Vec<String>) -> Checked {
    Checked {
        license: String::new(),
        skins: Vec::new(),
        problems,
    }
}

/// Reads and checks `collection.json`, adding what is wrong to `problems`, and returns its licence
/// and its skins. When only a field rule fails it still returns the skins, so the pictures are
/// checked in the same run; `None` when there is nothing to go by.
fn read_listing(
    folder: &Path,
    files: &packs::Files,
    problems: &mut Vec<String>,
) -> Option<(String, Vec<CollectionSkin>)> {
    match files.get(COLLECTION_FILE) {
        None => {
            problems.push(packs::missing(files, COLLECTION_FILE));
            return None;
        }
        Some(kind) if !kind.is_file() => {
            problems.push(packs::not_a_file(COLLECTION_FILE));
            return None;
        }
        Some(_) => {}
    }
    let bytes = match std::fs::read(folder.join(COLLECTION_FILE)) {
        Ok(bytes) => bytes,
        Err(e) => {
            problems.push(format!("{COLLECTION_FILE} couldn't be read: {e}"));
            return None;
        }
    };
    match Collection::parse(&bytes) {
        Ok(listing) => Some((listing.license, listing.skins)),
        Err(found) => {
            problems.extend(found);
            // The skins, whatever else is wrong, a missing licence included.
            #[derive(serde::Deserialize)]
            struct Skins {
                skins: Vec<CollectionSkin>,
            }
            let listed: Skins = serde_json::from_slice(&bytes).ok()?;
            Some((String::new(), listed.skins))
        }
    }
}

/// Checks one picture the collection lists, as a picture going into a pack now is checked
/// ([`pack::check_new_picture`]), and returns its SHA-256, size and dimensions. The error names
/// the file.
fn check_picture(
    folder: &Path,
    files: &packs::Files,
    file: &str,
) -> Result<(String, u64, (u32, u32)), String> {
    match files.get(file) {
        None => return Err(packs::missing(files, file)),
        Some(kind) if !kind.is_file() => return Err(packs::not_a_file(file)),
        Some(_) => {}
    }
    let bytes = packs::read_bytes(&folder.join(file), MAX_PICTURE_BYTES)
        .map_err(|e| format!("{file} {e}"))?;
    let dimensions = pack::check_new_picture(&bytes).map_err(|e| format!("{file} {e}"))?;
    let rgba = pack::decode_picture(&bytes).map_err(|e| format!("{file} {e}"))?;
    if matte::alpha_bounds(&rgba, 8).is_none() {
        return Err(format!("{file} is completely transparent"));
    }
    Ok((tree::sha256_hex(&bytes), bytes.len() as u64, dimensions))
}

/// Everything [`add`] needs besides the pictures.
#[derive(Debug, Clone)]
pub struct AddOptions {
    /// The community folder, holding `collection/` (made if it isn't there).
    pub dir: PathBuf,
    /// Tags every picture added gets: three at most, as any one skin of a pack adds.
    pub tags: Vec<String>,
    /// The largest a picture may be once compressed: [`MAX_PICTURE_BYTES`] or less.
    pub max_bytes: usize,
    /// Also cut away a flat backdrop of any colour, not only magenta.
    pub flat_backdrop: bool,
    /// Keep a finished folder too far off the others' shape as it is, instead of leaving it out.
    pub keep_outliers: bool,
    /// The licence a new collection gets, [`collection::DEFAULT_LICENSE`] when `None`. A collection
    /// that has one keeps it, and one asked for that isn't it is refused.
    pub license: Option<String>,
}

/// A picture [`add`] didn't add because the collection has it already.
#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    pub source: PathBuf,
    /// The collection's file with the same picture, which may be one added in the same run.
    pub same_as: String,
}

impl Skipped {
    /// "skipped: in/koi.png, the same picture as koi.webp in the collection".
    pub fn describe(&self) -> String {
        format!(
            "skipped: {}, the same picture as {} in the collection",
            self.source.display(),
            self.same_as
        )
    }
}

/// What [`add`] did.
#[derive(Debug, Clone)]
pub struct Added {
    /// `<dir>/collection`.
    pub folder: PathBuf,
    /// The skins added, in the order given.
    pub made: Vec<Made>,
    /// The pictures the collection had already.
    pub skipped: Vec<Skipped>,
    /// The shape the finished folders were given, when two or more were added.
    pub shape: Option<f32>,
    /// The pictures left out for being too far off that shape.
    pub left_out: Vec<make::LeftOut>,
    /// How many skins the collection has now.
    pub total: usize,
}

/// Adds `pictures` (files, or folders whose pictures are taken in name order) to the end of the
/// collection in `opts.dir`, dated today (UTC), and returns what went in. The collection has to
/// pass [`check`] first, and passes it again after, or it is left as it was.
pub fn add(pictures: &[PathBuf], opts: &AddOptions) -> Result<Added, String> {
    let tags = pack::clean_tags(&opts.tags, usize::MAX);
    if tags.len() > MAX_SKIN_TAGS {
        return Err(format!(
            "that's {} tags; a skin in the collection has {MAX_SKIN_TAGS} at most",
            tags.len()
        ));
    }
    let folder = opts.dir.join(COLLECTION_DIR);
    let before = check(&opts.dir);
    if !before.problems.is_empty() {
        return Err(format!(
            "{}, which has to be right before anything is added to it:\n{}",
            before.summary(),
            before.problems.join("\n")
        ));
    }
    // A new collection gets the licence asked for; one there is keeps its own.
    let license = match (before.license.as_str(), opts.license.as_deref()) {
        ("", asked) => asked.unwrap_or(collection::DEFAULT_LICENSE).to_string(),
        (have, Some(asked)) if asked != have => {
            return Err(format!(
                "the collection's licence is {have}, and every skin added to it has that one; \
                 change \"license\" in {COLLECTION_FILE} to give them all another"
            ))
        }
        (have, _) => have.to_string(),
    };
    if !pack::LICENSES.contains(&license.as_str()) {
        return Err(format!(
            "the licence has to be one of {}, not {license}",
            pack::LICENSES.join(", ")
        ));
    }
    let sources = make::collect(pictures)?;
    let prepared = make::prepare_pictures(
        &sources,
        &Preparing {
            max_bytes: opts.max_bytes.min(MAX_PICTURE_BYTES),
            flat_backdrop: opts.flat_backdrop,
            keep_outliers: opts.keep_outliers,
            shape: PackShape::Folder,
        },
    )?;

    // Every file's name and picture the collection has, to name the new ones apart from them and
    // skip the pictures it has already. Names are compared in lower case, since macOS and Windows
    // take two names a capital apart for one file.
    let mut taken: HashSet<String> = before
        .skins
        .iter()
        .map(|s| stem_of(&s.skin.file).to_ascii_lowercase())
        .collect();
    let mut have: HashMap<String, String> = before
        .skins
        .iter()
        .map(|s| (s.sha256.clone(), s.skin.file.clone()))
        .collect();
    let added_on = collection::date_of(now());
    let mut listing = Collection {
        version: collection::COLLECTION_VERSION,
        license,
        skins: before.skins.iter().map(|s| s.skin.clone()).collect(),
    };
    let mut made = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut skipped = Vec::new();
    for ready in prepared.ready {
        let sha256 = tree::sha256_hex(&ready.bytes);
        if let Some(file) = have.get(&sha256) {
            skipped.push(Skipped {
                source: ready.source,
                same_as: file.clone(),
            });
            continue;
        }
        let stem = make::file_stem(&ready.source);
        let file = format!("{}.webp", free_stem(&stem, &taken));
        taken.insert(stem_of(&file).to_ascii_lowercase());
        have.insert(sha256, file.clone());
        let name = make::display_name(&stem, listing.skins.len());
        listing.skins.push(CollectionSkin {
            file: file.clone(),
            name: name.clone(),
            tags: tags.clone(),
            added: added_on.clone(),
        });
        made.push(Made {
            source: ready.source,
            file: file.clone(),
            name,
            folder: ready.folder,
            bytes: ready.bytes.len(),
            scaled_to: ready.scaled_to,
            redrawn: ready.redrawn,
            outlier: ready.outlier,
        });
        files.push((file, ready.bytes));
    }
    let added = Added {
        folder: folder.clone(),
        made,
        skipped,
        shape: prepared.shape,
        left_out: prepared.left_out,
        total: listing.skins.len(),
    };
    if files.is_empty() {
        return Ok(added);
    }
    let problems = listing.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let json = serde_json::to_string_pretty(&listing).map_err(|e| e.to_string())? + "\n";
    put_in(&opts.dir, &folder, &files, json.as_bytes())?;
    Ok(added)
}

/// Moves the new `files` and `json`, the new `collection.json`, into `folder`, then checks the
/// whole collection again. When anything fails, the collection is put back as it was: the new
/// files go, the old list comes back, and a folder that wasn't there before is gone again.
fn put_in(
    dir: &Path,
    folder: &Path,
    files: &[(String, Vec<u8>)],
    json: &[u8],
) -> Result<(), String> {
    let existed = folder.is_dir();
    let listing = folder.join(COLLECTION_FILE);
    let old_json = std::fs::read(&listing).ok();
    // Beside the pictures, in a folder whose leading dot every check passes over, so the moves
    // below stay on one disk.
    let staging = folder.join(format!(".add-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let mut placed: Vec<PathBuf> = Vec::new();
    let result = (|| -> Result<(), String> {
        let write = |path: &Path, bytes: &[u8]| {
            std::fs::write(path, bytes)
                .map_err(|e| format!("couldn't write {}: {e}", path.display()))
        };
        std::fs::create_dir_all(&staging)
            .map_err(|e| format!("couldn't create {}: {e}", staging.display()))?;
        for (file, bytes) in files {
            write(&staging.join(file), bytes)?;
        }
        write(&staging.join(COLLECTION_FILE), json)?;
        for (file, _) in files {
            let to = folder.join(file);
            if to.exists() {
                return Err(format!("{} is there already", to.display()));
            }
            std::fs::rename(staging.join(file), &to)
                .map_err(|e| format!("couldn't write {}: {e}", to.display()))?;
            placed.push(to);
        }
        // The list last, in one step, so the collection never lists a picture that isn't there.
        std::fs::rename(staging.join(COLLECTION_FILE), &listing)
            .map_err(|e| format!("couldn't write {}: {e}", listing.display()))?;
        let after = check(dir);
        if !after.problems.is_empty() {
            return Err(format!(
                "{} once the pictures were in, so nothing was added:\n{}",
                after.summary(),
                after.problems.join("\n")
            ));
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    if result.is_err() {
        for path in &placed {
            let _ = std::fs::remove_file(path);
        }
        match &old_json {
            Some(bytes) => {
                let _ = std::fs::write(&listing, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&listing);
            }
        }
        if !existed {
            let _ = std::fs::remove_dir(folder);
        }
    }
    result
}

/// A file's name without its extension.
fn stem_of(file: &str) -> &str {
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

/// A file name stem for a picture called `stem`: its name made file-name-safe ([`pack::slug`]),
/// or `skin` when nothing of it is, with `-2`, `-3` and so on after it while that is `taken`
/// (stems in lower case).
fn free_stem(stem: &str, taken: &HashSet<String>) -> String {
    let base = Some(pack::slug(stem))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "skin".into());
    let mut candidate = base.clone();
    let mut i = 2;
    while taken.contains(&candidate) {
        candidate = format!("{base}-{i}");
        i += 1;
    }
    candidate
}

/// Now, in Unix seconds.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_core::raster;
    use image::{Rgba, RgbaImage};

    /// A community folder in the system temp folder, removed when the test ends.
    struct Community(PathBuf);

    impl Community {
        fn new(test: &str) -> Community {
            let dir = std::env::temp_dir().join(format!(
                "folderskin-collection-{test}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("in")).unwrap();
            Community(dir)
        }

        fn folder(&self) -> PathBuf {
            self.0.join(COLLECTION_DIR)
        }

        /// Writes `collection/<file>`.
        fn put(&self, file: &str, bytes: &[u8]) {
            std::fs::create_dir_all(self.folder()).unwrap();
            std::fs::write(self.folder().join(file), bytes).unwrap();
        }

        /// Writes `collection.json` listing each (file, name) and each picture.
        fn collection(&self, skins: &[(&str, &str, &RgbaImage)]) {
            let listed: Vec<String> = skins
                .iter()
                .map(|(file, name, _)| {
                    format!(r#"{{ "file": "{file}", "name": "{name}", "added": "2026-09-30" }}"#)
                })
                .collect();
            let json = format!(
                r#"{{ "version": 1, "license": "MIT", "skins": [{}] }}"#,
                listed.join(", ")
            );
            self.put(COLLECTION_FILE, json.as_bytes());
            for (file, _, picture) in skins {
                self.put(file, &raster::encode_png(picture));
            }
        }

        /// Saves `img` as `in/<file>`, to add.
        fn picture(&self, file: &str, img: &RgbaImage) -> PathBuf {
            let path = self.0.join("in").join(file);
            img.save(&path).unwrap();
            path
        }

        fn options(&self) -> AddOptions {
            AddOptions {
                dir: self.0.clone(),
                tags: vec!["Pop Art".into()],
                max_bytes: MAX_PICTURE_BYTES,
                flat_backdrop: false,
                keep_outliers: false,
                license: None,
            }
        }

        /// Every name in `collection/`, dotfiles too, sorted.
        fn files(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.folder())
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            names
        }
    }

    impl Drop for Community {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An opaque picture, which goes on FolderSkin's folder.
    fn artwork(rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(320, 300, |x, _| {
            Rgba([rgb[0], rgb[1].wrapping_add((x / 40) as u8), rgb[2], 255])
        })
    }

    /// A folder-shaped subject on magenta, as an image model paints one.
    fn on_magenta(w: u32, rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(w + 80, 480, |x, y| {
            if (40..40 + w).contains(&x) && (40..440).contains(&y) {
                Rgba([rgb[0], rgb[1], rgb[2], 255])
            } else {
                Rgba([255, 0, 255, 255])
            }
        })
    }

    #[test]
    fn no_collection_is_an_empty_one_and_a_good_one_passes() {
        let c = Community::new("good");
        let none = check(&c.0);
        assert!(none.problems.is_empty() && none.skins.is_empty());
        assert_eq!(none.summary(), "0 skins in the collection, all good");

        let (koi, fox) = (artwork([200, 40, 40]), artwork([40, 40, 200]));
        c.collection(&[("koi.png", "Koi", &koi), ("fox.png", "Fox", &fox)]);
        c.put(".DS_Store", b"Finder's");
        let checked = check(&c.0);
        assert_eq!(checked.problems, Vec::<String>::new());
        assert_eq!(checked.summary(), "2 skins in the collection, all good");
        let koi_png = std::fs::read(c.folder().join("koi.png")).unwrap();
        assert_eq!(
            checked.skins[0],
            CheckedSkin {
                skin: CollectionSkin {
                    file: "koi.png".into(),
                    name: "Koi".into(),
                    tags: Vec::new(),
                    added: "2026-09-30".into(),
                },
                sha256: tree::sha256_hex(&koi_png),
                bytes: koi_png.len() as u64,
                w: 320,
                h: 300,
            }
        );
    }

    #[test]
    fn every_problem_in_the_collection_is_reported_at_once() {
        let c = Community::new("problems");
        let koi = artwork([200, 40, 40]);
        let listed = [
            r#"{ "file": "koi.png", "name": "Koi", "added": "2026-09-30" }"#,
            r#"{ "file": "twin.png", "name": "Twin", "added": "2026-09-30" }"#,
            r#"{ "file": "Case.png", "name": "Case", "added": "2026-09-30" }"#,
            r#"{ "file": "gone.png", "name": "Gone", "added": "2026-09-30" }"#,
            r#"{ "file": "tiny.png", "name": "Tiny", "added": "2026-09-30" }"#,
            r#"{ "file": "clear.png", "name": "Clear", "added": "2026-09-30" }"#,
            r#"{ "file": "lossy.webp", "name": "Lossy", "added": "2026-09-30" }"#,
            r#"{ "file": "late.png", "name": "Late", "added": "2026-02-30" }"#,
        ];
        let json = format!(
            r#"{{ "version": 1, "license": "GPL-3.0", "skins": [{}] }}"#,
            listed.join(", ")
        );
        c.put(COLLECTION_FILE, json.as_bytes());
        c.put("koi.png", &raster::encode_png(&koi));
        c.put("twin.png", &raster::encode_png(&koi));
        c.put("case.png", &raster::encode_png(&artwork([1, 2, 3])));
        c.put(
            "tiny.png",
            &raster::encode_png(&RgbaImage::from_pixel(200, 200, Rgba([9, 9, 9, 255]))),
        );
        c.put("clear.png", &raster::encode_png(&RgbaImage::new(300, 300)));
        c.put("lossy.webp", &raster::encode_webp_lossy(&koi, 90.0));
        c.put("late.png", &raster::encode_png(&artwork([5, 90, 5])));
        c.put("notes.txt", b"hello");
        std::fs::create_dir_all(c.folder().join("raw")).unwrap();

        let checked = check(&c.0);
        assert!(checked.skins.is_empty());
        let all = checked.problems.join("\n");
        for problem in [
            "twin.png is the same picture as koi.png",
            "Case.png is missing; the folder has case.png, and names are case-sensitive",
            "gone.png is missing",
            "tiny.png is 200×200 px, but each side needs at least 256 px",
            "clear.png is completely transparent",
            "lossy.webp isn't lossless",
            r#"skin "late.png": "added" must be a day written YYYY-MM-DD"#,
            "notes.txt isn't listed in collection.json",
            "raw/ isn't listed in collection.json",
            r#""license" must be one of CC0-1.0, CC-BY-4.0, MIT"#,
        ] {
            let line = format!("{}: {problem}", c.folder().display());
            assert!(
                checked.problems.iter().any(|p| p.starts_with(&line)),
                "no {line:?} in:\n{all}"
            );
        }
        assert!(!all.contains("case.png isn't listed"), "{all}");
        assert!(!all.contains("DS_Store"), "{all}");
        assert_eq!(checked.problems.len(), 10, "{all}");
        assert_eq!(checked.summary(), "10 problems in the collection");

        // No list at all.
        std::fs::remove_file(c.folder().join(COLLECTION_FILE)).unwrap();
        let checked = check(&c.0);
        assert!(checked.problems[0].ends_with(": collection.json is missing"));
    }

    #[test]
    fn a_collection_needs_a_licence_and_its_pictures_are_checked_without_one() {
        let c = Community::new("licence");
        c.collection(&[("koi.png", "Koi", &artwork([200, 40, 40]))]);
        assert_eq!(check(&c.0).license, "MIT");
        let json = r#"{ "version": 1, "skins": [
            { "file": "koi.png", "name": "Koi", "added": "2026-09-30" },
            { "file": "gone.png", "name": "Gone", "added": "2026-09-30" } ] }"#;
        c.put(COLLECTION_FILE, json.as_bytes());
        let checked = check(&c.0);
        assert_eq!(checked.problems.len(), 2, "{:?}", checked.problems);
        assert!(checked.problems[0].contains("missing field `license`"));
        assert!(checked.problems[1].ends_with(": gone.png is missing"));
        assert_eq!(checked.license, "");
    }

    #[test]
    fn add_prepares_names_and_dates_each_picture_and_skips_the_ones_it_has() {
        let c = Community::new("add");
        c.picture("giraffe cola.png", &artwork([200, 120, 40]));
        c.picture("glass_folder.png", &on_magenta(500, [40, 120, 220]));
        let pictures = [c.0.join("in")];
        let added = add(&pictures, &c.options()).unwrap();
        assert_eq!(added.folder, c.folder());
        assert_eq!(added.total, 2);
        let made: Vec<(&str, &str, bool)> = added
            .made
            .iter()
            .map(|m| (m.file.as_str(), m.name.as_str(), m.folder))
            .collect();
        assert_eq!(
            made,
            [
                ("giraffe-cola.webp", "Giraffe cola", false),
                ("glass-folder.webp", "Glass folder", true),
            ]
        );
        let listing =
            Collection::parse(&std::fs::read(c.folder().join(COLLECTION_FILE)).unwrap()).unwrap();
        assert_eq!(listing.license, "MIT", "a new collection's, unless asked");
        assert_eq!(listing.skins.len(), 2);
        let today = collection::date_of(now());
        assert!(listing
            .skins
            .iter()
            .all(|s| s.tags == ["pop art"] && s.added == today));
        assert_eq!(
            c.files(),
            [COLLECTION_FILE, "giraffe-cola.webp", "glass-folder.webp"],
            "nothing else is left"
        );
        let cut = image::open(c.folder().join("glass-folder.webp"))
            .unwrap()
            .to_rgba8();
        assert_eq!(cut.dimensions(), (500, 400), "cut out of the magenta");
        assert!(check(&c.0).problems.is_empty());

        // The same pictures again are skipped; a new one with a name that's taken gets -2, and
        // goes at the end.
        c.picture("Giraffe-Cola!.png", &artwork([10, 200, 90]));
        let again = add(&pictures, &c.options()).unwrap();
        let skipped: Vec<&str> = again.skipped.iter().map(|s| s.same_as.as_str()).collect();
        assert_eq!(skipped, ["giraffe-cola.webp", "glass-folder.webp"]);
        assert!(again.skipped[0]
            .describe()
            .ends_with("the same picture as giraffe-cola.webp in the collection"));
        let files: Vec<&str> = again.made.iter().map(|m| m.file.as_str()).collect();
        assert_eq!(files, ["giraffe-cola-2.webp"]);
        assert_eq!(again.total, 3);
        let listing =
            Collection::parse(&std::fs::read(c.folder().join(COLLECTION_FILE)).unwrap()).unwrap();
        let order: Vec<&str> = listing.skins.iter().map(|s| s.file.as_str()).collect();
        assert_eq!(
            order,
            [
                "giraffe-cola.webp",
                "glass-folder.webp",
                "giraffe-cola-2.webp"
            ]
        );
        assert_eq!(listing.skins[2].name, "Giraffe Cola!");

        // Its licence stays whatever else is asked for.
        let other = AddOptions {
            license: Some("CC0-1.0".into()),
            ..c.options()
        };
        assert!(add(&pictures, &other)
            .unwrap_err()
            .starts_with("the collection's licence is MIT"));
        let same = AddOptions {
            license: Some("MIT".into()),
            ..c.options()
        };
        assert!(add(&pictures, &same).is_ok());

        // Nothing new at all changes nothing.
        let before = std::fs::read(c.folder().join(COLLECTION_FILE)).unwrap();
        let none = add(&pictures, &c.options()).unwrap();
        assert!(none.made.is_empty());
        assert_eq!(none.skipped.len(), 3);
        assert_eq!(
            std::fs::read(c.folder().join(COLLECTION_FILE)).unwrap(),
            before
        );
    }

    #[test]
    fn a_failed_add_leaves_the_collection_as_it_was() {
        let c = Community::new("refuse");
        let tiny = c.picture(
            "tiny.png",
            &RgbaImage::from_pixel(200, 200, Rgba([9, 9, 9, 255])),
        );
        let err = add(&[tiny], &c.options()).unwrap_err();
        assert!(err.contains("at least 256 px"), "{err}");
        assert!(!c.folder().exists(), "no collection made for nothing");

        let four = AddOptions {
            tags: ["a", "b", "c", "d"].map(String::from).to_vec(),
            ..c.options()
        };
        let good = c.picture("good.png", &artwork([1, 200, 3]));
        assert!(add(std::slice::from_ref(&good), &four)
            .unwrap_err()
            .contains("4 tags"));
        let gpl = AddOptions {
            license: Some("GPL-3.0".into()),
            ..c.options()
        };
        assert_eq!(
            add(std::slice::from_ref(&good), &gpl).unwrap_err(),
            "the licence has to be one of CC0-1.0, CC-BY-4.0, MIT, not GPL-3.0"
        );
        let cc0 = Community::new("cc0");
        let good_too = cc0.picture("good.png", &artwork([1, 200, 3]));
        let made = AddOptions {
            license: Some("CC0-1.0".into()),
            ..cc0.options()
        };
        add(&[good_too], &made).unwrap();
        assert_eq!(
            check(&cc0.0).license,
            "CC0-1.0",
            "a new collection's, as asked"
        );

        // A collection that's wrong already gets nothing added until it's put right.
        c.collection(&[("koi.png", "Koi", &artwork([200, 40, 40]))]);
        c.put("stray.png", b"not listed");
        let before = c.files();
        let err = add(&[good], &c.options()).unwrap_err();
        assert!(
            err.starts_with("1 problem in the collection, which has to be right"),
            "{err}"
        );
        assert!(err.contains("stray.png isn't listed"), "{err}");
        assert_eq!(c.files(), before);
    }

    #[test]
    fn names_are_made_safe_and_never_taken() {
        let taken: HashSet<String> = ["koi", "koi-2", "skin"].map(String::from).into();
        assert_eq!(free_stem("Koi", &taken), "koi-3");
        assert_eq!(free_stem("Fox & Hound", &taken), "fox-hound");
        assert_eq!(free_stem("東京", &taken), "skin-2");
        assert_eq!(free_stem("con", &taken), "con-1");
        assert_eq!(stem_of("koi.png"), "koi");
    }
}
