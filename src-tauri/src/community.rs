//! Community skin packs: the list on GitHub, each pack's preview, adding and removing a pack,
//! adding one from a folder on this computer, and saving your own skins as a pack to share.
//!
//! Packs live in the FolderSkin repository under `community/`, so reading them needs no account
//! and no server: `index.json` lists them, `previews/<id>.png` shows a few skins of each, and
//! `packs/<id>/` is the pack itself. The rules every pack follows are in
//! `folderskin_core::pack`, and docs/PACKS.md says the same in prose.

use crate::commands::{data_url, prepare_import, SkinDto};
use crate::pack_views::PackViews;
use crate::state::{parallel_map, AppState};
use crate::store::{self, NewSkin, SkinImage, SkinSource};
use folderskin_core::pack::{self, Index, Pack, PackSkin};
use futures_util::{StreamExt, TryStreamExt};
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

/// The `community/` folder of the repository on GitHub. Set `FOLDERSKIN_COMMUNITY_URL` to read
/// another copy of it instead, such as a checkout served locally while testing.
const COMMUNITY_URL: &str =
    "https://raw.githubusercontent.com/prajwal-svm/folderskin/main/community";
/// The largest `index.json` and preview the app downloads.
const MAX_INDEX_BYTES: usize = 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 1024 * 1024;
/// Shown when GitHub cannot be reached at all.
const OFFLINE: &str = "couldn't reach GitHub. Check your connection and try again";
/// How many of a pack's pictures download at once.
const PARALLEL_DOWNLOADS: usize = 4;

/// One pack in the Community list.
#[derive(Serialize)]
pub struct PackDto {
    pub id: String,
    pub name: String,
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub count: usize,
    /// The version on GitHub now (`pack::pack_hash`); empty when the index has none.
    pub hash: String,
    /// True when the pack's skins are in the library.
    pub added: bool,
    /// True when it was added and GitHub has a different version of it now.
    pub update: bool,
}

/// One skin of a pack being looked through before it's added.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PackSkinDto {
    pub name: String,
    pub tags: Vec<String>,
    /// The skin as the folder it makes, as a PNG data URL.
    pub thumbnail: String,
}

/// How big the folders are drawn when looking through a pack.
const PACK_VIEW_SIZE: u32 = 256;

/// How far adding a pack has got, sent to the webview as it goes:
/// `{"stage": "download", "done": 3, "total": 16}`.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackProgress {
    pub stage: Stage,
    /// Pictures done in this stage, out of `total`.
    pub done: usize,
    /// How many pictures the pack has.
    pub total: usize,
}

/// The two stages of adding a pack, in order.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// The pictures are downloading; `done` counts the ones that have arrived.
    Download,
    /// They are being checked, encoded and saved; `done` reaches `total` once all are saved.
    Save,
}

impl PackProgress {
    fn download(done: usize, total: usize) -> PackProgress {
        PackProgress {
            stage: Stage::Download,
            done,
            total,
        }
    }

    fn save(done: usize, total: usize) -> PackProgress {
        PackProgress {
            stage: Stage::Save,
            done,
            total,
        }
    }
}

/// Where the commands that report no progress send it.
fn no_progress(_: PackProgress) {}

/// What updating a pack changed: the old skins the new version doesn't have, and the new
/// version's skins.
#[derive(Serialize)]
pub struct PackUpdateDto {
    pub removed: Vec<String>,
    pub skins: Vec<SkinDto>,
}

/// The packs on GitHub, each marked with whether it has been added and whether it has changed
/// since. `fresh` asks past every cache on the way, for Refresh.
#[tauri::command]
pub async fn community_packs(
    state: State<'_, AppState>,
    fresh: bool,
) -> Result<Vec<PackDto>, String> {
    let url = format!("{}/index.json", base_url());
    let bytes = fetch(&uncached(&url, fresh), MAX_INDEX_BYTES)
        .await
        .map_err(|e| {
            if e == NOT_FOUND {
                "the community packs aren't published yet".to_string()
            } else {
                e
            }
        })?;
    let index = Index::parse(&bytes)?;
    let installed = state.installed_packs();
    Ok(index
        .packs
        .into_iter()
        .filter(|p| pack::is_pack_id(&p.id))
        .map(|p| {
            let have = installed.get(&p.id);
            PackDto {
                added: have.is_some(),
                update: has_update(have, &p.hash),
                id: p.id,
                name: p.name,
                author: p.author,
                license: p.license,
                tags: pack::clean_tags(&p.tags, usize::MAX),
                count: p.count,
                hash: p.hash,
            }
        })
        .collect())
}

/// Whether a pack added with hash `have` (itself `None` when it was added before FolderSkin kept
/// one) differs from the version the index lists as `listed`. An index without hashes offers no
/// updates; a pack with no recorded hash is offered one, since nothing says it's current.
fn has_update(have: Option<&Option<String>>, listed: &str) -> bool {
    match have {
        None => false,
        Some(_) if listed.is_empty() => false,
        Some(hash) => hash.as_deref() != Some(listed),
    }
}

/// A pack's preview (a few of its skins as folders, side by side) as a PNG data URL. Each is
/// downloaded once per session, and again when `fresh`.
#[tauri::command]
pub async fn community_preview(pack_id: String, fresh: bool) -> Result<String, String> {
    static PREVIEWS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let previews = PREVIEWS.get_or_init(Default::default);
    if !pack::is_pack_id(&pack_id) {
        return Err("that isn't a pack".into());
    }
    if !fresh {
        if let Some(url) = lock(previews).get(&pack_id) {
            return Ok(url.clone());
        }
    }
    let url = format!("{}/previews/{pack_id}.png", base_url());
    let png = fetch(&uncached(&url, fresh), MAX_PREVIEW_BYTES).await?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("that preview isn't a PNG".into());
    }
    let url = data_url(&png);
    lock(previews).insert(pack_id, url.clone());
    Ok(url)
}

/// Downloads a pack and saves its skins, all of them or none, and returns them in the pack's
/// order. The pictures download a few at a time and every one is checked before any is saved,
/// then they are saved in one go, so a dropped connection, a bad file or a full disk never
/// leaves half a pack behind.
///
/// `on_progress` hears how far it has got: `download` with nothing arrived, then as each picture
/// arrives; then `save` with nothing saved, as each is ready to write, and with all of them once
/// they are saved.
#[tauri::command]
pub async fn community_add(
    state: State<'_, AppState>,
    pack_id: String,
    on_progress: Channel<PackProgress>,
) -> Result<Vec<SkinDto>, String> {
    // The window may have gone, and the pack is saved all the same, so a failed send is ignored.
    let progress = move |p: PackProgress| {
        let _ = on_progress.send(p);
    };
    let (pack, hash, pictures) = download_pack(&pack_id, &progress).await?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        save_pack(&state, &pack_id, &pack, &pictures, Some(hash), &progress)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Every skin of pack `pack_id`, drawn as the folder it makes, to look through before adding it.
/// It downloads and checks the whole pack as adding it would, and adds nothing to the library.
/// The drawings are kept for a while (pack_views.rs), so looking again at the version the list
/// names as `hash` shows them straight away, with nothing downloaded.
#[tauri::command]
pub async fn community_pack_skins(
    app: AppHandle,
    pack_id: String,
    hash: String,
) -> Result<Vec<PackSkinDto>, String> {
    let views = app
        .path()
        .app_cache_dir()
        .ok()
        .map(|dir| PackViews::new(dir.join("pack-views")));
    pack_skins(&base_url(), views, pack_id, hash).await
}

/// [`community_pack_skins`] with the pack read from the copy of `community/` at `base`, and the
/// drawings kept in `views` (none: nowhere to keep them).
async fn pack_skins(
    base: &str,
    views: Option<PackViews>,
    pack_id: String,
    hash: String,
) -> Result<Vec<PackSkinDto>, String> {
    if let Some(views) = views.clone() {
        let id = pack_id.clone();
        let kept =
            tauri::async_runtime::spawn_blocking(move || views.get(&id, &hash, SystemTime::now()))
                .await
                .ok()
                .flatten();
        if let Some(skins) = kept {
            return Ok(skins);
        }
    }
    // Kept under the hash of what actually arrived, which the list names too unless the pack
    // changed in between; then the next look downloads it again rather than show a mismatch.
    let (pack, downloaded, pictures) = download_pack_from(base, &pack_id, &no_progress).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let ready = prepare_pack(&pack, &pictures)?;
        let skins = parallel_map(&ready, |(skin, _, image)| PackSkinDto {
            name: skin.name.trim().to_string(),
            tags: pack.tags_for(skin),
            thumbnail: data_url(&image.preview_png(PACK_VIEW_SIZE)),
        });
        if let Some(views) = views {
            views.put(&pack_id, &downloaded, &skins, SystemTime::now());
        }
        Ok(skins)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Replaces an added pack's skins with the version on GitHub now. The new version is downloaded
/// and checked before the old skins go, so a failed update leaves the pack as it was.
#[tauri::command]
pub async fn community_update(
    state: State<'_, AppState>,
    pack_id: String,
) -> Result<PackUpdateDto, String> {
    let (pack, hash, pictures) = download_pack(&pack_id, &no_progress).await?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        replace_pack(&state, &pack_id, &pack, &pictures, hash)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pack `pack_id` from GitHub, fetched past any cache: what it lists, its hash
/// ([`pack::pack_hash`]) and every picture in the pack's order, each within the pack limits.
async fn download_pack(
    pack_id: &str,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<(Pack, String, Vec<Vec<u8>>), String> {
    download_pack_from(&base_url(), pack_id, progress).await
}

/// [`download_pack`] from the copy of `community/` at `base`. The pictures download
/// [`PARALLEL_DOWNLOADS`] at a time; `progress` hears `download` with none arrived once the
/// pack's list is in, then again as each one arrives. The first picture that fails, in the
/// pack's order, is the error, and the downloads still going are dropped.
async fn download_pack_from(
    base: &str,
    pack_id: &str,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<(Pack, String, Vec<Vec<u8>>), String> {
    if !pack::is_pack_id(pack_id) {
        return Err("that isn't a pack".into());
    }
    let base = format!("{base}/packs/{pack_id}");
    let manifest = fetch(
        &uncached(&format!("{base}/{}", pack::MANIFEST_FILE), true),
        pack::MAX_MANIFEST_BYTES,
    )
    .await?;
    let pack = Pack::parse(&manifest).map_err(|problems| problems.join("; "))?;
    let total = pack.skins.len();
    progress(PackProgress::download(0, total));
    let arrived = AtomicUsize::new(0);
    // Owned names rather than borrowed skins: a future borrowing its stream's items can't be
    // proved `Send`, which every command's future must be.
    let files: Vec<String> = pack.skins.iter().map(|s| s.file.clone()).collect();
    let pictures: Vec<Vec<u8>> = futures_util::stream::iter(files)
        .map(|file| {
            // `file` passed `is_picture_file_name`, so it cannot leave the pack's folder.
            let url = uncached(&format!("{base}/{file}"), true);
            let arrived = &arrived;
            async move {
                let bytes = fetch(&url, pack::MAX_PICTURE_BYTES)
                    .await
                    .map_err(|e| format!("{file}: {e}"))?;
                let done = arrived.fetch_add(1, Ordering::Relaxed) + 1;
                progress(PackProgress::download(done, total));
                Ok::<_, String>(bytes)
            }
        })
        // Kept in the pack's order, whichever arrives first.
        .buffered(PARALLEL_DOWNLOADS)
        .try_collect()
        .await?;
    let hash = pack::pack_hash(
        &manifest,
        pack.skins
            .iter()
            .zip(&pictures)
            .map(|(s, bytes)| (s.file.as_str(), bytes.as_slice())),
    );
    Ok((pack, hash, pictures))
}

/// Deletes every skin a pack added and returns their ids.
#[tauri::command]
pub async fn community_remove(
    state: State<'_, AppState>,
    pack_id: String,
) -> Result<Vec<String>, String> {
    if !pack::is_pack_id(&pack_id) {
        return Err("that isn't a pack".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_pack(&pack_id))
        .await
        .map_err(|e| e.to_string())?
}

/// Adds a pack from a folder on this computer, with the same checks as one from GitHub. The
/// folder's name is the pack's id, so a pack tried out before it is shared and the same pack
/// added from GitHub later are one pack.
#[tauri::command]
pub async fn import_pack(state: State<'_, AppState>, path: String) -> Result<Vec<SkinDto>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let dir = PathBuf::from(&path);
        let id = dir
            .file_name()
            .map(|name| pack::slug(&name.to_string_lossy()))
            .filter(|id| pack::is_pack_id(id))
            .ok_or_else(|| "that folder's name has no letters or digits to use".to_string())?;
        let manifest = read_capped(&dir.join(pack::MANIFEST_FILE), pack::MAX_MANIFEST_BYTES)
            .map_err(|_| "that folder has no pack.json. See docs/PACKS.md".to_string())?;
        let pack = Pack::parse(&manifest).map_err(|problems| problems.join("; "))?;
        // GitHub's URLs are case-sensitive, so a name that only matches in another case here
        // would work on this Mac and then fail for everyone else.
        let on_disk: std::collections::HashSet<String> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        if let Some(skin) = pack.skins.iter().find(|s| !on_disk.contains(&s.file)) {
            return Err(format!(
                "{} is missing. File names have to match exactly, capitals included",
                skin.file
            ));
        }
        let pictures = pack
            .skins
            .iter()
            .map(|s| {
                read_capped(&dir.join(&s.file), pack::MAX_PICTURE_BYTES)
                    .map_err(|e| format!("{} {e}", s.file))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let hash = pack::pack_hash(
            &manifest,
            pack.skins
                .iter()
                .zip(&pictures)
                .map(|(s, bytes)| (s.file.as_str(), bytes.as_slice())),
        );
        save_pack(&state, &id, &pack, &pictures, Some(hash), &no_progress)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Writes some of the user's own skins as a pack folder inside `folder`, ready to upload to
/// GitHub, and returns the folder it made. The pack is checked like any other before anything
/// is written, so what this makes passes the pull-request checks.
#[tauri::command]
pub async fn export_pack(
    state: State<'_, AppState>,
    folder: String,
    name: String,
    author: String,
    license: String,
    tags: Vec<String>,
    skin_ids: Vec<String>,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let id = pack::slug(&name);
        if !pack::is_pack_id(&id) {
            return Err("give the pack a name with letters or digits in it".into());
        }
        let out = PathBuf::from(folder).join(&id);
        if out.exists() {
            return Err(format!("there's already a folder called {id} there"));
        }
        let pack_tags = pack::clean_tags(&tags, pack::MAX_PACK_TAGS);
        let mut skins = Vec::new();
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for skin_id in &skin_ids {
            let (entry, _) = state
                .find_saved(skin_id)
                .ok_or_else(|| "one of those skins isn't saved any more".to_string())?;
            if entry.source == SkinSource::Community {
                return Err(format!(
                    "{} came from someone else's pack, so it can't go in yours",
                    entry.name
                ));
            }
            let image = state.resolve(skin_id)?;
            let (bytes, ext) =
                encode_for_pack(&image).map_err(|e| format!("{} {e}", entry.name))?;
            let stem = unique_stem(&entry.name, files.len(), &files);
            let file = format!("{stem}.{ext}");
            let own: Vec<&String> = entry
                .tags
                .iter()
                .filter(|t| !pack_tags.contains(t))
                .collect();
            skins.push(PackSkin {
                file: file.clone(),
                name: entry.name.clone(),
                tags: pack::clean_tags(own, pack::MAX_SKIN_TAGS),
            });
            files.push((file, bytes));
        }
        let pack = Pack {
            version: pack::PACK_VERSION,
            name: name.trim().to_string(),
            author: author.trim().to_string(),
            license,
            tags: pack_tags,
            skins,
        };
        let problems = pack.problems();
        if !problems.is_empty() {
            return Err(problems.join("; "));
        }
        let json = serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())? + "\n";
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(&out)?;
            for (file, bytes) in &files {
                std::fs::write(out.join(file), bytes)?;
            }
            std::fs::write(out.join(pack::MANIFEST_FILE), json)
        };
        write().map_err(|e| {
            let _ = std::fs::remove_dir_all(&out);
            format!("couldn't write the pack: {e}")
        })?;
        Ok(out.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- helpers ----------

const NOT_FOUND: &str = "not found";

/// `url` with a query no cache has seen when `fresh`, so the answer comes from GitHub itself
/// rather than a copy up to a few minutes old.
fn uncached(url: &str, fresh: bool) -> String {
    if fresh {
        format!("{url}?t={}", store::now_ms())
    } else {
        url.to_string()
    }
}

fn base_url() -> String {
    std::env::var("FOLDERSKIN_COMMUNITY_URL")
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| COMMUNITY_URL.to_string())
}

fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("FolderSkin/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Downloads `url`, refusing a body over `max` bytes, including one that never says its length.
async fn fetch(url: &str, max: usize) -> Result<Vec<u8>, String> {
    let mut response = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| OFFLINE.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NOT_FOUND.into());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let too_big = || format!("is over {} KB", max / 1024);
    if response.content_length().is_some_and(|n| n > max as u64) {
        return Err(too_big());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| OFFLINE.to_string())? {
        body.extend_from_slice(&chunk);
        if body.len() > max {
            return Err(too_big());
        }
    }
    Ok(body)
}

/// Checks and decodes every picture of a pack, and only then saves them all as community skins
/// carrying the pack's tags, its id and its `hash`. `progress` hears `save` with nothing saved
/// first, and then as [`store_pack`] goes.
fn save_pack(
    state: &AppState,
    pack_id: &str,
    pack: &Pack,
    pictures: &[Vec<u8>],
    hash: Option<String>,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<Vec<SkinDto>, String> {
    progress(PackProgress::save(0, pictures.len()));
    let ready = prepare_pack(pack, pictures)?;
    store_pack(state, pack_id, pack, ready, hash, progress)
}

/// Swaps pack `pack_id`'s saved skins for this version of it. The new pictures are checked
/// before the old skins are removed. A picture both versions share keeps its id, so a favourite
/// of it stays.
fn replace_pack(
    state: &AppState,
    pack_id: &str,
    pack: &Pack,
    pictures: &[Vec<u8>],
    hash: String,
) -> Result<PackUpdateDto, String> {
    let ready = prepare_pack(pack, pictures)?;
    let before = state.remove_pack(pack_id)?;
    let skins = store_pack(state, pack_id, pack, ready, Some(hash), &no_progress)?;
    let removed = before
        .into_iter()
        .filter(|id| !skins.iter().any(|s| &s.id == id))
        .collect();
    Ok(PackUpdateDto { removed, skins })
}

/// A pack's pictures decoded and checked, each with its skin and id; nothing saved yet. They
/// are decoded on all cores at once; the first picture that fails, in the pack's order, is the
/// error.
fn prepare_pack<'p>(
    pack: &'p Pack,
    pictures: &[Vec<u8>],
) -> Result<Vec<(&'p PackSkin, String, SkinImage)>, String> {
    let listed: Vec<(&PackSkin, &Vec<u8>)> = pack.skins.iter().zip(pictures).collect();
    parallel_map(&listed, |&(skin, bytes)| {
        let rgba = pack::decode_picture(bytes).map_err(|e| format!("{} {e}", skin.file))?;
        let image = prepare_import(rgba).map_err(|e| format!("{}: {e}", skin.file))?;
        Ok((skin, store::skin_id(bytes), image))
    })
    .into_iter()
    .collect()
}

/// Saves prepared pictures as community skins carrying the pack's tags, its id and its `hash`,
/// all of them or none ([`AppState::save_many`]), and returns them in the pack's order. The
/// first is the newest, so the library shows the pack in its own order too. A picture the pack
/// lists twice is one skin, returned once.
///
/// `progress` hears `save` as each picture is ready to write, and with all of them once they
/// are saved.
fn store_pack(
    state: &AppState,
    pack_id: &str,
    pack: &Pack,
    ready: Vec<(&PackSkin, String, SkinImage)>,
    hash: Option<String>,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<Vec<SkinDto>, String> {
    let total = ready.len();
    let skins = ready
        .into_iter()
        .map(|(skin, id, image)| {
            let new = NewSkin {
                id,
                name: skin.name.trim().to_string(),
                source: SkinSource::Community,
                provider: None,
                model: None,
                idea: None,
                tags: pack.tags_for(skin),
                pack: Some(pack_id.to_string()),
                pack_name: Some(pack.name.trim().to_string()),
                author: Some(pack.author.clone()),
                license: Some(pack.license.clone()),
                pack_hash: hash.clone(),
            };
            (new, image)
        })
        .collect();
    // The pictures are encoded on several threads at once; the count is kept and sent under one
    // lock, so the webview hears it go up in order.
    let count = Mutex::new(0);
    let saved = state.save_many(skins, &|| {
        let mut done = lock(&count);
        *done += 1;
        progress(PackProgress::save(*done, total));
    })?;
    progress(PackProgress::save(total, total));
    let mut seen = HashSet::new();
    Ok(saved
        .iter()
        .filter(|entry| seen.insert(entry.id.clone()))
        .map(SkinDto::of)
        .collect())
}

/// Reads a file of at most `max` bytes.
fn read_capped(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|_| "is missing".to_string())?;
    if !meta.is_file() {
        return Err("is missing".into());
    }
    if meta.len() > max as u64 {
        return Err(format!("is over {} KB", max / 1024));
    }
    std::fs::read(path).map_err(|_| "couldn't be read".into())
}

/// A skin's picture the way a pack wants it: at most 1024 px on a side and 2 MB. A finished
/// folder stays PNG to keep its transparency; artwork has none to keep, so it becomes a JPEG at
/// a fraction of the size.
fn encode_for_pack(image: &SkinImage) -> Result<(Vec<u8>, &'static str), String> {
    let too_small = |img: &image::RgbaImage| {
        let (w, h) = img.dimensions();
        (w.min(h) < pack::MIN_PICTURE_SIDE).then(|| {
            format!(
                "is {w}×{h} px; a pack needs at least {} px on each side",
                pack::MIN_PICTURE_SIDE
            )
        })
    };
    match image {
        SkinImage::Folder(rgba) => {
            for side in [pack::MAX_PICTURE_SIDE, 896, 768] {
                let img = store::shrink_to((**rgba).clone(), side);
                if let Some(e) = too_small(&img) {
                    return Err(e);
                }
                let png = folderskin_core::raster::encode_png(&img);
                if png.len() <= pack::MAX_PICTURE_BYTES {
                    return Ok((png, "png"));
                }
            }
            Err("is too detailed to fit in 2 MB".into())
        }
        SkinImage::Artwork(art) => {
            let img = store::shrink_to(art.rgba.clone(), pack::MAX_PICTURE_SIDE);
            if let Some(e) = too_small(&img) {
                return Err(e);
            }
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            let mut jpg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpg, 90)
                .write_image(&rgb, rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())?;
            Ok((jpg, "jpg"))
        }
    }
}

/// A file name stem for a skin: its name as a slug, or `skin-<n>`, never one already used.
fn unique_stem(name: &str, n: usize, used: &[(String, Vec<u8>)]) -> String {
    let base = Some(pack::slug(name))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("skin-{}", n + 1));
    let taken = |stem: &str| {
        used.iter()
            .any(|(file, _)| file.rsplit_once('.').is_some_and(|(s, _)| s == stem))
    };
    let mut stem = base.clone();
    let mut k = 2;
    while taken(&stem) {
        stem = format!("{base}-{k}");
        k += 1;
    }
    stem
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SkinKind;
    use std::io::Cursor;
    use std::sync::Arc;

    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(w, h, image::Rgba(rgba))
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "folderskin-community-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const PACK: &str = r#"{
  "version": 1,
  "name": "Test colours",
  "author": "prajwal-svm",
  "license": "CC0-1.0",
  "tags": ["colour"],
  "skins": [
    { "file": "teal.png", "name": "Teal", "tags": ["cool"] },
    { "file": "rust.png", "name": "Rust" }
  ]
}"#;

    #[test]
    fn a_pack_folder_is_saved_as_community_skins_and_removed_as_one() {
        let dir = temp_dir("import").join("test-colours");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pack.json"), PACK).unwrap();
        std::fs::write(dir.join("teal.png"), png(512, 480, [20, 140, 150, 255])).unwrap();
        std::fs::write(dir.join("rust.png"), png(512, 480, [180, 70, 30, 255])).unwrap();

        let state = AppState::default();
        state.open_store(temp_dir("import-store"));
        let manifest = std::fs::read(dir.join("pack.json")).unwrap();
        let pack = Pack::parse(&manifest).unwrap();
        let pictures: Vec<Vec<u8>> = pack
            .skins
            .iter()
            .map(|s| std::fs::read(dir.join(&s.file)).unwrap())
            .collect();
        let saved = save_pack(
            &state,
            "test-colours",
            &pack,
            &pictures,
            Some("v1".into()),
            &no_progress,
        )
        .unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].tags, ["colour", "cool"]);
        assert_eq!(saved[1].tags, ["colour"]);
        assert!(saved
            .iter()
            .all(|s| s.pack.as_deref() == Some("test-colours")));
        assert_eq!(saved[0].pack_name.as_deref(), Some("Test colours"));
        assert_eq!(saved[0].author.as_deref(), Some("prajwal-svm"));
        assert_eq!(saved[0].license.as_deref(), Some("CC0-1.0"));
        assert!(saved.iter().all(|s| s.kind == SkinKind::Artwork));
        assert_eq!(
            state.installed_packs().get("test-colours"),
            Some(&Some("v1".to_string()))
        );

        let removed = state.remove_pack("test-colours").unwrap();
        assert_eq!(removed.len(), 2);
        assert!(state.installed_packs().is_empty());
    }

    #[test]
    fn an_update_swaps_the_skins_and_keeps_the_ones_both_versions_share() {
        let state = AppState::default();
        state.open_store(temp_dir("update-store"));
        let pack = Pack::parse(PACK.as_bytes()).unwrap();
        let teal = png(512, 480, [20, 140, 150, 255]);
        let old = vec![teal.clone(), png(512, 480, [180, 70, 30, 255])];
        let before = save_pack(
            &state,
            "test-colours",
            &pack,
            &old,
            Some("v1".into()),
            &no_progress,
        )
        .unwrap();

        let new = vec![teal, png(512, 480, [90, 60, 160, 255])];
        let update = replace_pack(&state, "test-colours", &pack, &new, "v2".into()).unwrap();
        assert_eq!(
            update.removed,
            [before[1].id.clone()],
            "only the changed picture goes"
        );
        assert_eq!(update.skins.len(), 2);
        assert_eq!(
            update.skins[0].id, before[0].id,
            "the shared picture keeps its id"
        );
        assert_eq!(state.saved_entries().len(), 2);
        assert_eq!(
            state.installed_packs().get("test-colours"),
            Some(&Some("v2".to_string()))
        );

        // A version with a bad picture changes nothing.
        let broken = vec![png(512, 480, [1, 2, 3, 255]), png(100, 100, [1, 2, 3, 255])];
        assert!(replace_pack(&state, "test-colours", &pack, &broken, "v3".into()).is_err());
        assert_eq!(state.saved_entries().len(), 2);
        assert_eq!(
            state.installed_packs().get("test-colours"),
            Some(&Some("v2".to_string()))
        );
    }

    #[test]
    fn an_update_is_offered_only_for_an_added_pack_that_changed() {
        let v1 = Some("v1".to_string());
        assert!(!has_update(None, "v1"), "not added");
        assert!(!has_update(Some(&v1), "v1"), "the same version");
        assert!(has_update(Some(&v1), "v2"));
        assert!(!has_update(Some(&v1), ""), "an index without hashes");
        assert!(
            has_update(Some(&None), "v1"),
            "added before hashes were kept"
        );
    }

    #[test]
    fn a_bad_picture_stops_the_whole_pack_before_anything_is_saved() {
        let pack = Pack::parse(PACK.as_bytes()).unwrap();
        let state = AppState::default();
        state.open_store(temp_dir("bad-store"));
        let pictures = vec![png(512, 480, [1, 2, 3, 255]), png(100, 100, [1, 2, 3, 255])];
        let err =
            save_pack(&state, "test-colours", &pack, &pictures, None, &no_progress).unwrap_err();
        assert!(err.contains("rust.png"), "{err}");
        assert!(state.saved_entries().is_empty(), "nothing was saved");
    }

    #[test]
    fn exported_pictures_fit_the_pack_limits() {
        let art = SkinImage::Artwork(std::sync::Arc::new(folderskin_core::compositor::Artwork {
            rgba: image::RgbaImage::from_pixel(2048, 1916, image::Rgba([200, 90, 40, 255])),
            focus: (0.5, 0.5),
        }));
        let (bytes, ext) = encode_for_pack(&art).unwrap();
        assert_eq!(ext, "jpg");
        assert_eq!(pack::check_picture(&bytes).unwrap(), (1024, 958));

        let folder = SkinImage::Folder(std::sync::Arc::new(image::RgbaImage::from_pixel(
            1100,
            1000,
            image::Rgba([0, 0, 0, 0]),
        )));
        let (bytes, ext) = encode_for_pack(&folder).unwrap();
        assert_eq!(ext, "png");
        assert!(pack::check_picture(&bytes).is_ok());

        let tiny = SkinImage::Folder(std::sync::Arc::new(image::RgbaImage::new(120, 120)));
        assert!(encode_for_pack(&tiny).unwrap_err().contains("at least 256"));
    }

    #[test]
    fn file_names_in_an_exported_pack_never_clash() {
        let used = vec![("sunset.jpg".to_string(), vec![])];
        assert_eq!(unique_stem("Sunset", 1, &used), "sunset-2");
        assert_eq!(unique_stem("***", 1, &used), "skin-2");
        assert_eq!(unique_stem("Night sky", 1, &used), "night-sky");
    }

    #[test]
    fn progress_is_the_shape_the_webview_expects() {
        assert_eq!(
            serde_json::to_value(PackProgress::download(0, 16)).unwrap(),
            serde_json::json!({"stage": "download", "done": 0, "total": 16})
        );
        assert_eq!(
            serde_json::to_value(PackProgress::save(16, 16)).unwrap(),
            serde_json::json!({"stage": "save", "done": 16, "total": 16})
        );
    }

    /// A `pack.json` listing `skins` as (file, name).
    fn listing(skins: &[(&str, &str)]) -> String {
        let skins: Vec<String> = skins
            .iter()
            .map(|(file, name)| format!(r#"{{ "file": "{file}", "name": "{name}" }}"#))
            .collect();
        format!(
            r#"{{ "version": 1, "name": "Test pack", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["test"], "skins": [{}] }}"#,
            skins.join(", ")
        )
    }

    #[test]
    fn a_pack_is_saved_in_its_own_order_and_says_how_far_it_has_got() {
        let state = AppState::default();
        state.open_store(temp_dir("order-store"));
        let pack = Pack::parse(
            listing(&[("a.png", "First"), ("b.png", "Second"), ("c.png", "Third")]).as_bytes(),
        )
        .unwrap();
        let pictures = vec![
            png(512, 480, [200, 40, 40, 255]),
            png(512, 480, [40, 200, 40, 255]),
            png(512, 480, [40, 40, 200, 255]),
        ];
        let heard = Mutex::new(Vec::new());
        let saved = save_pack(&state, "test-pack", &pack, &pictures, None, &|p| {
            lock(&heard).push(p)
        })
        .unwrap();

        let names: Vec<&str> = saved.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["First", "Second", "Third"]);
        let newest_first: Vec<String> = state
            .saved_entries()
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(
            newest_first,
            ["First", "Second", "Third"],
            "the pack's order"
        );

        let heard = heard.into_inner().unwrap();
        assert_eq!(heard.first(), Some(&PackProgress::save(0, 3)));
        assert_eq!(heard.last(), Some(&PackProgress::save(3, 3)));
        assert_eq!(
            heard.len(),
            5,
            "none, each of the three, then all saved: {heard:?}"
        );
        assert!(
            heard.windows(2).all(|w| w[0].done <= w[1].done),
            "it never goes back: {heard:?}"
        );
    }

    #[test]
    fn a_picture_listed_twice_is_one_skin() {
        let state = AppState::default();
        state.open_store(temp_dir("twice-store"));
        let pack =
            Pack::parse(listing(&[("a.png", "Teal"), ("b.png", "Teal again")]).as_bytes()).unwrap();
        let teal = png(512, 480, [20, 140, 150, 255]);
        let saved = save_pack(
            &state,
            "test-pack",
            &pack,
            &[teal.clone(), teal],
            None,
            &no_progress,
        )
        .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "Teal");
        assert_eq!(state.saved_entries().len(), 1);
    }

    #[test]
    fn a_pack_that_cannot_be_saved_adds_nothing() {
        let dir = temp_dir("full-store");
        let state = AppState::default();
        state.open_store(dir.clone());
        let pack = Pack::parse(PACK.as_bytes()).unwrap();
        let pictures = vec![png(512, 480, [1, 2, 3, 255]), png(512, 480, [4, 5, 6, 255])];
        // Something in the way of the second picture, as a full disk would be.
        let second = store::skin_id(&pictures[1]);
        let stem = second.strip_prefix("user:").unwrap();
        std::fs::create_dir(dir.join(format!("{stem}.png"))).unwrap();

        let err =
            save_pack(&state, "test-colours", &pack, &pictures, None, &no_progress).unwrap_err();
        assert!(err.starts_with("couldn't save those skins"), "{err}");
        assert!(state.saved_entries().is_empty());
        assert!(state.installed_packs().is_empty());
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, [format!("{stem}.png")], "only what was in the way");
    }

    /// Serves `files` (path, body, delay before answering) over HTTP on a free local port, and
    /// counts the most requests it had at once. Returns the base URL and that count.
    fn serve(files: Vec<(String, Vec<u8>, Duration)>) -> (String, Arc<AtomicUsize>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let files: Arc<HashMap<String, (Vec<u8>, Duration)>> = Arc::new(
            files
                .into_iter()
                .map(|(path, body, delay)| (path, (body, delay)))
                .collect(),
        );
        let (open, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let most_seen = most.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (files, open, most) = (files.clone(), open.clone(), most.clone());
                std::thread::spawn(move || {
                    most.fetch_max(open.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                    let mut reader = BufReader::new(&stream);
                    let mut request = String::new();
                    let _ = reader.read_line(&mut request);
                    let mut header = String::new();
                    while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                        header.clear();
                    }
                    let path = request.split(' ').nth(1).unwrap_or("");
                    let path = path.split('?').next().unwrap_or("");
                    let (status, body) = match files.get(path) {
                        Some((body, delay)) => {
                            std::thread::sleep(*delay);
                            ("200 OK", body.clone())
                        }
                        None => ("404 Not Found", Vec::new()),
                    };
                    // Before answering, so a request that follows this answer is never counted
                    // alongside it.
                    open.fetch_sub(1, Ordering::SeqCst);
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let mut stream = &stream;
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&body);
                });
            }
        });
        (base, most_seen)
    }

    #[test]
    fn a_pack_downloads_a_few_at_a_time_and_keeps_its_order() {
        let files: Vec<String> = (1..=6).map(|i| format!("{i}.png")).collect();
        let named: Vec<(&str, &str)> = files.iter().map(|f| (f.as_str(), "Skin")).collect();
        let manifest = listing(&named).into_bytes();
        let pictures: Vec<Vec<u8>> = (1..=6)
            .map(|i| png(256, 256, [i * 40, 90, 200, 255]))
            .collect();
        let mut served = vec![(
            "/packs/test-pack/pack.json".to_string(),
            manifest.clone(),
            Duration::ZERO,
        )];
        // The first pictures answer slowest, so they arrive out of order.
        for (i, (file, bytes)) in files.iter().zip(&pictures).enumerate() {
            let delay = Duration::from_millis(300 - 50 * i as u64);
            served.push((format!("/packs/test-pack/{file}"), bytes.clone(), delay));
        }
        let (base, most) = serve(served);

        let heard = Mutex::new(Vec::new());
        let (pack, hash, got) =
            tauri::async_runtime::block_on(download_pack_from(&base, "test-pack", &|p| {
                lock(&heard).push(p)
            }))
            .unwrap();

        assert_eq!(pack.skins.len(), 6);
        assert_eq!(got, pictures, "in the pack's order");
        let listed = files
            .iter()
            .map(String::as_str)
            .zip(pictures.iter().map(Vec::as_slice));
        assert_eq!(hash, pack::pack_hash(&manifest, listed));
        let heard = heard.into_inner().unwrap();
        let expected: Vec<PackProgress> = (0..=6).map(|n| PackProgress::download(n, 6)).collect();
        assert_eq!(heard, expected);
        let most = most.load(Ordering::SeqCst);
        assert!(
            (2..=PARALLEL_DOWNLOADS).contains(&most),
            "{most} requests at once"
        );
    }

    #[test]
    fn a_picture_that_is_missing_stops_the_download() {
        let manifest = listing(&[("a.png", "A"), ("b.png", "B")]).into_bytes();
        let (base, _) = serve(vec![
            (
                "/packs/test-pack/pack.json".to_string(),
                manifest,
                Duration::ZERO,
            ),
            (
                "/packs/test-pack/a.png".to_string(),
                png(256, 256, [9, 9, 9, 255]),
                Duration::ZERO,
            ),
        ]);
        let heard = Mutex::new(Vec::new());
        let err = tauri::async_runtime::block_on(download_pack_from(&base, "test-pack", &|p| {
            lock(&heard).push(p)
        }))
        .unwrap_err();
        assert_eq!(err, "b.png: not found");
        assert_eq!(
            heard.into_inner().unwrap().first(),
            Some(&PackProgress::download(0, 2))
        );

        let err =
            tauri::async_runtime::block_on(download_pack_from(&base, "../escape", &no_progress))
                .unwrap_err();
        assert_eq!(err, "that isn't a pack");
    }

    #[test]
    fn a_pack_looked_through_again_comes_from_the_cache_without_downloading() {
        let manifest = listing(&[("a.png", "Ada"), ("b.png", "Alan")]).into_bytes();
        let pictures = [
            png(256, 256, [200, 40, 40, 255]),
            png(256, 256, [40, 40, 200, 255]),
        ];
        let (base, _) = serve(vec![
            (
                "/packs/test-pack/pack.json".into(),
                manifest.clone(),
                Duration::ZERO,
            ),
            (
                "/packs/test-pack/a.png".into(),
                pictures[0].clone(),
                Duration::ZERO,
            ),
            (
                "/packs/test-pack/b.png".into(),
                pictures[1].clone(),
                Duration::ZERO,
            ),
        ]);
        let dir = temp_dir("pack-views");
        let views = || Some(PackViews::new(dir.clone()));
        let listed = pack::pack_hash(
            &manifest,
            [("a.png", &pictures[0][..]), ("b.png", &pictures[1][..])],
        );
        let look = |base: &str| {
            tauri::async_runtime::block_on(pack_skins(
                base,
                views(),
                "test-pack".into(),
                listed.clone(),
            ))
        };

        let first = look(&base).unwrap();
        let names: Vec<&str> = first.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Ada", "Alan"]);
        // Nothing listens here, so only the cache can answer.
        let nowhere = "http://127.0.0.1:9";
        assert_eq!(look(nowhere).unwrap(), first);
        // A version it hasn't drawn has to be downloaded.
        let other = tauri::async_runtime::block_on(pack_skins(
            nowhere,
            views(),
            "test-pack".into(),
            "0123456789abcdef".into(),
        ));
        assert!(other.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
