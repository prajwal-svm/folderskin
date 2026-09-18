//! Process-wide state shared by the commands: the decoded built-in skins, the store of saved
//! skins with a small cache of decoded ones in front of it, and the thumbnail cache.

use crate::store::{NewSkin, SavedSkin, SkinImage, Store, THUMB_SIZE};
use folderskin_core::compositor::Artwork;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// A built-in skin decoded to RGBA, ready for the compositor.
pub struct LoadedSkin {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub art: Arc<Artwork>,
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
    /// Decodes the embedded skins once (about 20 ms each) and keeps them for the process lifetime.
    pub fn builtin(&self) -> &Vec<LoadedSkin> {
        self.0.builtin.get_or_init(|| {
            crate::skins::SKINS
                .iter()
                .filter_map(|s| {
                    let rgba = image::load_from_memory(s.bytes).ok()?.to_rgba8();
                    Some(LoadedSkin {
                        id: s.id.to_string(),
                        name: s.name.to_string(),
                        collection: s.collection.to_string(),
                        art: Arc::new(Artwork {
                            rgba,
                            focus: (s.focus[0], s.focus[1]),
                        }),
                    })
                })
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
            .map(|s| SkinImage::Artwork(s.art.clone()))
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
}
