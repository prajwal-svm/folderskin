//! Process-wide state shared by the commands: the decoded built-in skins, the store of saved
//! skins with a small cache of decoded ones in front of it, and the thumbnail cache.

use crate::skins::{BuiltinPack, BuiltinPackSkin, BuiltinSkin};
use crate::store::{NewSkin, SavedSkin, SkinImage, Store, THUMB_SIZE};
use folderskin_core::compositor::Artwork;
use folderskin_core::pack;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// A built-in skin decoded, ready for the compositor.
pub struct LoadedSkin {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub tags: Vec<String>,
    /// For a skin from a built-in pack: the pack it belongs to.
    pub pack: Option<PackInfo>,
    pub image: SkinImage,
    /// The embedded file, which keys the thumbnail cache.
    pub bytes: &'static [u8],
}

/// The built-in pack a skin comes from.
pub struct PackInfo {
    pub id: String,
    pub name: String,
    /// The GitHub name of whoever made it.
    pub author: String,
    pub license: String,
}

/// How many decoded saved skins stay in memory (up to 16 MB each at 2048 px).
const MAX_RECENT: usize = 12;

/// Saved skins decoded recently; the least recently used one goes first. Evicting one loses
/// nothing, because the store still has it.
#[derive(Default)]
pub struct Recent {
    tick: u64,
    skins: HashMap<String, (u64, SkinImage)>,
}

impl Recent {
    fn get(&mut self, id: &str) -> Option<SkinImage> {
        self.tick += 1;
        let tick = self.tick;
        self.skins.get_mut(id).map(|(used, image)| {
            *used = tick;
            image.clone()
        })
    }

    fn put(&mut self, id: String, image: SkinImage) {
        if self.skins.len() >= MAX_RECENT && !self.skins.contains_key(&id) {
            let oldest = self
                .skins
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                self.skins.remove(&oldest);
            }
        }
        self.tick += 1;
        self.skins.insert(id, (self.tick, image));
    }

    fn remove(&mut self, id: &str) {
        self.skins.remove(id);
    }
}

/// A skin that could not be written to disk. It lasts until the app quits.
struct Unsaved {
    entry: SavedSkin,
    image: SkinImage,
    thumbnail_png: Vec<u8>,
}

#[derive(Default)]
pub struct Inner {
    pub builtin: OnceLock<Vec<LoadedSkin>>,
    /// Saved skins on disk, opened in `setup` once the app data folder is known.
    store: OnceLock<Store>,
    /// Saved skins decoded recently, keyed by id.
    recent: Mutex<Recent>,
    /// Skins that could not be saved: no data folder, or a failed write of an AI result.
    unsaved: Mutex<HashMap<String, Unsaved>>,
    /// Rendered thumbnails of the built-in skins and the default folder, as PNG data URLs.
    pub thumbs: Mutex<HashMap<String, String>>,
}

/// Cheap to clone; every command clones it before moving work to a blocking thread.
#[derive(Clone, Default)]
pub struct AppState(pub Arc<Inner>);

/// Locks a mutex even if another thread panicked while holding it: every map here stays valid
/// between statements, so a panic elsewhere must not take the gallery down with it.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl AppState {
    /// Decodes the embedded skins once and keeps them for the process lifetime: the manifest
    /// skins, then every built-in pack's, in order. They are decoded on all cores at once, since
    /// each takes 20 ms or more.
    pub fn builtin(&self) -> &Vec<LoadedSkin> {
        self.0.builtin.get_or_init(|| {
            let skins: Vec<Embedded> = crate::skins::SKINS
                .iter()
                .map(Embedded::Skin)
                .chain(
                    crate::skins::PACKS
                        .iter()
                        .flat_map(|p| p.skins.iter().map(move |s| Embedded::Pack(p, s))),
                )
                .collect();
            parallel_map(&skins, Embedded::decode)
                .into_iter()
                .flatten()
                .collect()
        })
    }

    /// Opens the saved-skin store in `dir`. Called once, from `setup`.
    pub fn open_store(&self, dir: PathBuf) {
        if self.0.store.set(Store::open(dir)).is_err() {
            eprintln!("folderskin: the saved-skin store was already open");
        }
    }

    /// The saved-skin store, or `None` when the app has no data folder to keep it in.
    pub fn store(&self) -> Option<&Store> {
        self.0.store.get()
    }

    /// The pixels of any skin: built in, saved, or kept for this session. A saved skin that is
    /// not in memory is read back from disk, so this can take a moment on a cache miss.
    pub fn resolve(&self, skin_id: &str) -> Result<SkinImage, String> {
        if let Some(image) = lock(&self.0.recent).get(skin_id) {
            return Ok(image);
        }
        if let Some(unsaved) = lock(&self.0.unsaved).get(skin_id) {
            return Ok(unsaved.image.clone());
        }
        if let Some(image) = self.store().map(|s| s.load(skin_id)).transpose()?.flatten() {
            lock(&self.0.recent).put(skin_id.to_string(), image.clone());
            return Ok(image);
        }
        self.builtin()
            .iter()
            .find(|s| s.id == skin_id)
            .map(|s| s.image.clone())
            .ok_or_else(|| "that skin isn't available any more".to_string())
    }

    /// A skin already saved (or kept for this session) under `id`, with its thumbnail PNG.
    pub fn find_saved(&self, id: &str) -> Option<(SavedSkin, Vec<u8>)> {
        if let Some(u) = lock(&self.0.unsaved).get(id) {
            return Some((u.entry.clone(), u.thumbnail_png.clone()));
        }
        let store = self.store()?;
        let entry = store.get(id)?;
        let thumb = store.thumbnail_png(&entry).ok()?;
        Some((entry, thumb))
    }

    /// Saves a new skin and keeps it decoded for the next apply. Returns its entry and
    /// thumbnail PNG; a skin whose id is already saved comes back as it was saved.
    ///
    /// Without a data folder the skin is kept for this session only, as FolderSkin did before
    /// it had a store. A failed write is returned as an error for the caller to handle.
    pub fn save(&self, new: NewSkin, image: SkinImage) -> Result<(SavedSkin, Vec<u8>), String> {
        let Some(store) = self.store() else {
            return Ok(self.keep_unsaved(new, image));
        };
        let (entry, fresh) = store.add(new, &image)?;
        // Saved now, so any session-only copy from an earlier failed write is redundant.
        lock(&self.0.unsaved).remove(&entry.id);
        if fresh {
            lock(&self.0.recent).put(entry.id.clone(), image);
        }
        let thumb = store.thumbnail_png(&entry)?;
        Ok((entry, thumb))
    }

    /// Keeps a skin that could not be saved for the rest of this session: it is listed, can be
    /// applied and deleted, and is gone when the app quits.
    pub fn keep_unsaved(&self, new: NewSkin, image: SkinImage) -> (SavedSkin, Vec<u8>) {
        let entry = new.entry(&image, crate::store::now_ms());
        let thumbnail_png = image.preview_png(THUMB_SIZE);
        lock(&self.0.unsaved).insert(
            entry.id.clone(),
            Unsaved {
                entry: entry.clone(),
                image,
                thumbnail_png: thumbnail_png.clone(),
            },
        );
        (entry, thumbnail_png)
    }

    /// Every saved skin, plus any kept for this session only, newest first, each with its
    /// thumbnail PNG. A skin whose thumbnail cannot be produced is left out and logged.
    pub fn saved_skins(&self) -> Vec<(SavedSkin, Vec<u8>)> {
        let mut skins: Vec<(SavedSkin, Vec<u8>)> = lock(&self.0.unsaved)
            .values()
            .map(|u| (u.entry.clone(), u.thumbnail_png.clone()))
            .collect();
        if let Some(store) = self.store() {
            for entry in store.list() {
                match store.thumbnail_png(&entry) {
                    Ok(png) => skins.push((entry, png)),
                    Err(e) => eprintln!("folderskin: leaving {} out of the gallery: {e}", entry.id),
                }
            }
        }
        skins.sort_by(|(a, _), (b, _)| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        skins
    }

    /// Gives a saved or session-only skin a new name and tags and returns its entry.
    pub fn edit(&self, skin_id: &str, name: &str, tags: &[String]) -> Result<SavedSkin, String> {
        if let Some(unsaved) = lock(&self.0.unsaved).get_mut(skin_id) {
            unsaved.entry.name =
                crate::store::clean_name(name).ok_or_else(|| "a skin needs a name".to_string())?;
            unsaved.entry.tags =
                folderskin_core::pack::clean_tags(tags, folderskin_core::pack::MAX_TAGS);
            return Ok(unsaved.entry.clone());
        }
        match self.store() {
            Some(store) => store.edit(skin_id, name, tags),
            None => Err("FolderSkin doesn't know that skin".into()),
        }
    }

    /// Deletes every saved skin that came from the community pack `pack_id` and returns their ids.
    pub fn remove_pack(&self, pack_id: &str) -> Result<Vec<String>, String> {
        let ids: Vec<String> = self
            .saved_skins()
            .into_iter()
            .filter(|(entry, _)| entry.pack.as_deref() == Some(pack_id))
            .map(|(entry, _)| entry.id)
            .collect();
        for id in &ids {
            self.delete(id)?;
        }
        Ok(ids)
    }

    /// The ids of the community packs that have at least one skin saved.
    pub fn added_packs(&self) -> std::collections::HashSet<String> {
        let mut packs: std::collections::HashSet<String> = lock(&self.0.unsaved)
            .values()
            .filter_map(|u| u.entry.pack.clone())
            .collect();
        if let Some(store) = self.store() {
            packs.extend(store.list().into_iter().filter_map(|e| e.pack));
        }
        packs
    }

    /// Removes a saved or session-only skin from disk and from memory.
    pub fn delete(&self, skin_id: &str) -> Result<(), String> {
        lock(&self.0.recent).remove(skin_id);
        let was_unsaved = lock(&self.0.unsaved).remove(skin_id).is_some();
        match self.store() {
            Some(store) => store.delete(skin_id),
            None if was_unsaved => Ok(()),
            None => Err("FolderSkin doesn't know that skin".into()),
        }
    }

    pub fn cached_thumb(&self, id: &str) -> Option<String> {
        lock(&self.0.thumbs).get(id).cloned()
    }

    pub fn remember_thumb(&self, id: String, data_url: String) {
        lock(&self.0.thumbs).insert(id, data_url);
    }
}

/// A skin embedded in the binary, before it is decoded.
enum Embedded {
    /// One of the manifest skins in `assets/skins`: always artwork, with its focus point.
    Skin(&'static BuiltinSkin),
    /// A skin of a built-in pack: a finished folder or artwork, depending on the picture.
    Pack(&'static BuiltinPack, &'static BuiltinPackSkin),
}

impl Embedded {
    /// The skin decoded, or `None` (and a line on stderr) when its picture can't be used.
    fn decode(&self) -> Option<LoadedSkin> {
        let (id, bytes) = match self {
            Embedded::Skin(s) => (s.id, s.bytes),
            Embedded::Pack(_, s) => (s.id, s.bytes),
        };
        let rgba = match image::load_from_memory(bytes) {
            Ok(img) => img.to_rgba8(),
            Err(e) => {
                eprintln!("folderskin: the built-in skin {id} couldn't be decoded: {e}");
                return None;
            }
        };
        Some(match self {
            Embedded::Skin(s) => LoadedSkin {
                id: s.id.to_string(),
                name: s.name.to_string(),
                collection: s.collection.to_string(),
                tags: s.tags.iter().map(|t| t.to_string()).collect(),
                pack: None,
                image: SkinImage::Artwork(Arc::new(Artwork {
                    rgba,
                    focus: (s.focus[0], s.focus[1]),
                })),
                bytes: s.bytes,
            },
            Embedded::Pack(p, s) => {
                // The same split a community pack's pictures get when they are added.
                let image = match crate::commands::prepare_import(rgba) {
                    Ok(image) => image,
                    Err(e) => {
                        eprintln!("folderskin: the built-in skin {id} can't be used: {e}");
                        return None;
                    }
                };
                let tags: Vec<String> =
                    p.tags.iter().chain(s.tags).map(|t| t.to_string()).collect();
                LoadedSkin {
                    id: s.id.to_string(),
                    name: s.name.to_string(),
                    collection: p.id.to_string(),
                    tags: pack::clean_tags(&tags, pack::MAX_TAGS),
                    pack: Some(PackInfo {
                        id: p.id.to_string(),
                        name: p.name.to_string(),
                        author: p.author.to_string(),
                        license: p.license.to_string(),
                    }),
                    image,
                    bytes: s.bytes,
                }
            }
        })
    }
}

/// `f` applied to every item, on up to one thread per core, with the results in item order.
fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, items.len().max(1));
    let per_thread = items.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = items
            .chunks(per_thread)
            .map(|part| scope.spawn(|| part.iter().map(&f).collect::<Vec<R>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| {
                w.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{skin_id, SkinSource};

    fn folder(shade: u8) -> SkinImage {
        SkinImage::Folder(Arc::new(image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([shade, shade, shade, 255]),
        )))
    }

    #[test]
    fn the_recent_cache_evicts_the_least_recently_used_skin() {
        let mut recent = Recent::default();
        for i in 0..MAX_RECENT {
            recent.put(format!("user:{i}"), folder(i as u8));
        }
        assert!(recent.get("user:0").is_some(), "touch the oldest");
        recent.put("user:new".into(), folder(99));
        assert_eq!(recent.skins.len(), MAX_RECENT);
        assert!(recent.get("user:0").is_some(), "recently used, so kept");
        assert!(
            recent.get("user:1").is_none(),
            "least recently used, so evicted"
        );
        assert!(recent.get("user:new").is_some());
    }

    #[test]
    fn without_a_store_skins_last_for_the_session() {
        let state = AppState::default();
        let id = skin_id(b"session");
        let new = NewSkin {
            id: id.clone(),
            name: "Session".into(),
            source: SkinSource::Import,
            provider: None,
            model: None,
            idea: None,
            tags: Vec::new(),
            pack: None,
            pack_name: None,
            author: None,
            license: None,
        };
        let (entry, thumb) = state.save(new, folder(10)).unwrap();
        assert_eq!(entry.id, id);
        assert!(thumb.starts_with(b"\x89PNG"));
        assert!(state.resolve(&id).is_ok());
        assert_eq!(state.saved_skins().len(), 1);
        assert!(state.find_saved(&id).is_some());
        state.delete(&id).unwrap();
        assert!(state.resolve(&id).is_err());
        assert!(state.saved_skins().is_empty());
    }

    #[test]
    fn a_saved_skin_resolves_from_disk_after_a_restart() {
        let dir = std::env::temp_dir().join(format!("folderskin-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let id = skin_id(b"restart");
        {
            let state = AppState::default();
            state.open_store(dir.clone());
            let new = NewSkin {
                id: id.clone(),
                name: "Restart".into(),
                source: SkinSource::Import,
                provider: None,
                model: None,
                idea: None,
                tags: Vec::new(),
                pack: None,
                pack_name: None,
                author: None,
                license: None,
            };
            state.save(new, folder(20)).unwrap();
        }
        let state = AppState::default();
        state.open_store(dir.clone());
        assert!(matches!(state.resolve(&id), Ok(SkinImage::Folder(_))));
        assert_eq!(state.saved_skins()[0].0.id, id);
        state.delete(&id).unwrap();
        assert!(state.resolve(&id).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_built_in_skin_decodes_once_with_its_own_id() {
        let state = AppState::default();
        let shipped = crate::skins::SKINS.len()
            + crate::skins::PACKS
                .iter()
                .map(|p| p.skins.len())
                .sum::<usize>();
        let loaded = state.builtin();
        assert_eq!(loaded.len(), shipped, "no built-in skin was dropped");
        let ids: std::collections::HashSet<&str> = loaded.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), loaded.len(), "ids are unique");
        assert!(loaded.iter().all(|s| crate::skins::is_builtin(&s.id)));
        assert!(std::ptr::eq(state.builtin(), loaded), "decoded once");
    }

    fn leak<T>(items: Vec<T>) -> &'static [T] {
        Box::leak(items.into_boxed_slice())
    }

    #[test]
    fn a_pack_skin_is_a_folder_or_artwork_and_carries_the_packs_tags_first() {
        let cutout = image::RgbaImage::from_fn(300, 300, |x, y| {
            let inside = (40..260).contains(&x) && (60..240).contains(&y);
            image::Rgba(if inside {
                [30, 90, 200, 255]
            } else {
                [0, 0, 0, 0]
            })
        });
        let opaque = image::RgbaImage::from_pixel(300, 280, image::Rgba([240, 160, 40, 255]));
        let skins = leak(vec![
            BuiltinPackSkin {
                id: "3d/glass",
                name: "Glass",
                tags: &["Shiny"],
                bytes: leak(folderskin_core::raster::encode_png(&cutout)),
            },
            BuiltinPackSkin {
                id: "3d/sky",
                name: "Sky",
                tags: &[],
                bytes: leak(folderskin_core::raster::encode_png(&opaque)),
            },
            BuiltinPackSkin {
                id: "3d/broken",
                name: "Broken",
                tags: &[],
                bytes: b"not a picture",
            },
        ]);
        let pack: &'static BuiltinPack = Box::leak(Box::new(BuiltinPack {
            id: "3d",
            name: "3D",
            author: "prajwal-svm",
            license: "CC0-1.0",
            tags: &["3d", "glossy"],
            skins,
        }));

        let glass = Embedded::Pack(pack, &skins[0]).decode().unwrap();
        assert!(
            matches!(glass.image, SkinImage::Folder(_)),
            "a cutout is the icon itself"
        );
        assert_eq!(glass.tags, ["3d", "glossy", "shiny"]);
        assert_eq!(glass.collection, "3d");
        let info = glass.pack.as_ref().unwrap();
        assert_eq!((info.id.as_str(), info.name.as_str()), ("3d", "3D"));
        assert_eq!(
            (info.author.as_str(), info.license.as_str()),
            ("prajwal-svm", "CC0-1.0")
        );

        let sky = Embedded::Pack(pack, &skins[1]).decode().unwrap();
        assert!(
            matches!(sky.image, SkinImage::Artwork(_)),
            "a picture goes on the template"
        );
        assert_eq!(sky.tags, ["3d", "glossy"]);

        assert!(
            Embedded::Pack(pack, &skins[2]).decode().is_none(),
            "a picture that can't be decoded is left out"
        );
    }

    #[test]
    fn parallel_map_keeps_the_order() {
        let numbers: Vec<u32> = (0..103).collect();
        let doubled = parallel_map(&numbers, |n| n * 2);
        assert_eq!(doubled, numbers.iter().map(|n| n * 2).collect::<Vec<_>>());
        assert!(parallel_map(&[] as &[u32], |n| *n).is_empty());
    }
}
