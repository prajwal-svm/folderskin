//! `community pull`: approved packs from the community service, written into community/packs as
//! ordinary pack folders.
//!
//! A pack shared without GitHub arrives here the way any other pack would: a folder of pictures
//! and a `pack.json`, credited to the handle of the computer that sent it. Every file is checked
//! against the size and SHA-256 the service recorded when it was uploaded, and the finished folder
//! against the same rules `packs check` holds every pack to, before it takes its place. Only then
//! is the service told the pack has been pulled, so a pack that fails comes back next time.
//! From there it is an ordinary change to review, commit and index.

use crate::packs;
use folderskin_core::pack::{self, Pack, MANIFEST_FILE};
use folderskin_share::api::Export;
use folderskin_share::sign::sha256_hex;
use std::path::{Path, PathBuf};

/// Where approved packs come from: the service, or a stand-in in the tests.
pub trait Exports {
    fn list(&mut self) -> Result<Vec<Export>, String>;
    fn manifest(&mut self, id: &str) -> Result<Vec<u8>, String>;
    fn file(&mut self, id: &str, file: &str) -> Result<Vec<u8>, String>;
    /// Tells the service the pack is in the repository now.
    fn done(&mut self, id: &str) -> Result<(), String>;
}

/// A pack that was written.
#[derive(Debug)]
pub struct Pulled {
    /// The submission it came from.
    pub id: String,
    pub folder: PathBuf,
    pub name: String,
    pub author: String,
    pub skins: usize,
}

/// What a pull did: the packs written, and a sentence for each that wasn't.
#[derive(Debug, Default)]
pub struct Report {
    pub pulled: Vec<Pulled>,
    pub problems: Vec<String>,
}

/// Pulls every approved pack into `packs_dir` (community/packs).
pub fn pull(source: &mut dyn Exports, packs_dir: &Path) -> Result<Report, String> {
    std::fs::create_dir_all(packs_dir)
        .map_err(|e| format!("couldn't make {}: {e}", packs_dir.display()))?;
    let mut report = Report::default();
    for export in source.list()? {
        match pull_one(source, &export, packs_dir) {
            Ok(pulled) => {
                // A pack written but not marked done would be written again next time, under a
                // new folder name; say so rather than leave it to be found.
                if let Err(e) = source.done(&export.id) {
                    report.problems.push(format!(
                        "{}: written to {}, but the service wasn't told ({e}); delete the folder \
                         before pulling again",
                        export.pack_id,
                        pulled.folder.display()
                    ));
                }
                report.pulled.push(pulled);
            }
            Err(e) => report.problems.push(format!("{}: {e}", export.pack_id)),
        }
    }
    Ok(report)
}

fn pull_one(source: &mut dyn Exports, export: &Export, packs_dir: &Path) -> Result<Pulled, String> {
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

    let name = free_folder(packs_dir, &export.pack_id)?;
    let partial = packs_dir.join(format!(".pull-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&partial);
    let written = (|| -> Result<(), String> {
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
        packs::check_pack(&partial, &name).map_err(|problems| problems.join("; "))?;
        let folder = packs_dir.join(&name);
        std::fs::rename(&partial, &folder).map_err(|e| e.to_string())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(e);
    }
    Ok(Pulled {
        id: export.id.clone(),
        folder: packs_dir.join(&name),
        name: pack.name,
        author: pack.author,
        skins: pack.skins.len(),
    })
}

/// The pack's folder name, or the first `-2`, `-3`… that no folder has: the service only knows
/// its own packs, and one from GitHub may already have the name.
fn free_folder(packs_dir: &Path, wanted: &str) -> Result<String, String> {
    if !pack::is_pack_id(wanted) {
        return Err(format!("{wanted:?} isn't a pack folder name"));
    }
    (1..100)
        .map(|n| {
            if n == 1 {
                wanted.to_string()
            } else {
                let suffix = format!("-{n}");
                let base = &wanted[..wanted.len().min(40 - suffix.len())];
                format!("{}{suffix}", base.trim_end_matches('-'))
            }
        })
        .find(|name| !packs_dir.join(name).exists())
        .ok_or_else(|| format!("every folder name like {wanted} is taken"))
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
        done: Vec<String>,
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
        fn done(&mut self, id: &str) -> Result<(), String> {
            self.done.push(id.to_string());
            Ok(())
        }
    }

    fn picture(shade: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([shade, 90, 160, 255]));
        folderskin_core::raster::encode_png(&img)
    }

    /// One approved pack of two pictures, as the service would hand it over.
    fn approved(id: &str, pack_id: &str, handle: &str) -> Fake {
        let pictures = [("dawn.png", picture(10)), ("dusk.png", picture(200))];
        let manifest = format!(
            r#"{{
  "version": 1,
  "name": "Sky moods",
  "author": "{handle}",
  "license": "CC0-1.0",
  "tags": ["sky"],
  "skins": [
    {{ "file": "dawn.png", "name": "Dawn" }},
    {{ "file": "dusk.png", "name": "Dusk" }}
  ]
}}
"#
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

    #[test]
    fn an_approved_pack_becomes_a_folder_that_packs_check_accepts() {
        let dir = community("ok");
        let mut source = approved("sub_aaaaaaaaaaaaaaaaaaaa", "sky-moods", "sunny-otter");
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR)).unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(report.pulled.len(), 1);
        assert_eq!(report.pulled[0].author, "sunny-otter");
        assert_eq!(source.done, ["sub_aaaaaaaaaaaaaaaaaaaa"]);

        let checked = packs::check(&dir).unwrap();
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        assert_eq!(checked.packs[0].0, "sky-moods");
        // Nothing half-written is left behind.
        let names: Vec<String> = std::fs::read_dir(dir.join(packs::PACKS_DIR))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["sky-moods"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_name_a_github_pack_already_has_gets_a_number() {
        let dir = community("taken");
        std::fs::create_dir_all(dir.join(packs::PACKS_DIR).join("sky-moods")).unwrap();
        let mut source = approved("sub_bbbbbbbbbbbbbbbbbbbb", "sky-moods", "sunny-otter");
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR)).unwrap();
        assert_eq!(
            report.pulled[0].folder,
            dir.join(packs::PACKS_DIR).join("sky-moods-2")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_picture_that_isnt_the_one_uploaded_stops_that_pack() {
        let dir = community("changed");
        let id = "sub_cccccccccccccccccccc";
        let mut source = approved(id, "sky-moods", "sunny-otter");
        source
            .files
            .insert((id.into(), "dusk.png".into()), picture(201));
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR)).unwrap();
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

    #[test]
    fn a_pack_credited_to_someone_else_is_refused() {
        let dir = community("credit");
        let mut source = approved("sub_dddddddddddddddddddd", "sky-moods", "sunny-otter");
        source.exports[0].handle = "someone-else".into();
        let report = pull(&mut source, &dir.join(packs::PACKS_DIR)).unwrap();
        assert!(report.pulled.is_empty());
        assert!(
            report.problems[0].contains("credits"),
            "{:?}",
            report.problems
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
