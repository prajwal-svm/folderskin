//! Saved skins: every picture the user imports and every AI result, kept on disk so they come
//! back after a restart.
//!
//! The store is one folder, `skins/` in the app data directory:
//!
//! ```text
//! skins/
//! ├── skins.json                 the index: {"version": 1, "skins": [...]}
//! ├── 3f2a9c0b1d4e.png           a skin's picture, longest side at most 2048 px
//! └── 3f2a9c0b1d4e.thumb-v2.png  its gallery thumbnail, 512 px, so a launch renders nothing
//! ```
//!
//! A skin's id is `user:` plus the first 12 hex digits of the SHA-256 of what the user brought in
//! (the picture file, or the image the provider returned), so the same picture always maps to the
//! same entry and importing it twice adds nothing.
//!
//! Files are written before the index entry that names them, and every file, the index included,
//! is written atomically (a temp file, then a rename). A crash therefore leaves at worst an
//! unreferenced picture, never an entry without one. An index that cannot be read is set aside
//! under another name rather than overwritten, and the store starts empty.

use folderskin_core::apply::paths::write_atomic;
use folderskin_core::compositor::{self, Artwork, IconSet};
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bump when the index format changes in a way an older build could misread.
const INDEX_VERSION: u32 = 1;
const INDEX_FILE: &str = "skins.json";
const ID_PREFIX: &str = "user:";
/// Longest side of a stored picture. The compositor's master render is 2048 px, so anything
/// larger would only be thrown away at render time.
pub const MAX_STORED_SIDE: u32 = 2048;
/// Edge of a saved skin's gallery thumbnail. Changing it needs a
/// [`THUMB_CACHE_VERSION`](crate::commands::THUMB_CACHE_VERSION) bump so old ones are redrawn.
pub const THUMB_SIZE: u32 = 512;

/// How a skin becomes an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinKind {
    /// A flat picture the compositor wraps onto the folder template around a focus point.
    Artwork,
    /// A finished folder image, used as the icon as it is.
    Folder,
}

/// Where a skin came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinSource {
    /// Shipped inside the app. Never stored.
    Builtin,
    /// A picture the user imported.
    Import,
    /// A result from the AI assistant.
    Ai,
}

/// One saved skin, as the index records it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedSkin {
    /// `user:<12 hex digits>`.
    pub id: String,
    pub name: String,
    pub kind: SkinKind,
    pub source: SkinSource,
    /// When the skin was added, in Unix milliseconds.
    pub created_at: u64,
    /// Artwork only: the point cover-fit crops keep centred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<[f32; 2]>,
    /// AI results only: which provider made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// AI results only: which of the provider's models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// AI results only: the user's own words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idea: Option<String>,
}

/// What the caller knows about a skin it wants saved. The store adds the kind and focus (from
/// the image) and the time.
#[derive(Clone, Debug)]
pub struct NewSkin {
    pub id: String,
    pub name: String,
    pub source: SkinSource,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub idea: Option<String>,
}

impl NewSkin {
    /// The index entry this skin gets when it is added at `created_at`.
    pub fn entry(self, image: &SkinImage, created_at: u64) -> SavedSkin {
        SavedSkin {
            id: self.id,
            name: self.name,
            kind: image.kind(),
            source: self.source,
            created_at,
            focus: image.focus(),
            provider: self.provider,
            model: self.model,
            idea: self.idea,
        }
    }
}

/// A skin's pixels, in the form the renderer needs.
#[derive(Clone)]
pub enum SkinImage {
    /// Flat artwork for the compositor.
    Artwork(Arc<Artwork>),
    /// A finished folder image for [`compositor::icon_set_from_image`].
    Folder(Arc<RgbaImage>),
}

impl SkinImage {
    pub fn kind(&self) -> SkinKind {
        match self {
            SkinImage::Artwork(_) => SkinKind::Artwork,
            SkinImage::Folder(_) => SkinKind::Folder,
        }
    }

    fn rgba(&self) -> &RgbaImage {
        match self {
            SkinImage::Artwork(art) => &art.rgba,
            SkinImage::Folder(img) => img,
        }
    }

    fn focus(&self) -> Option<[f32; 2]> {
        match self {
            SkinImage::Artwork(art) => Some([art.focus.0, art.focus.1]),
            SkinImage::Folder(_) => None,
        }
    }

    /// The icon at every requested size: composited for artwork, fitted for a finished folder.
    pub fn icon_set(&self, sizes: &[u32]) -> IconSet {
        match self {
            SkinImage::Artwork(art) => compositor::render_icon_set(art, sizes),
            SkinImage::Folder(img) => compositor::icon_set_from_image(img, sizes),
        }
    }

    /// PNG preview at `size` px, through the same render as the applied icon.
    pub fn preview_png(&self, size: u32) -> Vec<u8> {
        match self {
            SkinImage::Artwork(art) => compositor::render_preview_png(art, size),
            SkinImage::Folder(img) => compositor::preview_png_from_image(img, size),
        }
    }

    /// The same skin with its picture no larger than [`MAX_STORED_SIDE`].
    fn bounded(&self) -> SkinImage {
        let (w, h) = self.rgba().dimensions();
        if w.max(h) <= MAX_STORED_SIDE {
            return self.clone();
        }
        let rgba = shrink_to(self.rgba().clone(), MAX_STORED_SIDE);
        match self {
            SkinImage::Artwork(art) => SkinImage::Artwork(Arc::new(Artwork {
                rgba,
                focus: art.focus,
            })),
            SkinImage::Folder(_) => SkinImage::Folder(Arc::new(rgba)),
        }
    }
}

/// Downscales `img` so its longer side is at most `max_side`, keeping the aspect ratio.
pub fn shrink_to(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= max_side {
        return img;
    }
    let s = max_side as f32 / longest as f32;
    image::imageops::resize(
        &img,
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
        image::imageops::FilterType::Lanczos3,
    )
}

/// Longest name a skin can have, in characters. The webview's name field stops at the same length.
pub const MAX_NAME_CHARS: usize = 60;

/// A name as the user typed it, made fit to show: runs of white space become one space,
/// control characters go, and it is trimmed and cut to [`MAX_NAME_CHARS`]. `None` when nothing
/// is left.
pub fn clean_name(name: &str) -> Option<String> {
    let one_line = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = one_line
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    let cut = cut.trim_end();
    (!cut.is_empty()).then(|| cut.to_string())
}

/// The id of a skin made from `content`: the same bytes always give the same id.
pub fn skin_id(content: &[u8]) -> String {
    format!("{ID_PREFIX}{}", crate::ai::hash12(content))
}

/// The file-name stem of a well-formed saved-skin id, or `None` for anything else. Ids reach
/// the store from the webview, so this is what keeps them from naming other files.
fn stem(id: &str) -> Option<&str> {
    let stem = id.strip_prefix(ID_PREFIX)?;
    let hex = stem
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (stem.len() == 12 && hex).then_some(stem)
}

fn image_file(stem: &str) -> String {
    format!("{stem}.png")
}

fn thumb_file(stem: &str) -> String {
    format!("{stem}.thumb-v{}.png", crate::commands::THUMB_CACHE_VERSION)
}

/// The current time in Unix milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Encodes a stored picture. Level 2 takes about a tenth of the time of the default level on a
/// 2048 px photo for about a tenth more bytes, which keeps an import quick.
fn encode_stored_png(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    PngEncoder::new_with_quality(&mut buf, CompressionType::Level(2), PngFilterType::Adaptive)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("encoding a PNG into memory cannot fail");
    buf
}

fn save_error(e: std::io::Error) -> String {
    format!("couldn't save that skin: {e}")
}

#[derive(Deserialize)]
struct IndexIn {
    version: u32,
    /// Read entry by entry, so one damaged entry does not cost the others.
    #[serde(default)]
    skins: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct IndexOut<'a> {
    version: u32,
    skins: &'a [SavedSkin],
}

/// The saved skins on disk. Safe to share between threads; every method locks for as short a
/// time as it can, and the slow work (encoding, rendering) happens outside the lock.
pub struct Store {
    dir: PathBuf,
    index: Mutex<Vec<SavedSkin>>,
}

impl Store {
    /// Opens the store in `dir`, which need not exist yet. Never fails: a missing index is an
    /// empty store, and one that cannot be read is set aside and logged.
    pub fn open(dir: PathBuf) -> Store {
        let skins = read_index(&dir);
        Store {
            dir,
            index: Mutex::new(skins),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<SavedSkin>> {
        self.index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Every saved skin, newest first.
    pub fn list(&self) -> Vec<SavedSkin> {
        let mut skins = self.lock().clone();
        skins.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        skins
    }

    pub fn get(&self, id: &str) -> Option<SavedSkin> {
        self.lock().iter().find(|s| s.id == id).cloned()
    }

    /// Saves a skin: its picture and thumbnail first, then its index entry.
    ///
    /// Returns the entry and whether it is new. When a skin with the same id is already saved,
    /// that one comes back unchanged and nothing is written.
    pub fn add(&self, new: NewSkin, image: &SkinImage) -> Result<(SavedSkin, bool), String> {
        let stem = stem(&new.id)
            .ok_or_else(|| "that skin id isn't one FolderSkin made".to_string())?
            .to_string();
        if let Some(existing) = self.get(&new.id) {
            return Ok((existing, false));
        }
        let image = image.bounded();
        let png = encode_stored_png(image.rgba());
        let thumb = image.preview_png(THUMB_SIZE);

        let mut index = self.lock();
        if let Some(existing) = index.iter().find(|s| s.id == new.id) {
            return Ok((existing.clone(), false));
        }
        let image_path = self.dir.join(image_file(&stem));
        let thumb_path = self.dir.join(thumb_file(&stem));
        let discard = || {
            let _ = std::fs::remove_file(&image_path);
            let _ = std::fs::remove_file(&thumb_path);
        };
        let written = std::fs::create_dir_all(&self.dir)
            .and_then(|()| write_atomic(&image_path, &png))
            .and_then(|()| write_atomic(&thumb_path, &thumb));
        if let Err(e) = written {
            discard();
            return Err(save_error(e));
        }

        // Strictly increasing, so "newest first" is well defined even within one millisecond.
        let latest = index.iter().map(|s| s.created_at).max().unwrap_or(0);
        let entry = new.entry(&image, now_ms().max(latest + 1));
        index.push(entry.clone());
        if let Err(e) = self.write_index(&index) {
            index.pop();
            discard();
            return Err(save_error(e));
        }
        Ok((entry, true))
    }

    /// Reads a saved skin's picture back. `Ok(None)` when no saved skin has this id.
    pub fn load(&self, id: &str) -> Result<Option<SkinImage>, String> {
        let (Some(entry), Some(stem)) = (self.get(id), stem(id)) else {
            return Ok(None);
        };
        let bytes = std::fs::read(self.dir.join(image_file(stem)))
            .map_err(|_| format!("the picture for {} is missing", entry.name))?;
        let rgba = image::load_from_memory(&bytes)
            .map_err(|_| format!("the picture for {} is damaged", entry.name))?
            .to_rgba8();
        Ok(Some(match entry.kind {
            SkinKind::Artwork => {
                let [x, y] = entry.focus.unwrap_or([0.5, 0.5]);
                SkinImage::Artwork(Arc::new(Artwork {
                    rgba,
                    focus: (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)),
                }))
            }
            SkinKind::Folder => SkinImage::Folder(Arc::new(rgba)),
        }))
    }

    /// A saved skin's thumbnail PNG: the cached file, or a fresh render (which is then cached)
    /// when the file is missing, damaged or from an older renderer.
    pub fn thumbnail_png(&self, entry: &SavedSkin) -> Result<Vec<u8>, String> {
        let stem = stem(&entry.id).ok_or_else(|| "that skin isn't saved".to_string())?;
        let path = self.dir.join(thumb_file(stem));
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Ok(bytes);
            }
        }
        let image = self
            .load(&entry.id)?
            .ok_or_else(|| "that skin isn't saved any more".to_string())?;
        let png = image.preview_png(THUMB_SIZE);
        if let Err(e) = write_atomic(&path, &png) {
            eprintln!(
                "folderskin: couldn't cache the thumbnail {}: {e}",
                path.display()
            );
        }
        Ok(png)
    }

    /// Removes a saved skin: its index entry first, then its picture and thumbnails. An id that
    /// is well formed but not saved is already gone, which is not an error.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let stem = stem(id).ok_or_else(|| "FolderSkin doesn't know that skin".to_string())?;
        let mut index = self.lock();
        let Some(pos) = index.iter().position(|s| s.id == id) else {
            return Ok(());
        };
        let removed = index.remove(pos);
        if let Err(e) = self.write_index(&index) {
            index.insert(pos, removed);
            return Err(format!("couldn't delete that skin: {e}"));
        }
        drop(index);
        self.remove_files(stem);
        Ok(())
    }

    /// Gives a saved skin a new name, cleaned by [`clean_name`], and returns its entry. Only the
    /// index changes: the picture and thumbnails are named after the id, not the name.
    pub fn rename(&self, id: &str, name: &str) -> Result<SavedSkin, String> {
        let name = clean_name(name).ok_or_else(|| "a skin needs a name".to_string())?;
        let mut index = self.lock();
        let pos = index
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| "that skin isn't saved any more".to_string())?;
        let before = std::mem::replace(&mut index[pos].name, name);
        if let Err(e) = self.write_index(&index) {
            index[pos].name = before;
            return Err(format!("couldn't rename that skin: {e}"));
        }
        Ok(index[pos].clone())
    }

    /// Deletes a skin's picture and every thumbnail it has had. Leftovers are only logged: the
    /// index no longer names them, so they cannot come back.
    fn remove_files(&self, stem: &str) {
        let thumb_prefix = format!("{stem}.thumb");
        let mut doomed = vec![self.dir.join(image_file(stem))];
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            doomed.extend(
                entries
                    .flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with(&thumb_prefix))
                    .map(|e| e.path()),
            );
        }
        for path in doomed {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("folderskin: couldn't remove {}: {e}", path.display()),
            }
        }
    }

    fn write_index(&self, skins: &[SavedSkin]) -> std::io::Result<()> {
        let text = serde_json::to_vec_pretty(&IndexOut {
            version: INDEX_VERSION,
            skins,
        })?;
        std::fs::create_dir_all(&self.dir)?;
        write_atomic(&self.dir.join(INDEX_FILE), &text)
    }
}

/// Reads the index in `dir`. Entries that are damaged, repeated or whose picture is gone are
/// dropped and logged; an index that cannot be read at all is set aside.
fn read_index(dir: &Path) -> Vec<SavedSkin> {
    let path = dir.join(INDEX_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            set_aside(&path, &e.to_string());
            return Vec::new();
        }
    };
    let index: IndexIn = match serde_json::from_slice(&bytes) {
        Ok(index) => index,
        Err(e) => {
            set_aside(&path, &e.to_string());
            return Vec::new();
        }
    };
    if index.version != INDEX_VERSION {
        set_aside(&path, &format!("index version {}", index.version));
        return Vec::new();
    }
    let mut seen = HashSet::new();
    index
        .skins
        .into_iter()
        .filter_map(|value| {
            let skin: SavedSkin = match serde_json::from_value(value) {
                Ok(skin) => skin,
                Err(e) => {
                    eprintln!("folderskin: dropping a damaged saved-skin entry: {e}");
                    return None;
                }
            };
            let Some(stem) = stem(&skin.id) else {
                eprintln!(
                    "folderskin: dropping saved skin with a bad id {:?}",
                    skin.id
                );
                return None;
            };
            if !dir.join(image_file(stem)).is_file() {
                eprintln!(
                    "folderskin: dropping saved skin {}: its picture is gone",
                    skin.id
                );
                return None;
            }
            if !seen.insert(skin.id.clone()) {
                eprintln!(
                    "folderskin: dropping a second entry for saved skin {}",
                    skin.id
                );
                return None;
            }
            Some(skin)
        })
        .collect()
}

/// Renames an unreadable index out of the way, so the next save starts a fresh one without
/// destroying whatever the old one held.
fn set_aside(path: &Path, why: &str) {
    let aside = path.with_file_name(format!("skins-unreadable-{}.json", now_ms()));
    match std::fs::rename(path, &aside) {
        Ok(()) => eprintln!(
            "folderskin: {} could not be read ({why}); moved it to {} and started with no saved skins",
            path.display(),
            aside.display()
        ),
        Err(e) => eprintln!(
            "folderskin: {} could not be read ({why}) or moved aside ({e}); starting with no saved skins",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty directory for one test.
    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("folderskin-store-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn artwork(colour: [u8; 3], focus: (f32, f32)) -> SkinImage {
        let [r, g, b] = colour;
        SkinImage::Artwork(Arc::new(Artwork {
            rgba: RgbaImage::from_pixel(64, 60, image::Rgba([r, g, b, 255])),
            focus,
        }))
    }

    /// A small finished "folder": opaque in the middle, transparent around it.
    fn folder() -> SkinImage {
        SkinImage::Folder(Arc::new(RgbaImage::from_fn(50, 40, |x, y| {
            let inside = (5..45).contains(&x) && (5..35).contains(&y);
            image::Rgba([30, 140, 220, if inside { 255 } else { 0 }])
        })))
    }

    fn new_skin(id: &str, name: &str, source: SkinSource) -> NewSkin {
        NewSkin {
            id: id.into(),
            name: name.into(),
            source,
            provider: None,
            model: None,
            idea: None,
        }
    }

    #[test]
    fn skins_survive_a_restart_newest_first() {
        let dir = temp_dir("roundtrip");
        let store = Store::open(dir.clone());
        let art = artwork([200, 60, 30], (0.25, 0.75));
        let (a, fresh) = store
            .add(
                new_skin(&skin_id(b"beach"), "Beach", SkinSource::Import),
                &art,
            )
            .unwrap();
        assert!(fresh);
        let mut ai = new_skin(&skin_id(b"render"), "A night sky", SkinSource::Ai);
        ai.provider = Some("openai".into());
        ai.model = Some("gpt-image-1".into());
        ai.idea = Some("a night sky with aurora".into());
        let (b, _) = store.add(ai, &folder()).unwrap();
        drop(store);

        let store = Store::open(dir.clone());
        let list = store.list();
        assert_eq!(
            list,
            vec![b.clone(), a.clone()],
            "newest first, every field kept"
        );
        assert!(b.created_at > a.created_at);
        assert_eq!(a.kind, SkinKind::Artwork);
        assert_eq!(a.focus, Some([0.25, 0.75]));
        assert_eq!(b.kind, SkinKind::Folder);
        assert_eq!(b.focus, None);
        assert_eq!(b.provider.as_deref(), Some("openai"));
        assert_eq!(b.idea.as_deref(), Some("a night sky with aurora"));

        match store.load(&a.id).unwrap().unwrap() {
            SkinImage::Artwork(loaded) => {
                let SkinImage::Artwork(original) = &art else {
                    unreachable!()
                };
                assert_eq!(loaded.rgba, original.rgba, "pixels come back exactly");
                assert_eq!(loaded.focus, (0.25, 0.75));
            }
            SkinImage::Folder(_) => panic!("an artwork skin came back as a folder"),
        }
        assert!(matches!(
            store.load(&b.id).unwrap(),
            Some(SkinImage::Folder(_))
        ));
        assert!(store.load(&skin_id(b"never saved")).unwrap().is_none());

        let thumb = store.thumbnail_png(&a).unwrap();
        let decoded = image::load_from_memory(&thumb).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (THUMB_SIZE, THUMB_SIZE)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_the_same_picture_twice_keeps_one_entry() {
        let dir = temp_dir("dedupe");
        let store = Store::open(dir.clone());
        let id = skin_id(b"same bytes");
        let (first, fresh) = store
            .add(new_skin(&id, "First", SkinSource::Import), &folder())
            .unwrap();
        assert!(fresh);
        let (again, fresh) = store
            .add(new_skin(&id, "Second name", SkinSource::Import), &folder())
            .unwrap();
        assert!(!fresh);
        assert_eq!(again, first, "the saved entry comes back unchanged");
        assert_eq!(Store::open(dir.clone()).list().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn renaming_changes_only_the_name_and_survives_a_restart() {
        let dir = temp_dir("rename");
        let store = Store::open(dir.clone());
        let id = skin_id(b"rename");
        store
            .add(
                new_skin(&id, "A night sky", SkinSource::Ai),
                &artwork([9, 9, 9], (0.5, 0.5)),
            )
            .unwrap();

        let renamed = store.rename(&id, "  Beach   trip\n2026 ").unwrap();
        assert_eq!(renamed.name, "Beach trip 2026");
        assert_eq!(renamed.source, SkinSource::Ai, "nothing else changes");
        assert!(
            store.rename(&id, " \t ").is_err(),
            "a blank name is refused"
        );
        assert!(
            store.rename(&skin_id(b"unknown"), "x").is_err(),
            "so is a skin that isn't saved"
        );

        let reopened = Store::open(dir.clone());
        assert_eq!(reopened.get(&id).unwrap().name, "Beach trip 2026");
        assert!(
            reopened.load(&id).unwrap().is_some(),
            "the picture is untouched"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_are_cleaned_and_cut_between_characters() {
        assert_eq!(clean_name("  a \n b\t c "), Some("a b c".into()));
        assert_eq!(clean_name("x\u{7}y"), Some("xy".into()));
        assert_eq!(clean_name("   "), None);
        let long = clean_name(&"🦊".repeat(80)).unwrap();
        assert_eq!(long.chars().count(), MAX_NAME_CHARS);
        assert_eq!(
            clean_name(&format!("{} tail", "a".repeat(59))),
            Some("a".repeat(59)),
            "no trailing space where the cut lands"
        );
    }

    #[test]
    fn deleting_removes_the_entry_and_its_files() {
        let dir = temp_dir("delete");
        let store = Store::open(dir.clone());
        let keep = skin_id(b"keep");
        let gone = skin_id(b"gone");
        store
            .add(
                new_skin(&keep, "Keep", SkinSource::Import),
                &artwork([1, 2, 3], (0.5, 0.5)),
            )
            .unwrap();
        let (entry, _) = store
            .add(new_skin(&gone, "Gone", SkinSource::Ai), &folder())
            .unwrap();
        store.thumbnail_png(&entry).unwrap();

        store.delete(&gone).unwrap();
        assert!(store.get(&gone).is_none());
        let stem = stem(&gone).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(stem))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let reopened = Store::open(dir.clone());
        assert_eq!(reopened.list().len(), 1);
        assert_eq!(reopened.list()[0].id, keep);

        // Already gone is fine; an id that names something else is refused.
        assert!(reopened.delete(&gone).is_ok());
        assert!(reopened.delete("user:../skins").is_err());
        assert!(reopened.delete("aurora").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_index_is_set_aside_and_the_store_starts_empty() {
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join(INDEX_FILE), b"{\"version\": 1, \"skins\": [").unwrap();
        let store = Store::open(dir.clone());
        assert!(store.list().is_empty());
        let aside: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("skins-unreadable-"))
            .collect();
        assert_eq!(aside.len(), 1, "the damaged index is kept for inspection");

        // And the store keeps working.
        store
            .add(new_skin(&skin_id(b"x"), "X", SkinSource::Import), &folder())
            .unwrap();
        assert_eq!(Store::open(dir.clone()).list().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_store_is_empty_and_created_on_first_save() {
        let dir = temp_dir("missing").join("skins");
        let store = Store::open(dir.clone());
        assert!(store.list().is_empty());
        assert!(!dir.exists(), "opening writes nothing");
        store
            .add(new_skin(&skin_id(b"y"), "Y", SkinSource::Import), &folder())
            .unwrap();
        assert!(dir.join(INDEX_FILE).is_file());
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn damaged_entries_and_missing_pictures_are_dropped_on_open() {
        let dir = temp_dir("entries");
        let store = Store::open(dir.clone());
        let (good, _) = store
            .add(
                new_skin(&skin_id(b"good"), "Good", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        let (lost, _) = store
            .add(
                new_skin(&skin_id(b"lost"), "Lost", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        std::fs::remove_file(dir.join(image_file(stem(&lost.id).unwrap()))).unwrap();

        // Hand-edit the index: a duplicate, an entry from nowhere, and a path-shaped id.
        let mut index: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(INDEX_FILE)).unwrap()).unwrap();
        let skins = index["skins"].as_array_mut().unwrap();
        skins.push(skins[0].clone());
        skins.push(serde_json::json!({"id": "user:0123456789ab", "kind": "sticker"}));
        let mut evil = skins[0].clone();
        evil["id"] = "user:../../etc".into();
        skins.push(evil);
        std::fs::write(dir.join(INDEX_FILE), serde_json::to_vec(&index).unwrap()).unwrap();

        assert_eq!(Store::open(dir.clone()).list(), vec![good]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_thumbnail_is_rendered_again() {
        let dir = temp_dir("thumb");
        let store = Store::open(dir.clone());
        let (entry, _) = store
            .add(
                new_skin(&skin_id(b"t"), "T", SkinSource::Import),
                &artwork([9, 9, 9], (0.5, 0.5)),
            )
            .unwrap();
        let path = dir.join(thumb_file(stem(&entry.id).unwrap()));
        std::fs::write(&path, b"not a png").unwrap();
        let png = store.thumbnail_png(&entry).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert_eq!(std::fs::read(&path).unwrap(), png, "and cached again");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn oversized_pictures_are_stored_at_the_bound() {
        let dir = temp_dir("bound");
        let store = Store::open(dir.clone());
        let big = SkinImage::Folder(Arc::new(RgbaImage::from_pixel(
            MAX_STORED_SIDE * 2,
            10,
            image::Rgba([1, 2, 3, 255]),
        )));
        let (entry, _) = store
            .add(new_skin(&skin_id(b"big"), "Big", SkinSource::Import), &big)
            .unwrap();
        let SkinImage::Folder(img) = store.load(&entry.id).unwrap().unwrap() else {
            panic!("a folder skin came back as artwork");
        };
        assert_eq!(img.dimensions(), (MAX_STORED_SIDE, 5));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_well_formed_ids_name_files() {
        assert_eq!(stem("user:0123456789ab"), Some("0123456789ab"));
        for bad in [
            "user:0123456789AB",
            "user:0123456789a",
            "user:0123456789abc",
            "user:../../etc/pw",
            "custom:0123456789ab",
            "aurora",
            "",
        ] {
            assert_eq!(stem(bad), None, "{bad:?}");
        }
        let id = skin_id(b"hello");
        assert!(stem(&id).is_some(), "{id}");
        assert_eq!(id, skin_id(b"hello"));
        assert_ne!(id, skin_id(b"hello!"));
    }
}
