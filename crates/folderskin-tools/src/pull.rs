//! `community pull`: approved packs from the community service, written into community/packs as
//! ordinary pack folders.
//!
//! A pack shared from the app arrives here the way any other pack would: a folder of pictures and
//! a `pack.json`, credited to the handle of the computer that sent it, under the generated id the
//! service gave it on approval. Every file is checked against the size and SHA-256 the service
//! recorded when it was uploaded. Its finished folders are then given one shape, as `packs
//! normalize` gives them, so packs people share arrive consistent; a folder too far off that
//! shape to reshape is kept as it is and reported, and nobody's skin is ever dropped here. The
//! pack is checked against the same rules `packs check` holds every pack to before it takes its
//! place. A folder that is there already is never renumbered or written over.
//!
//! Only then is the service told the pack has been pulled (`done`), so a pack that fails comes
//! back next time. A person pulling by hand tells it at once. The Packs workflow in
//! folderskin-community tells it later, from the list [`write_list`] saves, once the packs are
//! pushed: a check or a push that fails leaves them at the service for the next run.

use crate::normalize::{self, Normalized};
use crate::packs;
use folderskin_core::pack::{self, Pack, MANIFEST_FILE};
use folderskin_share::api::Export;
use folderskin_share::sign::sha256_hex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where approved packs come from: the service, or a stand-in in the tests.
pub trait Exports {
    fn list(&mut self) -> Result<Vec<Export>, String>;
    fn manifest(&mut self, id: &str) -> Result<Vec<u8>, String>;
    fn file(&mut self, id: &str, file: &str) -> Result<Vec<u8>, String>;
    /// Tells the service the pack is in the repository now, in `folder`.
    fn done(&mut self, id: &str, folder: &str) -> Result<(), String>;
}

/// When the service hears that a pack was pulled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tell {
    /// As soon as its folder is written: a person pulling by hand, who commits what they pull.
    Now,
    /// Not here: [`done`] tells it, from the list [`write_list`] saves.
    Later,
}

/// A pack that was pulled.
#[derive(Debug)]
pub struct Pulled {
    /// The submission it came from.
    pub id: String,
    /// Its folder name in community/packs: the id the service gave it.
    pub pack_id: String,
    pub folder: PathBuf,
    pub name: String,
    pub author: String,
    pub skins: usize,
    /// The folder held exactly this pack already: an earlier pull wrote it, but the service
    /// never heard, so it is told now instead of the pack being refused.
    pub was_there: bool,
    /// What giving its finished folders one shape did: the folders redrawn, and those kept as
    /// they are for being too far off the pack's shape.
    pub shaped: Normalized,
}

/// What a pull did: the packs written, and a sentence for each that wasn't.
#[derive(Debug, Default)]
pub struct Report {
    pub pulled: Vec<Pulled>,
    pub problems: Vec<String>,
}

/// One pack in the list [`write_list`] saves for `community done`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Entry {
    /// The submission, `sub_…`.
    pub submission: String,
    /// The pack's folder in community/packs, which is its id.
    pub folder: String,
    /// The pack's name, for the commit that adds it.
    pub name: String,
}

/// Pulls every approved pack into `packs_dir` (community/packs), telling the service about each
/// one as `tell` says.
pub fn pull(source: &mut dyn Exports, packs_dir: &Path, tell: Tell) -> Result<Report, String> {
    std::fs::create_dir_all(packs_dir)
        .map_err(|e| format!("couldn't make {}: {e}", packs_dir.display()))?;
    // An id a pack had before is never given out again, so the service's choice is checked
    // against moved.json beside packs/.
    let root = packs_dir.parent().unwrap_or(Path::new("."));
    let moved = packs::read_moved(root)?;
    let mut report = Report::default();
    for export in source.list()? {
        if let Some(now) = moved.moved.get(&export.pack_id) {
            report.problems.push(format!(
                "{}: that id moved to {now}, and an old id is never given out again; nothing \
                 was written",
                export.pack_id
            ));
            continue;
        }
        match pull_one(source, &export, packs_dir) {
            Ok(pulled) => {
                // A pack written but never marked done comes back next time, and meets its own
                // folder: say so rather than leave it to be found.
                if tell == Tell::Now {
                    if let Err(e) = source.done(&export.id, &pulled.pack_id) {
                        report.problems.push(format!(
                            "{}: written to {}, but the service wasn't told ({e}); run `community \
                             pull` again once the service answers",
                            export.pack_id,
                            pulled.folder.display()
                        ));
                    }
                }
                report.pulled.push(pulled);
            }
            Err(e) => report.problems.push(format!("{}: {e}", export.pack_id)),
        }
    }
    Ok(report)
}

/// Saves the packs `pulled` as a JSON list for `community done --from`, as `pull --no-done`
/// leaves them.
pub fn write_list(path: &Path, pulled: &[Pulled]) -> Result<(), String> {
    let entries: Vec<Entry> = pulled
        .iter()
        .map(|p| Entry {
            submission: p.id.clone(),
            folder: p.pack_id.clone(),
            name: p.name.trim().to_string(),
        })
        .collect();
    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, json).map_err(|e| format!("couldn't write {}: {e}", path.display()))
}

/// Reads the list [`write_list`] saved.
pub fn read_list(path: &Path) -> Result<Vec<Entry>, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
    let entries: Vec<Entry> = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "{} isn't a list of pulled packs, as `community pull --pulled` writes: {e}",
            path.display()
        )
    })?;
    if let Some(bad) = entries.iter().find(|e| !pack::is_pack_id(&e.folder)) {
        return Err(format!(
            "{} names the folder {:?}, which isn't a pack id",
            path.display(),
            bad.folder
        ));
    }
    Ok(entries)
}

/// Tells the service that each pack in `entries` is in the repository now. Returns a sentence for
/// each it couldn't tell; telling it twice does no harm, so a failed run can be run again whole.
pub fn done(source: &mut dyn Exports, entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|e| {
            source.done(&e.submission, &e.folder).err().map(|why| {
                format!(
                    "{} ({}): the service wasn't told: {why}",
                    e.folder, e.submission
                )
            })
        })
        .collect()
}

fn pull_one(source: &mut dyn Exports, export: &Export, packs_dir: &Path) -> Result<Pulled, String> {
    let id = &export.pack_id;
    if !pack::is_generated_id(id) {
        return Err(format!(
            "the service gave it the id {id:?}, which isn't a generated one (a name and six \
             random characters, such as sky-moods-k7q2mx); nothing was written"
        ));
    }
    let manifest = source.manifest(&export.id)?;
    let pack = Pack::parse(&manifest).map_err(|problems| problems.join("; "))?;
    if pack.author != export.handle {
        return Err(format!(
            "pack.json credits {:?}, but the pack came from {:?}",
            pack.author, export.handle
        ));
    }
    let mut listed: Vec<&str> = pack.skins.iter().map(|s| s.file.as_str()).collect();
    let mut recorded: Vec<&str> = export.files.iter().map(|f| f.file.as_str()).collect();
    listed.sort_unstable();
    recorded.sort_unstable();
    if listed != recorded {
        return Err("pack.json doesn't list the files the service holds".into());
    }

    let folder = packs_dir.join(id);
    let pulled = |was_there, shaped| Pulled {
        id: export.id.clone(),
        folder: folder.clone(),
        pack_id: id.clone(),
        name: pack.name.clone(),
        author: pack.author.clone(),
        skins: pack.skins.len(),
        was_there,
        shaped,
    };
    let there = std::fs::symlink_metadata(&folder).is_ok();
    // Exactly as the service recorded it: a pull from before packs were given one shape as they
    // arrive wrote it, and the service never heard.
    if there && holds_exactly(&folder, &manifest, export) {
        return Ok(pulled(true, Normalized::default()));
    }
    let partial = packs_dir.join(format!(".pull-{id}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&partial);
    // Written, given one shape and checked beside the packs first, in a folder whose leading dot
    // every check passes over.
    let written = (|| -> Result<Normalized, String> {
        std::fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
        std::fs::write(partial.join(MANIFEST_FILE), &manifest).map_err(|e| e.to_string())?;
        for file in &export.files {
            // Only names pack.json allows reach the file system; Pack::parse checked them all.
            let bytes = source.file(&export.id, &file.file)?;
            if bytes.len() != file.bytes || sha256_hex(&bytes) != file.sha256 {
                return Err(format!(
                    "{} isn't the picture that was uploaded; nothing was written",
                    file.file
                ));
            }
            std::fs::write(partial.join(&file.file), &bytes).map_err(|e| e.to_string())?;
        }
        // Outliers are kept as they are: nobody's skin is dropped without a person deciding.
        let shaped = normalize::normalize_folder(&partial, &normalize::Options::default())?;
        packs::check_pack(&partial, id).map_err(|problems| problems.join("; "))?;
        Ok(shaped)
    })();
    let shaped = match written {
        Ok(shaped) => shaped,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&partial);
            return Err(e);
        }
    };
    if there {
        // An earlier pull wrote it and the service never heard, or it is another pack's.
        let same = same_files(&partial, &folder);
        let _ = std::fs::remove_dir_all(&partial);
        if same {
            return Ok(pulled(true, shaped));
        }
        return Err(format!(
            "{} is there already and holds something else, so it isn't written over; nothing \
             was written",
            folder.display()
        ));
    }
    if let Err(e) = std::fs::rename(&partial, &folder) {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(e.to_string());
    }
    Ok(pulled(false, shaped))
}

/// Whether `folder` holds exactly the pack `export` describes: `manifest` as its `pack.json`, and
/// every picture the service recorded, byte for byte, with nothing else.
fn holds_exactly(folder: &Path, manifest: &[u8], export: &Export) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_unstable();
    let mut expected: Vec<String> = export
        .files
        .iter()
        .map(|f| f.file.clone())
        .chain([MANIFEST_FILE.to_string()])
        .collect();
    expected.sort_unstable();
    names == expected
        && std::fs::read(folder.join(MANIFEST_FILE)).is_ok_and(|kept| kept == manifest)
        && export.files.iter().all(|f| {
            std::fs::read(folder.join(&f.file))
                .is_ok_and(|kept| kept.len() == f.bytes && sha256_hex(&kept) == f.sha256)
        })
}

/// Whether folders `a` and `b` hold the same files, byte for byte, leaving out dotfiles.
fn same_files(a: &Path, b: &Path) -> bool {
    let names = |dir: &Path| -> Option<Vec<String>> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect();
        names.sort_unstable();
        Some(names)
    };
    match (names(a), names(b)) {
        (Some(ours), Some(theirs)) if ours == theirs => ours.iter().all(|name| {
            match (std::fs::read(a.join(name)), std::fs::read(b.join(name))) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            }
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_share::api::ExportFile;
    use std::collections::HashMap;

    struct Fake {
        exports: Vec<Export>,
        manifests: HashMap<String, Vec<u8>>,
        files: HashMap<(String, String), Vec<u8>>,
        done: Vec<(String, String)>,
    }

    impl Exports for Fake {
        fn list(&mut self) -> Result<Vec<Export>, String> {
            Ok(self.exports.clone())
        }
        fn manifest(&mut self, id: &str) -> Result<Vec<u8>, String> {
            self.manifests.get(id).cloned().ok_or("gone".into())
        }
        fn file(&mut self, id: &str, file: &str) -> Result<Vec<u8>, String> {
            self.files
                .get(&(id.to_string(), file.to_string()))
                .cloned()
                .ok_or("gone".into())
        }
        fn done(&mut self, id: &str, folder: &str) -> Result<(), String> {
            self.done.push((id.to_string(), folder.to_string()));
            Ok(())
        }
    }

    fn picture(shade: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([shade, 90, 160, 255]));
        folderskin_core::raster::encode_png(&img)
    }

    /// One approved pack of two pictures, as the service would hand it over.
    fn approved(id: &str, pack_id: &str, handle: &str) -> Fake {
        approved_with(
            id,
            pack_id,
            handle,
            vec![("dawn.png", picture(10)), ("dusk.png", picture(200))],
        )
    }

    /// One approved pack of `pictures`, each skin named after its file, as the service would
    /// hand it over.
    fn approved_with(
        id: &str,
        pack_id: &str,
        handle: &str,
        pictures: Vec<(&str, Vec<u8>)>,
    ) -> Fake {
        let skins: Vec<String> = pictures
            .iter()
            .map(|(file, _)| {
                let stem = file.split('.').next().unwrap_or(file);
                let name = stem[..1].to_uppercase() + &stem[1..];
                format!(r#"    {{ "file": "{file}", "name": "{name}" }}"#)
            })
            .collect();
        let manifest = format!(
            r#"{{
  "version": 1,
  "name": "Sky moods",
  "author": "{handle}",
  "license": "CC0-1.0",
  "tags": ["sky"],
  "skins": [
{}
  ]
}}
"#,
            skins.join(",\n")
        );
        Fake {
            exports: vec![Export {
                id: id.into(),
                pack_id: pack_id.into(),
                name: "Sky moods".into(),
                license: "CC0-1.0".into(),
                handle: handle.into(),
                files: pictures
                    .iter()
                    .map(|(file, bytes)| ExportFile {
                        file: (*file).into(),
                        sha256: sha256_hex(bytes),
                        bytes: bytes.len(),
                    })
                    .collect(),
            }],
            manifests: HashMap::from([(id.to_string(), manifest.into_bytes())]),
            files: pictures
                .into_iter()
                .map(|(file, bytes)| ((id.to_string(), file.to_string()), bytes))
                .collect(),
            done: vec![],
        }
    }

    fn community(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("folderskin-pull-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(packs::PACKS_DIR)).unwrap();
        dir
    }

    /// The names in `<dir>/packs`, dotfolders too.
    fn folders(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir.join(packs::PACKS_DIR))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    const SKY: &str = "sky-moods-k7q2mx";

    #[test]
    fn an_approved_pack_becomes_a_folder_that_packs_check_accepts() {
        let dir = community("ok");
        let mut source = approved("sub_aaaaaaaaaaaaaaaaaaaa", SKY, "sunny-otter");
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR), Tell::Now).unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(report.pulled.len(), 1);
        assert_eq!(report.pulled[0].author, "sunny-otter");
        assert!(!report.pulled[0].was_there);
        assert_eq!(
            source.done,
            [("sub_aaaaaaaaaaaaaaaaaaaa".to_string(), SKY.to_string())]
        );

        let checked = packs::check(&dir).unwrap();
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        assert_eq!(checked.packs[0].0, SKY);
        // Nothing half-written is left behind.
        assert_eq!(folders(&dir), [SKY]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_that_is_there_already_is_never_written_over_or_renumbered() {
        let dir = community("taken");
        let taken = dir.join(packs::PACKS_DIR).join(SKY);
        std::fs::create_dir_all(&taken).unwrap();
        std::fs::write(taken.join("pack.json"), b"another pack's").unwrap();
        let mut source = approved("sub_bbbbbbbbbbbbbbbbbbbb", SKY, "sunny-otter");
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR), Tell::Now).unwrap();
        assert!(report.pulled.is_empty());
        assert_eq!(report.problems.len(), 1);
        assert!(
            report.problems[0].starts_with(&format!("{SKY}: {}", taken.display()))
                && report.problems[0].contains("isn't written over"),
            "{:?}",
            report.problems
        );
        assert!(source.done.is_empty(), "it comes back next time");
        assert_eq!(folders(&dir), [SKY]);
        assert_eq!(
            std::fs::read(taken.join("pack.json")).unwrap(),
            b"another pack's"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_service_hears_later_when_asked_and_a_pack_it_never_heard_about_is_told_again() {
        let dir = community("later");
        let packs_dir = dir.join(packs::PACKS_DIR);
        let mut source = approved("sub_eeeeeeeeeeeeeeeeeeee", SKY, "sunny-otter");
        let report = pull(&mut source, &packs_dir, Tell::Later).unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert!(source.done.is_empty(), "not yet");

        let list = dir.join("pulled.json");
        write_list(&list, &report.pulled).unwrap();
        let entries = read_list(&list).unwrap();
        assert_eq!(
            entries,
            [Entry {
                submission: "sub_eeeeeeeeeeeeeeeeeeee".into(),
                folder: SKY.into(),
                name: "Sky moods".into(),
            }]
        );

        // The push failed, say, and the next run meets the folder it wrote: the same pack, so it
        // is reported again rather than refused.
        let again = pull(&mut source, &packs_dir, Tell::Later).unwrap();
        assert!(again.problems.is_empty(), "{:?}", again.problems);
        assert!(again.pulled[0].was_there);
        assert_eq!(folders(&dir), [SKY]);

        assert!(done(&mut source, &entries).is_empty());
        assert!(done(&mut source, &entries).is_empty(), "twice does no harm");
        assert_eq!(
            source.done,
            [
                ("sub_eeeeeeeeeeeeeeeeeeee".to_string(), SKY.to_string()),
                ("sub_eeeeeeeeeeeeeeeeeeee".to_string(), SKY.to_string())
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_list_names_pack_folders_and_nothing_else() {
        let dir = community("list");
        let list = dir.join("pulled.json");
        std::fs::write(
            &list,
            r#"[{ "submission": "sub_x", "folder": "../up", "name": "Up" }]"#,
        )
        .unwrap();
        assert!(read_list(&list).unwrap_err().contains("isn't a pack id"));
        std::fs::write(&list, r#"{ "pulled": [] }"#).unwrap();
        assert!(read_list(&list)
            .unwrap_err()
            .contains("isn't a list of pulled packs"));
        std::fs::write(&list, "[]").unwrap();
        assert!(read_list(&list).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_a_generated_id_that_no_pack_had_before_is_taken_from_the_service() {
        let dir = community("ids");
        let packs_dir = dir.join(packs::PACKS_DIR);
        let mut source = approved("sub_ffffffffffffffffffff", "sky-moods", "sunny-otter");
        let report = pull(&mut source, &packs_dir, Tell::Now).unwrap();
        assert!(
            report.problems[0].contains("isn't a generated one"),
            "{:?}",
            report.problems
        );

        std::fs::write(
            dir.join(pack::MOVED_FILE),
            format!(r#"{{ "version": 1, "moved": {{ "{SKY}": "other-a2b3c4" }} }}"#),
        )
        .unwrap();
        let mut source = approved("sub_gggggggggggggggggggg", SKY, "sunny-otter");
        let report = pull(&mut source, &packs_dir, Tell::Now).unwrap();
        assert!(
            report.problems[0].contains("that id moved to other-a2b3c4"),
            "{:?}",
            report.problems
        );
        assert!(report.pulled.is_empty() && source.done.is_empty());
        assert!(folders(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_picture_that_isnt_the_one_uploaded_stops_that_pack() {
        let dir = community("changed");
        let id = "sub_cccccccccccccccccccc";
        let mut source = approved(id, SKY, "sunny-otter");
        source
            .files
            .insert((id.into(), "dusk.png".into()), picture(201));
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR), Tell::Now).unwrap();
        assert!(report.pulled.is_empty());
        assert_eq!(report.problems.len(), 1);
        assert!(
            report.problems[0].contains("dusk.png isn't the picture that was uploaded"),
            "{:?}",
            report.problems
        );
        assert!(source.done.is_empty(), "it comes back next time");
        assert_eq!(
            std::fs::read_dir(dir.join(packs::PACKS_DIR))
                .unwrap()
                .count(),
            0,
            "nothing was left behind"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A finished folder `w`×300 px on transparency, as a PNG.
    fn folder_png(w: u32, shade: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w + 40, 340, |x, y| {
            if (20..20 + w).contains(&x) && (20..320).contains(&y) {
                image::Rgba([shade, 90, 160, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        });
        folderskin_core::raster::encode_png(&img)
    }

    /// Four finished folders, 1.2, 1.15, 1.24 and 1.7 times as wide as tall: the pack's shape
    /// is 1.22, and the last is 39% off it.
    fn folders_of_four_shapes() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("a.png", folder_png(360, 10)),
            ("b.png", folder_png(345, 60)),
            ("c.png", folder_png(372, 110)),
            ("odd.png", folder_png(510, 160)),
        ]
    }

    #[test]
    fn a_pulled_packs_folders_arrive_one_shape_and_an_outlier_is_kept_as_it_is() {
        let dir = community("shape");
        let packs_dir = dir.join(packs::PACKS_DIR);
        let id = "sub_hhhhhhhhhhhhhhhhhhhh";
        let mut source = approved_with(id, SKY, "sunny-otter", folders_of_four_shapes());
        let report = pull(&mut source, &packs_dir, Tell::Later).unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        let shaped = &report.pulled[0].shaped;
        assert_eq!(shaped.folders, 4);
        assert!((shaped.shape.unwrap() - 1.22).abs() < 0.001);
        let redrawn: Vec<&str> = shaped.redrawn.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(redrawn, ["a.webp", "b.webp", "c.webp"]);
        assert_eq!(shaped.outliers.len(), 1);
        assert_eq!(
            (
                shaped.outliers[0].name.as_str(),
                shaped.outliers[0].file.as_str()
            ),
            ("Odd", "odd.png")
        );
        assert_eq!(report.pulled[0].skins, 4, "nothing is dropped");

        // The pack as it now is: WebPs at one shape, the outlier as it came, and it checks.
        let pack =
            Pack::parse(&std::fs::read(packs_dir.join(SKY).join(MANIFEST_FILE)).unwrap()).unwrap();
        let files: Vec<&str> = pack.skins.iter().map(|s| s.file.as_str()).collect();
        assert_eq!(files, ["a.webp", "b.webp", "c.webp", "odd.png"]);
        assert_eq!(
            std::fs::read(packs_dir.join(SKY).join("odd.png")).unwrap(),
            folders_of_four_shapes()[3].1
        );
        assert!(packs::check(&dir).unwrap().problems.is_empty());
        assert_eq!(folders(&dir), [SKY], "nothing half-written is left");

        // Pulled again after a push that failed, it meets the folder the first pull wrote.
        let again = pull(&mut source, &packs_dir, Tell::Later).unwrap();
        assert!(again.problems.is_empty(), "{:?}", again.problems);
        assert!(again.pulled[0].was_there);
        assert_eq!(folders(&dir), [SKY]);

        // A pull from before packs were given one shape wrote the pack as it was uploaded: that
        // is the same pack too, and it is left as it is.
        let folder = packs_dir.join(SKY);
        std::fs::remove_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(MANIFEST_FILE), &source.manifests[id]).unwrap();
        for (file, bytes) in folders_of_four_shapes() {
            std::fs::write(folder.join(file), bytes).unwrap();
        }
        let older = pull(&mut source, &packs_dir, Tell::Later).unwrap();
        assert!(older.problems.is_empty(), "{:?}", older.problems);
        assert!(older.pulled[0].was_there);
        assert!(folder.join("a.png").is_file(), "left as it was");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_pack_credited_to_someone_else_is_refused() {
        let dir = community("credit");
        let mut source = approved("sub_dddddddddddddddddddd", SKY, "sunny-otter");
        source.exports[0].handle = "someone-else".into();
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR), Tell::Now).unwrap();
        assert!(report.pulled.is_empty());
        assert!(
            report.problems[0].contains("credits"),
            "{:?}",
            report.problems
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
