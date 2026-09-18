//! Process-wide state shared by the commands: decoded built-in skins, imported pictures,
//! and the thumbnail cache.

use folderskin_core::compositor::Artwork;
use image::RgbaImage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// A built-in skin decoded to RGBA, ready for the compositor.
pub struct LoadedSkin {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub art: Arc<Artwork>,
}

#[derive(Default)]
pub struct Inner {
    pub builtin: OnceLock<Vec<LoadedSkin>>,
    /// Pictures the user dropped or picked, keyed by `custom:<hash>`.
    pub custom: Mutex<HashMap<String, Arc<Artwork>>>,
    /// Finished folder images from the AI assistant, used as the icon without compositing.
    pub prerendered: Mutex<HashMap<String, Arc<RgbaImage>>>,
    /// Rendered thumbnails as PNG data URLs, keyed by skin id.
    pub thumbs: Mutex<HashMap<String, String>>,
}

/// Cheap to clone; every command clones it before moving work to a blocking thread.
#[derive(Clone, Default)]
pub struct AppState(pub Arc<Inner>);

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

    pub fn artwork(&self, skin_id: &str) -> Option<Arc<Artwork>> {
        if let Some(art) = self
            .0
            .custom
            .lock()
            .ok()
            .and_then(|m| m.get(skin_id).cloned())
        {
            return Some(art);
        }
        self.builtin()
            .iter()
            .find(|s| s.id == skin_id)
            .map(|s| s.art.clone())
    }

    /// Keeps at most `MAX_CUSTOM` imported pictures (about 16 MB each) in memory.
    pub fn remember_custom(&self, id: String, art: Arc<Artwork>) {
        const MAX_CUSTOM: usize = 12;
        if let Ok(mut m) = self.0.custom.lock() {
            if m.len() >= MAX_CUSTOM && !m.contains_key(&id) {
                if let Some(oldest) = m.keys().next().cloned() {
                    m.remove(&oldest);
                    if let Ok(mut t) = self.0.thumbs.lock() {
                        t.remove(&oldest);
                    }
                }
            }
            m.insert(id, art);
        }
    }

    /// A finished folder image, when this id names one.
    pub fn prerendered(&self, skin_id: &str) -> Option<Arc<RgbaImage>> {
        self.0.prerendered.lock().ok().and_then(|m| m.get(skin_id).cloned())
    }

    /// Keeps at most `MAX_PRERENDERED` whole-folder renders in memory.
    pub fn remember_prerendered(&self, id: String, img: Arc<RgbaImage>) {
        const MAX_PRERENDERED: usize = 12;
        if let Ok(mut m) = self.0.prerendered.lock() {
            if m.len() >= MAX_PRERENDERED && !m.contains_key(&id) {
                if let Some(oldest) = m.keys().next().cloned() {
                    m.remove(&oldest);
                    if let Ok(mut t) = self.0.thumbs.lock() {
                        t.remove(&oldest);
                    }
                }
            }
            m.insert(id, img);
        }
    }

    pub fn cached_thumb(&self, id: &str) -> Option<String> {
        self.0.thumbs.lock().ok().and_then(|m| m.get(id).cloned())
    }

    pub fn remember_thumb(&self, id: String, data_url: String) {
        if let Ok(mut m) = self.0.thumbs.lock() {
            m.insert(id, data_url);
        }
    }
}
