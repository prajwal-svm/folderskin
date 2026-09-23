//! The community catalog as the app holds it: which one is current, where its files come from,
//! and searching it.
//!
//! The first search fetches `v2/head.json` from the community folder. It names the current
//! catalog, which is downloaded once, checked against its SHA-256 and kept in the app's cache
//! folder under its generation; every search after that is answered on this computer, in a few
//! milliseconds, with no request at all. Until a tree is published (there is no head.json), the
//! catalog is made in memory from `index.json`, as the app read it before, so nothing regresses.
//! When nothing can be reached, the catalog from the last visit is used and marked offline.
//!
//! head.json, or index.json standing in for it, is the one file fetched past the caches, and
//! only for Refresh: everything else is named after its contents.

use crate::community::{fetch, uncached, NOT_FOUND};
use crate::previews::{DiskCache, CACHE_BYTES};
use folderskin_catalog::tree::{self, Head};
use folderskin_catalog::{build, Catalog, PackRecord, PackRow, Query, Results};
use folderskin_core::apply::paths::write_atomic;
use folderskin_core::pack::{self, Index};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Manager, Runtime};

/// How many strips and thumbnails download at once.
pub const PREVIEW_DOWNLOADS: usize = 6;
/// The largest `index.json` the app downloads.
pub const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;

/// The community packs as this session knows them. Managed by Tauri, one per app.
pub struct Community {
    /// The catalog in use, once one has loaded. The lock is held while one loads, so the searches
    /// the Community view sends as it opens wait for one download instead of each starting one.
    loaded: tokio::sync::Mutex<Option<Arc<Source>>>,
    /// `<app cache>/community`, once the app has said where that is; `None` inside when it has
    /// none, and then catalogs live in memory and nothing is kept between sessions.
    cache: OnceLock<Option<PathBuf>>,
    files: OnceLock<Option<DiskCache>>,
    downloads: tokio::sync::Semaphore,
}

impl Default for Community {
    fn default() -> Self {
        Community {
            loaded: tokio::sync::Mutex::new(None),
            cache: OnceLock::new(),
            files: OnceLock::new(),
            downloads: tokio::sync::Semaphore::new(PREVIEW_DOWNLOADS),
        }
    }
}

impl Community {
    /// A community whose files are kept in `cache`, or nowhere.
    pub fn with_cache(cache: Option<PathBuf>) -> Community {
        let community = Community::default();
        let _ = community.cache.set(cache);
        community
    }

    /// Keeps the catalog and pictures under the app's cache folder. The first call decides.
    pub fn init_cache<R: Runtime>(&self, app: &AppHandle<R>) {
        self.cache.get_or_init(|| {
            app.path()
                .app_cache_dir()
                .ok()
                .map(|dir| dir.join("community"))
        });
    }

    fn cache(&self) -> Option<&Path> {
        self.cache.get().and_then(Option::as_deref)
    }

    /// The strips, thumbnails and manifests kept on disk.
    pub fn files(&self) -> Option<&DiskCache> {
        self.files
            .get_or_init(|| {
                self.cache()
                    .map(|dir| DiskCache::new(dir.join("files"), CACHE_BYTES))
            })
            .as_ref()
    }

    /// Turns for downloading pictures, [`PREVIEW_DOWNLOADS`] of them.
    pub fn downloads(&self) -> &tokio::sync::Semaphore {
        &self.downloads
    }

    /// The catalog in use, loading it from the community folder at `base` the first time.
    pub async fn current(&self, base: &str) -> Result<Arc<Source>, String> {
        let mut loaded = self.loaded.lock().await;
        if let Some(source) = loaded.as_ref() {
            return Ok(source.clone());
        }
        let source = Arc::new(load(base, self.cache(), false).await?);
        *loaded = Some(source.clone());
        Ok(source)
    }

    /// Asks the community folder at `base` again, past every cache, for Refresh. When that fails
    /// the catalog in use stays.
    pub async fn refresh(&self, base: &str) -> Result<Arc<Source>, String> {
        let mut loaded = self.loaded.lock().await;
        let source = Arc::new(load(base, self.cache(), true).await?);
        *loaded = Some(source.clone());
        Ok(source)
    }
}

/// One catalog, and where the files it names come from.
pub struct Source {
    catalog: Mutex<Catalog>,
    /// The folders files are fetched from, in the order to try them: for a tree, its mirrors and
    /// then the folder head.json came from; for index.json, the community folder.
    bases: Vec<String>,
    /// The first base worth trying, moved past one that couldn't be reached.
    first: AtomicUsize,
    tree: bool,
    pub featured: Vec<String>,
    pub generation: String,
    /// True when nothing could be reached and this is the catalog from the last visit.
    pub offline: bool,
}

impl Source {
    /// True for a published tree, false for a list made from index.json.
    pub fn is_tree(&self) -> bool {
        self.tree
    }

    /// Where a pack's files are when the list came from index.json: the community folder.
    pub fn index_base(&self) -> Option<&str> {
        (!self.tree).then(|| self.bases[0].as_str())
    }

    pub fn search(&self, query: &Query) -> Result<Results, String> {
        lock(&self.catalog).search(query)
    }

    /// Packs by id, in the order asked; ids the catalog doesn't have are left out.
    pub fn packs(&self, ids: &[String]) -> Result<Vec<PackRow>, String> {
        lock(&self.catalog).packs(ids)
    }

    /// How many packs and skins it lists.
    pub fn counts(&self) -> (usize, usize) {
        lock(&self.catalog).counts()
    }

    /// Downloads `path` from the first folder that has it, trying the next one when a folder
    /// can't be reached or hasn't got the file yet (a mirror a moment behind the others).
    pub async fn get(&self, path: &str, max: usize) -> Result<Vec<u8>, String> {
        let first = self.first.load(Ordering::Relaxed).min(self.bases.len() - 1);
        let order = (first..self.bases.len()).chain(0..first);
        let mut error = String::new();
        for i in order {
            match fetch(&format!("{}/{path}", self.bases[i]), max).await {
                Ok(bytes) => {
                    if i != first {
                        self.first.store(i, Ordering::Relaxed);
                    }
                    return Ok(bytes);
                }
                Err(e) => error = e,
            }
        }
        Err(error)
    }
}

/// Loads the current catalog from the community folder at `base`: the published tree when there
/// is one, index.json when there isn't, and the last visit's copy when neither can be reached
/// (only when not `fresh`: Refresh says it failed rather than showing old packs as new).
pub async fn load(base: &str, cache: Option<&Path>, fresh: bool) -> Result<Source, String> {
    let head_url = format!("{base}/v{}/{}", tree::HEAD_VERSION, tree::HEAD_FILE);
    match fetch(&uncached(&head_url, fresh), tree::MAX_HEAD_BYTES).await {
        Ok(bytes) => {
            let head = Head::parse(&bytes)?;
            let source = from_head(&head, tree_bases(base, &head), cache, false).await?;
            if let Some(dir) = cache {
                keep(dir, tree::HEAD_FILE, &bytes);
            }
            Ok(source)
        }
        Err(e) if e == NOT_FOUND => {
            let index_url = format!("{base}/index.json");
            let bytes = fetch(&uncached(&index_url, fresh), MAX_INDEX_BYTES)
                .await
                .map_err(|e| {
                    if e == NOT_FOUND {
                        "the community packs aren't published yet".to_string()
                    } else {
                        e
                    }
                })?;
            let source = from_index(&bytes, base, false)?;
            if let Some(dir) = cache {
                keep(dir, "index.json", &bytes);
            }
            Ok(source)
        }
        Err(e) => match cache.filter(|_| !fresh) {
            Some(dir) => last_visit(base, dir).await.ok_or(e),
            None => Err(e),
        },
    }
}

/// The catalog the last visit used, from the cache folder, marked offline.
async fn last_visit(base: &str, dir: &Path) -> Option<Source> {
    if let Some(head) = std::fs::read(dir.join(tree::HEAD_FILE))
        .ok()
        .and_then(|bytes| Head::parse(&bytes).ok())
    {
        if dir.join(catalog_file(&head.generation)).is_file() {
            return from_head(&head, tree_bases(base, &head), Some(dir), true)
                .await
                .ok();
        }
    }
    let index = std::fs::read(dir.join("index.json")).ok()?;
    from_index(&index, base, true).ok()
}

/// Where a tree's files are fetched from: its mirrors first, then the folder head.json is in.
fn tree_bases(base: &str, head: &Head) -> Vec<String> {
    let mut bases: Vec<String> = head
        .mirrors
        .iter()
        .map(|m| m.trim_end_matches('/').to_string())
        .collect();
    bases.push(format!("{base}/v{}", tree::HEAD_VERSION));
    bases
}

fn catalog_file(generation: &str) -> String {
    format!("catalog-{generation}.sqlite")
}

/// The catalog `head` names: the copy kept in `cache` when there is one, downloaded and checked
/// otherwise. Offline, only a kept copy will do.
async fn from_head(
    head: &Head,
    bases: Vec<String>,
    cache: Option<&Path>,
    offline: bool,
) -> Result<Source, String> {
    let kept = cache.map(|dir| dir.join(catalog_file(&head.generation)));
    let opened = match kept.as_deref().filter(|path| path.is_file()) {
        Some(path) => {
            let path = path.to_path_buf();
            blocking(move || Catalog::open(&path)).await.ok()
        }
        None => None,
    };
    let catalog = match opened {
        Some(catalog) => catalog,
        None if offline => return Err("the packs from the last visit aren't kept".into()),
        None => download_catalog(head, &bases, kept).await?,
    };
    Ok(Source {
        catalog: Mutex::new(catalog),
        first: AtomicUsize::new(0),
        bases,
        tree: true,
        featured: head.featured_ids(),
        generation: head.generation.clone(),
        offline,
    })
}

/// Downloads the catalog `head` names from the first of `bases` that has it, checks it is the
/// file head.json describes, and keeps it at `keep` (then opens it there) or in memory. The
/// other generations kept beside it go.
async fn download_catalog(
    head: &Head,
    bases: &[String],
    keep: Option<PathBuf>,
) -> Result<Catalog, String> {
    let url = &head.catalog.url;
    let max = head.catalog.bytes as usize;
    let gz = if url.starts_with("https://") {
        fetch(url, max).await?
    } else {
        let mut error = String::new();
        let mut got = None;
        for base in bases {
            match fetch(&format!("{base}/{url}"), max).await {
                Ok(bytes) => {
                    got = Some(bytes);
                    break;
                }
                Err(e) => error = e,
            }
        }
        got.ok_or(error)?
    };
    let head = head.clone();
    blocking(move || {
        let damaged = || "the catalog of community packs arrived damaged; try again".to_string();
        if gz.len() as u64 != head.catalog.bytes || tree::sha256_hex(&gz) != head.catalog.sha256 {
            return Err(damaged());
        }
        let bytes = tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED)?;
        if tree::sha256_hex(&bytes)[..16] != head.generation {
            return Err(damaged());
        }
        let Some(path) = keep else {
            return Catalog::from_bytes(&bytes);
        };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| write_atomic(&path, &bytes));
        match written {
            Ok(()) => {
                forget_other_catalogs(&path);
                Catalog::open(&path)
            }
            // A full disk only means keeping it in memory this time.
            Err(_) => Catalog::from_bytes(&bytes),
        }
    })
    .await
}

/// Deletes the catalogs kept beside `current`. One still open elsewhere can't be deleted on
/// Windows; it goes next time.
fn forget_other_catalogs(current: &Path) {
    let Some(dir) = current.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path != current && name.starts_with("catalog-") && name.ends_with(".sqlite") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A catalog made in memory from `index.json`: the packs with their counts, and no skin names.
pub fn from_index(bytes: &[u8], base: &str, offline: bool) -> Result<Source, String> {
    let index = Index::parse(bytes)?;
    let records: Vec<PackRecord> = index
        .packs
        .into_iter()
        .filter(|p| pack::is_pack_id(&p.id))
        .map(|p| PackRecord {
            tags: pack::clean_tags(&p.tags, usize::MAX),
            id: p.id,
            name: p.name,
            author: p.author,
            license: p.license,
            hash: p.hash,
            manifest: String::new(),
            added: 0,
            count: p.count,
            bytes: 0,
            skin_names: Vec::new(),
        })
        .collect();
    // Two entries with one id would be a broken index; the first one stands.
    let mut seen = std::collections::HashSet::new();
    let records: Vec<PackRecord> = records
        .into_iter()
        .filter(|r| seen.insert(r.id.clone()))
        .collect();
    let catalog = Catalog::from_connection(build::in_memory(&records)?)?;
    Ok(Source {
        catalog: Mutex::new(catalog),
        bases: vec![base.to_string()],
        first: AtomicUsize::new(0),
        tree: false,
        featured: Vec::new(),
        generation: String::new(),
        offline,
    })
}

/// Writes `bytes` to `<dir>/<name>`, for the next offline visit. A failure only means there
/// won't be one.
fn keep(dir: &Path, name: &str, bytes: &[u8]) {
    let _ = std::fs::create_dir_all(dir).and_then(|()| write_atomic(&dir.join(name), bytes));
}

/// Runs `f` on a blocking thread: unpacking and opening a catalog is too much for the async ones.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
