//! What looking through a community pack drew, kept so looking at it again is instant.
//!
//! The Community viewer downloads a whole pack (up to 50 pictures, often several MB) and draws
//! every skin as its folder. The drawings go in the app cache directory, one file per pack at one
//! version: the file is named after the pack's hash, so a pack that changed on GitHub is drawn
//! afresh. Drawings older than [`TTL`] are thrown away, and so is every other version of a pack
//! once a new one is kept. The operating system may also empty the cache directory whenever it
//! needs the room; that only means drawing the pack again.

use crate::community::PackSkinDto;
use folderskin_core::apply::paths::write_atomic;
use folderskin_core::pack;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How long a pack's drawings are kept after they are made.
pub const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Part of every file name: the thumbnails' tag, which changes when skins are drawn differently
/// and names the folder they go on, so drawings made the old way or on the other folder are never
/// shown.
fn version() -> String {
    format!("view-{}", crate::commands::thumb_tag())
}

/// The kept drawings, in their own folder.
#[derive(Clone)]
pub struct PackViews {
    dir: PathBuf,
}

impl PackViews {
    pub fn new(dir: PathBuf) -> PackViews {
        PackViews { dir }
    }

    /// Where pack `id` at version `hash` is kept. `None` unless both are what they claim to be,
    /// a pack id and a hex hash, so neither can name a file outside the folder.
    fn file(&self, id: &str, hash: &str) -> Option<PathBuf> {
        let hex = !hash.is_empty()
            && hash.len() <= 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        (pack::is_pack_id(id) && hex)
            .then(|| self.dir.join(format!("{id}.{hash}.{}.json", version())))
    }

    /// Pack `id`'s drawings at version `hash`, if they were made less than [`TTL`] before `now`.
    /// Expired ones are deleted on the way.
    pub fn get(&self, id: &str, hash: &str, now: SystemTime) -> Option<Vec<PackSkinDto>> {
        let path = self.file(id, hash)?;
        let made = std::fs::metadata(&path).ok()?.modified().ok()?;
        if expired(made, now) {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        serde_json::from_slice(&std::fs::read(&path).ok()?).ok()
    }

    /// Keeps `skins` as pack `id`'s drawings at version `hash`, then drops the pack's other
    /// versions and every drawing past [`TTL`]. A failed write only means drawing it again.
    pub fn put(&self, id: &str, hash: &str, skins: &[PackSkinDto], now: SystemTime) {
        let Some(path) = self.file(id, hash) else {
            return;
        };
        let kept = std::fs::create_dir_all(&self.dir)
            .map_err(|e| e.to_string())
            .and_then(|()| serde_json::to_vec(skins).map_err(|e| e.to_string()))
            .and_then(|json| write_atomic(&path, &json).map_err(|e| e.to_string()));
        if let Err(e) = kept {
            eprintln!("folderskin: couldn't keep the drawings of {id}: {e}");
        }
        self.prune(now, id, &path);
    }

    /// Deletes drawings past [`TTL`], ones made another way ([`version`]), and pack `id`'s other
    /// versions: everything but `current`, the file just kept.
    fn prune(&self, now: SystemTime, id: &str, current: &Path) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let ours = format!(".{}.json", version());
        let same_pack = format!("{id}.");
        for entry in entries.flatten() {
            let path = entry.path();
            if path == current {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let old_version = !name.ends_with(&ours);
            let superseded = name.starts_with(&same_pack);
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .map_or(true, |made| expired(made, now));
            if old_version || superseded || stale {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

fn expired(made: SystemTime, now: SystemTime) -> bool {
    now.duration_since(made).is_ok_and(|age| age > TTL)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of kept drawings in the system temp folder, removed when the test ends.
    struct Views(PackViews);

    impl Views {
        fn new(test: &str) -> Views {
            let dir = std::env::temp_dir().join(format!(
                "folderskin-pack-views-{test}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            Views(PackViews::new(dir))
        }

        fn files(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.0.dir)
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

    impl Drop for Views {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0.dir);
        }
    }

    fn skins(names: &[&str]) -> Vec<PackSkinDto> {
        names
            .iter()
            .map(|name| PackSkinDto {
                name: name.to_string(),
                tags: vec!["pop art".into()],
                thumbnail: format!("data:image/png;base64,{name}"),
            })
            .collect()
    }

    #[test]
    fn a_pack_looked_at_again_comes_back_as_it_was_drawn() {
        let v = Views::new("again");
        let now = SystemTime::now();
        assert_eq!(v.0.get("scientists", "148c2ab9e61f5eaf", now), None);
        v.0.put(
            "scientists",
            "148c2ab9e61f5eaf",
            &skins(&["Ada", "Alan"]),
            now,
        );
        assert_eq!(
            v.0.get("scientists", "148c2ab9e61f5eaf", now),
            Some(skins(&["Ada", "Alan"]))
        );
        // Another version of the pack is drawn afresh.
        assert_eq!(v.0.get("scientists", "0000000000000000", now), None);
    }

    #[test]
    fn drawings_expire_after_the_ttl_and_are_deleted() {
        let v = Views::new("ttl");
        let now = SystemTime::now();
        v.0.put("colours", "8c46dc19991a23f9", &skins(&["Blue"]), now);
        let almost = now + TTL - Duration::from_secs(60);
        assert!(v.0.get("colours", "8c46dc19991a23f9", almost).is_some());
        let later = now + TTL + Duration::from_secs(60);
        assert_eq!(v.0.get("colours", "8c46dc19991a23f9", later), None);
        assert_eq!(v.files(), Vec::<String>::new());
    }

    #[test]
    fn keeping_a_new_version_drops_the_old_one_and_anything_expired() {
        let v = Views::new("prune");
        let now = SystemTime::now();
        v.0.put("colours", "1111111111111111", &skins(&["Blue"]), now);
        v.0.put("colours-2", "2222222222222222", &skins(&["Red"]), now);
        std::fs::write(v.0.dir.join("colours.3333.view-v0.json"), b"[]").unwrap();
        v.0.put("colours", "4444444444444444", &skins(&["Green"]), now);
        let tag = version();
        assert_eq!(
            v.files(),
            [
                format!("colours-2.2222222222222222.{tag}.json"),
                format!("colours.4444444444444444.{tag}.json")
            ]
        );
        // A week on, keeping another pack clears both.
        v.0.put(
            "soft-rainbow",
            "5555555555555555",
            &skins(&["Mauve"]),
            now + TTL * 2,
        );
        assert_eq!(
            v.files(),
            [format!("soft-rainbow.5555555555555555.{tag}.json")]
        );
    }

    #[test]
    fn nothing_outside_the_folder_can_be_named() {
        let v = Views::new("names");
        let now = SystemTime::now();
        for (id, hash) in [
            ("../escape", "148c2ab9e61f5eaf"),
            ("scientists", "../../etc"),
            ("scientists", "148C2AB9E61F5EAF"),
            ("scientists", ""),
            ("", "148c2ab9e61f5eaf"),
        ] {
            v.0.put(id, hash, &skins(&["x"]), now);
            assert_eq!(v.0.get(id, hash, now), None, "{id:?} {hash:?}");
        }
        assert_eq!(v.files(), Vec::<String>::new());
    }

    #[test]
    fn a_damaged_file_is_drawn_again() {
        let v = Views::new("damaged");
        let now = SystemTime::now();
        v.0.put("scientists", "148c2ab9e61f5eaf", &skins(&["Ada"]), now);
        let file = v.0.file("scientists", "148c2ab9e61f5eaf").unwrap();
        std::fs::write(&file, b"{ not json").unwrap();
        assert_eq!(v.0.get("scientists", "148c2ab9e61f5eaf", now), None);
    }
}
