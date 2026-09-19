//! `assets/skins/manifest.json` — the list of built-in skins and their metadata.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SKIN_WIDTH: u32 = 1024;
pub const SKIN_HEIGHT: u32 = 958;
pub const MAX_SKIN_BYTES: u64 = 400 * 1024;
pub const SHIPPED_SKIN_COUNT: usize = 10;

fn default_focus() -> [f32; 2] {
    [0.5, 0.5]
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SkinEntry {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub file: String,
    #[serde(default = "default_focus")]
    pub focus: [f32; 2],
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    pub version: u32,
    pub skins: Vec<SkinEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("could not read {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("manifest is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 1,
            skins: Vec::new(),
        }
    }
}

impl Manifest {
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join("manifest.json")
    }

    pub fn load(dir: &Path) -> Result<Manifest, ManifestError> {
        let p = Self::path_in(dir);
        let text = std::fs::read_to_string(&p).map_err(|e| ManifestError::Io(p, e))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, dir: &Path) -> Result<(), ManifestError> {
        let p = Self::path_in(dir);
        let text = serde_json::to_string_pretty(self)? + "\n";
        std::fs::write(&p, text).map_err(|e| ManifestError::Io(p, e))
    }

    /// Adds or replaces the entry with the same id.
    pub fn upsert(&mut self, entry: SkinEntry) {
        if let Some(existing) = self.skins.iter_mut().find(|s| s.id == entry.id) {
            *existing = entry;
        } else {
            self.skins.push(entry);
        }
    }

    pub fn is_valid_id(id: &str) -> bool {
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }

    /// Returns human-readable problems; an empty list means the manifest and files are good.
    pub fn validate(&self, dir: &Path) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for s in &self.skins {
            if !Self::is_valid_id(&s.id) {
                problems.push(format!(
                    "skin id {:?} must be lowercase letters, digits and dashes",
                    s.id
                ));
            }
            if !seen.insert(s.id.clone()) {
                problems.push(format!("duplicate skin id {:?}", s.id));
            }
            if s.name.trim().is_empty() {
                problems.push(format!("skin {:?} has no name", s.id));
            }
            if !(0.0..=1.0).contains(&s.focus[0]) || !(0.0..=1.0).contains(&s.focus[1]) {
                problems.push(format!("skin {:?} focus must be within 0..1", s.id));
            }
            let file = dir.join(&s.file);
            match std::fs::metadata(&file) {
                Err(_) => problems.push(format!("skin {:?}: file {} is missing", s.id, s.file)),
                Ok(meta) => {
                    if meta.len() > MAX_SKIN_BYTES {
                        problems.push(format!(
                            "skin {:?}: {} is {} KB, above the {} KB limit",
                            s.id,
                            s.file,
                            meta.len() / 1024,
                            MAX_SKIN_BYTES / 1024
                        ));
                    }
                    match image::ImageReader::open(&file).and_then(|r| r.with_guessed_format()) {
                        Ok(reader) => match reader.into_dimensions() {
                            Ok((w, h)) if (w, h) != (SKIN_WIDTH, SKIN_HEIGHT) => {
                                problems.push(format!(
                                    "skin {:?}: {} is {}×{}, expected {}×{}",
                                    s.id, s.file, w, h, SKIN_WIDTH, SKIN_HEIGHT
                                ))
                            }
                            Ok(_) => {}
                            Err(e) => problems
                                .push(format!("skin {:?}: cannot read {}: {}", s.id, s.file, e)),
                        },
                        Err(e) => {
                            problems.push(format!("skin {:?}: cannot open {}: {}", s.id, s.file, e))
                        }
                    }
                }
            }
        }
        if self.skins.len() != SHIPPED_SKIN_COUNT {
            problems.push(format!(
                "manifest lists {} skins; the shipped set is exactly {} (replace one instead of adding or removing)",
                self.skins.len(),
                SHIPPED_SKIN_COUNT
            ));
        }
        problems
    }

    /// Total bytes of all skin files that exist.
    pub fn total_bytes(&self, dir: &Path) -> u64 {
        self.skins
            .iter()
            .filter_map(|s| std::fs::metadata(dir.join(&s.file)).ok().map(|m| m.len()))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "folderskin-manifest-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]))
            .save(path)
            .unwrap();
    }

    #[test]
    fn round_trips_through_json_with_defaults() {
        let text = r#"{"version":1,"skins":[{"id":"aurora","name":"Aurora","collection":"glow","file":"aurora.jpg"}]}"#;
        let m: Manifest = serde_json::from_str(text).unwrap();
        assert_eq!(m.skins[0].focus, [0.5, 0.5]);
        assert_eq!(m.skins[0].author, "");
        let again: Manifest = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, again);
    }

    #[test]
    fn validate_reports_missing_wrong_size_and_bad_ids() {
        let dir = temp_dir("validate");
        write_png(&dir.join("good.png"), SKIN_WIDTH, SKIN_HEIGHT);
        write_png(&dir.join("small.png"), 100, 100);
        let mut m = Manifest::default();
        for (id, file) in [
            ("good", "good.png"),
            ("small", "small.png"),
            ("Bad Id", "good.png"),
            ("gone", "gone.png"),
        ] {
            m.skins.push(SkinEntry {
                id: id.into(),
                name: id.into(),
                collection: "glow".into(),
                file: file.into(),
                focus: [0.5, 0.5],
                author: String::new(),
                license: String::new(),
            });
        }
        let problems = m.validate(&dir);
        assert!(
            problems.iter().any(|p| p.contains("small.png is 100×100")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("\"Bad Id\" must be lowercase")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("gone.png is missing")),
            "{problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p.contains("\"good\"")),
            "{problems:?}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn upsert_replaces_same_id() {
        let mut m = Manifest::default();
        let mk = |name: &str| SkinEntry {
            id: "x".into(),
            name: name.into(),
            collection: "pop".into(),
            file: "x.png".into(),
            focus: [0.5, 0.5],
            author: String::new(),
            license: String::new(),
        };
        m.upsert(mk("one"));
        m.upsert(mk("two"));
        assert_eq!(m.skins.len(), 1);
        assert_eq!(m.skins[0].name, "two");
    }

    #[test]
    fn save_and_load() {
        let dir = temp_dir("save");
        let mut m = Manifest::default();
        m.upsert(SkinEntry {
            id: "a".into(),
            name: "A".into(),
            collection: "glow".into(),
            file: "a.png".into(),
            focus: [0.4, 0.6],
            author: "me".into(),
            license: "CC0-1.0".into(),
        });
        m.save(&dir).unwrap();
        assert_eq!(Manifest::load(&dir).unwrap(), m);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
