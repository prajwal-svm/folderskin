//! Saved skins: every picture the user imports and every AI result, kept on disk so they come
//! back after a restart.
//!
//! The store is one folder, `skins/` in the app data directory:
//!
//! ```text
//! skins/
//! ├── skins.json                 the index: {"version": 1, "skins": [...]}
//! ├── 3f2a9c0b1d4e.webp          a skin's picture, a lossless WebP, longest side at most 2048 px
//! ├── 3f2a9c0b1d4e.thumb-v2.png  its gallery thumbnail, 512 px, so a launch renders nothing
//! │                              (thumb-v2-windows.png on Windows, drawn on Windows' folder)
//! └── 3f2a9c0b1d4e.design.json   a design from the composer: the document it was made from, so
//!                                it can be edited again
//! ```
//!
//! A picture saved by FolderSkin 0.1.6 or before is a PNG, `3f2a9c0b1d4e.png`, and is read as it
//! is: only a new picture is written as WebP, which keeps every pixel as the PNG did in about two
//! thirds of the space ([`encode_stored_picture`]).
//!
//! A skin's id is `user:` plus the first 12 hex digits of the SHA-256 of what the user brought in
//! (the picture file, or the image the provider returned), so the same picture always maps to the
//! same entry and importing it twice adds nothing.
//!
//! Files are written before the index entry that names them, and every file, the index included,
//! is written atomically (a temp file, then a rename). A crash therefore leaves at worst an
//! unreferenced picture, design or temp file, never an entry without its picture, and the next
//! launch removes those ([`Store::open`]). Several skins saved together, such as a community pack,
//! share one index write, so they are saved all together or not at all ([`Store::add_many`]). A
//! design saved over the one it was made from swaps the two in one index write too
//! ([`Store::replace_design`]). An index that cannot be read is set aside under another name
//! rather than overwritten, and the store starts empty.

use folderskin_core::apply::paths::write_atomic;
use folderskin_core::compositor::{self, Artwork, IconSet};
use folderskin_core::{pack, raster};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Bump when the index format changes in a way an older build could misread.
const INDEX_VERSION: u32 = 1;
const INDEX_FILE: &str = "skins.json";
/// What an index that could not be read is renamed to start with ([`set_aside`]).
const SET_ASIDE_PREFIX: &str = "skins-unreadable-";
const ID_PREFIX: &str = "user:";
/// How long a file the index doesn't name is left alone before [`Store::open`] removes it:
/// another FolderSkin running at the same time may be part way through saving it.
const LEFTOVER_AGE: Duration = Duration::from_secs(10 * 60);
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
    /// A picture the user imported.
    Import,
    /// A result from the AI assistant.
    Ai,
    /// A skin from a community pack.
    Community,
    /// A design the user made in the composer.
    Composer,
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
    /// What the gallery can filter it by, cleaned by [`pack::clean_tags`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Community skins only: the id of the pack it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
    /// Community skins only: the pack's name, its author and its licence, kept so the skin can
    /// always be credited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Community skins only: the pack's contents when it was added
    /// ([`folderskin_core::pack::pack_hash`]), to tell when it has an update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_hash: Option<String>,
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
    pub tags: Vec<String>,
    pub pack: Option<String>,
    pub pack_name: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub pack_hash: Option<String>,
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
            tags: pack::clean_tags(&self.tags, pack::MAX_TAGS),
            pack: self.pack,
            pack_name: self.pack_name,
            author: self.author,
            license: self.license,
            pack_hash: self.pack_hash,
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

    /// The picture itself: the flat artwork, or the finished folder.
    pub fn rgba(&self) -> &RgbaImage {
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

    /// The icon at every requested size: artwork composited onto the folder this computer draws
    /// (Windows' own on Windows, FolderSkin's elsewhere), a finished folder fitted as it is.
    pub fn icon_set(&self, sizes: &[u32]) -> IconSet {
        match self {
            SkinImage::Artwork(art) => {
                compositor::render_icon_set_in(art, sizes, crate::look::current())
            }
            SkinImage::Folder(img) => compositor::icon_set_from_image(img, sizes),
        }
    }

    /// PNG preview at `size` px, through the same render as the applied icon.
    pub fn preview_png(&self, size: u32) -> Vec<u8> {
        match self {
            SkinImage::Artwork(art) => {
                compositor::render_preview_png_in(art, size, crate::look::current())
            }
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

/// Downscales `img` so its longer side is at most `max_side`, keeping the aspect ratio: the
/// resampling a pack's pictures get too ([`raster::shrink_to`]).
pub fn shrink_to(img: RgbaImage, max_side: u32) -> RgbaImage {
    raster::shrink_to(img, max_side)
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

/// Whether `id` has the shape of a saved skin's id, whether or not that skin is saved.
pub fn is_skin_id(id: &str) -> bool {
    stem(id).is_some()
}

/// The file-name stem of a well-formed saved-skin id, or `None` for anything else. Ids reach
/// the store from the webview, so this is what keeps them from naming other files.
fn stem(id: &str) -> Option<&str> {
    id.strip_prefix(ID_PREFIX).filter(|stem| is_stem(stem))
}

/// Twelve lower-case hex digits: the stem of every file a skin has.
fn is_stem(s: &str) -> bool {
    s.len() == 12
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A skin's picture, as it is written now.
fn image_file(stem: &str) -> String {
    format!("{stem}.webp")
}

/// A skin's picture as FolderSkin 0.1.6 and before wrote it. Read, and removed with its skin,
/// but never written.
fn old_image_file(stem: &str) -> String {
    format!("{stem}.png")
}

/// Where the picture of the skin with this stem is in `dir`: the WebP, or the PNG an older
/// FolderSkin saved. `None` when it has neither.
fn picture_path(dir: &Path, stem: &str) -> Option<PathBuf> {
    [image_file(stem), old_image_file(stem)]
        .into_iter()
        .map(|file| dir.join(file))
        .find(|path| path.is_file())
}

fn thumb_file(stem: &str) -> String {
    format!("{stem}.thumb-{}.png", crate::commands::thumb_tag())
}

/// What follows the stem in the name of a composer design's document.
const DESIGN_SUFFIX: &str = ".design.json";

fn design_file(stem: &str) -> String {
    format!("{stem}{DESIGN_SUFFIX}")
}

/// Why a design can't be saved over a skin that isn't one.
pub const NOT_A_DESIGN: &str = "that skin isn't one of your designs";
/// Why a design can't be saved over one that has gone.
pub const DESIGN_GONE: &str = "that design isn't saved any more";

/// The current time in Unix milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Encodes a stored picture: a lossless WebP at libwebp's quickest setting, which takes about the
/// time the PNG it replaced did (a fraction of a second for a 2048 px photo) and is about a
/// third smaller. The thorough setting a pack's pictures get would take many seconds a picture,
/// too long for an import.
pub(crate) fn encode_stored_picture(img: &RgbaImage) -> Vec<u8> {
    raster::encode_webp_lossless_quick(img)
}

fn save_error(e: std::io::Error) -> String {
    format!("couldn't save that skin: {e}")
}

fn save_many_error(e: std::io::Error) -> String {
    format!("couldn't save those skins: {e}")
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

/// A skin [`Store::add_many`] saved, or found saved already.
#[derive(Clone, Debug)]
pub struct Added {
    pub entry: SavedSkin,
    /// Its gallery thumbnail, as a PNG.
    pub thumbnail_png: Vec<u8>,
    /// False when it was saved before, by an earlier call or earlier in the same one.
    pub fresh: bool,
}

/// What [`Store::add_many`] does with one of the skins it was given.
enum Step {
    /// Nothing: it is saved already, with this entry and thumbnail.
    Saved(Box<SavedSkin>, Vec<u8>),
    /// Nothing: it has the same id as the skin at this earlier position.
    Repeat(usize),
    /// Write it: its encoded picture and thumbnail.
    New(Vec<u8>, Vec<u8>),
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
    ///
    /// Then it clears away what a crash or an unfinished write left behind: pictures, thumbnails
    /// and designs no entry names, and temp files ([`remove_leftovers`]). Only when the index was
    /// read whole, or there is none, and no unreadable index was ever set aside here: otherwise a
    /// file the index doesn't name may belong to a skin it has lost, and is kept.
    pub fn open(dir: PathBuf) -> Store {
        let (skins, whole) = read_index(&dir);
        if whole && !has_set_aside(&dir) {
            remove_leftovers(&dir, &skins, LEFTOVER_AGE);
        }
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
        self.add_with(new, image, None)
    }

    /// Saves a design from the composer the way [`Store::add`] saves a skin, with `design`, the
    /// document it was made from, written first, beside its picture.
    pub fn add_design(
        &self,
        new: NewSkin,
        image: &SkinImage,
        design: &[u8],
    ) -> Result<(SavedSkin, bool), String> {
        self.add_with(new, image, Some(design))
    }

    fn add_with(
        &self,
        new: NewSkin,
        image: &SkinImage,
        design: Option<&[u8]>,
    ) -> Result<(SavedSkin, bool), String> {
        let stem = stem(&new.id)
            .ok_or_else(|| "that skin id isn't one FolderSkin made".to_string())?
            .to_string();
        if let Some(existing) = self.get(&new.id) {
            return Ok((existing, false));
        }
        let image = image.bounded();
        let picture = encode_stored_picture(image.rgba());
        let thumb = image.preview_png(THUMB_SIZE);

        let mut index = self.lock();
        if let Some(existing) = index.iter().find(|s| s.id == new.id) {
            return Ok((existing.clone(), false));
        }
        let written = self
            .write_files(&stem, design, &picture, &thumb)
            .map_err(save_error)?;

        // Strictly increasing, so "newest first" is well defined even within one millisecond.
        let latest = index.iter().map(|s| s.created_at).max().unwrap_or(0);
        let entry = new.entry(&image, now_ms().max(latest + 1));
        index.push(entry.clone());
        if let Err(e) = self.write_index(&index) {
            index.pop();
            remove_written(&written);
            return Err(save_error(e));
        }
        Ok((entry, true))
    }

    /// Saves a design over the one it was made from, `old_id`, which must be a saved design.
    ///
    /// The new design takes the old one's place: its files are written first, then one index
    /// write swaps the two entries, the new one keeping the old one's `created_at` so it stays
    /// where it was in the library, and then the old one's files are removed. If anything fails,
    /// the files this call wrote are removed again and the index is left as it was.
    ///
    /// Returns the entry and whether it is new. A design that hasn't changed (the same id) comes
    /// back as it was saved, with only the name and tags it was given this time. One that is now
    /// the same as another saved design removes the old one, and the other comes back unchanged.
    pub fn replace_design(
        &self,
        old_id: &str,
        new: NewSkin,
        image: &SkinImage,
        design: &[u8],
    ) -> Result<(SavedSkin, bool), String> {
        let new_stem = stem(&new.id)
            .ok_or_else(|| "that skin id isn't one FolderSkin made".to_string())?
            .to_string();
        let old_stem = stem(old_id).ok_or_else(|| DESIGN_GONE.to_string())?;
        if new.id == old_id {
            return self.rename_design(old_id, &new);
        }
        // Checked before the slow part, so a refusal costs nothing, and again under the lock.
        design_at(&self.lock(), old_id)?;
        let image = image.bounded();
        let picture = encode_stored_picture(image.rgba());
        let thumb = image.preview_png(THUMB_SIZE);

        let mut index = self.lock();
        let pos = design_at(&index, old_id)?;
        if let Some(existing) = index.iter().find(|s| s.id == new.id).cloned() {
            let old = index.remove(pos);
            if let Err(e) = self.write_index(&index) {
                index.insert(pos, old);
                return Err(save_error(e));
            }
            self.remove_files(old_stem);
            return Ok((existing, false));
        }
        let written = self
            .write_files(&new_stem, Some(design), &picture, &thumb)
            .map_err(save_error)?;
        let entry = new.entry(&image, index[pos].created_at);
        let old = std::mem::replace(&mut index[pos], entry.clone());
        if let Err(e) = self.write_index(&index) {
            index[pos] = old;
            remove_written(&written);
            return Err(save_error(e));
        }
        // Under the lock, so the old design can't be saved again before its files are gone.
        self.remove_files(old_stem);
        Ok((entry, true))
    }

    /// A saved design whose picture and document haven't changed, given the name and tags `new`
    /// has. Only the index is written, and only when they differ.
    fn rename_design(&self, id: &str, new: &NewSkin) -> Result<(SavedSkin, bool), String> {
        let mut index = self.lock();
        let pos = design_at(&index, id)?;
        let before = index[pos].clone();
        if let Some(name) = clean_name(&new.name) {
            index[pos].name = name;
        }
        index[pos].tags = pack::clean_tags(&new.tags, pack::MAX_TAGS);
        if index[pos] != before {
            if let Err(e) = self.write_index(&index) {
                index[pos] = before;
                return Err(format!("couldn't save that skin's changes: {e}"));
            }
        }
        Ok((index[pos].clone(), false))
    }

    /// The document a saved design was made from. `Ok(None)` when no saved skin has this id, or
    /// the skin has none because it wasn't made in the composer.
    pub fn design(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        let (Some(entry), Some(stem)) = (self.get(id), stem(id)) else {
            return Ok(None);
        };
        match std::fs::read(self.dir.join(design_file(stem))) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("couldn't read the design for {}: {e}", entry.name)),
        }
    }

    /// Writes a skin's files, its design first when it has one, and returns their paths. When
    /// one can't be written, the ones written before it are removed again.
    fn write_files(
        &self,
        stem: &str,
        design: Option<&[u8]>,
        picture: &[u8],
        thumb: &[u8],
    ) -> std::io::Result<Vec<PathBuf>> {
        let files = design
            .map(|bytes| (design_file(stem), bytes))
            .into_iter()
            .chain([(image_file(stem), picture), (thumb_file(stem), thumb)]);
        let mut written = Vec::with_capacity(3);
        let wrote = std::fs::create_dir_all(&self.dir).and_then(|()| {
            for (file, bytes) in files {
                let path = self.dir.join(file);
                write_atomic(&path, bytes)?;
                written.push(path);
            }
            Ok(())
        });
        if let Err(e) = wrote {
            remove_written(&written);
            return Err(e);
        }
        Ok(written)
    }

    /// Saves several skins at once, all of them or none: every new picture and thumbnail is
    /// encoded first, on all cores and outside the lock, then all of their files are written, then
    /// the index, once. If anything fails, the files this call wrote are removed again and the
    /// index is left exactly as it was.
    ///
    /// Returns one [`Added`] per skin, in the order given. A skin whose id is saved already, or
    /// that came up earlier in `skins`, comes back as it was saved, and nothing is written for it.
    /// The first new skin gets the newest `created_at` and each after it an older one, all newer
    /// than any skin saved before, so a list sorted newest first shows them in the order given.
    /// `encoded` is called as each new skin's files are encoded, from whichever thread did it.
    pub fn add_many(
        &self,
        skins: Vec<(NewSkin, SkinImage)>,
        encoded: &(dyn Fn() + Sync),
    ) -> Result<Vec<Added>, String> {
        let stems = skins
            .iter()
            .map(|(new, _)| stem(&new.id))
            .collect::<Option<Vec<&str>>>()
            .ok_or_else(|| "that skin id isn't one FolderSkin made".to_string())?;

        // Which are saved already or repeated. A saved one's thumbnail is read now, so that
        // nothing can fail once new files are written.
        let mut first_at: HashMap<&str, usize> = HashMap::new();
        let mut known: Vec<Option<Step>> = Vec::with_capacity(skins.len());
        for (i, (new, _)) in skins.iter().enumerate() {
            if let Some(&j) = first_at.get(new.id.as_str()) {
                known.push(Some(Step::Repeat(j)));
                continue;
            }
            first_at.insert(&new.id, i);
            known.push(match self.get(&new.id) {
                Some(entry) => {
                    let thumbnail_png = self.thumbnail_png(&entry)?;
                    Some(Step::Saved(Box::new(entry), thumbnail_png))
                }
                None => None,
            });
        }

        // The slow part, on all cores and outside the lock.
        let fresh: Vec<usize> = (0..skins.len()).filter(|&i| known[i].is_none()).collect();
        let mut files = crate::state::parallel_map(&fresh, |&i| {
            let image = skins[i].1.bounded();
            let files = (
                encode_stored_picture(image.rgba()),
                image.preview_png(THUMB_SIZE),
            );
            encoded();
            files
        })
        .into_iter();
        let mut steps: Vec<Step> = known
            .into_iter()
            .map(|step| {
                step.unwrap_or_else(|| {
                    let (picture, thumb) = files.next().expect("every new skin was encoded");
                    Step::New(picture, thumb)
                })
            })
            .collect();

        let mut index = self.lock();
        for (step, (new, _)) in steps.iter_mut().zip(&skins) {
            if let Step::New(_, thumb) = step {
                // Saved by another call since this one looked. The same id is the same picture,
                // so the thumbnail just drawn is its thumbnail too.
                if let Some(entry) = index.iter().find(|s| s.id == new.id) {
                    *step = Step::Saved(Box::new(entry.clone()), std::mem::take(thumb));
                }
            }
        }
        let adding: Vec<usize> = (0..steps.len())
            .filter(|&i| matches!(steps[i], Step::New(..)))
            .collect();
        if !adding.is_empty() {
            let mut written = Vec::with_capacity(adding.len() * 2);
            let wrote = (|| -> std::io::Result<()> {
                std::fs::create_dir_all(&self.dir)?;
                for &i in &adding {
                    let Step::New(picture, thumb) = &steps[i] else {
                        continue;
                    };
                    for (file, bytes) in [
                        (image_file(stems[i]), picture),
                        (thumb_file(stems[i]), thumb),
                    ] {
                        let path = self.dir.join(file);
                        write_atomic(&path, bytes)?;
                        written.push(path);
                    }
                }
                Ok(())
            })();
            if let Err(e) = wrote {
                remove_written(&written);
                return Err(save_many_error(e));
            }

            // Strictly decreasing from the newest, and all above the newest saved before.
            let latest = index.iter().map(|s| s.created_at).max().unwrap_or(0);
            let newest = now_ms().max(latest + adding.len() as u64);
            let before = index.len();
            for (k, &i) in adding.iter().enumerate() {
                let (new, image) = &skins[i];
                index.push(new.clone().entry(image, newest - k as u64));
            }
            if let Err(e) = self.write_index(&index) {
                index.truncate(before);
                remove_written(&written);
                return Err(save_many_error(e));
            }
        }
        let new_entries: Vec<SavedSkin> = index[index.len() - adding.len()..].to_vec();
        drop(index);
        let mut new_entries = new_entries.into_iter();

        let mut added: Vec<Added> = Vec::with_capacity(steps.len());
        for step in steps {
            added.push(match step {
                Step::Saved(entry, thumbnail_png) => Added {
                    entry: *entry,
                    thumbnail_png,
                    fresh: false,
                },
                Step::Repeat(j) => Added {
                    fresh: false,
                    ..added[j].clone()
                },
                Step::New(_, thumbnail_png) => Added {
                    entry: new_entries.next().expect("an entry for every skin written"),
                    thumbnail_png,
                    fresh: true,
                },
            });
        }
        Ok(added)
    }

    /// Reads a saved skin's picture back. `Ok(None)` when no saved skin has this id.
    pub fn load(&self, id: &str) -> Result<Option<SkinImage>, String> {
        let (Some(entry), Some(stem)) = (self.get(id), stem(id)) else {
            return Ok(None);
        };
        let bytes = picture_path(&self.dir, stem)
            .and_then(|path| std::fs::read(path).ok())
            .ok_or_else(|| format!("the picture for {} is missing", entry.name))?;
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

    /// Removes a saved skin: its index entry first, then its picture, thumbnails and design. An
    /// id that is well formed but not saved is already gone, which is not an error.
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

    /// The folder the skins are saved in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Gives a saved skin a new name and tags, cleaned by [`clean_name`] and
    /// [`pack::clean_tags`], and returns its entry. Only the index changes: the picture and
    /// thumbnails are named after the id, not the name.
    pub fn edit(&self, id: &str, name: &str, tags: &[String]) -> Result<SavedSkin, String> {
        let name = clean_name(name).ok_or_else(|| "a skin needs a name".to_string())?;
        let tags = pack::clean_tags(tags, pack::MAX_TAGS);
        let mut index = self.lock();
        let pos = index
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| "that skin isn't saved any more".to_string())?;
        let before = index[pos].clone();
        index[pos].name = name;
        index[pos].tags = tags;
        if let Err(e) = self.write_index(&index) {
            index[pos] = before;
            return Err(format!("couldn't save that skin's changes: {e}"));
        }
        Ok(index[pos].clone())
    }

    /// Moves the community skins of every pack `moved` names from its old id to the id it has
    /// now, all in one write of the index, and says how many moved. Nothing is written when none
    /// did, and a failed write leaves every one as it was.
    pub fn move_packs(&self, moved: &BTreeMap<String, String>) -> Result<usize, String> {
        let mut index = self.lock();
        let moving: Vec<(usize, String)> = index
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| moved_to(entry, moved).map(|now| (i, now)))
            .collect();
        if moving.is_empty() {
            return Ok(0);
        }
        let was: Vec<Option<String>> = moving
            .iter()
            .map(|(i, now)| index[*i].pack.replace(now.clone()))
            .collect();
        if let Err(e) = self.write_index(&index) {
            for ((i, _), pack) in moving.iter().zip(was) {
                index[*i].pack = pack;
            }
            return Err(format!("couldn't save the packs' new ids: {e}"));
        }
        Ok(moving.len())
    }

    /// Deletes a skin's picture, its design and every thumbnail it has had. Leftovers are only
    /// logged: the index no longer names them, so they cannot come back.
    fn remove_files(&self, stem: &str) {
        let thumb_prefix = format!("{stem}.thumb");
        let mut doomed = vec![
            self.dir.join(image_file(stem)),
            self.dir.join(old_image_file(stem)),
            self.dir.join(design_file(stem)),
        ];
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

/// The id `entry`'s pack has now, when it is a community skin whose pack `moved` says moved.
pub fn moved_to(entry: &SavedSkin, moved: &BTreeMap<String, String>) -> Option<String> {
    let pack = entry.pack.as_deref()?;
    let now = moved.get(pack)?;
    (entry.source == SkinSource::Community && now != pack).then(|| now.clone())
}

/// Reads the index in `dir`, and says whether it was read whole. Entries that are damaged,
/// repeated or whose picture is gone are dropped and logged; an index that cannot be read at all
/// is set aside. It was not read whole when it was set aside or an entry was dropped as damaged
/// or with a bad id, since those may name files nothing else does; a missing index is whole.
fn read_index(dir: &Path) -> (Vec<SavedSkin>, bool) {
    let path = dir.join(INDEX_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), true),
        Err(e) => {
            set_aside(&path, &e.to_string());
            return (Vec::new(), false);
        }
    };
    let index: IndexIn = match serde_json::from_slice(&bytes) {
        Ok(index) => index,
        Err(e) => {
            set_aside(&path, &e.to_string());
            return (Vec::new(), false);
        }
    };
    if index.version != INDEX_VERSION {
        set_aside(&path, &format!("index version {}", index.version));
        return (Vec::new(), false);
    }
    let mut seen = HashSet::new();
    let mut whole = true;
    let skins = index
        .skins
        .into_iter()
        .filter_map(|value| {
            let skin: SavedSkin = match serde_json::from_value(value) {
                Ok(skin) => skin,
                Err(e) => {
                    eprintln!("folderskin: dropping a damaged saved-skin entry: {e}");
                    whole = false;
                    return None;
                }
            };
            let Some(stem) = stem(&skin.id) else {
                eprintln!(
                    "folderskin: dropping saved skin with a bad id {:?}",
                    skin.id
                );
                whole = false;
                return None;
            };
            if picture_path(dir, stem).is_none() {
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
        .collect();
    (skins, whole)
}

/// Removes the files a crash or an unfinished write left in `dir`, each logged: a picture,
/// thumbnail or design of a skin `skins` doesn't list, and a temp file of [`write_atomic`]. Only
/// files with exactly those shapes of name ([`is_leftover`]), and only ones last changed at least
/// `min_age` ago. Nothing else in the folder is touched.
fn remove_leftovers(dir: &Path, skins: &[SavedSkin], min_age: Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let listed: HashSet<&str> = skins.iter().filter_map(|s| stem(&s.id)).collect();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_str().is_some_and(|n| is_leftover(n, &listed)) {
            continue;
        }
        // Not a link or a folder, and not written in the last `min_age`.
        let settled = entry.file_type().is_ok_and(|t| t.is_file())
            && entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|changed| changed.elapsed().ok())
                .is_some_and(|age| age >= min_age);
        if !settled {
            continue;
        }
        let path = entry.path();
        match std::fs::remove_file(&path) {
            Ok(()) => eprintln!(
                "folderskin: removed {}, which an earlier run left unfinished",
                path.display()
            ),
            Err(e) => eprintln!("folderskin: couldn't remove {}: {e}", path.display()),
        }
    }
}

/// Whether `name` is a file the store writes that no skin in `listed` owns: a `<stem>.webp` or
/// `<stem>.png` picture, a `<stem>.thumb….png` thumbnail or a `<stem>.design.json` design whose
/// stem isn't listed, or a temp file of [`write_atomic`] for any file the store writes,
/// `.<name>.folderskin-<pid>-<seq>.tmp`.
fn is_leftover(name: &str, listed: &HashSet<&str>) -> bool {
    if let Some(temp) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) {
        let Some((target, tag)) = temp.rsplit_once(".folderskin-") else {
            return false;
        };
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        let tagged = tag
            .split_once('-')
            .is_some_and(|(pid, seq)| digits(pid) && digits(seq));
        return tagged && (target == INDEX_FILE || skin_file_stem(target).is_some());
    }
    skin_file_stem(name).is_some_and(|stem| !listed.contains(stem))
}

/// The stem of `<stem>.webp`, `<stem>.png`, `<stem>.thumb….png` or `<stem>.design.json`, the
/// names a skin's files have.
fn skin_file_stem(name: &str) -> Option<&str> {
    let stem = name.get(..12).filter(|s| is_stem(s))?;
    let rest = &name[12..];
    let picture = rest == ".webp" || rest == ".png";
    let thumbnail = rest.starts_with(".thumb") && rest.ends_with(".png");
    let design = rest == DESIGN_SUFFIX;
    (picture || thumbnail || design).then_some(stem)
}

/// Where the saved design `id` is in `index`, or why a design can't be saved over it.
fn design_at(index: &[SavedSkin], id: &str) -> Result<usize, String> {
    let pos = index
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| DESIGN_GONE.to_string())?;
    if index[pos].source != SkinSource::Composer {
        return Err(NOT_A_DESIGN.into());
    }
    Ok(pos)
}

/// Whether an index that could not be read was ever set aside in `dir`. Its skins' files are
/// still there, and nothing else says which they are.
fn has_set_aside(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with(SET_ASIDE_PREFIX) && name.ends_with(".json")
        })
    })
}

/// Removes the files a failed save wrote.
fn remove_written(paths: &[PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}

/// Renames an unreadable index out of the way, so the next save starts a fresh one without
/// destroying whatever the old one held.
fn set_aside(path: &Path, why: &str) {
    let aside = path.with_file_name(format!("{SET_ASIDE_PREFIX}{}.json", now_ms()));
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    const HOUR: Duration = Duration::from_secs(60 * 60);

    /// The names in `dir`, sorted.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Makes the file at `path` look `age` old.
    fn age(path: &Path, age: Duration) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - age)
            .unwrap();
    }

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
            tags: Vec::new(),
            pack: None,
            pack_name: None,
            author: None,
            license: None,
            pack_hash: None,
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
    fn community_skins_follow_their_pack_to_its_new_id_once_and_for_good() {
        let dir = temp_dir("moved");
        let store = Store::open(dir.clone());
        let skin = |bytes: &[u8], pack: &str, source: SkinSource| {
            let mut new = new_skin(&skin_id(bytes), "Skin", source);
            new.pack = Some(pack.into());
            new.pack_hash = Some("0123456789abcdef".into());
            store.add(new, &folder()).unwrap().0
        };
        let a = skin(b"a", "classic-art", SkinSource::Community);
        let b = skin(b"b", "classic-art", SkinSource::Community);
        let c = skin(b"c", "colours", SkinSource::Community);
        // Only a community skin belongs to a community pack, whatever else carries the id.
        let d = skin(b"d", "classic-art", SkinSource::Import);

        let moved = BTreeMap::from([
            ("classic-art".to_string(), "classic-art-k7q2mx".to_string()),
            ("greek-art".to_string(), "greek-art-a2b3c4".to_string()),
        ]);
        assert_eq!(store.move_packs(&moved).unwrap(), 2);

        let reopened = Store::open(dir.clone());
        let pack = |entry: &SavedSkin| reopened.get(&entry.id).unwrap().pack.unwrap();
        assert_eq!(pack(&a), "classic-art-k7q2mx");
        assert_eq!(pack(&b), "classic-art-k7q2mx");
        assert_eq!(pack(&c), "colours");
        assert_eq!(pack(&d), "classic-art");
        // The same skins, added at the same version, under the new id.
        let moved_a = reopened.get(&a.id).unwrap();
        assert_eq!(moved_a.pack_hash.as_deref(), Some("0123456789abcdef"));
        assert_eq!(
            (moved_a.created_at, moved_a.name.as_str()),
            (a.created_at, "Skin")
        );

        // With nothing left under an old id, nothing moves and nothing is written.
        std::fs::remove_file(dir.join(INDEX_FILE)).unwrap();
        assert_eq!(store.move_packs(&moved).unwrap(), 0);
        assert!(!dir.join(INDEX_FILE).exists());
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
    fn editing_changes_the_name_and_tags_and_survives_a_restart() {
        let dir = temp_dir("edit");
        let store = Store::open(dir.clone());
        let id = skin_id(b"edit");
        let mut new = new_skin(&id, "A night sky", SkinSource::Community);
        new.tags = vec!["Airbrush".into(), "airbrush".into()];
        new.pack = Some("night-skies".into());
        let (entry, _) = store.add(new, &artwork([9, 9, 9], (0.5, 0.5))).unwrap();
        assert_eq!(entry.tags, ["airbrush"], "tags are cleaned on the way in");

        let tags = vec!["Beach".into(), " summer  2026 ".into(), "beach".into()];
        let edited = store.edit(&id, "  Beach   trip\n2026 ", &tags).unwrap();
        assert_eq!(edited.name, "Beach trip 2026");
        assert_eq!(edited.tags, ["beach", "summer 2026"]);
        assert_eq!(
            edited.pack.as_deref(),
            Some("night-skies"),
            "nothing else changes"
        );
        assert!(
            store.edit(&id, " \t ", &[]).is_err(),
            "a blank name is refused"
        );
        assert!(
            store.edit(&skin_id(b"unknown"), "x", &[]).is_err(),
            "so is a skin that isn't saved"
        );

        let reopened = Store::open(dir.clone());
        let saved = reopened.get(&id).unwrap();
        assert_eq!(saved.name, "Beach trip 2026");
        assert_eq!(saved.tags, ["beach", "summer 2026"]);
        assert_eq!(saved.source, SkinSource::Community);
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

    #[test]
    fn a_batch_is_listed_in_its_own_order_above_what_was_saved_before() {
        let dir = temp_dir("batch-order");
        let store = Store::open(dir.clone());
        let (earlier, _) = store
            .add(
                new_skin(&skin_id(b"earlier"), "Earlier", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        let ids: Vec<String> = ["mona lisa", "starry night", "great wave"]
            .iter()
            .map(|name| skin_id(name.as_bytes()))
            .collect();
        let batch = ids
            .iter()
            .map(|id| {
                let art = artwork([90, 60, 30], (0.5, 0.5));
                (new_skin(id, "Painting", SkinSource::Community), art)
            })
            .collect();
        let encoded = AtomicUsize::new(0);
        let added = store
            .add_many(batch, &|| {
                encoded.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        assert_eq!(encoded.load(Ordering::Relaxed), 3);
        assert!(added.iter().all(|a| a.fresh));
        let returned: Vec<&String> = added.iter().map(|a| &a.entry.id).collect();
        assert_eq!(
            returned,
            ids.iter().collect::<Vec<_>>(),
            "in the order given"
        );
        assert!(added
            .windows(2)
            .all(|w| w[0].entry.created_at > w[1].entry.created_at));
        assert!(added[2].entry.created_at > earlier.created_at);
        for a in &added {
            let written = std::fs::read(dir.join(thumb_file(stem(&a.entry.id).unwrap()))).unwrap();
            assert_eq!(a.thumbnail_png, written, "the thumbnail that was saved");
        }

        let newest_first: Vec<String> = Store::open(dir.clone())
            .list()
            .into_iter()
            .map(|s| s.id)
            .collect();
        let mut expected = ids.clone();
        expected.push(earlier.id.clone());
        assert_eq!(
            newest_first, expected,
            "the batch's own order, after a restart too"
        );

        let (later, _) = store
            .add(
                new_skin(&skin_id(b"later"), "Later", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        assert!(later.created_at > added[0].entry.created_at);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_batch_returns_saved_skins_as_they_were_and_saves_a_repeat_once() {
        let dir = temp_dir("batch-saved");
        let store = Store::open(dir.clone());
        let old = skin_id(b"old");
        let (saved, _) = store
            .add(new_skin(&old, "Old name", SkinSource::Import), &folder())
            .unwrap();
        let new = skin_id(b"new");
        let batch = vec![
            (new_skin(&new, "New", SkinSource::Community), folder()),
            (
                new_skin(&old, "Another name", SkinSource::Community),
                folder(),
            ),
            (new_skin(&new, "New again", SkinSource::Community), folder()),
        ];
        let encoded = AtomicUsize::new(0);
        let added = store
            .add_many(batch, &|| {
                encoded.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        assert_eq!(encoded.load(Ordering::Relaxed), 1, "only the new picture");
        assert!(added[0].fresh);
        assert_eq!(added[0].entry.name, "New");
        assert!(!added[1].fresh);
        assert_eq!(added[1].entry, saved, "a saved skin comes back unchanged");
        assert_eq!(added[1].thumbnail_png, store.thumbnail_png(&saved).unwrap());
        assert!(!added[2].fresh);
        assert_eq!(added[2].entry, added[0].entry, "a repeat is the first one");
        assert_eq!(store.list().len(), 2);

        // Nothing new: nothing is encoded, and the index isn't written again.
        let index = std::fs::read(dir.join(INDEX_FILE)).unwrap();
        let again = store
            .add_many(
                vec![(new_skin(&old, "X", SkinSource::Import), folder())],
                &|| panic!("encoded a saved skin"),
            )
            .unwrap();
        assert_eq!(again[0].entry, saved);
        assert_eq!(std::fs::read(dir.join(INDEX_FILE)).unwrap(), index);
        assert!(store.add_many(Vec::new(), &|| {}).unwrap().is_empty());

        // An id FolderSkin didn't make stops the whole batch before anything is encoded.
        let fine = skin_id(b"fine");
        let err = store
            .add_many(
                vec![
                    (new_skin(&fine, "Fine", SkinSource::Import), folder()),
                    (
                        new_skin("user:../../etc", "Bad", SkinSource::Import),
                        folder(),
                    ),
                ],
                &|| panic!("encoded before every id was checked"),
            )
            .unwrap_err();
        assert!(err.contains("isn't one FolderSkin made"), "{err}");
        assert!(store.get(&fine).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_batch_that_cannot_be_saved_leaves_the_store_as_it_was() {
        let dir = temp_dir("batch-fail");
        let store = Store::open(dir.clone());
        let (kept, _) = store
            .add(
                new_skin(&skin_id(b"kept"), "Kept", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        let index = std::fs::read(dir.join(INDEX_FILE)).unwrap();
        let ids = [skin_id(b"a"), skin_id(b"b"), skin_id(b"c")];
        let batch = || -> Vec<(NewSkin, SkinImage)> {
            ids.iter()
                .map(|id| (new_skin(id, "Pack skin", SkinSource::Community), folder()))
                .collect()
        };

        // A folder where the second picture goes: its write fails after the first skin's
        // picture and thumbnail are written.
        let blocker = image_file(stem(&ids[1]).unwrap());
        std::fs::create_dir(dir.join(&blocker)).unwrap();
        let mut expected = names_in(&dir);
        let err = store.add_many(batch(), &|| {}).unwrap_err();
        assert!(err.starts_with("couldn't save those skins"), "{err}");
        assert_eq!(store.list(), vec![kept.clone()], "nothing was added");
        assert_eq!(std::fs::read(dir.join(INDEX_FILE)).unwrap(), index);
        assert_eq!(
            names_in(&dir),
            expected,
            "the files it wrote are gone again"
        );
        std::fs::remove_dir(dir.join(&blocker)).unwrap();

        // An index that can't be written: every file goes again, and the list is unchanged.
        std::fs::remove_file(dir.join(INDEX_FILE)).unwrap();
        std::fs::create_dir(dir.join(INDEX_FILE)).unwrap();
        expected = names_in(&dir);
        let err = store.add_many(batch(), &|| {}).unwrap_err();
        assert!(err.starts_with("couldn't save those skins"), "{err}");
        assert_eq!(store.list(), vec![kept.clone()]);
        assert_eq!(names_in(&dir), expected);
        std::fs::remove_dir(dir.join(INDEX_FILE)).unwrap();
        std::fs::write(dir.join(INDEX_FILE), &index).unwrap();

        assert_eq!(Store::open(dir.clone()).list(), vec![kept]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn leftovers_of_a_crash_are_removed_on_open_and_nothing_else() {
        let dir = temp_dir("leftovers");
        let store = Store::open(dir.clone());
        let (kept, _) = store
            .add(
                new_skin(&skin_id(b"kept"), "Kept", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        let (lost, _) = store
            .add(
                new_skin(&skin_id(b"lost"), "Lost", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        drop(store);
        // A picture that went missing drops its entry cleanly and leaves its thumbnail over.
        let lost_stem = stem(&lost.id).unwrap();
        std::fs::remove_file(dir.join(image_file(lost_stem))).unwrap();

        let leftovers = [
            "0123456789ab.png".to_string(),
            "0123456789ab.webp".to_string(),
            "0123456789ab.thumb-v2.png".to_string(),
            "0123456789ab.thumb-v1.png".to_string(),
            thumb_file(lost_stem),
            ".skins.json.folderskin-4242-7.tmp".to_string(),
            ".0123456789ab.thumb-v2.png.folderskin-1-0.tmp".to_string(),
        ];
        let others = [
            "notes.txt",
            ".DS_Store",
            // Upper case, on a stem no leftover has: some disks ignore case in names.
            "ABCDEF012345.png",
            "0123456789a.png",
            "0123456789abc.png",
            "0123456789ab.jpg",
            "0123456789ab.png.bak",
            "0123456789ab.webp.bak",
            "skins.json.folderskin-1-2.tmp",
            ".notes.txt.folderskin-1-2.tmp",
            ".skins.json.folderskin-x-2.tmp",
            ".skins.json.folderskin-12.tmp",
        ];
        for name in leftovers.iter().map(String::as_str).chain(others) {
            if !dir.join(name).exists() {
                std::fs::write(dir.join(name), b"x").unwrap();
            }
        }
        // Everything is an hour old, the kept skin's own files included...
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            age(&entry.path(), HOUR);
        }
        // ...but this one, which another FolderSkin could be saving right now.
        std::fs::write(dir.join("fedcba987654.png"), b"x").unwrap();
        // A folder named like a picture is never a file FolderSkin wrote.
        std::fs::create_dir(dir.join("abcdefabcdef.png")).unwrap();

        let store = Store::open(dir.clone());
        assert_eq!(store.list(), vec![kept.clone()]);
        let names = names_in(&dir);
        for gone in &leftovers {
            assert!(!names.contains(gone), "{gone} is still there: {names:?}");
        }
        let kept_stem = stem(&kept.id).unwrap();
        let stays = others
            .iter()
            .map(|n| n.to_string())
            .chain([INDEX_FILE, "fedcba987654.png", "abcdefabcdef.png"].map(String::from))
            .chain([image_file(kept_stem), thumb_file(kept_stem)]);
        for name in stays {
            assert!(names.contains(&name), "{name} was removed");
        }
        assert!(store.load(&kept.id).unwrap().is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_library_saved_as_png_by_an_older_version_still_loads() {
        // A library as FolderSkin 0.1.6 left it: an index and a PNG for each skin.
        let dir = temp_dir("old-png");
        let old = RgbaImage::from_fn(300, 200, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 7, 255])
        });
        let id = skin_id(b"an old one");
        let old_stem = stem(&id).unwrap();
        std::fs::write(dir.join(old_image_file(old_stem)), raster::encode_png(&old)).unwrap();
        let index = serde_json::json!({"version": 1, "skins": [{
            "id": id, "name": "Old", "kind": "artwork", "focus": [0.5, 0.5],
            "source": "import", "created_at": 1
        }]});
        std::fs::write(dir.join(INDEX_FILE), serde_json::to_vec(&index).unwrap()).unwrap();
        age(&dir.join(old_image_file(old_stem)), HOUR);

        let store = Store::open(dir.clone());
        assert_eq!(store.list().len(), 1, "it's listed, not dropped as missing");
        let SkinImage::Artwork(art) = store.load(&id).unwrap().unwrap() else {
            panic!("an artwork skin came back as a folder");
        };
        assert_eq!(art.rgba, old, "every pixel of the PNG");
        let entry = store.get(&id).unwrap();
        assert!(store.thumbnail_png(&entry).unwrap().starts_with(b"\x89PNG"));

        // A new picture is written as a lossless WebP, beside the old PNG, which stays a PNG.
        let (new, _) = store
            .add(
                new_skin(&skin_id(b"a new one"), "New", SkinSource::Import),
                &artwork([40, 90, 200], (0.5, 0.5)),
            )
            .unwrap();
        let new_stem = stem(&new.id).unwrap();
        let written = std::fs::read(dir.join(image_file(new_stem))).unwrap();
        assert!(written.starts_with(b"RIFF") && pack::is_lossless_picture(&written));
        assert!(!dir.join(old_image_file(new_stem)).exists());
        assert!(
            dir.join(old_image_file(old_stem)).is_file(),
            "left as it was"
        );

        // Both come back after a restart, and deleting the old one removes its PNG.
        let store = Store::open(dir.clone());
        assert_eq!(store.list().len(), 2);
        assert!(store.load(&new.id).unwrap().is_some());
        store.delete(&id).unwrap();
        assert!(!dir.join(old_image_file(old_stem)).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_stored_picture_is_lossless() {
        let dir = temp_dir("lossless");
        let store = Store::open(dir.clone());
        // A cut-out folder: its alpha, and every colour that shows, come back exactly.
        let cut = RgbaImage::from_fn(640, 600, |x, y| {
            let alpha = if x < 60 { 0 } else { ((x + y) % 256) as u8 };
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 99, alpha])
        });
        let (entry, _) = store
            .add(
                new_skin(&skin_id(b"cut"), "Cut", SkinSource::Import),
                &SkinImage::Folder(Arc::new(cut.clone())),
            )
            .unwrap();
        let SkinImage::Folder(back) = store.load(&entry.id).unwrap().unwrap() else {
            panic!("a folder skin came back as artwork");
        };
        for (got, want) in back.pixels().zip(cut.pixels()) {
            assert_eq!(got.0[3], want.0[3], "alpha");
            if want.0[3] > 0 {
                assert_eq!(got, want, "every colour that shows");
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn leftovers_are_kept_while_a_lost_entry_could_name_them() {
        let orphan = "0123456789ab.png";
        let prepare = |test: &str| {
            let dir = temp_dir(test);
            std::fs::write(dir.join(orphan), b"x").unwrap();
            age(&dir.join(orphan), HOUR);
            dir
        };

        // An entry dropped as damaged, or with an id that names no file, may have named it.
        for broken in [
            serde_json::json!({"id": "user:0123456789ab", "kind": "sticker"}),
            serde_json::json!({"id": "user:../0123456789ab", "name": "X", "kind": "folder",
                "source": "import", "created_at": 1}),
        ] {
            let dir = prepare("leftovers-damaged");
            let index = serde_json::json!({"version": 1, "skins": [broken]});
            std::fs::write(dir.join(INDEX_FILE), serde_json::to_vec(&index).unwrap()).unwrap();
            assert!(Store::open(dir.clone()).list().is_empty());
            assert!(dir.join(orphan).exists(), "{broken}");
            std::fs::remove_dir_all(&dir).unwrap();
        }

        // So may an index set aside, whether on this launch or an earlier one.
        let dir = prepare("leftovers-set-aside");
        std::fs::write(dir.join("skins-unreadable-1790000000000.json"), b"{").unwrap();
        Store::open(dir.clone());
        assert!(dir.join(orphan).exists());
        std::fs::remove_dir_all(&dir).unwrap();

        let dir = prepare("leftovers-corrupt");
        std::fs::write(dir.join(INDEX_FILE), b"{\"version\": 1, \"skins\": [").unwrap();
        Store::open(dir.clone());
        assert!(dir.join(orphan).exists());
        std::fs::remove_dir_all(&dir).unwrap();

        // With no index at all, nothing can name it, and it goes.
        let dir = prepare("leftovers-no-index");
        Store::open(dir.clone());
        assert!(!dir.join(orphan).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A design document as the composer's webview sends it; the store only keeps the bytes.
    const DESIGN: &[u8] = br#"{"version":1,"layers":[{"kind":"text","text":"Taxes 2026"}]}"#;

    fn design_skin(id: &str, name: &str) -> NewSkin {
        new_skin(id, name, SkinSource::Composer)
    }

    #[test]
    fn a_design_is_saved_beside_its_skin_and_survives_a_restart() {
        let dir = temp_dir("design-roundtrip");
        let store = Store::open(dir.clone());
        let id = skin_id(b"taxes");
        let (entry, fresh) = store
            .add_design(design_skin(&id, "Taxes"), &folder(), DESIGN)
            .unwrap();
        assert!(fresh);
        assert_eq!(entry.source, SkinSource::Composer);
        assert_eq!(entry.kind, SkinKind::Folder);
        let file = dir.join(design_file(stem(&id).unwrap()));
        assert_eq!(std::fs::read(&file).unwrap(), DESIGN);

        // The same design saved again comes back as it was saved, and nothing is written.
        let (again, fresh) = store
            .add_design(design_skin(&id, "Another name"), &folder(), b"{}")
            .unwrap();
        assert!(!fresh);
        assert_eq!(again, entry);
        assert_eq!(std::fs::read(&file).unwrap(), DESIGN);
        drop(store);

        let store = Store::open(dir.clone());
        assert_eq!(store.list(), vec![entry]);
        assert_eq!(store.design(&id).unwrap().as_deref(), Some(DESIGN));
        assert!(matches!(
            store.load(&id).unwrap(),
            Some(SkinImage::Folder(_))
        ));

        // A skin made anywhere else has no design, and neither has one that isn't saved.
        let imported = skin_id(b"imported");
        store
            .add(
                new_skin(&imported, "Imported", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        assert_eq!(store.design(&imported).unwrap(), None);
        assert_eq!(store.design(&skin_id(b"never saved")).unwrap(), None);
        assert_eq!(store.design("user:../skins").unwrap(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deleting_a_design_removes_its_document_too() {
        let dir = temp_dir("design-delete");
        let store = Store::open(dir.clone());
        let id = skin_id(b"gone design");
        store
            .add_design(design_skin(&id, "Gone"), &folder(), DESIGN)
            .unwrap();
        store.delete(&id).unwrap();
        let stem = stem(&id).unwrap();
        let left: Vec<String> = names_in(&dir)
            .into_iter()
            .filter(|n| n.starts_with(stem))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        assert_eq!(store.design(&id).unwrap(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_design_saved_over_its_old_one_takes_its_place() {
        let dir = temp_dir("design-replace");
        let store = Store::open(dir.clone());
        let (old, _) = store
            .add_design(
                design_skin(&skin_id(b"v1"), "Draft"),
                &folder(),
                b"{\"v\":1}",
            )
            .unwrap();
        let (later, _) = store
            .add(
                new_skin(&skin_id(b"later"), "Later", SkinSource::Import),
                &folder(),
            )
            .unwrap();

        let mut new = design_skin(&skin_id(b"v2"), "Final");
        new.tags = vec!["Work".into()];
        let (entry, fresh) = store
            .replace_design(&old.id, new, &folder(), b"{\"v\":2}")
            .unwrap();
        assert!(fresh);
        assert_eq!(
            entry.created_at, old.created_at,
            "it keeps the old one's place"
        );
        assert_eq!(
            (entry.name.as_str(), &entry.tags[..]),
            ("Final", &["work".to_string()][..])
        );
        let newest_first: Vec<String> = store.list().into_iter().map(|s| s.id).collect();
        assert_eq!(newest_first, [later.id.clone(), entry.id.clone()]);
        let old_stem = stem(&old.id).unwrap();
        let left: Vec<String> = names_in(&dir)
            .into_iter()
            .filter(|n| n.starts_with(old_stem))
            .collect();
        assert!(left.is_empty(), "the old design's files are gone: {left:?}");
        assert_eq!(
            store.design(&entry.id).unwrap().as_deref(),
            Some(&b"{\"v\":2}"[..])
        );
        assert_eq!(store.design(&old.id).unwrap(), None);
        assert_eq!(
            Store::open(dir.clone()).list(),
            store.list(),
            "after a restart too"
        );

        // Saved again without a change to the design: only the name and tags it's given change.
        let mut same = design_skin(&entry.id, "  Final   version ");
        same.tags = vec!["work".into(), "Taxes".into()];
        let (renamed, fresh) = store
            .replace_design(&entry.id, same, &folder(), b"{\"v\":2}")
            .unwrap();
        assert!(!fresh);
        assert_eq!(renamed.name, "Final version");
        assert_eq!(renamed.tags, ["work", "taxes"]);
        assert_eq!(renamed.created_at, entry.created_at);
        assert_eq!(Store::open(dir.clone()).get(&entry.id), Some(renamed));

        // Changed into a design saved already: that one stays as it is, and this one goes.
        let (other, _) = store
            .add_design(design_skin(&skin_id(b"other"), "Other"), &folder(), DESIGN)
            .unwrap();
        let (kept, fresh) = store
            .replace_design(
                &entry.id,
                design_skin(&other.id, "Renamed"),
                &folder(),
                DESIGN,
            )
            .unwrap();
        assert!(!fresh);
        assert_eq!(kept, other);
        assert!(store.get(&entry.id).is_none());
        assert_eq!(store.design(&entry.id).unwrap(), None);
        let ids: Vec<String> = Store::open(dir.clone())
            .list()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, [other.id, later.id]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_a_saved_design_can_be_saved_over() {
        let dir = temp_dir("design-refuse");
        let store = Store::open(dir.clone());
        let (photo, _) = store
            .add(
                new_skin(&skin_id(b"photo"), "Photo", SkinSource::Import),
                &folder(),
            )
            .unwrap();
        let before = names_in(&dir);
        let new = || design_skin(&skin_id(b"design"), "Design");

        let refused = |old: &str, new: NewSkin| {
            store
                .replace_design(old, new, &folder(), DESIGN)
                .unwrap_err()
        };
        assert_eq!(refused(&photo.id, new()), NOT_A_DESIGN);
        assert_eq!(
            refused(&photo.id, design_skin(&photo.id, "Photo")),
            NOT_A_DESIGN,
            "even when nothing about it changed"
        );
        for gone in [
            skin_id(b"never saved"),
            "user:../skins".into(),
            "aurora".into(),
        ] {
            assert_eq!(refused(&gone, new()), DESIGN_GONE, "{gone}");
        }
        let err = refused(&photo.id, design_skin("user:../../etc", "Evil"));
        assert!(err.contains("isn't one FolderSkin made"), "{err}");

        assert_eq!(names_in(&dir), before, "nothing was written");
        assert_eq!(store.list(), vec![photo]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_design_that_cannot_be_saved_over_its_old_one_leaves_that_one_be() {
        let dir = temp_dir("design-replace-fail");
        let store = Store::open(dir.clone());
        let (old, _) = store
            .add_design(
                design_skin(&skin_id(b"old design"), "Old"),
                &folder(),
                DESIGN,
            )
            .unwrap();
        let index = std::fs::read(dir.join(INDEX_FILE)).unwrap();
        let new = || design_skin(&skin_id(b"new design"), "New");

        // A folder where the new picture goes: its design is written, then the picture fails.
        let blocker = image_file(stem(&new().id).unwrap());
        std::fs::create_dir(dir.join(&blocker)).unwrap();
        let expected = names_in(&dir);
        let err = store
            .replace_design(&old.id, new(), &folder(), b"{}")
            .unwrap_err();
        assert!(err.starts_with("couldn't save that skin"), "{err}");
        assert_eq!(
            names_in(&dir),
            expected,
            "the design it wrote is gone again"
        );
        std::fs::remove_dir(dir.join(&blocker)).unwrap();

        // An index that can't be written: every new file goes, and the old design stays.
        std::fs::remove_file(dir.join(INDEX_FILE)).unwrap();
        std::fs::create_dir(dir.join(INDEX_FILE)).unwrap();
        let expected = names_in(&dir);
        let err = store
            .replace_design(&old.id, new(), &folder(), b"{}")
            .unwrap_err();
        assert!(err.starts_with("couldn't save that skin"), "{err}");
        assert_eq!(names_in(&dir), expected);
        assert_eq!(store.list(), vec![old.clone()]);
        assert_eq!(store.design(&old.id).unwrap().as_deref(), Some(DESIGN));
        std::fs::remove_dir(dir.join(INDEX_FILE)).unwrap();
        std::fs::write(dir.join(INDEX_FILE), &index).unwrap();

        assert_eq!(Store::open(dir.clone()).list(), vec![old]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_orphaned_design_is_removed_on_open_and_nothing_else() {
        let dir = temp_dir("design-leftovers");
        let store = Store::open(dir.clone());
        let (kept, _) = store
            .add_design(
                design_skin(&skin_id(b"kept design"), "Kept"),
                &folder(),
                DESIGN,
            )
            .unwrap();
        drop(store);

        let leftovers = [
            "0123456789ab.design.json",
            ".0123456789ab.design.json.folderskin-7-3.tmp",
        ];
        let others = [
            "0123456789ab.design.json.bak",
            "0123456789ab.design.jsonl",
            "0123456789ab.design-v2.json",
            "0123456789ab.designs.json",
            "0123456789ab.json",
            // Upper case, on a stem no leftover has: some disks ignore case in names.
            "ABCDEF012345.design.json",
            "design.json",
            ".0123456789ab.design.json.folderskin-7.tmp",
        ];
        for name in leftovers.iter().chain(&others) {
            std::fs::write(dir.join(name), b"{}").unwrap();
        }
        // Everything is an hour old, the kept design's own files included.
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            age(&entry.path(), HOUR);
        }

        let store = Store::open(dir.clone());
        assert_eq!(store.list(), vec![kept.clone()]);
        let names = names_in(&dir);
        for gone in leftovers {
            assert!(!names.contains(&gone.to_string()), "{gone} is still there");
        }
        for stays in others {
            assert!(names.contains(&stays.to_string()), "{stays} was removed");
        }
        assert_eq!(store.design(&kept.id).unwrap().as_deref(), Some(DESIGN));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_well_formed_ids_are_skin_ids() {
        assert!(is_skin_id(&skin_id(b"anything")));
        for bad in ["user:0123456789AB", "user:../../etc/pw", "__default__", ""] {
            assert!(!is_skin_id(bad), "{bad:?}");
        }
    }
}
