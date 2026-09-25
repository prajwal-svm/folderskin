//! The community catalog as the app holds it: which one is current, where its files come from,
//! and searching it.
//!
//! The first search fetches `v2/head.json` from the community folder. It names the current
//! catalog, which is downloaded once, checked against its SHA-256 and kept in the app's cache
//! folder under its generation; every search after that is answered on this computer, in a few
//! milliseconds, with no request at all. Until a tree is published (there is no head.json), the
//! catalog is made in memory from `index.json`, as the app read it before, so nothing regresses.
//! When the packs published now can't be had (no connection, the host answering with an error,
//! a catalog that doesn't arrive), the catalog from the last visit stands in and says why, and
//! the first request a minute later asks again.
//!
//! head.json, or index.json standing in for it, is the one file fetched past the caches, and
//! only for Refresh: everything else is named after its contents.

use crate::community::{fetch, host, uncached, Fetch};
use crate::previews::{DiskCache, CACHE_BYTES};
use folderskin_catalog::tree::{self, Head};
use folderskin_catalog::{build, Catalog, PackRecord, PackRow, Query, Results};
use folderskin_core::apply::paths::write_atomic;
use folderskin_core::pack::{self, Index};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Runtime};

/// How many strips and thumbnails download at once.
pub const PREVIEW_DOWNLOADS: usize = 6;
/// The largest `index.json` the app downloads.
pub const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
/// How long the packs from the last visit stand in before the next request asks again for the
/// ones published now.
pub const RETRY_LAST_VISIT: Duration = Duration::from_secs(60);
/// The most the strips of a list made from index.json may take in memory.
const SESSION_STRIP_BYTES: usize = 32 * 1024 * 1024;

/// The community packs as this session knows them. Managed by Tauri, one per app.
pub struct Community {
    /// The catalog in use, and when it was loaded or last asked about again. Swapped whole and
    /// never locked across a download, so a search is answered from it even while Refresh
    /// fetches the next one.
    in_use: Mutex<Option<InUse>>,
    /// Held while a catalog loads, so the searches the Community view sends as it opens wait
    /// for one download instead of each starting one, and two Refreshes don't both download.
    loading: tokio::sync::Mutex<()>,
    retry_after: Duration,
    /// `<app cache>/community`, once the app has said where that is; `None` inside when it has
    /// none, and then catalogs live in memory and nothing is kept between sessions.
    cache: OnceLock<Option<PathBuf>>,
    files: OnceLock<Option<DiskCache>>,
    downloads: tokio::sync::Semaphore,
}

#[derive(Clone)]
struct InUse {
    source: Arc<Source>,
    since: Instant,
}

impl Default for Community {
    fn default() -> Self {
        Community {
            in_use: Mutex::new(None),
            loading: tokio::sync::Mutex::new(()),
            retry_after: RETRY_LAST_VISIT,
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

    /// The same, asking again after `after` rather than a minute while the last visit's packs
    /// stand in.
    #[cfg(test)]
    pub fn retrying_after(mut self, after: Duration) -> Community {
        self.retry_after = after;
        self
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

    /// The catalog in use, loading it from the community folder at `base` the first time. While
    /// the last visit's catalog stands in, the first call after [`RETRY_LAST_VISIT`] asks again
    /// for the one published now.
    pub async fn current(&self, base: &str) -> Result<Arc<Source>, String> {
        if let Some(in_use) = self.in_use() {
            if in_use.source.last_visit.is_some() && in_use.since.elapsed() >= self.retry_after {
                return Ok(self.try_again(base, in_use.source).await);
            }
            return Ok(in_use.source);
        }
        let _loading = self.loading.lock().await;
        // Another call may have loaded it while this one waited its turn.
        if let Some(in_use) = self.in_use() {
            return Ok(in_use.source);
        }
        Ok(self.put(load(base, self.cache(), false).await?))
    }

    /// Asks the community folder at `base` again, past every cache, for Refresh. When that fails
    /// the catalog in use stays. Searches carry on with it meanwhile.
    pub async fn refresh(&self, base: &str) -> Result<Arc<Source>, String> {
        let _loading = self.loading.lock().await;
        Ok(self.put(load(base, self.cache(), true).await?))
    }

    /// Asks again for the packs published now while `old`, the last visit's, stands in. One call
    /// asks; the ones that come meanwhile carry on with `old`. Whatever the answer, nobody asks
    /// again until [`RETRY_LAST_VISIT`] has passed once more.
    async fn try_again(&self, base: &str, old: Arc<Source>) -> Arc<Source> {
        let Ok(_loading) = self.loading.try_lock() else {
            return old;
        };
        match lock(&self.in_use).as_mut() {
            Some(now) if Arc::ptr_eq(&now.source, &old) => now.since = Instant::now(),
            // Refresh put another in while this call was on its way here.
            Some(now) => return now.source.clone(),
            None => {}
        }
        match load_current(base, self.cache(), false).await {
            Ok(source) => self.put(source),
            Err(_) => old,
        }
    }

    fn in_use(&self) -> Option<InUse> {
        lock(&self.in_use).clone()
    }

    fn put(&self, source: Source) -> Arc<Source> {
        let source = Arc::new(source);
        *lock(&self.in_use) = Some(InUse {
            source: source.clone(),
            since: Instant::now(),
        });
        source
    }
}

/// Why the catalog from the last visit stands in for the one published now. Reads as the start
/// of a sentence: "you're offline, so these are the packs from your last visit".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LastVisit {
    /// Nothing answered.
    Offline,
    /// This host answered, but not with the packs: an error such as 503, or a page that isn't
    /// the list.
    Unanswered(String),
    /// head.json named a catalog that didn't arrive whole.
    Behind,
}

impl std::fmt::Display for LastVisit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LastVisit::Offline => write!(f, "you're offline"),
            LastVisit::Unanswered(host) => write!(f, "{host} isn't answering"),
            LastVisit::Behind => write!(f, "the newest list of packs didn't arrive"),
        }
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
    /// The packs the maintainer marks as official: `official` in head.json, or the entries of
    /// index.json that say `"official": true`.
    pub official: Vec<String>,
    /// Which catalog this is: a tree's generation, or sixteen hex digits of the SHA-256 of the
    /// index.json it was made from.
    pub generation: String,
    /// Why this is the catalog from the last visit; `None` when it is the one published now.
    pub last_visit: Option<LastVisit>,
    /// The preview strips fetched for a list made from index.json. Those are replaced in place
    /// (`previews/<id>.png`), so nothing ties one to a version: they are kept only as long as
    /// the list that asked for them, and never on disk. By pack id, with the bytes they take.
    strips: Mutex<(HashMap<String, Vec<u8>>, usize)>,
}

impl Source {
    fn new(
        catalog: Catalog,
        bases: Vec<String>,
        tree: bool,
        generation: String,
        last_visit: Option<LastVisit>,
    ) -> Source {
        Source {
            catalog: Mutex::new(catalog),
            bases,
            first: AtomicUsize::new(0),
            tree,
            featured: Vec::new(),
            official: Vec::new(),
            generation,
            last_visit,
            strips: Mutex::default(),
        }
    }

    /// True for a published tree, false for a list made from index.json.
    pub fn is_tree(&self) -> bool {
        self.tree
    }

    /// True for a pack the maintainer marks as official.
    pub fn is_official(&self, id: &str) -> bool {
        self.official.iter().any(|o| o == id)
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

    /// Downloads `path` from the first folder that has it whole (`whole` says whether bytes are
    /// the file), trying the next one when a folder can't be reached, hasn't got the file yet or
    /// has the wrong bytes (a mirror a moment behind the others, or serving an old copy).
    pub async fn get(
        &self,
        path: &str,
        max: usize,
        whole: impl Fn(&[u8]) -> bool,
    ) -> Result<Vec<u8>, Fetch> {
        let first = self.first.load(Ordering::Relaxed).min(self.bases.len() - 1);
        let order = (first..self.bases.len()).chain(0..first);
        let mut error = None;
        for i in order {
            match fetch(&format!("{}/{path}", self.bases[i]), max).await {
                Ok(bytes) if whole(&bytes) => {
                    if i != first {
                        self.first.store(i, Ordering::Relaxed);
                    }
                    return Ok(bytes);
                }
                Ok(_) => error = Some(Fetch::Damaged),
                // Wrong bytes somewhere say more than a folder that hasn't got the file.
                Err(e) if error != Some(Fetch::Damaged) => error = Some(e),
                Err(_) => {}
            }
        }
        Err(error.unwrap_or(Fetch::Damaged))
    }

    /// A strip kept for this list, when it came from index.json.
    pub fn session_strip(&self, id: &str) -> Option<Vec<u8>> {
        lock(&self.strips).0.get(id).cloned()
    }

    /// Keeps a strip for this list. Past [`SESSION_STRIP_BYTES`] the ones kept go, to be
    /// downloaded again if they are looked at again.
    pub fn keep_session_strip(&self, id: &str, bytes: Vec<u8>) {
        let mut strips = lock(&self.strips);
        if strips.1 + bytes.len() > SESSION_STRIP_BYTES {
            *strips = Default::default();
        }
        strips.1 += bytes.len();
        if let Some(old) = strips.0.insert(id.to_string(), bytes) {
            strips.1 -= old.len();
        }
    }
}

/// Why the packs published now didn't load, and what stands in for them if anything may.
pub struct Failed {
    pub error: String,
    /// What to say if the last visit's packs stand in; `None` when they mustn't, as when no
    /// packs are published any more.
    last_visit: Option<LastVisit>,
}

impl Failed {
    fn new(error: impl Into<String>, last_visit: Option<LastVisit>) -> Failed {
        Failed {
            error: error.into(),
            last_visit,
        }
    }
}

/// Loads the current catalog from the community folder at `base`: the published tree when there
/// is one, index.json when there isn't, and the last visit's copy when neither can be had (only
/// when not `fresh`: Refresh says it failed rather than pass old packs off as new).
pub async fn load(base: &str, cache: Option<&Path>, fresh: bool) -> Result<Source, String> {
    match load_current(base, cache, fresh).await {
        Ok(source) => Ok(source),
        Err(Failed {
            error,
            last_visit: Some(why),
        }) if !fresh => match cache {
            Some(dir) => last_visit(base, dir, why).await.ok_or(error),
            None => Err(error),
        },
        Err(failed) => Err(failed.error),
    }
}

/// [`load`] without the last visit's copy to fall back on.
pub async fn load_current(base: &str, cache: Option<&Path>, fresh: bool) -> Result<Source, Failed> {
    let head_url = format!("{base}/v{}/{}", tree::HEAD_VERSION, tree::HEAD_FILE);
    let bytes = match fetch(&uncached(&head_url, fresh), tree::MAX_HEAD_BYTES).await {
        Ok(bytes) => bytes,
        Err(Fetch::NotFound(_)) => return load_index(base, cache, fresh).await,
        Err(e) => return Err(Failed::new(e.to_string(), Some(why(base, &e)))),
    };
    // A page that isn't head.json at all is a host not answering properly, such as a network's
    // sign-in page.
    let head =
        Head::parse(&bytes).map_err(|e| Failed::new(e, Some(LastVisit::Unanswered(host(base)))))?;
    let source = from_head(&head, tree_bases(base, &head), cache, None)
        .await
        .map_err(|e| Failed::new(e, Some(LastVisit::Behind)))?;
    if let Some(dir) = cache {
        keep(dir, tree::HEAD_FILE, &bytes);
    }
    Ok(source)
}

/// The list made from index.json, when no tree is published.
async fn load_index(base: &str, cache: Option<&Path>, fresh: bool) -> Result<Source, Failed> {
    let index_url = format!("{base}/index.json");
    let bytes = match fetch(&uncached(&index_url, fresh), MAX_INDEX_BYTES).await {
        Ok(bytes) => bytes,
        Err(Fetch::NotFound(_)) => {
            return Err(Failed::new(
                "the community packs aren't published yet",
                None,
            ))
        }
        Err(e) => return Err(Failed::new(e.to_string(), Some(why(base, &e)))),
    };
    let source = from_index(&bytes, base, None)
        .map_err(|e| Failed::new(e, Some(LastVisit::Unanswered(host(base)))))?;
    if let Some(dir) = cache {
        keep(dir, "index.json", &bytes);
    }
    Ok(source)
}

/// What to say when a download from `base` fails with `e` and the last visit's packs stand in.
fn why(base: &str, e: &Fetch) -> LastVisit {
    if e.is_offline() {
        LastVisit::Offline
    } else {
        LastVisit::Unanswered(host(base))
    }
}

/// The catalog the last visit used, from the cache folder, saying `why` it stands in.
async fn last_visit(base: &str, dir: &Path, why: LastVisit) -> Option<Source> {
    if let Some(head) = std::fs::read(dir.join(tree::HEAD_FILE))
        .ok()
        .and_then(|bytes| Head::parse(&bytes).ok())
    {
        let kept = from_head(&head, tree_bases(base, &head), Some(dir), Some(why.clone()));
        if let Ok(source) = kept.await {
            return Some(source);
        }
    }
    let index = std::fs::read(dir.join("index.json")).ok()?;
    from_index(&index, base, Some(why)).ok()
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
/// otherwise. For the last visit (`last_visit` says why it stands in) only a kept copy will do.
async fn from_head(
    head: &Head,
    bases: Vec<String>,
    cache: Option<&Path>,
    last_visit: Option<LastVisit>,
) -> Result<Source, String> {
    let kept = cache.map(|dir| dir.join(catalog_file(&head.generation)));
    let opened = match kept.clone().filter(|path| path.is_file()) {
        Some(path) => {
            let generation = head.generation.clone();
            blocking(move || open_kept(&path, &generation)).await.ok()
        }
        None => None,
    };
    let catalog = match opened {
        Some(catalog) => catalog,
        None if last_visit.is_some() => {
            return Err("the packs from the last visit aren't kept".into())
        }
        None => download_catalog(head, &bases, kept).await?,
    };
    let mut source = Source::new(catalog, bases, true, head.generation.clone(), last_visit);
    source.featured = head.featured_ids();
    source.official = head.official_ids();
    Ok(source)
}

/// The catalog kept at `path`, when it is still the file `generation` names: one damaged on disk
/// since it was kept is deleted, to be downloaded again, rather than searched.
fn open_kept(path: &Path, generation: &str) -> Result<Catalog, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if tree::sha256_hex(&bytes)[..16] != *generation {
        let _ = std::fs::remove_file(path);
        return Err("the kept catalog of community packs is damaged".into());
    }
    Catalog::open(path)
}

/// Downloads the catalog `head` names from the first of `bases` that has it whole, and keeps it
/// at `keep` (then opens it there) or in memory. The other generations kept beside it go.
async fn download_catalog(
    head: &Head,
    bases: &[String],
    keep: Option<PathBuf>,
) -> Result<Catalog, String> {
    let url = &head.catalog.url;
    let max = head.catalog.bytes as usize;
    let tries: Vec<String> = if url.starts_with("https://") {
        vec![url.clone()]
    } else {
        bases.iter().map(|base| format!("{base}/{url}")).collect()
    };
    let mut error = None;
    let mut arrived = None;
    for from in tries {
        match fetch(&from, max).await {
            Ok(gz) => {
                let want = head.catalog.clone();
                let (gz, whole) = blocking(move || {
                    let whole =
                        gz.len() as u64 == want.bytes && tree::sha256_hex(&gz) == want.sha256;
                    Ok((gz, whole))
                })
                .await?;
                if whole {
                    arrived = Some(gz);
                    break;
                }
                // A mirror with an old or wrong copy: the next folder may have the right one.
                error = Some(Fetch::Damaged);
            }
            Err(e) if error != Some(Fetch::Damaged) => error = Some(e),
            Err(_) => {}
        }
    }
    let Some(gz) = arrived else {
        return Err(match error {
            Some(Fetch::NotFound(_)) | None => {
                "the newest list of packs isn't there yet; try again in a minute".into()
            }
            // Its size is known, so one bigger is the wrong file too.
            Some(Fetch::Damaged | Fetch::TooBig(_)) => DAMAGED.into(),
            Some(e) => e.to_string(),
        });
    };
    let generation = head.generation.clone();
    blocking(move || {
        let bytes = tree::gunzip(&gz, tree::MAX_CATALOG_UNPACKED)?;
        if tree::sha256_hex(&bytes)[..16] != generation {
            return Err(DAMAGED.into());
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

const DAMAGED: &str = "the catalog of community packs arrived damaged; try again";

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

/// A catalog made in memory from `index.json`: the packs with their counts and, where the index
/// says, when each was added and whether it is official; no skin names. `last_visit` says why,
/// when it is the last visit's copy.
pub fn from_index(
    bytes: &[u8],
    base: &str,
    last_visit: Option<LastVisit>,
) -> Result<Source, String> {
    let index = Index::parse(bytes)?;
    // Two entries with one id would be a broken index; the first one stands.
    let mut seen = std::collections::HashSet::new();
    let entries: Vec<_> = index
        .packs
        .into_iter()
        .filter(|p| pack::is_pack_id(&p.id) && seen.insert(p.id.clone()))
        .collect();
    let official: Vec<String> = entries
        .iter()
        .filter(|p| p.official)
        .map(|p| p.id.clone())
        .collect();
    let records: Vec<PackRecord> = entries
        .into_iter()
        .map(|p| PackRecord {
            tags: pack::clean_tags(&p.tags, usize::MAX),
            id: p.id,
            name: p.name,
            author: p.author,
            license: p.license,
            hash: p.hash,
            manifest: String::new(),
            added: p.added.unwrap_or(0).max(0),
            count: p.count,
            bytes: 0,
            skin_names: Vec::new(),
        })
        .collect();
    let catalog = Catalog::from_connection(build::in_memory(&records)?)?;
    let generation = tree::sha256_hex(bytes)[..16].to_string();
    let mut source = Source::new(
        catalog,
        vec![base.to_string()],
        false,
        generation,
        last_visit,
    );
    source.official = official;
    Ok(source)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::tests::{colour_packs, png, serve_logged, tree_files, Served};
    use tauri::async_runtime::block_on;

    #[test]
    fn a_folder_with_the_wrong_bytes_is_passed_over_for_the_next() {
        let (files, head, _) = tree_files(&colour_packs());
        let (other, _, _) = tree_files(&colour_packs()[..1]);
        let other_catalog = other
            .into_iter()
            .find(|(path, ..)| path.ends_with(".sqlite.gz"))
            .unwrap()
            .1;
        // A mirror serving another generation's catalog under this one's name, and other
        // pictures under their names: an old copy, or one that was never right.
        let wrong: Served = files
            .iter()
            .map(|(path, body, delay)| {
                let body = if path.ends_with(".sqlite.gz") {
                    other_catalog.clone()
                } else if path.starts_with("/v2/pictures/") {
                    png(256, 256, [1, 2, 3, 255])
                } else {
                    body.clone()
                };
                (path.clone(), body, *delay)
            })
            .collect();
        let (mirror, _, mirror_asked) = serve_logged(wrong);
        let (github, _, _) = serve_logged(files);
        let bases = vec![format!("{mirror}/v2"), format!("{github}/v2")];

        let catalog = block_on(download_catalog(&head, &bases, None)).unwrap();
        assert_eq!(
            catalog.counts().0,
            2,
            "the right copy, from the next folder"
        );
        assert!(lock(&mirror_asked)
            .iter()
            .any(|path| path.ends_with(".sqlite.gz")));

        let source = Source::new(catalog, bases, true, head.generation.clone(), None);
        let navy = colour_packs()[1].3[0].2.clone();
        let sha = tree::sha256_hex(&navy);
        let path = tree::picture_path(&sha, "png");
        let is_navy = |bytes: &[u8]| tree::sha256_hex(bytes) == sha;
        assert_eq!(block_on(source.get(&path, 1 << 20, is_navy)).unwrap(), navy);
        assert!(lock(&mirror_asked).iter().any(|p| p.contains(&sha)));

        // Wrong everywhere is damaged, rather than whatever the last folder said.
        let nowhere_right = block_on(source.get(&path, 1 << 20, |_| false)).unwrap_err();
        assert_eq!(nowhere_right, Fetch::Damaged);
        let missing = block_on(source.get("pictures/none.png", 1 << 20, |_| true)).unwrap_err();
        assert_eq!(missing, Fetch::NotFound("127.0.0.1".into()));
    }
}
