//! The community skin packs in a folderskin-community checkout: `packs check` and `packs index`.
//!
//! `check` holds every folder in `packs/` to the contract in [`folderskin_core::pack`], and to
//! what only a folder on disk can get wrong: its name, a picture that is missing or named with
//! different capitals (GitHub serves names exactly as written, though macOS and Windows open
//! either), and files the pack does not list. It also holds `moved.json`, the ids packs had
//! before, to its rules. `index` writes what the app downloads: `index.json` and one preview
//! strip per pack. Both are pure functions of the packs (and, for the dates in `index.json`, of
//! the git history), so running `index` again changes nothing.
//!
//! Names may repeat: a hundred packs can be called "Classic Art". Only the id, the folder's
//! name, has to be unique, which is why nothing here compares names.

use crate::skin::Skin;
use folderskin_core::pack::{
    self, Index, IndexEntry, Moved, Pack, INDEX_VERSION, MANIFEST_FILE, MAX_PICTURE_BYTES,
    MOVED_FILE,
};
use folderskin_core::{matte, raster};
use image::RgbaImage;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::FileType;
use std::path::{Path, PathBuf};

/// The packs, inside the community folder.
pub const PACKS_DIR: &str = "packs";
/// The preview strips, one `<id>.png` per pack.
pub const PREVIEWS_DIR: &str = "previews";
/// The list of packs the app reads.
pub const INDEX_FILE: &str = "index.json";
/// The packs the maintainer vouches for, which the app and the website mark as official: a JSON
/// list of ids beside `packs/`. Optional.
pub const OFFICIAL_FILE: &str = "official.json";
/// How many skins a preview strip shows.
pub const PREVIEW_SKINS: usize = 4;
/// The side of each folder in a preview strip.
pub const PREVIEW_SIDE: u32 = 128;

/// What [`check`] found in a community folder.
#[derive(Debug, Default)]
pub struct Report {
    /// The packs with nothing wrong, as (id, pack), sorted by id.
    pub packs: Vec<(String, Pack)>,
    /// Every problem as `<dir>/packs/<folder>: <problem>`, grouped by folder in name order, then
    /// those in `moved.json` as `<dir>/moved.json: <problem>`.
    pub problems: Vec<String>,
    /// How many entries of `packs/` the problems are about.
    pub failed: usize,
    /// The files beside `packs/` that have problems, such as `moved.json`.
    pub failed_files: Vec<String>,
    /// `moved.json`: each id a pack had before, to the one it has now. Empty when there is no such
    /// file, or when it can't be read, which is one of the problems.
    pub moved: Moved,
}

/// How [`check_with`] holds the packs.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    /// The largest a picture may be: the pack limit, or less, such as the 400 KB `packs make`
    /// keeps to.
    pub max_bytes: usize,
    /// Turn down a pack whose id isn't a generated one ([`pack::is_generated_id`]). Off unless
    /// asked, because the packs from before generated ids have ids made from their names alone,
    /// until `packs rename` gives them new ones.
    pub require_generated_ids: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        CheckOptions {
            max_bytes: MAX_PICTURE_BYTES,
            require_generated_ids: false,
        }
    }
}

impl Report {
    /// "2 packs, 32 skins": what passed.
    pub fn totals(&self) -> String {
        let skins = self.packs.iter().map(|(_, p)| p.skins.len()).sum();
        format!(
            "{}, {}",
            count(self.packs.len(), "pack"),
            count(skins, "skin")
        )
    }

    /// "2 packs, 32 skins, all good", or "3 problems in 2 packs", or "4 problems in 2 packs and
    /// moved.json".
    pub fn summary(&self) -> String {
        if self.problems.is_empty() {
            return format!("{}, all good", self.totals());
        }
        let problems = count(self.problems.len(), "problem");
        let mut places = Vec::new();
        if self.failed > 0 {
            places.push(count(self.failed, "pack"));
        }
        places.extend(self.failed_files.iter().cloned());
        if places.is_empty() {
            problems
        } else {
            format!("{problems} in {}", places.join(" and "))
        }
    }
}

/// What [`write_index`] changed on disk.
#[derive(Debug, Default)]
pub struct Changes {
    pub written: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.written.is_empty() && self.removed.is_empty()
    }
}

/// Checks every entry of `<dir>/packs`, and `<dir>/moved.json` when there is one. Fails only when
/// the packs folder cannot be read at all, so a mistyped `--dir` is an error rather than "0 packs,
/// all good".
pub fn check(dir: &Path) -> Result<Report, String> {
    check_with(dir, &CheckOptions::default())
}

/// [`check`] with a tighter limit on each picture's size, such as the 400 KB `packs make` keeps
/// to.
pub fn check_within(dir: &Path, max_bytes: usize) -> Result<Report, String> {
    check_with(
        dir,
        &CheckOptions {
            max_bytes,
            ..CheckOptions::default()
        },
    )
}

/// [`check`], holding the packs to `opts`.
pub fn check_with(dir: &Path, opts: &CheckOptions) -> Result<Report, String> {
    let packs_dir = dir.join(PACKS_DIR);
    let entries =
        list(&packs_dir).map_err(|e| format!("couldn't read {}: {e}", packs_dir.display()))?;
    let twins = case_twins(entries.keys());
    let mut report = Report::default();
    for (name, kind) in &entries {
        let (pack, mut problems) = if kind.is_dir() {
            match check_pack_within(&packs_dir.join(name), name, opts.max_bytes) {
                Ok(pack) => (Some(pack), Vec::new()),
                Err(problems) => (None, problems),
            }
        } else {
            let problem = "isn't a folder; every pack is a folder of its own";
            (None, vec![problem.to_string()])
        };
        // Linux keeps both; macOS and Windows open either name as the other, so a checkout there
        // would mix the two packs' files in one folder.
        for twin in twins.get(name).into_iter().flatten() {
            problems.push(format!(
                "differs from {twin} only in capitals, and macOS and Windows take the two for one \
                 folder"
            ));
        }
        if opts.require_generated_ids && pack::is_pack_id(name) && !pack::is_generated_id(name) {
            problems.push(not_generated(name));
        }
        match pack {
            Some(pack) if problems.is_empty() => report.packs.push((name.clone(), pack)),
            _ => {
                let label = packs_dir.join(name);
                report.failed += 1;
                report.problems.extend(
                    problems
                        .into_iter()
                        .map(|p| format!("{}: {p}", label.display())),
                );
            }
        }
    }
    check_moved(dir, &entries, &mut report);
    Ok(report)
}

/// Reads `<dir>/moved.json` into `report.moved` and adds what is wrong with it: every old id is a
/// pack id that no folder in `packs/` has, so a pack can't take an id another pack moved away
/// from, and leads straight to a folder that is there. No file is fine: nothing has moved.
fn check_moved(dir: &Path, entries: &Files, report: &mut Report) {
    let path = dir.join(MOVED_FILE);
    let problems = match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => vec![format!("{MOVED_FILE} couldn't be read: {e}")],
        Ok(bytes) => match Moved::parse(&bytes) {
            Err(e) => vec![e],
            Ok(moved) => {
                let folders: BTreeSet<String> = entries
                    .iter()
                    .filter(|(_, kind)| kind.is_dir())
                    .map(|(name, _)| name.clone())
                    .collect();
                let problems = moved.problems(&folders);
                report.moved = moved;
                problems
            }
        },
    };
    if problems.is_empty() {
        return;
    }
    report
        .problems
        .extend(problems.iter().map(|p| at_path(&path, p)));
    report.failed_files.push(MOVED_FILE.to_string());
}

/// `<dir>/moved.json`, or an empty one when there is none. One that can't be read is an error
/// rather than empty, so nothing that relies on it hands out an old id again.
pub fn read_moved(dir: &Path) -> Result<Moved, String> {
    let path = dir.join(MOVED_FILE);
    match std::fs::read(&path) {
        Ok(bytes) => Moved::parse(&bytes).map_err(|e| at_path(&path, &e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Moved::default()),
        Err(e) => Err(format!("couldn't read {}: {e}", path.display())),
    }
}

/// A sentence about `moved.json` with the file's whole path in place of its name, which says
/// which checkout it is in.
fn at_path(path: &Path, sentence: &str) -> String {
    let label = path.display();
    match sentence.strip_prefix(MOVED_FILE) {
        Some(rest) => format!("{label}{rest}"),
        None => format!("{label}: {sentence}"),
    }
}

/// Each of `names` that differs from another only in capitals, with the others it matches.
fn case_twins<'a>(names: impl IntoIterator<Item = &'a String>) -> BTreeMap<String, Vec<String>> {
    let mut by_case: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in names {
        by_case
            .entry(name.to_lowercase())
            .or_default()
            .push(name.clone());
    }
    let mut twins = BTreeMap::new();
    for group in by_case.into_values().filter(|group| group.len() > 1) {
        for name in &group {
            let others = group.iter().filter(|n| *n != name).cloned().collect();
            twins.insert(name.clone(), others);
        }
    }
    twins
}

/// Why a pack's id has to change once ids are generated, and how to change it.
fn not_generated(id: &str) -> String {
    format!(
        "{id} isn't a generated id, a name and six random characters such as classic-art-k7q2mx; \
         `folderskin-tools packs rename {id}` gives the pack one"
    )
}

/// Checks one pack folder called `name`: the name, `pack.json`, every picture it lists, and
/// that nothing else is in it. Dotfiles such as `.DS_Store` are ignored.
pub fn check_pack(folder: &Path, name: &str) -> Result<Pack, Vec<String>> {
    check_pack_within(folder, name, MAX_PICTURE_BYTES)
}

/// [`check_pack`] with pictures of at most `max_bytes`.
pub fn check_pack_within(folder: &Path, name: &str, max_bytes: usize) -> Result<Pack, Vec<String>> {
    let mut problems = Vec::new();
    if !pack::is_pack_id(name) {
        problems.push(bad_id(name));
    }
    let files = match list(folder) {
        Ok(files) => files,
        Err(e) => {
            problems.push(format!("couldn't be read: {e}"));
            return Err(problems);
        }
    };
    let Some(listing) = read_manifest(folder, &files, &mut problems) else {
        return Err(problems);
    };
    for skin in &listing.skins {
        // A name Pack::parse turned down is reported already, and could point outside the folder.
        if pack::is_picture_file_name(&skin.file) {
            if let Err(e) = check_listed_picture(folder, &files, &skin.file, max_bytes) {
                problems.push(e);
            }
        }
    }
    let listed: Vec<&str> = listing
        .skins
        .iter()
        .map(|s| s.file.as_str())
        .chain([MANIFEST_FILE])
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
                "{file}{slash} isn't listed in {MANIFEST_FILE}; a pack holds only {MANIFEST_FILE} \
                 and the pictures it lists"
            ));
        }
    }
    if problems.is_empty() {
        Ok(listing)
    } else {
        Err(problems)
    }
}

/// Writes `<dir>/index.json` and `<dir>/previews/<id>.png` for every pack in `report`, and removes
/// the previews of packs that are gone. Each entry says when its pack was first published, from
/// `dates` (Unix seconds by id, as `catalog::git_dates` reads them; a pack missing from it isn't
/// dated), and whether `<dir>/official.json` lists it. The index carries `moved.json` as its
/// `moved`, for the website and the community service. Writes nothing when the report has a
/// problem or official.json does. A file that already holds the right bytes is left alone, so
/// [`Changes`] says what really changed.
pub fn write_index(
    dir: &Path,
    report: &Report,
    dates: &HashMap<String, i64>,
) -> Result<Changes, String> {
    if !report.problems.is_empty() {
        return Err(format!("{}; nothing was written", report.summary()));
    }
    let official = read_pack_list(dir, report, OFFICIAL_FILE)?;
    let mut packs: Vec<&(String, Pack)> = report.packs.iter().collect();
    packs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut changes = Changes::default();

    let previews = dir.join(PREVIEWS_DIR);
    std::fs::create_dir_all(&previews)
        .map_err(|e| format!("couldn't create {}: {e}", previews.display()))?;
    // Stale previews go first: on a case-insensitive disk `Old.png` and `old.png` are one file.
    let existing =
        list(&previews).map_err(|e| format!("couldn't read {}: {e}", previews.display()))?;
    for (file, kind) in existing {
        let gone = file
            .strip_suffix(".png")
            .is_some_and(|id| !packs.iter().any(|(p, _)| p == id));
        if gone && kind.is_file() {
            let path = previews.join(&file);
            std::fs::remove_file(&path)
                .map_err(|e| format!("couldn't remove {}: {e}", path.display()))?;
            changes.removed.push(path);
        }
    }
    for (id, pack) in &packs {
        let strip =
            preview_strip(&dir.join(PACKS_DIR).join(id), pack).map_err(|e| format!("{id}: {e}"))?;
        let path = previews.join(format!("{id}.png"));
        write_if_changed(&path, &raster::encode_png(&strip), &mut changes)?;
    }

    let entries = packs
        .iter()
        .map(|(id, pack)| {
            let hash =
                hash_pack(&dir.join(PACKS_DIR).join(id), pack).map_err(|e| format!("{id}: {e}"))?;
            let mut entry = IndexEntry::new(id, pack, hash);
            entry.added = dates.get(id).copied().filter(|&at| at > 0);
            entry.official = official.contains(id);
            Ok(entry)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let index = Index {
        version: INDEX_VERSION,
        packs: entries,
        moved: report.moved.moved.clone(),
    };
    let json = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())? + "\n";
    write_if_changed(&dir.join(INDEX_FILE), json.as_bytes(), &mut changes)?;
    Ok(changes)
}

/// The pack ids listed in `<dir>/<file>`, such as `featured.json` or `official.json`, in order and
/// without repeats; none when there is no such file. Every id in it has to be a pack in `report`,
/// so a pack that is renamed or removed can't leave a gap.
pub fn read_pack_list(dir: &Path, report: &Report, file: &str) -> Result<Vec<String>, String> {
    let path = dir.join(file);
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

/// [`pack::pack_hash`] of the pack in `folder`, from the files as they are on disk: the bytes
/// the app downloads.
pub fn hash_pack(folder: &Path, pack: &Pack) -> Result<String, String> {
    let read = |file: &str| {
        std::fs::read(folder.join(file)).map_err(|e| format!("{file} couldn't be read: {e}"))
    };
    let manifest = read(MANIFEST_FILE)?;
    let pictures = pack
        .skins
        .iter()
        .map(|s| read(&s.file).map(|bytes| (s.file.as_str(), bytes)))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(pack::pack_hash(
        &manifest,
        pictures
            .iter()
            .map(|(file, bytes)| (*file, bytes.as_slice())),
    ))
}

/// A pack's preview: its first [`PREVIEW_SKINS`] skins as folders side by side, each
/// [`PREVIEW_SIDE`] px square, on transparency.
pub fn preview_strip(folder: &Path, pack: &Pack) -> Result<RgbaImage, String> {
    let shown = &pack.skins[..pack.skins.len().min(PREVIEW_SKINS)];
    let mut strip = RgbaImage::new(PREVIEW_SIDE * shown.len() as u32, PREVIEW_SIDE);
    for (i, skin) in shown.iter().enumerate() {
        let rgba = read_picture(&folder.join(&skin.file), MAX_PICTURE_BYTES)
            .map_err(|e| format!("{} {e}", skin.file))?;
        let icon = render_skin(rgba, PREVIEW_SIDE).map_err(|e| format!("{} {e}", skin.file))?;
        // Copied rather than blended: the tiles never overlap, and copying keeps them exact.
        image::imageops::replace(&mut strip, &icon, i64::from(i as u32 * PREVIEW_SIDE), 0);
    }
    Ok(strip)
}

/// The contact sheet's background.
const SHEET_GREY: [u8; 3] = [235, 235, 235];

/// Every skin of a pack as the folder the app makes of it, `side` px square, `columns` to a row,
/// on light grey: a sheet to look over a pack before it ships. Not on transparency, which most
/// viewers draw black: a cut-out that went into a dark coat or dark hair would look whole there.
pub fn contact_sheet(
    folder: &Path,
    pack: &Pack,
    side: u32,
    columns: u32,
) -> Result<RgbaImage, String> {
    let count = pack.skins.len() as u32;
    let columns = columns.clamp(1, count.max(1));
    let rows = count.div_ceil(columns).max(1);
    let [r, g, b] = SHEET_GREY;
    let mut sheet = RgbaImage::from_pixel(side * columns, side * rows, image::Rgba([r, g, b, 255]));
    for (i, skin) in pack.skins.iter().enumerate() {
        let rgba = read_picture(&folder.join(&skin.file), MAX_PICTURE_BYTES)
            .map_err(|e| format!("{} {e}", skin.file))?;
        let icon = render_skin(rgba, side).map_err(|e| format!("{} {e}", skin.file))?;
        let (col, row) = (i as u32 % columns, i as u32 / columns);
        image::imageops::replace(
            &mut sheet,
            &matte::flatten(&icon, SHEET_GREY),
            i64::from(col * side),
            i64::from(row * side),
        );
    }
    Ok(sheet)
}

/// A skin as the folder the app makes of it, `size` px square: a finished folder picture (cut
/// out, or on the magenta key) as it is, and anything else as artwork on FolderSkin's template
/// ([`Skin`]). The error finishes a sentence that starts with the picture's name.
pub fn render_skin(rgba: RgbaImage, size: u32) -> Result<RgbaImage, String> {
    let png = Skin::from_picture(rgba, (0.5, 0.5))?.preview_png(size);
    image::load_from_memory(&png)
        .map(|img| img.to_rgba8())
        .map_err(|e| format!("couldn't be rendered: {e}"))
}

// ---------- helpers ----------

/// A folder's entries by name.
type Files = BTreeMap<String, FileType>;

/// Lists a folder, leaving out dotfiles such as `.DS_Store`. Links are not followed.
fn list(dir: &Path) -> std::io::Result<Files> {
    let mut files = Files::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') {
            files.insert(name, entry.file_type()?);
        }
    }
    Ok(files)
}

/// Reads and checks `pack.json`, adding what is wrong to `problems`. When only a field rule
/// fails it still returns the listing, so the pictures are checked in the same run; `None` when
/// there is nothing to go by.
fn read_manifest(folder: &Path, files: &Files, problems: &mut Vec<String>) -> Option<Pack> {
    match files.get(MANIFEST_FILE) {
        None => {
            problems.push(missing(files, MANIFEST_FILE));
            return None;
        }
        Some(kind) if !kind.is_file() => {
            problems.push(not_a_file(MANIFEST_FILE));
            return None;
        }
        Some(_) => {}
    }
    let bytes = match std::fs::read(folder.join(MANIFEST_FILE)) {
        Ok(bytes) => bytes,
        Err(e) => {
            problems.push(format!("{MANIFEST_FILE} couldn't be read: {e}"));
            return None;
        }
    };
    match Pack::parse(&bytes) {
        Ok(pack) => Some(pack),
        Err(found) => {
            problems.extend(found);
            serde_json::from_slice(&bytes).ok()
        }
    }
}

/// Checks one picture a pack lists, the way the app does before it saves it.
fn check_listed_picture(
    folder: &Path,
    files: &Files,
    file: &str,
    max_bytes: usize,
) -> Result<(), String> {
    match files.get(file) {
        None => return Err(missing(files, file)),
        Some(kind) if !kind.is_file() => return Err(not_a_file(file)),
        Some(_) => {}
    }
    let rgba = read_picture(&folder.join(file), max_bytes).map_err(|e| format!("{file} {e}"))?;
    if matte::alpha_bounds(&rgba, 8).is_none() {
        return Err(format!("{file} is completely transparent"));
    }
    Ok(())
}

/// Reads and decodes a picture within the pack limits and at most `max_bytes`. The error
/// finishes a sentence that starts with the file's name.
fn read_picture(path: &Path, max_bytes: usize) -> Result<RgbaImage, String> {
    let len = std::fs::metadata(path)
        .map_err(|e| format!("couldn't be read: {e}"))?
        .len();
    // Measured before reading, so an oversized file is never loaded.
    let most = max_bytes.min(MAX_PICTURE_BYTES);
    if len > most as u64 {
        return Err(format!(
            "is {} KB; the most is {} KB",
            len.div_ceil(1024),
            most / 1024
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("couldn't be read: {e}"))?;
    // decode_picture runs check_picture first: the real file type, then the dimensions.
    pack::decode_picture(&bytes)
}

/// Writes `bytes` to `path` unless it holds them already.
pub(crate) fn write_if_changed(
    path: &Path,
    bytes: &[u8],
    changes: &mut Changes,
) -> Result<(), String> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    std::fs::write(path, bytes).map_err(|e| format!("couldn't write {}: {e}", path.display()))?;
    changes.written.push(path.to_path_buf());
    Ok(())
}

/// Why a folder name is not a pack id, with one that would be.
fn bad_id(name: &str) -> String {
    let rule = "a pack's folder name is lower-case letters and digits in words joined by single \
                dashes, at most 40 characters";
    let id = pack::slug(name);
    if pack::is_pack_id(&id) {
        format!("rename the folder to {id}: {rule}")
    } else {
        format!("rename the folder: {rule}")
    }
}

/// "red.png is missing", pointing out a file that differs only in case.
fn missing(files: &Files, name: &str) -> String {
    match files.keys().find(|f| f.eq_ignore_ascii_case(name)) {
        Some(near) => {
            format!("{name} is missing; the folder has {near}, and names are case-sensitive")
        }
        None => format!("{name} is missing"),
    }
}

fn not_a_file(name: &str) -> String {
    format!("{name} is a link or a folder; it has to be the file itself")
}

/// "1 pack", "2 packs".
fn count(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A community folder in the system temp folder, removed when the test ends.
    struct Community(PathBuf);

    impl Community {
        fn new(test: &str) -> Community {
            let dir = std::env::temp_dir()
                .join(format!("folderskin-packs-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(PACKS_DIR)).unwrap();
            Community(dir)
        }

        /// Writes `packs/<folder>/<file>`.
        fn put(&self, folder: &str, file: &str, bytes: &[u8]) {
            let dir = self.0.join(PACKS_DIR).join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(file), bytes).unwrap();
        }

        /// Writes a pack whose `pack.json` lists each (file, picture) in order.
        fn pack(&self, folder: &str, skins: &[(&str, &RgbaImage)]) {
            let listed: Vec<String> = skins.iter().map(|(file, _)| skin(file)).collect();
            self.put(folder, MANIFEST_FILE, manifest(&listed).as_bytes());
            for (file, picture) in skins {
                self.put(folder, file, &raster::encode_png(picture));
            }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.0.join(relative)
        }
    }

    impl Drop for Community {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn skin(file: &str) -> String {
        format!(r#"{{ "file": "{file}", "name": "Skin" }}"#)
    }

    fn manifest(skins: &[String]) -> String {
        format!(
            r#"{{ "version": 1, "name": "Test", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["test"], "skins": [{}] }}"#,
            skins.join(", ")
        )
    }

    /// An opaque picture, which the app puts on its folder template.
    fn artwork(rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_pixel(320, 300, Rgba([rgb[0], rgb[1], rgb[2], 255]))
    }

    /// A finished folder: a subject on real transparency, which the app uses as it is.
    fn cutout(rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(300, 300, |x, y| {
            if (50..250).contains(&x) && (50..250).contains(&y) {
                Rgba([rgb[0], rgb[1], rgb[2], 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        })
    }

    #[test]
    fn good_packs_pass_and_are_counted() {
        let c = Community::new("good");
        c.pack(
            "reds",
            &[
                ("a.png", &artwork([200, 40, 40])),
                ("b.png", &cutout([40, 40, 200])),
            ],
        );
        c.pack("blues", &[("c.png", &artwork([40, 40, 200]))]);
        c.put("reds", ".DS_Store", b"Finder's");
        let report = check(&c.0).unwrap();
        assert_eq!(report.problems, Vec::<String>::new());
        let ids: Vec<&str> = report.packs.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["blues", "reds"]);
        assert_eq!(report.summary(), "2 packs, 3 skins, all good");
    }

    #[test]
    fn every_problem_in_every_folder_is_reported_at_once() {
        let c = Community::new("problems");
        c.pack("Bad_Name", &[("a.png", &artwork([1, 2, 3]))]);
        // A field rule fails too, and the pictures are still checked from the listing.
        let listed: Vec<String> = ["tiny.png", "clear.png", "gone.png", "Case.png", "fake.png"]
            .iter()
            .map(|f| skin(f))
            .collect();
        let json = manifest(&listed).replace("CC0-1.0", "All rights reserved");
        c.put("broken", MANIFEST_FILE, json.as_bytes());
        let tiny = RgbaImage::from_pixel(200, 200, Rgba([9, 9, 9, 255]));
        c.put("broken", "tiny.png", &raster::encode_png(&tiny));
        c.put(
            "broken",
            "clear.png",
            &raster::encode_png(&RgbaImage::new(300, 300)),
        );
        c.put(
            "broken",
            "case.png",
            &raster::encode_png(&artwork([5, 5, 5])),
        );
        c.put("broken", "fake.png", b"GIF89a, whatever the name says");
        c.put("broken", "notes.txt", b"hello");
        c.put("broken", ".DS_Store", b"Finder's");
        std::fs::create_dir_all(c.path("packs/broken/raw")).unwrap();
        std::fs::create_dir_all(c.path("packs/empty")).unwrap();
        std::fs::write(c.path("packs/stray.png"), b"not a pack").unwrap();

        let report = check(&c.0).unwrap();
        assert!(report.packs.is_empty());
        let all = report.problems.join("\n");
        for (folder, problem) in [
            ("Bad_Name", "rename the folder to bad-name: "),
            ("broken", "\"license\" must be one of "),
            (
                "broken",
                "tiny.png is 200×200 px; each side needs at least 256 px",
            ),
            ("broken", "clear.png is completely transparent"),
            ("broken", "gone.png is missing"),
            (
                "broken",
                "Case.png is missing; the folder has case.png, and names are case-sensitive",
            ),
            ("broken", "fake.png isn't a PNG, JPEG or WebP picture"),
            ("broken", "notes.txt isn't listed in pack.json"),
            ("broken", "raw/ isn't listed in pack.json"),
            ("empty", "pack.json is missing"),
            ("stray.png", "isn't a folder"),
        ] {
            let line = format!("{}: {problem}", c.path(PACKS_DIR).join(folder).display());
            assert!(
                report.problems.iter().any(|p| p.starts_with(&line)),
                "no {line:?} in:\n{all}"
            );
        }
        assert!(!all.contains("DS_Store"), "dotfiles are ignored:\n{all}");
        assert!(
            !all.contains("case.png isn't listed"),
            "a name in the wrong case is reported once:\n{all}"
        );
        assert_eq!(report.problems.len(), 11, "{all}");
        assert_eq!(report.summary(), "11 problems in 4 packs");
    }

    #[test]
    fn a_missing_packs_folder_is_an_error_not_an_empty_pass() {
        let c = Community::new("nowhere");
        std::fs::remove_dir_all(c.path(PACKS_DIR)).unwrap();
        assert!(check(&c.0).unwrap_err().contains("couldn't read"));
    }

    #[test]
    fn index_writes_the_list_and_a_strip_per_pack_then_nothing_the_second_time() {
        let c = Community::new("index");
        let greys: Vec<RgbaImage> = (1..=5).map(|i| artwork([i * 40, i * 40, i * 40])).collect();
        let names = ["1.png", "2.png", "3.png", "4.png", "5.png"];
        let five: Vec<(&str, &RgbaImage)> = names.into_iter().zip(&greys).collect();
        c.pack("zebra", &[("z.png", &artwork([10, 10, 10]))]);
        c.pack("greys", &five);
        std::fs::create_dir_all(c.path(PREVIEWS_DIR)).unwrap();
        std::fs::write(c.path("previews/gone.png"), b"an old pack").unwrap();
        std::fs::write(c.path("previews/notes.txt"), b"not a preview").unwrap();

        let report = check(&c.0).unwrap();
        let changes = write_index(&c.0, &report, &HashMap::new()).unwrap();
        assert_eq!(changes.removed, [c.path("previews/gone.png")]);
        assert_eq!(
            changes.written,
            [
                c.path("previews/greys.png"),
                c.path("previews/zebra.png"),
                c.path(INDEX_FILE)
            ]
        );
        assert!(
            c.path("previews/notes.txt").exists(),
            "only PNGs are pruned"
        );

        let json = std::fs::read_to_string(c.path(INDEX_FILE)).unwrap();
        assert!(json.starts_with("{\n  \"version\": 1,\n") && json.ends_with("}\n"));
        let index: Index = serde_json::from_str(&json).unwrap();
        assert_eq!(index.version, INDEX_VERSION);
        let ids: Vec<&str> = index.packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["greys", "zebra"]);
        assert_eq!((index.packs[0].count, index.packs[1].count), (5, 1));

        let strip = |id: &str| {
            image::open(c.path(PREVIEWS_DIR).join(format!("{id}.png")))
                .unwrap()
                .to_rgba8()
        };
        assert_eq!(
            strip("greys").dimensions(),
            (4 * PREVIEW_SIDE, PREVIEW_SIDE)
        );
        assert_eq!(strip("zebra").dimensions(), (PREVIEW_SIDE, PREVIEW_SIDE));

        let again = write_index(&c.0, &report, &HashMap::new()).unwrap();
        assert!(again.is_empty(), "a second run rewrote {again:?}");
    }

    #[test]
    fn index_says_when_each_pack_was_added_and_which_are_official() {
        let c = Community::new("official");
        c.pack("reds", &[("a.png", &artwork([200, 40, 40]))]);
        c.pack("blues", &[("b.png", &artwork([40, 40, 200]))]);
        c.pack("greens", &[("g.png", &artwork([40, 200, 40]))]);
        // Listed twice, which counts once.
        std::fs::write(c.path(OFFICIAL_FILE), r#"["reds", "blues", "reds"]"#).unwrap();
        let dates = HashMap::from([
            ("reds".to_string(), 1_790_000_000),
            ("blues".to_string(), 0),
        ]);

        let report = check(&c.0).unwrap();
        write_index(&c.0, &report, &dates).unwrap();
        let json = std::fs::read_to_string(c.path(INDEX_FILE)).unwrap();
        let index: Index = serde_json::from_str(&json).unwrap();
        let said: Vec<(&str, Option<i64>, bool)> = index
            .packs
            .iter()
            .map(|p| (p.id.as_str(), p.added, p.official))
            .collect();
        assert_eq!(
            said,
            [
                ("blues", None, true),
                ("greens", None, false),
                ("reds", Some(1_790_000_000), true)
            ],
            "a time of 0 is no date"
        );
        assert_eq!(json.matches("\"official\": true").count(), 2, "{json}");
        assert!(!json.contains("\"official\": false"), "{json}");
        assert_eq!(json.matches("\"added\"").count(), 1, "{json}");
        assert!(write_index(&c.0, &report, &dates).unwrap().is_empty());

        // Without official.json no pack is official.
        std::fs::remove_file(c.path(OFFICIAL_FILE)).unwrap();
        write_index(&c.0, &report, &dates).unwrap();
        let json = std::fs::read_to_string(c.path(INDEX_FILE)).unwrap();
        assert!(!json.contains("official"), "{json}");
    }

    #[test]
    fn a_bad_official_json_writes_nothing() {
        let c = Community::new("bad-official");
        c.pack("reds", &[("a.png", &artwork([200, 40, 40]))]);
        let report = check(&c.0).unwrap();
        for (content, says) in [
            (r#"["reds", "gone"]"#, "names \"gone\", which isn't a pack"),
            (r#"{"reds": true}"#, "has to be a list of pack ids"),
            ("reds", "has to be a list of pack ids"),
        ] {
            std::fs::write(c.path(OFFICIAL_FILE), content).unwrap();
            let err = write_index(&c.0, &report, &HashMap::new()).unwrap_err();
            assert!(
                err.starts_with(&c.path(OFFICIAL_FILE).display().to_string()) && err.contains(says),
                "{err}"
            );
            assert!(!c.path(INDEX_FILE).exists() && !c.path(PREVIEWS_DIR).exists());
        }
    }

    #[test]
    fn index_writes_nothing_when_a_pack_has_a_problem() {
        let c = Community::new("refuse");
        c.pack("good", &[("a.png", &artwork([1, 2, 3]))]);
        c.put("bad", MANIFEST_FILE, b"{}");
        let report = check(&c.0).unwrap();
        let err = write_index(&c.0, &report, &HashMap::new()).unwrap_err();
        assert_eq!(err, "1 problem in 1 pack; nothing was written");
        assert!(!c.path(INDEX_FILE).exists() && !c.path(PREVIEWS_DIR).exists());
    }

    #[test]
    fn a_strip_shows_artwork_on_the_template_and_a_finished_folder_as_it_is() {
        let c = Community::new("strip");
        c.pack(
            "mixed",
            &[
                ("art.png", &artwork([200, 40, 40])),
                ("folder.png", &cutout([40, 40, 200])),
            ],
        );
        let report = check(&c.0).unwrap();
        let strip = preview_strip(&c.path("packs/mixed"), &report.packs[0].1).unwrap();
        assert_eq!(strip.dimensions(), (2 * PREVIEW_SIDE, PREVIEW_SIDE));
        // Artwork: red on the front panel, and nothing in the corner above the tab.
        let front = strip.get_pixel(64, 75).0;
        assert!(
            front[0] > 180 && front[2] < 60 && front[3] == 255,
            "{front:?}"
        );
        assert_eq!(strip.get_pixel(2, 2).0[3], 0);
        // A finished folder: the picture itself, trimmed and filling its square, corners too.
        for (x, y) in [(2, 2), (64, 64), (125, 125)] {
            let px = strip.get_pixel(PREVIEW_SIDE + x, y).0;
            assert!(
                px[2] > 180 && px[0] < 60 && px[3] == 255,
                "({x},{y}) {px:?}"
            );
        }
    }

    #[test]
    fn a_contact_sheet_is_opaque_so_a_hole_in_a_dark_folder_shows() {
        let c = Community::new("sheet");
        // A dark finished folder with a see-through hole in the middle, as a cut-out that ran
        // into a dark coat leaves: on black it would look whole.
        let holed = RgbaImage::from_fn(300, 300, |x, y| {
            let hole = (130..170).contains(&x) && (130..170).contains(&y);
            if (50..250).contains(&x) && (50..250).contains(&y) && !hole {
                Rgba([20, 20, 20, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        c.pack(
            "sheet",
            &[("art.png", &artwork([200, 40, 40])), ("holed.png", &holed)],
        );
        let report = check(&c.0).unwrap();
        let sheet = contact_sheet(&c.path("packs/sheet"), &report.packs[0].1, 128, 6).unwrap();
        assert_eq!(sheet.dimensions(), (256, 128), "two skins make one row");
        assert!(sheet.pixels().all(|p| p.0[3] == 255));
        // Above the artwork's tab, and through the hole in the dark folder: the grey.
        let [r, g, b] = SHEET_GREY;
        assert_eq!(sheet.get_pixel(2, 2).0, [r, g, b, 255]);
        assert_eq!(sheet.get_pixel(128 + 64, 64).0, [r, g, b, 255]);
        let dark = sheet.get_pixel(128 + 20, 20).0;
        assert!(dark[0] < 40, "{dark:?}");
    }

    #[test]
    fn packs_may_share_a_name_since_only_ids_are_unique() {
        let c = Community::new("same-name");
        // Community::pack calls every pack "Test".
        c.pack("reds-k7q2mx", &[("a.png", &artwork([200, 40, 40]))]);
        c.pack("reds-a2b3c4", &[("a.png", &artwork([200, 40, 40]))]);
        c.pack("reds", &[("a.png", &artwork([200, 40, 40]))]);
        let report = check(&c.0).unwrap();
        assert_eq!(report.problems, Vec::<String>::new());
        assert_eq!(report.packs.len(), 3);
        assert!(report.packs.iter().all(|(_, p)| p.name == "Test"));
    }

    #[test]
    fn generated_ids_are_required_only_when_asked() {
        let c = Community::new("generated");
        c.pack("classic-art", &[("a.png", &artwork([200, 40, 40]))]);
        c.pack("classic-art-k7q2mx", &[("a.png", &artwork([40, 40, 200]))]);
        c.pack("Bad_Name", &[("a.png", &artwork([40, 200, 40]))]);
        let lenient = check(&c.0).unwrap();
        assert_eq!(lenient.failed, 1, "{:?}", lenient.problems);

        let strict = check_with(
            &c.0,
            &CheckOptions {
                require_generated_ids: true,
                ..CheckOptions::default()
            },
        )
        .unwrap();
        let ids: Vec<&str> = strict.packs.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["classic-art-k7q2mx"]);
        let line = format!(
            "{}: classic-art isn't a generated id",
            c.path("packs/classic-art").display()
        );
        assert!(
            strict.problems.iter().any(|p| p.starts_with(&line)),
            "{:?}",
            strict.problems
        );
        assert!(strict
            .problems
            .iter()
            .any(|p| p.contains("`folderskin-tools packs rename classic-art`")));
        // A folder name that isn't an id at all is told to change once, not twice.
        let bad: Vec<&String> = strict
            .problems
            .iter()
            .filter(|p| p.contains("Bad_Name"))
            .collect();
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert_eq!(strict.summary(), "2 problems in 2 packs");
    }

    #[test]
    fn names_one_capital_apart_are_one_folder_on_macos_and_windows() {
        let names: Vec<String> = ["Reds", "blues", "reds", "REDS"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let twins = case_twins(&names);
        assert_eq!(twins.len(), 3);
        assert_eq!(twins["reds"], ["Reds", "REDS"]);
        assert_eq!(twins["REDS"], ["Reds", "reds"]);
        assert!(!twins.contains_key("blues"));

        // On a disk that keeps both, as Linux's do, check says so about each of them.
        let c = Community::new("capitals");
        c.pack("reds", &[("a.png", &artwork([200, 40, 40]))]);
        c.pack("Reds", &[("a.png", &artwork([200, 40, 40]))]);
        if std::fs::read_dir(c.path(PACKS_DIR)).unwrap().count() < 2 {
            eprintln!("this disk doesn't tell capitals apart; skipping the folder half");
            return;
        }
        let report = check(&c.0).unwrap();
        for (folder, twin) in [("reds", "Reds"), ("Reds", "reds")] {
            let line = format!(
                "{}: differs from {twin} only in capitals",
                c.path(PACKS_DIR).join(folder).display()
            );
            assert!(
                report.problems.iter().any(|p| p.starts_with(&line)),
                "{:?}",
                report.problems
            );
        }
        assert!(report.packs.is_empty());
    }

    #[test]
    fn moved_json_is_read_and_held_to_its_rules() {
        let c = Community::new("moved");
        c.pack("reds-k7q2mx", &[("a.png", &artwork([200, 40, 40]))]);
        c.pack("blues", &[("b.png", &artwork([40, 40, 200]))]);
        let moved = c.path(MOVED_FILE);

        // Without one, nothing has moved.
        let report = check(&c.0).unwrap();
        assert!(report.problems.is_empty() && report.moved.moved.is_empty());

        std::fs::write(
            &moved,
            r#"{ "version": 1, "moved": { "reds": "reds-k7q2mx" } }"#,
        )
        .unwrap();
        let report = check(&c.0).unwrap();
        assert_eq!(report.problems, Vec::<String>::new());
        assert_eq!(report.moved.current("reds"), "reds-k7q2mx");

        // A pack with an id that moved away, an old id that leads nowhere, and one that leads to
        // another old id.
        std::fs::write(
            &moved,
            r#"{ "version": 1, "moved": { "blues": "reds-k7q2mx", "greens": "gone-a2b3c4",
                 "old-reds": "reds" , "reds": "reds-k7q2mx" } }"#,
        )
        .unwrap();
        let report = check(&c.0).unwrap();
        let label = moved.display().to_string();
        for says in [
            "blues moved to reds-k7q2mx, but a pack still has the id blues",
            "greens moved to gone-a2b3c4, and there's no pack gone-a2b3c4",
            "old-reds moved to reds, which moved again",
        ] {
            let line = format!("{label}: {says}");
            assert!(
                report.problems.iter().any(|p| p.starts_with(&line)),
                "no {line:?} in {:?}",
                report.problems
            );
        }
        assert_eq!(report.failed, 0, "the packs themselves are fine");
        assert_eq!(report.summary(), "3 problems in moved.json");

        // One it can't read at all is one problem, which names the file.
        std::fs::write(&moved, r#"{ "version": 2, "moved": {} }"#).unwrap();
        let report = check(&c.0).unwrap();
        assert_eq!(report.problems.len(), 1, "{:?}", report.problems);
        assert!(
            report.problems[0].starts_with(&format!("{label} is version 2")),
            "{:?}",
            report.problems
        );
        // Problems in moved.json stop the index as a pack's would.
        let err = write_index(&c.0, &report, &HashMap::new()).unwrap_err();
        assert_eq!(err, "1 problem in moved.json; nothing was written");

        // With a broken pack as well, the summary names both.
        c.put("broken", MANIFEST_FILE, b"{}");
        assert_eq!(
            check(&c.0).unwrap().summary(),
            "2 problems in 1 pack and moved.json"
        );
    }

    #[test]
    fn index_json_carries_moved_json() {
        let c = Community::new("index-moved");
        c.pack("reds-k7q2mx", &[("a.png", &artwork([200, 40, 40]))]);
        let report = check(&c.0).unwrap();
        write_index(&c.0, &report, &HashMap::new()).unwrap();
        let json = std::fs::read_to_string(c.path(INDEX_FILE)).unwrap();
        assert!(!json.contains("moved"), "left out while nothing moved");

        std::fs::write(
            c.path(MOVED_FILE),
            r#"{ "version": 1, "moved": { "reds": "reds-k7q2mx", "rubies": "reds-k7q2mx" } }"#,
        )
        .unwrap();
        let report = check(&c.0).unwrap();
        let changes = write_index(&c.0, &report, &HashMap::new()).unwrap();
        assert_eq!(changes.written, [c.path(INDEX_FILE)]);
        let index: Index =
            serde_json::from_str(&std::fs::read_to_string(c.path(INDEX_FILE)).unwrap()).unwrap();
        assert_eq!(index.moved.len(), 2);
        assert_eq!(index.moved["rubies"], "reds-k7q2mx");
        assert!(write_index(&c.0, &report, &HashMap::new())
            .unwrap()
            .is_empty());
    }
}
