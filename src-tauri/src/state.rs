//! Process-wide state shared by the commands: the store of saved skins with a small cache of
//! decoded ones in front of it, the skins that could not be saved, and the default folder's
//! thumbnail.

use crate::store::{self, NewSkin, SavedSkin, SkinImage, SkinSource, Store, THUMB_SIZE};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// How much memory decoded saved skins may take between applies. A skin is its pixels, four
/// bytes each: artwork a folder's shape is about 4 MB, and a square photo kept at the largest
/// size the store holds is 16 MB. Counting entries instead bounded the first at 48 MB and the
/// second at 200 MB for the same twelve skins; counting bytes bounds both, and still keeps more
/// ordinary skins than twelve.
const RECENT_BUDGET: usize = 64 << 20;

/// Saved skins decoded recently; the least recently used one goes first when they no longer fit
/// in [`RECENT_BUDGET`]. Evicting one loses nothing, because the store still has it — only the
/// time to read and decode it again on the next apply.
pub struct Recent {
    tick: u64,
    skins: HashMap<String, (u64, SkinImage)>,
    /// What the skins held here take, counted as each was put in. A skin shared with an apply
    /// that is still running is counted here as well as there, which is the safe way round.
    bytes: usize,
    budget: usize,
}

impl Default for Recent {
    fn default() -> Recent {
        Recent {
            tick: 0,
            skins: HashMap::new(),
            bytes: 0,
            budget: RECENT_BUDGET,
        }
    }
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
        self.tick += 1;
        self.bytes += image.bytes();
        if let Some((_, replaced)) = self.skins.insert(id, (self.tick, image)) {
            self.bytes -= replaced.bytes();
        }
        // The skin just put in always stays, however big: it is the one the apply that asked for
        // it is about to use.
        while self.bytes > self.budget && self.skins.len() > 1 {
            let oldest = self
                .skins
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else { break };
            self.remove(&oldest);
        }
    }

    fn remove(&mut self, id: &str) {
        if let Some((_, gone)) = self.skins.remove(id) {
            self.bytes -= gone.bytes();
        }
    }

    #[cfg(test)]
    fn with_budget(budget: usize) -> Recent {
        Recent {
            budget,
            ..Recent::default()
        }
    }
}

/// A skin that could not be written to disk. It lasts until the app quits.
struct Unsaved {
    entry: SavedSkin,
    image: SkinImage,
    thumbnail_png: Vec<u8>,
    /// A design's document, so the composer can open it again this session.
    design: Option<Arc<Vec<u8>>>,
}

#[derive(Default)]
pub struct Inner {
    /// Saved skins on disk, opened in `setup` once the app data folder is known.
    store: OnceLock<Store>,
    /// Saved skins decoded recently, keyed by id.
    recent: Mutex<Recent>,
    /// Skins that could not be saved: no data folder, or a failed write of an AI result.
    unsaved: Mutex<HashMap<String, Unsaved>>,
    /// The plain default folder's thumbnail PNG, once it has been drawn.
    default_thumb: OnceLock<Vec<u8>>,
    /// Set while the colours of skins saved before FolderSkin kept them are being read, so the
    /// filter can say a colour it doesn't offer yet may still turn up.
    reading_palettes: AtomicBool,
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

    /// The pixels of a saved skin, or of one kept for this session. A saved skin that is not in
    /// memory is read back from disk, so this can take a moment on a cache miss.
    pub fn resolve(&self, skin_id: &str) -> Result<SkinImage, String> {
        if let Some(image) = lock(&self.0.recent).get(skin_id) {
            return Ok(image);
        }
        if let Some(unsaved) = lock(&self.0.unsaved).get(skin_id) {
            return Ok(unsaved.image.clone());
        }
        let image = self
            .store()
            .map(|s| s.load(skin_id))
            .transpose()?
            .flatten()
            .ok_or_else(|| "that skin isn't available any more".to_string())?;
        lock(&self.0.recent).put(skin_id.to_string(), image.clone());
        Ok(image)
    }

    /// The entry of a skin saved (or kept for this session) under `id`.
    pub fn entry(&self, id: &str) -> Option<SavedSkin> {
        if let Some(u) = lock(&self.0.unsaved).get(id) {
            return Some(u.entry.clone());
        }
        self.store()?.get(id)
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

    /// Saves a new skin and keeps it decoded for the next apply. Returns its entry; a skin whose
    /// id is already saved comes back as it was saved.
    ///
    /// Without a data folder the skin is kept for this session only, as FolderSkin did before
    /// it had a store. A failed write is returned as an error for the caller to handle.
    pub fn save(&self, new: NewSkin, image: SkinImage) -> Result<SavedSkin, String> {
        let Some(store) = self.store() else {
            return Ok(self.keep_unsaved(new, image));
        };
        let (entry, fresh) = store.add(new, &image)?;
        // Saved now, so any session-only copy from an earlier failed write is redundant.
        lock(&self.0.unsaved).remove(&entry.id);
        if fresh {
            lock(&self.0.recent).put(entry.id.clone(), image);
        }
        Ok(entry)
    }

    /// Saves new skins all together or not at all ([`Store::add_many`]) and returns their entries
    /// in the order given; one whose id is already saved comes back as it was saved. The first is
    /// the newest, so the library lists them in the order given.
    ///
    /// Unlike [`AppState::save`], it keeps none of them decoded: a pack of sixteen would push
    /// every other skin out of `recent`. Without a data folder each is kept for this session
    /// only. `encoded` is called as each new skin is ready to be written, from whichever thread
    /// prepared it.
    pub fn save_many(
        &self,
        skins: Vec<(NewSkin, SkinImage)>,
        encoded: &(dyn Fn() + Sync),
    ) -> Result<Vec<SavedSkin>, String> {
        let Some(store) = self.store() else {
            return Ok(skins
                .into_iter()
                .map(|(new, image)| {
                    let kept = self.keep_unsaved(new, image);
                    encoded();
                    kept
                })
                .collect());
        };
        let added = store.add_many(skins, encoded)?;
        let mut unsaved = lock(&self.0.unsaved);
        for skin in &added {
            // Saved now, so any session-only copy from an earlier failed write is redundant.
            unsaved.remove(&skin.entry.id);
        }
        drop(unsaved);
        Ok(added.into_iter().map(|skin| skin.entry).collect())
    }

    /// Keeps a skin that could not be saved for the rest of this session: it is listed, can be
    /// applied and deleted, and is gone when the app quits.
    pub fn keep_unsaved(&self, new: NewSkin, image: SkinImage) -> SavedSkin {
        self.keep(new, image, None, store::now_ms())
    }

    /// [`AppState::keep_unsaved`] with a design's document, added at `created_at`.
    fn keep(
        &self,
        new: NewSkin,
        image: SkinImage,
        design: Option<Arc<Vec<u8>>>,
        created_at: u64,
    ) -> SavedSkin {
        let (thumbnail_png, palette) = image.preview(THUMB_SIZE);
        let entry = new.entry(&image, Some(palette), created_at);
        lock(&self.0.unsaved).insert(
            entry.id.clone(),
            Unsaved {
                entry: entry.clone(),
                image,
                thumbnail_png,
                design,
            },
        );
        entry
    }

    /// Saves a design from the composer with the document it was made from
    /// ([`Store::add_design`]), and keeps it decoded for the next apply. Returns its entry and
    /// thumbnail PNG; a design saved before comes back as it was saved.
    ///
    /// Without a data folder it is kept for this session only, its document with it.
    pub fn save_design(
        &self,
        new: NewSkin,
        image: SkinImage,
        design: Vec<u8>,
    ) -> Result<SavedSkin, String> {
        let Some(store) = self.store() else {
            if let Some((kept, _)) = self.find_saved(&new.id) {
                return Ok(kept);
            }
            return Ok(self.keep(new, image, Some(Arc::new(design)), store::now_ms()));
        };
        let (entry, fresh) = store.add_design(new, &image, &design)?;
        lock(&self.0.unsaved).remove(&entry.id);
        if fresh {
            lock(&self.0.recent).put(entry.id.clone(), image);
        }
        Ok(entry)
    }

    /// Saves a design over the one it was made from, `old_id` ([`Store::replace_design`]): it
    /// takes the old one's place in the library, and the old one goes, from memory too. Returns
    /// the entry of whichever design the library now has in its place.
    ///
    /// A design kept for this session only is swapped the same way, in memory.
    pub fn replace_design(
        &self,
        old_id: &str,
        new: NewSkin,
        image: SkinImage,
        design: Vec<u8>,
    ) -> Result<SavedSkin, String> {
        let mut unsaved = lock(&self.0.unsaved);
        if let Some(old) = unsaved.get_mut(old_id) {
            if old.entry.source != SkinSource::Composer {
                return Err(store::NOT_A_DESIGN.into());
            }
            if new.id == old_id {
                if let Some(name) = store::clean_name(&new.name) {
                    old.entry.name = name;
                }
                old.entry.tags =
                    folderskin_core::pack::clean_tags(&new.tags, folderskin_core::pack::MAX_TAGS);
                return Ok(old.entry.clone());
            }
            let created_at = old.entry.created_at;
            unsaved.remove(old_id);
            drop(unsaved);
            lock(&self.0.recent).remove(old_id);
            if let Some((kept, _)) = self.find_saved(&new.id) {
                return Ok(kept);
            }
            return Ok(self.keep(new, image, Some(Arc::new(design)), created_at));
        }
        drop(unsaved);

        let store = self.store().ok_or_else(|| store::DESIGN_GONE.to_string())?;
        let (entry, fresh) = store.replace_design(old_id, new, &image, &design)?;
        {
            let mut recent = lock(&self.0.recent);
            if entry.id != old_id {
                recent.remove(old_id);
            }
            if fresh {
                recent.put(entry.id.clone(), image);
            }
        }
        lock(&self.0.unsaved).remove(&entry.id);
        Ok(entry)
    }

    /// The document a design was made from, saved or kept for this session. `Ok(None)` for a
    /// skin with none, or no skin at all.
    pub fn design(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        if let Some(u) = lock(&self.0.unsaved).get(id) {
            return Ok(u.design.as_ref().map(|d| d.to_vec()));
        }
        match self.store() {
            Some(store) => store.design(id),
            None => Ok(None),
        }
    }

    /// Every saved skin, plus any kept for this session only, newest first. One with nothing left
    /// to draw a thumbnail from is left out and logged: the gallery has no picture to show for it.
    ///
    /// The pictures themselves are not read here. The gallery is given the address each thumbnail
    /// is served at ([`crate::thumbs`]) and asks for the ones it shows, so listing a library of
    /// any size costs a directory's worth of `stat`s rather than every picture in it.
    pub fn saved_entries(&self) -> Vec<SavedSkin> {
        let mut skins: Vec<SavedSkin> = lock(&self.0.unsaved)
            .values()
            .map(|u| u.entry.clone())
            .collect();
        if let Some(store) = self.store() {
            for entry in store.list() {
                if store.has_thumbnail_source(&entry) {
                    skins.push(entry);
                } else {
                    eprintln!(
                        "folderskin: leaving {} out of the gallery: its picture has gone",
                        entry.id
                    );
                }
            }
        }
        skins.sort_by(|a, b| {
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
            .saved_entries()
            .into_iter()
            .filter(|entry| entry.pack.as_deref() == Some(pack_id))
            .map(|entry| entry.id)
            .collect();
        for id in &ids {
            self.delete(id)?;
        }
        Ok(ids)
    }

    /// The community packs with at least one skin saved, each with the pack hash recorded when
    /// it was added (`None` for one added before FolderSkin kept one).
    pub fn installed_packs(&self) -> HashMap<String, Option<String>> {
        let mut packs = HashMap::new();
        let mut note = |entry: &SavedSkin| {
            if let Some(id) = &entry.pack {
                packs
                    .entry(id.clone())
                    .or_insert_with(|| entry.pack_hash.clone());
            }
        };
        for unsaved in lock(&self.0.unsaved).values() {
            note(&unsaved.entry);
        }
        if let Some(store) = self.store() {
            for entry in store.list() {
                note(&entry);
            }
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

    /// Whether the colours of older skins are still being read ([`crate::read_palettes`]).
    pub fn reading_palettes(&self) -> bool {
        self.0.reading_palettes.load(Ordering::Relaxed)
    }

    pub fn set_reading_palettes(&self, reading: bool) {
        self.0.reading_palettes.store(reading, Ordering::Relaxed);
    }

    /// The plain default folder's thumbnail: `draw()`'s picture the first time, and the same one
    /// after that. Callers that ask while it is being drawn wait for it.
    pub fn default_thumbnail(&self, draw: impl FnOnce() -> Vec<u8>) -> Vec<u8> {
        self.0.default_thumb.get_or_init(draw).clone()
    }
}

/// `f` applied to every item, on up to one thread per core, with the results in item order.
pub(crate) fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
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

    /// A skin of `side` px square, so a test can say what it costs: `side * side * 4` bytes.
    fn sized(side: u32, shade: u8) -> SkinImage {
        SkinImage::Folder(Arc::new(image::RgbaImage::from_pixel(
            side,
            side,
            image::Rgba([shade, shade, shade, 255]),
        )))
    }

    #[test]
    fn the_recent_cache_evicts_the_least_recently_used_skin_to_stay_inside_its_budget() {
        // Four skins' worth of room, so the fifth pushes one out.
        let one = 64 * 64 * 4;
        let mut recent = Recent::with_budget(one * 4);
        for i in 0..4 {
            recent.put(format!("user:{i}"), sized(64, i as u8));
        }
        assert_eq!(recent.bytes, one * 4);
        assert!(recent.get("user:0").is_some(), "touch the oldest");

        recent.put("user:new".into(), sized(64, 99));
        assert_eq!(recent.skins.len(), 4, "still four, and still inside");
        assert_eq!(recent.bytes, one * 4);
        assert!(recent.get("user:0").is_some(), "recently used, so kept");
        assert!(
            recent.get("user:1").is_none(),
            "least recently used, so evicted"
        );
        assert!(recent.get("user:new").is_some());
    }

    #[test]
    fn one_big_skin_costs_what_several_small_ones_do() {
        let small = 64 * 64 * 4;
        let mut recent = Recent::with_budget(small * 4);
        // A skin four times the side is sixteen times the pixels: over budget on its own, and
        // kept anyway, because the apply that asked for it is about to use it.
        recent.put("user:big".into(), sized(256, 1));
        assert_eq!(recent.skins.len(), 1);
        assert!(recent.get("user:big").is_some());
        // The next one in pushes it straight back out.
        recent.put("user:small".into(), sized(64, 2));
        assert_eq!(recent.bytes, small);
        assert!(recent.get("user:big").is_none());
        assert!(recent.get("user:small").is_some());
    }

    #[test]
    fn deleting_a_skin_gives_its_room_back() {
        let one = 64 * 64 * 4;
        let mut recent = Recent::with_budget(one * 4);
        recent.put("user:a".into(), sized(64, 1));
        recent.put("user:b".into(), sized(64, 2));
        assert_eq!(recent.bytes, one * 2);
        recent.remove("user:a");
        assert_eq!(recent.bytes, one);
        // Putting the same id in twice counts it once.
        recent.put("user:b".into(), sized(64, 3));
        assert_eq!(recent.bytes, one);
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
            pack_hash: None,
        };
        let entry = state.save(new, folder(10)).unwrap();
        assert_eq!(entry.id, id);
        assert!(state.resolve(&id).is_ok());
        assert_eq!(state.saved_entries().len(), 1);
        let (_, thumb) = state.find_saved(&id).expect("its thumbnail is there to serve");
        assert!(thumb.starts_with(b"\x89PNG"));
        state.delete(&id).unwrap();
        assert!(state.resolve(&id).is_err());
        assert!(state.saved_entries().is_empty());
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
                pack_hash: None,
            };
            state.save(new, folder(20)).unwrap();
        }
        let state = AppState::default();
        state.open_store(dir.clone());
        assert!(matches!(state.resolve(&id), Ok(SkinImage::Folder(_))));
        assert_eq!(state.saved_entries()[0].id, id);
        state.delete(&id).unwrap();
        assert!(state.resolve(&id).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn pack_skin(id: &str) -> NewSkin {
        NewSkin {
            id: id.into(),
            name: "Pack skin".into(),
            source: SkinSource::Community,
            provider: None,
            model: None,
            idea: None,
            tags: Vec::new(),
            pack: Some("test-pack".into()),
            pack_name: None,
            author: None,
            license: None,
            pack_hash: None,
        }
    }

    #[test]
    fn a_batch_is_not_kept_decoded_and_replaces_session_only_copies() {
        let dir =
            std::env::temp_dir().join(format!("folderskin-state-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = AppState::default();
        state.open_store(dir.clone());
        let (a, b) = (skin_id(b"batch a"), skin_id(b"batch b"));
        // `a` couldn't be written before, so it was kept for the session.
        state.keep_unsaved(pack_skin(&a), folder(1));

        let batch = vec![(pack_skin(&a), folder(1)), (pack_skin(&b), folder(2))];
        let saved = state.save_many(batch, &|| {}).unwrap();
        let ids: Vec<&str> = saved.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, [a.as_str(), b.as_str()]);
        assert!(
            lock(&state.0.unsaved).is_empty(),
            "saved now, so the session-only copy goes"
        );
        assert!(
            lock(&state.0.recent).skins.is_empty(),
            "a pack doesn't push every other skin out of the cache"
        );
        let newest_first: Vec<String> =
            state.saved_entries().into_iter().map(|e| e.id).collect();
        assert_eq!(newest_first, [a.clone(), b.clone()]);
        assert!(matches!(state.resolve(&b), Ok(SkinImage::Folder(_))));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_a_store_a_batch_lasts_for_the_session() {
        let state = AppState::default();
        let ids = [skin_id(b"loose a"), skin_id(b"loose b")];
        let ready = std::sync::atomic::AtomicUsize::new(0);
        let batch = ids.iter().map(|id| (pack_skin(id), folder(5))).collect();
        let kept = state
            .save_many(batch, &|| {
                ready.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
            .unwrap();
        assert_eq!(kept.len(), 2);
        assert_eq!(ready.into_inner(), 2);
        assert_eq!(state.saved_entries().len(), 2);
        assert!(state.resolve(&ids[1]).is_ok());
    }

    fn design_skin(id: &str, name: &str) -> NewSkin {
        NewSkin {
            source: SkinSource::Composer,
            name: name.into(),
            pack: None,
            ..pack_skin(id)
        }
    }

    #[test]
    fn without_a_store_a_design_and_its_document_last_for_the_session() {
        let state = AppState::default();
        let (draft_id, final_id) = (skin_id(b"draft"), skin_id(b"final"));
        let draft = state
            .save_design(design_skin(&draft_id, "Draft"), folder(1), b"[1]".to_vec())
            .unwrap();
        assert_eq!(
            state.design(&draft_id).unwrap().as_deref(),
            Some(&b"[1]"[..])
        );
        let again = state
            .save_design(design_skin(&draft_id, "Again"), folder(1), b"[]".to_vec())
            .unwrap();
        assert_eq!(again, draft, "saved already, so it comes back as it was");

        let done = state
            .replace_design(
                &draft_id,
                design_skin(&final_id, "Final"),
                folder(2),
                b"[2]".to_vec(),
            )
            .unwrap();
        assert_eq!(
            done.created_at, draft.created_at,
            "it keeps the draft's place"
        );
        assert_eq!(
            state.design(&final_id).unwrap().as_deref(),
            Some(&b"[2]"[..])
        );
        assert_eq!(state.design(&draft_id).unwrap(), None);
        assert!(state.resolve(&draft_id).is_err());
        let ids: Vec<String> = state.saved_entries().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, [final_id]);

        // A picture that was imported isn't a design, and a design that's gone can't be saved over.
        let photo = skin_id(b"photo");
        state.keep_unsaved(
            NewSkin {
                source: SkinSource::Import,
                pack: None,
                ..pack_skin(&photo)
            },
            folder(3),
        );
        let over = |old: &str| {
            state
                .replace_design(
                    old,
                    design_skin(&skin_id(b"x"), "X"),
                    folder(4),
                    b"[]".to_vec(),
                )
                .unwrap_err()
        };
        assert_eq!(over(&photo), store::NOT_A_DESIGN);
        assert_eq!(over(&draft_id), store::DESIGN_GONE);
        assert_eq!(state.design(&photo).unwrap(), None);
    }

    #[test]
    fn a_design_saved_over_its_old_one_takes_its_place_in_the_cache() {
        let dir =
            std::env::temp_dir().join(format!("folderskin-state-design-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = AppState::default();
        state.open_store(dir.clone());
        let (v1, v2) = (skin_id(b"cache v1"), skin_id(b"cache v2"));
        state
            .save_design(design_skin(&v1, "V1"), folder(1), b"[1]".to_vec())
            .unwrap();
        assert!(lock(&state.0.recent).skins.contains_key(&v1));

        let entry = state
            .replace_design(&v1, design_skin(&v2, "V2"), folder(2), b"[2]".to_vec())
            .unwrap();
        assert_eq!(entry.id, v2);
        {
            let recent = lock(&state.0.recent);
            assert!(!recent.skins.contains_key(&v1), "the old design is dropped");
            assert!(
                recent.skins.contains_key(&v2),
                "the new one is ready to apply"
            );
        }
        assert_eq!(state.design(&v2).unwrap().as_deref(), Some(&b"[2]"[..]));
        assert!(state.resolve(&v1).is_err());
        assert!(matches!(state.resolve(&v2), Ok(SkinImage::Folder(_))));
        assert_eq!(state.entry(&v2).map(|e| e.name).as_deref(), Some("V2"));
        assert!(state.entry(&v1).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parallel_map_keeps_the_order() {
        let numbers: Vec<u32> = (0..103).collect();
        let doubled = parallel_map(&numbers, |n| n * 2);
        assert_eq!(doubled, numbers.iter().map(|n| n * 2).collect::<Vec<_>>());
        assert!(parallel_map(&[] as &[u32], |n| *n).is_empty());
    }
}
