//! Community skin packs: searching them, each pack's preview, adding and removing a pack,
//! adding one from a folder on this computer, and saving your own skins as a pack.
//!
//! Packs live in their own repository, github.com/prajwal-svm/folderskin-community, and its
//! published tree is copied to packs.folderskin.app, so reading them needs no account. The app
//! searches a catalog of all of them on this computer (catalog.rs), and fetches pictures from the
//! published tree, `v2/`, where each file is named after its contents; until that tree is
//! published it reads `index.json`, `previews/<id>.png` and `packs/<id>/` as it always has. The
//! rules every pack follows are in `folderskin_core::pack`, and docs/PACKS.md says the same in
//! prose.
//!
//! A pack's id can change (head.json's `moved`, from `moved.json` beside `packs/`). Every id that
//! comes from outside the catalog is looked up through [`Source::current_id`], and the library's
//! records of packs added under an old id move to the new one ([`follow_moves`]), so a pack added
//! before it moved still shows as added and still gets its updates.

use crate::catalog::{Community, Origin, Source};
use crate::commands::{data_url, prepare_import, SkinDto};
use crate::pack_views::PackViews;
use crate::previews;
use crate::state::{parallel_map, parallel_queue, AppState};
use crate::store::{self, NewSkin, SkinImage, SkinSource};
use folderskin_catalog::tree::{self, PublishedPack};
use folderskin_catalog::{Facet, PackRow, Query, Sort};
use folderskin_core::pack::{self, Pack, PackSkin};
use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

/// The packs repository on GitHub. Set `FOLDERSKIN_COMMUNITY_URL` to read another copy of it
/// instead, such as a checkout served locally while testing.
const COMMUNITY_URL: &str =
    "https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main";
/// FolderSkin's own copy of the repository's published tree (`<url>/v2/`), kept in step with it
/// as packs are published and served from a CDN: head.json is asked for here first, and GitHub
/// answers when this doesn't.
const PACKS_URL: &str = "https://packs.folderskin.app";
/// How many of a pack's pictures download at once.
const PARALLEL_DOWNLOADS: usize = 4;
/// How many packs the first launch offers when no packs are featured.
const FIRST_PACKS: usize = 24;
/// How many of a pack's pictures are made ready at once as it is shared or saved as a folder.
/// Each takes libwebp several seconds and about 60 MB, so more at once would use a lot of memory
/// for little more speed.
const PARALLEL_ENCODES: usize = 6;
const MB: usize = 1024 * 1024;

/// One pack in the Community list.
#[derive(Serialize)]
pub struct PackDto {
    pub id: String,
    pub name: String,
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub count: usize,
    /// What adding it downloads, in bytes; 0 when the list doesn't say.
    pub bytes: u64,
    /// The version published now (`pack::pack_hash`); empty when the index has none.
    pub hash: String,
    /// The address of its preview strip (previews.rs).
    pub preview: String,
    /// True when the pack's skins are in the library.
    pub added: bool,
    /// True when it was added and a different version of it is published now.
    pub update: bool,
    /// True for a pack the maintainer marks as official (`official.json` in folderskin-community).
    pub official: bool,
}

impl PackDto {
    /// A catalog row, marked with whether it's in the library, whether that copy is old, and
    /// whether `source` names it official.
    fn new(row: PackRow, installed: &HashMap<String, Option<String>>, source: &Source) -> PackDto {
        let have = installed.get(&row.id);
        PackDto {
            added: have.is_some(),
            update: has_update(have, &row.hash),
            official: source.is_official(&row.id),
            preview: previews::strip_url(&row.id, &row.hash),
            id: row.id,
            name: row.name,
            author: row.author,
            license: row.license,
            tags: row.tags,
            count: row.count,
            bytes: row.bytes,
            hash: row.hash,
        }
    }
}

/// A skin whose name matches a search, to open its pack at.
#[derive(Serialize, Debug, PartialEq)]
pub struct SkinHitDto {
    pub pack: String,
    pub pack_name: String,
    pub name: String,
    /// Where it is in its pack, from 0.
    pub index: usize,
    /// The address of its thumbnail.
    pub thumbnail: String,
}

/// One page of a search, and what goes with it.
#[derive(Serialize)]
pub struct SearchDto {
    /// Packs matching the words and the tag.
    pub total: usize,
    /// Packs matching the words, whatever their tag.
    pub all: usize,
    pub packs: Vec<PackDto>,
    pub skins: Vec<SkinHitDto>,
    /// The packs `skins` are in, so one can be opened wherever it is in the list.
    pub hit_packs: Vec<PackDto>,
    /// Tags of the packs the words match, most used first.
    pub facets: Vec<Facet>,
    /// Why these are the packs from the last visit rather than the ones published now, as the
    /// start of a sentence ("you're offline"); `None` when they are the ones published now.
    pub last_visit: Option<String>,
    /// Which catalog answered, so a page that comes from a newer one than the rest of the list
    /// can be told apart.
    pub generation: String,
}

/// What Refresh found.
#[derive(Serialize)]
pub struct RefreshDto {
    /// How many packs in the library have a newer version.
    pub updates: usize,
    pub packs: usize,
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

/// What the first launch offers: its packs, and where any old id moved among them.
#[derive(Serialize)]
pub struct FirstPacksDto {
    pub packs: Vec<PackDto>,
    /// Each old id that now leads to one of `packs`, to the id it leads to: how the onboarding
    /// finds the pack it picks by the id that pack had first.
    pub moved: BTreeMap<String, String>,
}

/// The packs the first launch offers: the featured ones, or the first few of the best order
/// when none are, each marked with whether it has been added and whether it has changed since.
/// `fresh` asks past every cache on the way, for Refresh.
#[tauri::command]
pub async fn community_packs(
    app: AppHandle,
    state: State<'_, AppState>,
    community: State<'_, Community>,
    fresh: bool,
) -> Result<FirstPacksDto, String> {
    community.init_cache(&app);
    let source = if fresh {
        community.refresh(&origin()).await?
    } else {
        community.current(&origin()).await?
    };
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || first_packs(&state, &source))
        .await
        .map_err(|e| e.to_string())?
}

/// [`community_packs`] from `source`.
fn first_packs(state: &AppState, source: &Source) -> Result<FirstPacksDto, String> {
    let installed = library(state, source);
    let rows = if source.featured.is_empty() {
        source
            .search(&Query {
                limit: FIRST_PACKS,
                ..Query::default()
            })?
            .packs
    } else {
        source.packs(&source.featured)?
    };
    let packs: Vec<PackDto> = rows
        .into_iter()
        .map(|row| PackDto::new(row, &installed, source))
        .collect();
    let moved = source
        .moved()
        .iter()
        .filter(|(_, now)| packs.iter().any(|p| &p.id == *now))
        .map(|(old, now)| (old.clone(), now.clone()))
        .collect();
    Ok(FirstPacksDto { packs, moved })
}

/// Pack `pack_id` as the Community list shows it, for a `folderskin://install` link; `None` when
/// no pack has that id or had it before it moved. A pack the list this session loaded doesn't
/// have is looked for again past every cache, as Refresh does, since the link may be for one
/// published since; when that can't be done (offline, say), the error says why.
#[tauri::command]
pub async fn community_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    community: State<'_, Community>,
    pack_id: String,
) -> Result<Option<PackDto>, String> {
    community.init_cache(&app);
    let state = state.inner();
    look_up(&community, &origin(), &pack_id, |source| {
        library(state, source)
    })
    .await
}

/// [`community_pack`] with the packs at `origin`, marked against what `installed` says the
/// library holds by the catalog it's given.
async fn look_up(
    community: &Community,
    origin: &Origin,
    pack_id: &str,
    installed: impl Fn(&Source) -> HashMap<String, Option<String>>,
) -> Result<Option<PackDto>, String> {
    if !pack::is_pack_id(pack_id) {
        return Ok(None);
    }
    let source = community.current(origin).await?;
    if let Some(found) = find_pack(&source, pack_id, &installed(&source))? {
        return Ok(Some(found));
    }
    let source = community.refresh(origin).await?;
    find_pack(&source, pack_id, &installed(&source))
}

/// Pack `id`, or the pack it moved to, in `source`, marked against the library; `None` when it
/// isn't listed.
fn find_pack(
    source: &Source,
    id: &str,
    installed: &HashMap<String, Option<String>>,
) -> Result<Option<PackDto>, String> {
    Ok(source
        .packs(&[source.current_id(id).to_string()])?
        .into_iter()
        .next()
        .map(|row| PackDto::new(row, installed, source)))
}

/// The packs in the library, by the id each has now, with the version each was added at: what
/// every list is marked against. The library's records follow `source`'s moves first.
fn library(state: &AppState, source: &Source) -> HashMap<String, Option<String>> {
    follow_moves(state, source);
    state.installed_packs()
}

/// Moves the library's records of packs added under an id that `source` says moved to the id
/// the pack has now, once for each catalog: after that there's nothing left under an old id, and
/// a later catalog finds nothing to do unless another pack moved. A record that can't be moved
/// (a full disk) is tried again with the next command, and until then its pack shows as not
/// added, as it did before.
pub(crate) fn follow_moves(state: &AppState, source: &Source) {
    let Some(moved) = source.moves_to_follow() else {
        return;
    };
    match state.follow_moved(moved) {
        Ok(_) => source.followed_moves(),
        Err(e) => eprintln!("folderskin: couldn't move packs to their new ids: {e}"),
    }
}

/// One page of the packs matching `q` (every word the start of a word in a pack's name,
/// author, tags or skin names) and `tag`, in `sort` order ("best", "newest", "name" or
/// "skins"), with the skins whose names match and the tags of the matching packs. The first
/// search loads the catalog; every one after is answered on this computer.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn community_search(
    app: AppHandle,
    state: State<'_, AppState>,
    community: State<'_, Community>,
    q: String,
    tag: String,
    sort: String,
    offset: usize,
    limit: usize,
) -> Result<SearchDto, String> {
    community.init_cache(&app);
    let source = community.current(&origin()).await?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        search(
            &source,
            &library(&state, &source),
            &Query {
                q: &q,
                tag: &tag,
                sort: Sort::parse(&sort),
                offset,
                limit,
                featured: &source.featured,
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

/// [`community_search`] over `source`, marked against the packs in the library. Only the page
/// is marked, so a search costs the same with ten packs installed as with none.
fn search(
    source: &Source,
    installed: &HashMap<String, Option<String>>,
    query: &Query,
) -> Result<SearchDto, String> {
    let results = source.search(query)?;
    let tree = source.is_tree();
    let mut hit_ids: Vec<String> = Vec::new();
    for hit in &results.skins {
        if !hit_ids.contains(&hit.pack) {
            hit_ids.push(hit.pack.clone());
        }
    }
    let hit_packs = source
        .packs(&hit_ids)?
        .into_iter()
        .map(|row| PackDto::new(row, installed, source))
        .collect();
    Ok(SearchDto {
        hit_packs,
        total: results.total,
        all: results.all,
        packs: results
            .packs
            .into_iter()
            .map(|row| PackDto::new(row, installed, source))
            .collect(),
        skins: results
            .skins
            .into_iter()
            .map(|hit| SkinHitDto {
                thumbnail: if tree {
                    previews::skin_url(&hit.pack, &hit.pack_hash, hit.position)
                } else {
                    String::new()
                },
                pack: hit.pack,
                pack_name: hit.pack_name,
                name: hit.name,
                index: hit.position,
            })
            .collect(),
        facets: results.facets,
        last_visit: source.last_visit.as_ref().map(|why| why.to_string()),
        generation: source.generation.clone(),
    })
}

/// Asks for the packs again, past every cache, and says how many of the library's packs have a
/// newer version. The webview searches again afterwards.
#[tauri::command]
pub async fn community_refresh(
    app: AppHandle,
    state: State<'_, AppState>,
    community: State<'_, Community>,
) -> Result<RefreshDto, String> {
    community.init_cache(&app);
    let source = community.refresh(&origin()).await?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let installed = library(&state, &source);
        let ids: Vec<String> = installed.keys().cloned().collect();
        let updates = source
            .packs(&ids)?
            .iter()
            .filter(|row| has_update(installed.get(&row.id), &row.hash))
            .count();
        Ok(RefreshDto {
            updates,
            packs: source.counts().0,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The packs in the library now, each with the version it was added at (`None` when it was
/// added before FolderSkin kept one). The Community view keeps its list while it is away, and
/// the library may have changed meanwhile: this marks the packs it shows again, from this
/// computer alone, by the ids the catalog in use gives them.
#[tauri::command]
pub async fn community_installed(
    state: State<'_, AppState>,
    community: State<'_, Community>,
) -> Result<HashMap<String, Option<String>>, String> {
    let state = state.inner().clone();
    let source = community.in_use_now();
    tauri::async_runtime::spawn_blocking(move || match source {
        Some(source) => library(&state, &source),
        None => state.installed_packs(),
    })
    .await
    .map_err(|e| e.to_string())
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

/// Downloads a pack and saves its skins, all of them or none, and returns them in the pack's
/// order. The pictures download a few at a time and every one is checked before any is saved,
/// then they are saved in one go, so a dropped connection, a bad file or a full disk never
/// leaves half a pack behind.
///
/// `on_progress` hears how far it has got: `download` with nothing arrived, then as each picture
/// arrives; then `save` with nothing saved, as each is ready to write, and with all of them once
/// they are saved.
///
/// Once it is added, the community service is told the pack's id, the one it has now however it
/// was asked for, so folderskin.app can count it ([`crate::installs`]). That goes on by itself:
/// the command doesn't wait for it.
#[tauri::command]
pub async fn community_add(
    app: AppHandle,
    state: State<'_, AppState>,
    community: State<'_, Community>,
    pack_id: String,
    on_progress: Channel<PackProgress>,
) -> Result<Vec<SkinDto>, String> {
    // The window may have gone, and the pack is saved all the same, so a failed send is ignored.
    let progress = move |p: PackProgress| {
        let _ = on_progress.send(p);
    };
    community.init_cache(&app);
    let source = community.current(&origin()).await?;
    let (id, skins) = add_pack(state.inner(), source, &pack_id, progress).await?;
    crate::installs::report(&id);
    Ok(skins)
}

/// [`community_add`] from `source`: the id pack `pack_id` has now, which its skins are saved
/// under, and the skins.
async fn add_pack(
    state: &AppState,
    source: Arc<Source>,
    pack_id: &str,
    progress: impl Fn(PackProgress) + Send + Sync + 'static,
) -> Result<(String, Vec<SkinDto>), String> {
    let id = source.current_id(pack_id).to_string();
    let (pack, hash, pictures) = download_pack(&source, &id, &progress).await?;
    let (state, saved_as) = (state.clone(), id.clone());
    let skins = tauri::async_runtime::spawn_blocking(move || {
        // A picture it shares with a copy added under an old id comes back as that copy saved
        // it, so the copy's records move to this id first.
        follow_moves(&state, &source);
        save_pack(&state, &saved_as, &pack, &pictures, Some(hash), &progress)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok((id, skins))
}

/// Every skin of pack `pack_id`, drawn as the folder it makes, to look through before adding it.
/// From the published tree that is its manifest, with each skin's thumbnail loaded as it scrolls
/// into view. From index.json it downloads and checks the whole pack as adding it would, and adds
/// nothing to the library; those drawings are kept for a while (pack_views.rs), so looking again
/// at the version the list names as `hash` shows them straight away, with nothing downloaded.
#[tauri::command]
pub async fn community_pack_skins(
    app: AppHandle,
    community: State<'_, Community>,
    pack_id: String,
    hash: String,
) -> Result<Vec<PackSkinDto>, String> {
    community.init_cache(&app);
    let source = community.current(&origin()).await?;
    let pack_id = source.current_id(&pack_id).to_string();
    let Some(base) = source.index_base() else {
        let published = published_pack(&source, community.files(), &pack_id).await?;
        return Ok(published_skins(&published));
    };
    let views = app
        .path()
        .app_cache_dir()
        .ok()
        .map(|dir| PackViews::new(dir.join("pack-views")));
    pack_skins(base, views, pack_id, hash).await
}

/// A published pack's skins to look through: each with its thumbnail's address, which the
/// webview loads as the skin scrolls into view.
fn published_skins(published: &PublishedPack) -> Vec<PackSkinDto> {
    let pack = published.to_pack();
    pack.skins
        .iter()
        .zip(&published.skins)
        .map(|(skin, meta)| PackSkinDto {
            name: skin.name.trim().to_string(),
            tags: pack.tags_for(skin),
            thumbnail: previews::thumb_url(&meta.sha256),
        })
        .collect()
}

/// The manifest of the version of pack `pack_id` the catalog lists now.
async fn published_pack(
    source: &Source,
    files: Option<&previews::DiskCache>,
    pack_id: &str,
) -> Result<PublishedPack, String> {
    if !pack::is_pack_id(pack_id) {
        return Err("that isn't a pack".into());
    }
    let row = source
        .packs(&[pack_id.to_string()])?
        .into_iter()
        .next()
        .ok_or_else(|| "that pack isn't listed any more; try Refresh".to_string())?;
    previews::manifest(source, files, &row).await
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

/// Replaces an added pack's skins with the version published now. The new version is
/// downloaded and checked before the old skins go, so a failed update leaves the pack as it was.
/// `on_progress` hears how far it has got, as [`community_add`]'s does. A pack added before it
/// moved to a new id is updated under the new one.
#[tauri::command]
pub async fn community_update(
    app: AppHandle,
    state: State<'_, AppState>,
    community: State<'_, Community>,
    pack_id: String,
    on_progress: Channel<PackProgress>,
) -> Result<PackUpdateDto, String> {
    let progress = move |p: PackProgress| {
        let _ = on_progress.send(p);
    };
    community.init_cache(&app);
    let source = community.current(&origin()).await?;
    update_pack(state.inner(), source, &pack_id, progress).await
}

/// [`community_update`] from `source`.
async fn update_pack(
    state: &AppState,
    source: Arc<Source>,
    pack_id: &str,
    progress: impl Fn(PackProgress) + Send + Sync + 'static,
) -> Result<PackUpdateDto, String> {
    let id = source.current_id(pack_id).to_string();
    let (pack, hash, pictures) = download_pack(&source, &id, &progress).await?;
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // The old version's skins are the ones under this id, wherever they were added.
        follow_moves(&state, &source);
        replace_pack(&state, &id, &pack, &pictures, hash, &progress)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pack `pack_id` as `source` publishes it: what it lists, its hash ([`pack::pack_hash`]) and
/// every picture in the pack's order, each within the pack limits.
async fn download_pack(
    source: &Source,
    pack_id: &str,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<(Pack, String, Vec<Vec<u8>>), String> {
    match source.index_base() {
        Some(base) => download_pack_from(base, pack_id, progress).await,
        None => download_published(source, pack_id, progress).await,
    }
}

/// [`download_pack`] from the published tree: the manifest of the version the catalog lists,
/// then each picture by its SHA-256, checked against it. Progress is as
/// [`download_pack_from`] reports it.
async fn download_published(
    source: &Source,
    pack_id: &str,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<(Pack, String, Vec<Vec<u8>>), String> {
    let published = published_pack(source, None, pack_id).await?;
    let total = published.skins.len();
    progress(PackProgress::download(0, total));
    let arrived = AtomicUsize::new(0);
    let pictures: Vec<Vec<u8>> = futures_util::stream::iter(published.skins.clone())
        .map(|skin| {
            let arrived = &arrived;
            async move {
                let path = tree::picture_path(&skin.sha256, &skin.ext());
                // Checked as it arrives, so a mirror with the wrong bytes is passed over for
                // the next folder rather than failing the pack.
                let bytes = source
                    .get(&path, skin.bytes as usize, |b| {
                        tree::sha256_hex(b) == skin.sha256
                    })
                    .await
                    .map_err(|e| match e {
                        // Its size is known, so one bigger is the wrong file too.
                        Fetch::Damaged | Fetch::TooBig(_) => {
                            format!("{} arrived damaged; try again", skin.file)
                        }
                        e => format!("{}: {e}", skin.file),
                    })?;
                let done = arrived.fetch_add(1, Ordering::Relaxed) + 1;
                progress(PackProgress::download(done, total));
                Ok::<_, String>(bytes)
            }
        })
        .buffered(PARALLEL_DOWNLOADS)
        .try_collect()
        .await?;
    Ok((published.to_pack(), published.hash, pictures))
}

/// Pack `pack_id` from the copy of `community/` at `base`, as index.json lists it. The pictures
/// download [`PARALLEL_DOWNLOADS`] at a time; `progress` hears `download` with none arrived once
/// the pack's list is in, then again as each one arrives. The first picture that fails, in the
/// pack's order, is the error, and the downloads still going are dropped.
async fn download_pack_from(
    base: &str,
    pack_id: &str,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<(Pack, String, Vec<Vec<u8>>), String> {
    if !pack::is_pack_id(pack_id) {
        return Err("that isn't a pack".into());
    }
    // No query to get past the caches: a copy a few minutes old hashes to what it is, and an
    // update shows again until the new version arrives.
    let base = format!("{base}/packs/{pack_id}");
    let manifest = fetch(
        &format!("{base}/{}", pack::MANIFEST_FILE),
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
            let url = format!("{base}/{file}");
            let arrived = &arrived;
            async move {
                let bytes = fetch(&url, pack::MAX_READ_PICTURE_BYTES)
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

/// Deletes every skin a pack added and returns their ids. Nothing is downloaded for it: when a
/// catalog is in use, the library's records follow its moves first and the id is looked up
/// through it, and otherwise the id is taken as it is.
#[tauri::command]
pub async fn community_remove(
    state: State<'_, AppState>,
    community: State<'_, Community>,
    pack_id: String,
) -> Result<Vec<String>, String> {
    if !pack::is_pack_id(&pack_id) {
        return Err("that isn't a pack".into());
    }
    let state = state.inner().clone();
    let source = community.in_use_now();
    tauri::async_runtime::spawn_blocking(move || {
        let id = match &source {
            Some(source) => {
                follow_moves(&state, source);
                source.current_id(&pack_id)
            }
            None => &pack_id,
        };
        state.remove_pack(id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Adds a pack from a folder on this computer, with the same checks as one from Community. The
/// folder's name is the pack's id, so a copy of a community pack's folder and the same pack added
/// from Community are one pack.
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
        // Where packs are published, addresses are case-sensitive, so a name that only matches
        // in another case here would work on this Mac and then fail for everyone else.
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
                read_capped(&dir.join(&s.file), pack::MAX_READ_PICTURE_BYTES)
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

/// A pack's files: each name with its bytes.
pub(crate) type PackFiles = Vec<(String, Vec<u8>)>;

/// How far making a pack's pictures ready has got: `done` of `total`. Several are made at once.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MakeProgress {
    pub done: usize,
    pub total: usize,
}

/// A picture made smaller than 1024 px to fit the size a pack's picture can be
/// ([`pack::MAX_PICTURE_BYTES`]), still lossless: its skin's name and the side it was made.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Scaled {
    pub name: String,
    pub side: u32,
}

/// A pack [`build_pack`] made.
pub(crate) struct Built {
    /// Its new id.
    pub id: String,
    /// Every picture, then `pack.json`.
    pub files: PackFiles,
    /// The pictures made smaller to fit, in the pack's order.
    pub scaled: Vec<Scaled>,
}

/// The files a pack is made of: every picture as a lossless WebP ([`encode_for_pack`]), then
/// `pack.json`, and a new id for it ([`pack::new_id`]: the name, then six random characters) that
/// `taken` says nothing has. The pack is checked here, so whatever comes back already passes the
/// checks every pack does, whether it is written to a folder ([`export_pack`]) or sent for review
/// ([`crate::share::share_submit`]).
///
/// The pictures are made [`PARALLEL_ENCODES`] at a time, and `progress` hears how many are ready
/// as each one is. Everything that can be refused without them is refused first.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_pack(
    state: &AppState,
    name: &str,
    author: &str,
    license: &str,
    tags: &[String],
    skin_ids: &[String],
    taken: impl Fn(&str) -> bool,
    progress: &(dyn Fn(MakeProgress) + Sync),
) -> Result<Built, String> {
    let id = pack::new_id(name, taken)?;
    let pack_tags = pack::clean_tags(tags, pack::MAX_PACK_TAGS);
    if skin_ids.len() > pack::MAX_SKINS {
        return Err(format!(
            "that's {} skins; a pack holds at most {}",
            skin_ids.len(),
            pack::MAX_SKINS
        ));
    }
    let mut chosen = Vec::with_capacity(skin_ids.len());
    for skin_id in skin_ids {
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
        let (w, h) = image.rgba().dimensions();
        pack::check_picture_size(w, h).map_err(|e| format!("{} {e}", entry.name))?;
        chosen.push((entry, image));
    }

    // The slow part: seconds a picture.
    let total = chosen.len();
    progress(MakeProgress { done: 0, total });
    let ready = AtomicUsize::new(0);
    let made = parallel_queue(&chosen, PARALLEL_ENCODES, |(entry, image)| {
        let made = encode_for_pack(image).map_err(|e| format!("{} {e}", entry.name));
        let done = ready.fetch_add(1, Ordering::Relaxed) + 1;
        progress(MakeProgress { done, total });
        made
    });

    let mut skins = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut scaled = Vec::new();
    for ((entry, _), made) in chosen.iter().zip(made) {
        let made = made?;
        let stem = unique_stem(&entry.name, files.len(), &files);
        let file = format!("{stem}.webp");
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
        if let Some(side) = made.scaled_to {
            scaled.push(Scaled {
                name: entry.name.clone(),
                side,
            });
        }
        files.push((file, made.webp));
    }
    let bytes: usize = files.iter().map(|(_, b)| b.len()).sum();
    if bytes > pack::MAX_PACK_BYTES {
        return Err(format!(
            "the pictures come to {} MB, and a pack's come to {} MB at most; split them into two \
             packs",
            bytes.div_ceil(MB),
            pack::MAX_PACK_BYTES / MB
        ));
    }
    let pack = Pack {
        version: pack::PACK_VERSION,
        name: name.trim().to_string(),
        author: author.trim().to_string(),
        license: license.to_string(),
        tags: pack_tags,
        skins,
    };
    let problems = pack.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let json = serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())? + "\n";
    files.push((pack::MANIFEST_FILE.to_string(), json.into_bytes()));
    Ok(Built { id, files, scaled })
}

/// A pack saved as a folder: where it is, and the pictures made smaller to fit.
#[derive(Serialize, Clone, Debug)]
pub struct Exported {
    pub folder: String,
    pub scaled: Vec<Scaled>,
}

/// Writes some of the user's own skins as a pack folder inside `folder`, and returns the folder
/// it made, with the pictures made smaller to fit: pack.json and the pictures, for anyone who
/// wants the pack as files. The folder is named after the pack's new id, which no folder there
/// has yet, and adds to FolderSkin as it is ([`import_pack`]). `on_progress` hears how many
/// pictures are ready as each one is.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn export_pack(
    state: State<'_, AppState>,
    folder: String,
    name: String,
    author: String,
    license: String,
    tags: Vec<String>,
    skin_ids: Vec<String>,
    on_progress: Channel<MakeProgress>,
) -> Result<Exported, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let folder = PathBuf::from(folder);
        let progress = |p: MakeProgress| {
            let _ = on_progress.send(p);
        };
        let Built { id, files, scaled } = build_pack(
            &state,
            &name,
            &author,
            &license,
            &tags,
            &skin_ids,
            |id| folder.join(id).exists(),
            &progress,
        )?;
        let out = folder.join(&id);
        // Taken between the id being drawn and now: a clash of ids nothing else will ever see.
        if out.exists() {
            return Err(format!("there's already a folder called {id} there"));
        }
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(&out)?;
            for (file, bytes) in &files {
                std::fs::write(out.join(file), bytes)?;
            }
            Ok(())
        };
        write().map_err(|e| {
            let _ = std::fs::remove_dir_all(&out);
            format!("couldn't write the pack: {e}")
        })?;
        Ok(Exported {
            folder: out.display().to_string(),
            scaled,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- helpers ----------

/// Why a download didn't arrive. Callers that know what the file was say it better; the rest
/// pass it on as the sentence it reads as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetch {
    /// Nothing answered at all, from this host: offline, most likely.
    Unreachable(String),
    /// The host answered that it hasn't got the file.
    NotFound(String),
    /// The host answered with another error, such as 503 or 429: the host and the status.
    Refused(String, String),
    /// Bigger than it may be, in bytes.
    TooBig(usize),
    /// It arrived, but its size or SHA-256 isn't the file's.
    Damaged,
}

impl Fetch {
    /// True when nothing answered, as opposed to an answer that wasn't the file.
    pub fn is_offline(&self) -> bool {
        matches!(self, Fetch::Unreachable(_))
    }
}

impl std::fmt::Display for Fetch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fetch::Unreachable(host) => write!(
                f,
                "couldn't reach {host}. Check your connection and try again"
            ),
            Fetch::NotFound(host) => write!(f, "it isn't on {host} any more; try Refresh"),
            Fetch::Refused(host, status) => {
                write!(f, "{host} answered {status}; try again in a minute")
            }
            Fetch::TooBig(max) => write!(f, "it's over {} KB", max / 1024),
            Fetch::Damaged => write!(f, "it arrived damaged; try again"),
        }
    }
}

impl From<Fetch> for String {
    fn from(e: Fetch) -> String {
        e.to_string()
    }
}

/// `url` with a query no cache has seen when `fresh`, so the answer comes from the host itself
/// rather than a copy up to a few minutes old. Only head.json, or index.json standing in for
/// it, is ever asked for this way, and only for Refresh.
pub(crate) fn uncached(url: &str, fresh: bool) -> String {
    if fresh {
        format!("{url}?t={}", store::now_ms())
    } else {
        url.to_string()
    }
}

/// Where the packs are: packs.folderskin.app, then the packs repository on GitHub; or only the
/// copy of the repository `FOLDERSKIN_COMMUNITY_URL` names, which is what testing wants.
pub(crate) fn origin() -> Origin {
    origin_for(std::env::var("FOLDERSKIN_COMMUNITY_URL").ok().as_deref())
}

/// [`origin`] with the value `FOLDERSKIN_COMMUNITY_URL` has, if any.
fn origin_for(named: Option<&str>) -> Origin {
    match named.map(|url| url.trim().trim_end_matches('/')) {
        Some(url) if !url.is_empty() => Origin::repo(url),
        _ => Origin {
            trees: vec![PACKS_URL.to_string()],
            repo: COMMUNITY_URL.to_string(),
        },
    }
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

/// Who serves `url`, for a sentence: "GitHub" for GitHub's hosts, otherwise the host's own name.
pub(crate) fn host(url: &str) -> String {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default();
    let github = ["github.com", "githubusercontent.com"]
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")));
    if github {
        "GitHub".into()
    } else if host.is_empty() {
        "the community packs".into()
    } else {
        host
    }
}

/// Downloads `url`, refusing a body over `max` bytes, including one that never says its length.
pub(crate) async fn fetch(url: &str, max: usize) -> Result<Vec<u8>, Fetch> {
    let unreachable = || Fetch::Unreachable(host(url));
    // A client that can't be made is as good as no connection.
    let mut response = client()
        .map_err(|_| unreachable())?
        .get(url)
        .send()
        .await
        .map_err(|_| unreachable())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Fetch::NotFound(host(url)));
    }
    if !response.status().is_success() {
        return Err(Fetch::Refused(host(url), response.status().to_string()));
    }
    if response.content_length().is_some_and(|n| n > max as u64) {
        return Err(Fetch::TooBig(max));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unreachable())? {
        body.extend_from_slice(&chunk);
        if body.len() > max {
            return Err(Fetch::TooBig(max));
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
/// of it stays. `progress` hears `save` as [`save_pack`] reports it.
fn replace_pack(
    state: &AppState,
    pack_id: &str,
    pack: &Pack,
    pictures: &[Vec<u8>],
    hash: String,
    progress: &(dyn Fn(PackProgress) + Sync),
) -> Result<PackUpdateDto, String> {
    progress(PackProgress::save(0, pictures.len()));
    let ready = prepare_pack(pack, pictures)?;
    let before = state.remove_pack(pack_id)?;
    let skins = store_pack(state, pack_id, pack, ready, Some(hash), progress)?;
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
        .filter(|(entry, _)| seen.insert(entry.id.clone()))
        .map(|(entry, thumb)| SkinDto::saved(entry, thumb))
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

/// A skin's picture the way a pack wants it: a lossless WebP at most 1024 px on a side and
/// within [`pack::MAX_PICTURE_BYTES`], so the pack looks exactly as the skin does, a finished
/// folder's transparency included. One too detailed for that at 1024 px is made 896 px, then
/// 768 px, still lossless, and says so ([`pack::encode_picture`], which `packs make` uses too).
fn encode_for_pack(image: &SkinImage) -> Result<pack::EncodedPicture, String> {
    pack::encode_picture(image.rgba().clone(), pack::MAX_PICTURE_BYTES)
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
pub(crate) mod tests {
    use super::*;
    use crate::store::SkinKind;
    use std::io::Cursor;
    use std::sync::Arc;

    pub(crate) fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(w, h, image::Rgba(rgba))
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    /// A scratch folder for one test, removed when it drops: when the test ends, and when an
    /// assertion fails part-way through. Plain folders were left behind by every run of the
    /// tests that didn't remove theirs, several hundred of them in the temp folder.
    pub(crate) struct TempDir(PathBuf);

    impl std::ops::Deref for TempDir {
        type Target = PathBuf;
        fn deref(&self) -> &PathBuf {
            &self.0
        }
    }

    impl AsRef<Path> for TempDir {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Made before the state that opens a store in it, so it drops after the store has closed.
    pub(crate) fn temp_dir(name: &str) -> TempDir {
        let d = std::env::temp_dir().join(format!(
            "folderskin-community-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        TempDir(d)
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
        let (import, store) = (temp_dir("import"), temp_dir("import-store"));
        let dir = import.join("test-colours");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pack.json"), PACK).unwrap();
        std::fs::write(dir.join("teal.png"), png(512, 480, [20, 140, 150, 255])).unwrap();
        std::fs::write(dir.join("rust.png"), png(512, 480, [180, 70, 30, 255])).unwrap();

        let state = AppState::default();
        state.open_store(store.to_path_buf());
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
        let store = temp_dir("update-store");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
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
        let heard = Mutex::new(Vec::new());
        let update = replace_pack(&state, "test-colours", &pack, &new, "v2".into(), &|p| {
            lock(&heard).push(p)
        })
        .unwrap();
        let heard = heard.into_inner().unwrap();
        assert_eq!(heard.first(), Some(&PackProgress::save(0, 2)));
        assert_eq!(heard.last(), Some(&PackProgress::save(2, 2)), "{heard:?}");
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
        assert_eq!(state.saved_skins().len(), 2);
        assert_eq!(
            state.installed_packs().get("test-colours"),
            Some(&Some("v2".to_string()))
        );

        // A version with a bad picture changes nothing.
        let broken = vec![png(512, 480, [1, 2, 3, 255]), png(100, 100, [1, 2, 3, 255])];
        assert!(replace_pack(
            &state,
            "test-colours",
            &pack,
            &broken,
            "v3".into(),
            &no_progress
        )
        .is_err());
        assert_eq!(state.saved_skins().len(), 2);
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
        let store = temp_dir("bad-store");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
        let pictures = vec![png(512, 480, [1, 2, 3, 255]), png(100, 100, [1, 2, 3, 255])];
        let err =
            save_pack(&state, "test-colours", &pack, &pictures, None, &no_progress).unwrap_err();
        assert!(err.contains("rust.png"), "{err}");
        assert!(state.saved_skins().is_empty(), "nothing was saved");
    }

    #[test]
    fn exported_pictures_are_lossless_and_fit_the_pack_limits() {
        let rgba = image::RgbaImage::from_fn(2048, 1916, |x, y| {
            image::Rgba([(x / 9) as u8, (y / 8) as u8, 90, 255])
        });
        let art = SkinImage::Artwork(std::sync::Arc::new(folderskin_core::compositor::Artwork {
            rgba: rgba.clone(),
            focus: (0.5, 0.5),
        }));
        let made = encode_for_pack(&art).unwrap();
        assert_eq!(made.scaled_to, None);
        assert_eq!(pack::check_new_picture(&made.webp).unwrap(), (1024, 958));
        let back = image::load_from_memory(&made.webp).unwrap().to_rgba8();
        assert_eq!(back, store::shrink_to(rgba, 1024), "every pixel");

        // A finished folder keeps its transparency exactly.
        let cut = image::RgbaImage::from_fn(1100, 1000, |x, y| {
            let alpha = if x < 100 { 0 } else { (x / 5) as u8 };
            image::Rgba([(y / 4) as u8, 40, 200, alpha])
        });
        let folder = SkinImage::Folder(std::sync::Arc::new(cut.clone()));
        let made = encode_for_pack(&folder).unwrap();
        assert!(pack::check_new_picture(&made.webp).is_ok());
        let back = image::load_from_memory(&made.webp).unwrap().to_rgba8();
        let expected = store::shrink_to(cut, 1024);
        assert_eq!(back.dimensions(), expected.dimensions());
        for (got, want) in back.pixels().zip(expected.pixels()) {
            assert_eq!(got.0[3], want.0[3], "alpha");
            if want.0[3] > 0 {
                assert_eq!(got, want, "every colour that shows");
            }
        }

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
        let store = temp_dir("order-store");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
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
            .saved_skins()
            .into_iter()
            .map(|(entry, _)| entry.name)
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
        let store = temp_dir("twice-store");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
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
        assert_eq!(state.saved_skins().len(), 1);
    }

    #[test]
    fn a_pack_that_cannot_be_saved_adds_nothing() {
        let dir = temp_dir("full-store");
        let state = AppState::default();
        state.open_store(dir.to_path_buf());
        let pack = Pack::parse(PACK.as_bytes()).unwrap();
        let pictures = vec![png(512, 480, [1, 2, 3, 255]), png(512, 480, [4, 5, 6, 255])];
        // Something in the way of the second picture, as a full disk would be.
        let second = store::skin_id(&pictures[1]);
        let stem = second.strip_prefix("user:").unwrap();
        std::fs::create_dir(dir.join(format!("{stem}.webp"))).unwrap();

        let err =
            save_pack(&state, "test-colours", &pack, &pictures, None, &no_progress).unwrap_err();
        assert!(err.starts_with("couldn't save those skins"), "{err}");
        assert!(state.saved_skins().is_empty());
        assert!(state.installed_packs().is_empty());
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, [format!("{stem}.webp")], "only what was in the way");
    }

    /// Serves `files` (path, body, delay before answering) over HTTP on a free local port, and
    /// counts the most requests it had at once. Returns the base URL and that count. A request
    /// with a query is answered from `<path>?` when that is one of the files, so a test can
    /// answer Refresh's cache-busting request differently.
    pub(crate) fn serve(files: Vec<(String, Vec<u8>, Duration)>) -> (String, Arc<AtomicUsize>) {
        let (base, most, _) = serve_logged(files);
        (base, most)
    }

    /// [`serve`], also keeping every path asked for, query and all, in the order they came.
    pub(crate) fn serve_logged(
        files: Vec<(String, Vec<u8>, Duration)>,
    ) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Write};
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
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
                let log = log.clone();
                std::thread::spawn(move || {
                    most.fetch_max(open.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                    let mut reader = BufReader::new(&stream);
                    let mut request = String::new();
                    let _ = reader.read_line(&mut request);
                    let mut header = String::new();
                    while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                        header.clear();
                    }
                    let asked = request.split(' ').nth(1).unwrap_or("");
                    lock(&log).push(asked.to_string());
                    let path = asked.split('?').next().unwrap_or("");
                    let with_query = files
                        .get(&format!("{path}?"))
                        .filter(|_| asked.contains('?'));
                    let (status, body) = match with_query.or_else(|| files.get(path)) {
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
        (base, most_seen, asked)
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
        assert_eq!(err, "b.png: it isn't on 127.0.0.1 any more; try Refresh");
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

    // ---------- the catalog, the published tree and index.json ----------

    use crate::catalog::{self, Community, LastVisit};
    use crate::previews::Asked;
    use folderskin_catalog::build;
    use folderskin_catalog::tree::{CatalogRef, Head, PublishedSkin};
    use tauri::async_runtime::block_on;
    use tauri::http::StatusCode;

    /// A skin of a published pack: its file, its name and its picture.
    pub(crate) type Picture = (&'static str, &'static str, Vec<u8>);
    /// A published pack: id, name, tags and skins.
    pub(crate) type Published<'a> = (&'a str, &'a str, &'a [&'a str], Vec<Picture>);
    pub(crate) type Served = Vec<(String, Vec<u8>, Duration)>;

    /// A WebP as far as the preview scheme can tell, which says whose it is.
    pub(crate) fn webp(of: &str) -> Vec<u8> {
        [b"RIFF\0\0\0\0WEBPVP8 ".as_slice(), of.as_bytes()].concat()
    }

    /// `packs` published as `packs catalog` writes them, as files served under `/v2`, with the
    /// head.json among them and each pack's hash.
    pub(crate) fn tree_files(packs: &[Published]) -> (Served, Head, HashMap<String, String>) {
        let mut files: Served = Vec::new();
        let mut serve = |path: String, body: Vec<u8>| files.push((path, body, Duration::ZERO));
        let mut records = Vec::new();
        let mut hashes = HashMap::new();
        for (id, name, tags, skins) in packs {
            let listed: Vec<(&str, &str)> = skins.iter().map(|(f, n, _)| (*f, *n)).collect();
            let manifest = listing(&listed);
            let hash = pack::pack_hash(
                manifest.as_bytes(),
                skins.iter().map(|(f, _, b)| (*f, b.as_slice())),
            );
            let published = PublishedPack {
                version: tree::MANIFEST_VERSION,
                id: id.to_string(),
                hash: hash.clone(),
                name: name.to_string(),
                author: "prajwal-svm".into(),
                license: "CC0-1.0".into(),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                skins: skins
                    .iter()
                    .map(|(file, name, bytes)| PublishedSkin {
                        file: file.to_string(),
                        name: name.to_string(),
                        tags: Vec::new(),
                        sha256: tree::sha256_hex(bytes),
                        bytes: bytes.len() as u64,
                        w: 256,
                        h: 256,
                    })
                    .collect(),
            };
            for (skin, (_, _, bytes)) in published.skins.iter().zip(skins) {
                serve(
                    format!("/v2/{}", tree::picture_path(&skin.sha256, "png")),
                    bytes.clone(),
                );
                serve(
                    format!("/v2/{}", tree::thumb_path(&skin.sha256)),
                    webp(&skin.sha256),
                );
            }
            serve(format!("/v2/{}", tree::strip_path(&hash)), webp(&hash));
            let manifest = serde_json::to_vec(&published).unwrap();
            let manifest_sha = tree::sha256_hex(&manifest);
            serve(format!("/v2/{}", tree::manifest_path(id, &hash)), manifest);
            records.push(folderskin_catalog::PackRecord {
                id: id.to_string(),
                name: name.to_string(),
                author: "prajwal-svm".into(),
                license: "CC0-1.0".into(),
                tags: published.tags.clone(),
                hash: hash.clone(),
                manifest: manifest_sha,
                added: 0,
                count: skins.len(),
                bytes: 0,
                skin_names: skins.iter().map(|(_, n, _)| n.to_string()).collect(),
            });
            hashes.insert(id.to_string(), hash);
        }
        let bytes = build::to_bytes(&records).unwrap();
        let generation = tree::sha256_hex(&bytes)[..16].to_string();
        let gz = tree::gzip(&bytes);
        let head = Head {
            version: tree::HEAD_VERSION,
            generation: generation.clone(),
            packs: records.len(),
            skins: records.iter().map(|r| r.count).sum(),
            catalog: CatalogRef {
                url: tree::catalog_path(&generation),
                sha256: tree::sha256_hex(&gz),
                bytes: gz.len() as u64,
            },
            featured: Vec::new(),
            official: Vec::new(),
            mirrors: Vec::new(),
            moved: Default::default(),
        };
        serve(format!("/v2/{}", tree::catalog_path(&generation)), gz);
        serve("/v2/head.json".into(), serde_json::to_vec(&head).unwrap());
        (files, head, hashes)
    }

    /// `files` with their head.json changed by `change`.
    pub(crate) fn changed_head(files: Served, change: impl Fn(&mut Head)) -> Served {
        files
            .into_iter()
            .map(|(path, body, delay)| {
                if path != "/v2/head.json" {
                    return (path, body, delay);
                }
                let mut head = Head::parse(&body).unwrap();
                change(&mut head);
                (path, serde_json::to_vec(&head).unwrap(), delay)
            })
            .collect()
    }

    pub(crate) fn colour_packs() -> Vec<Published<'static>> {
        vec![
            (
                "reds",
                "Reds",
                &["warm"],
                vec![("ruby.png", "Ruby", png(256, 256, [200, 30, 30, 255]))],
            ),
            (
                "blues",
                "Blues",
                &["cool"],
                vec![
                    ("navy.png", "Navy", png(256, 256, [20, 30, 120, 255])),
                    ("sky.png", "Sky", png(256, 256, [90, 160, 230, 255])),
                ],
            ),
        ]
    }

    fn query(q: &str) -> Query<'_> {
        Query {
            q,
            limit: 20,
            ..Query::default()
        }
    }

    #[test]
    fn until_a_tree_is_published_the_list_comes_from_index_json() {
        let index = r#"{ "version": 1, "packs": [
  { "id": "colours", "name": "Colours", "author": "prajwal-svm", "license": "CC0-1.0",
    "tags": ["colour"], "count": 8, "hash": "8c46dc19991a23f9" },
  { "id": "greek-art", "name": "Greek Art", "author": "someone", "license": "CC0-1.0",
    "tags": ["classic art"], "count": 16 },
  { "id": "../escape", "name": "No", "author": "x", "license": "MIT", "tags": [], "count": 1 } ] }"#;
        let (base, _, asked) = serve_logged(vec![(
            "/index.json".into(),
            index.as_bytes().to_vec(),
            Duration::ZERO,
        )]);
        let source = block_on(catalog::load(&Origin::repo(&base), None, false)).unwrap();
        assert!(!source.is_tree());
        assert_eq!(source.index_base(), Some(base.as_str()));

        let installed = HashMap::from([("colours".to_string(), Some("old".to_string()))]);
        let found = search(&source, &installed, &query("col")).unwrap();
        assert_eq!((found.total, found.all), (1, 1));
        let colours = &found.packs[0];
        assert!(colours.added && colours.update, "added, and changed since");
        assert_eq!(
            colours.preview,
            previews::strip_url("colours", "8c46dc19991a23f9")
        );
        assert!(found.skins.is_empty(), "index.json has no skin names");

        let all = search(&source, &installed, &query("")).unwrap();
        assert_eq!(all.total, 2, "a pack with a bad id is left out");
        let greek = all.packs.iter().find(|p| p.id == "greek-art").unwrap();
        assert!(!greek.added && !greek.update);
        assert!(
            greek.preview.ends_with("/strip/greek-art/-"),
            "no version to name it by"
        );
        assert_eq!(all.facets.len(), 2);
        assert_eq!(
            lock(&asked).as_slice(),
            ["/v2/head.json", "/index.json"],
            "nothing asked past the caches"
        );

        let (nothing, _) = serve(Vec::new());
        let err = block_on(catalog::load(&Origin::repo(&nothing), None, false))
            .err()
            .unwrap();
        assert_eq!(err, "the community packs aren't published yet");
    }

    #[test]
    fn a_published_tree_is_searched_here_and_kept_for_a_visit_without_a_connection() {
        let (files, head, hashes) = tree_files(&colour_packs());
        let (base, _, asked) = serve_logged(files);
        let dir = temp_dir("tree-cache");
        let source = block_on(catalog::load(&Origin::repo(&base), Some(&dir), false)).unwrap();
        assert!(source.is_tree() && source.last_visit.is_none());
        let found = search(&source, &HashMap::new(), &query("nav")).unwrap();
        assert_eq!(
            (found.last_visit, found.generation),
            (None, head.generation.clone())
        );
        assert_eq!(found.packs[0].id, "blues");
        assert_eq!(
            found.skins,
            [SkinHitDto {
                pack: "blues".into(),
                pack_name: "Blues".into(),
                name: "Navy".into(),
                index: 0,
                thumbnail: previews::skin_url("blues", &hashes["blues"], 0),
            }]
        );
        let hit_packs: Vec<&str> = found.hit_packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(hit_packs, ["blues"], "the pack to open it in");
        assert!(dir
            .join(format!("catalog-{}.sqlite", head.generation))
            .is_file());
        assert!(
            lock(&asked).iter().all(|p| !p.contains('?')),
            "nothing asked past the caches: {:?}",
            lock(&asked)
        );
        drop(source);

        // Nothing listens here: the copy kept from the visit before, marked as such.
        let nowhere = "http://127.0.0.1:9";
        let offline = block_on(catalog::load(&Origin::repo(nowhere), Some(&dir), false)).unwrap();
        assert!(offline.is_tree());
        assert_eq!(offline.last_visit, Some(LastVisit::Offline));
        let found = search(&offline, &HashMap::new(), &query("sky")).unwrap();
        assert_eq!(found.total, 1);
        assert_eq!(found.last_visit.as_deref(), Some("you're offline"));
        drop(offline);

        // A host answering with something that isn't head.json, such as a network's sign-in
        // page, isn't called offline.
        let (portal, _) = serve(vec![(
            "/v2/head.json".into(),
            b"<html>Sign in to the network</html>".to_vec(),
            Duration::ZERO,
        )]);
        let signed_out =
            block_on(catalog::load(&Origin::repo(&portal), Some(&dir), false)).unwrap();
        assert_eq!(
            signed_out.last_visit,
            Some(LastVisit::Unanswered("127.0.0.1".into()))
        );
        assert_eq!(
            search(&signed_out, &HashMap::new(), &query("sky"))
                .unwrap()
                .last_visit
                .as_deref(),
            Some("127.0.0.1 isn't answering")
        );
        // Windows can't change a file SQLite still has open.
        drop(signed_out);

        // The kept catalog is used only while it is still the file its generation names.
        let kept = dir.join(format!("catalog-{}.sqlite", head.generation));
        let mut bytes = std::fs::read(&kept).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        std::fs::write(&kept, bytes).unwrap();
        assert!(block_on(catalog::load(&Origin::repo(nowhere), Some(&dir), false)).is_err());
        let again = block_on(catalog::load(&Origin::repo(&base), Some(&dir), false)).unwrap();
        assert_eq!(
            search(&again, &HashMap::new(), &query("sky"))
                .unwrap()
                .total,
            1
        );
        drop(again);
        // Refresh says it couldn't get through, rather than pass old packs off as new.
        let err = block_on(catalog::load(&Origin::repo(nowhere), Some(&dir), true))
            .err()
            .unwrap();
        assert!(err.starts_with("couldn't reach 127.0.0.1."), "{err}");

        // Refresh asks for head.json past the caches, and for nothing else it already has.
        lock(&asked).clear();
        drop(block_on(catalog::load(&Origin::repo(&base), Some(&dir), true)).unwrap());
        let asked = lock(&asked).clone();
        assert_eq!(asked.len(), 1, "{asked:?}");
        assert!(asked[0].starts_with("/v2/head.json?t="), "{asked:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_catalog_that_isnt_the_one_head_json_describes_is_refused() {
        let (mut files, _, _) = tree_files(&colour_packs());
        let (other, _, _) = tree_files(&colour_packs()[..1]);
        let other_catalog = other
            .into_iter()
            .find(|(path, ..)| path.ends_with(".sqlite.gz"))
            .unwrap()
            .1;
        for (path, body, _) in &mut files {
            if path.ends_with(".sqlite.gz") {
                *body = other_catalog.clone();
            }
        }
        let (base, _) = serve(files);
        let dir = temp_dir("tampered");
        let err = block_on(catalog::load(&Origin::repo(&base), Some(&dir), false))
            .err()
            .unwrap();
        assert!(err.contains("arrived damaged"), "{err}");
        let kept: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert!(kept.is_empty(), "nothing is kept: {kept:?}");
    }

    #[test]
    fn a_published_pack_arrives_by_content_and_every_picture_is_checked() {
        let packs = colour_packs();
        let (files, _, hashes) = tree_files(&packs);
        let (base, _, asked) = serve_logged(files.clone());
        let source = block_on(catalog::load(&Origin::repo(&base), None, false)).unwrap();
        let heard = Mutex::new(Vec::new());
        let (pack, hash, pictures) =
            block_on(download_pack(&source, "blues", &|p| lock(&heard).push(p))).unwrap();
        assert_eq!(hash, hashes["blues"]);
        let names: Vec<&str> = pack.skins.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Navy", "Sky"]);
        assert_eq!(pictures, [packs[1].3[0].2.clone(), packs[1].3[1].2.clone()]);
        let expected: Vec<PackProgress> = (0..=2).map(|n| PackProgress::download(n, 2)).collect();
        assert_eq!(heard.into_inner().unwrap(), expected);
        let pictures_asked: Vec<String> = lock(&asked)
            .iter()
            .filter(|p| p.starts_with("/v2/pictures/"))
            .cloned()
            .collect();
        assert_eq!(pictures_asked.len(), 2);
        assert!(pictures_asked.iter().all(|p| !p.contains('?')));

        // A picture whose bytes aren't the ones the manifest names stops the whole pack.
        let navy = tree::sha256_hex(&packs[1].3[0].2);
        let tampered: Served = files
            .into_iter()
            .map(|(path, body, delay)| {
                if path.contains(&navy) && path.starts_with("/v2/pictures/") {
                    (path, png(256, 256, [1, 2, 3, 255]), delay)
                } else {
                    (path, body, delay)
                }
            })
            .collect();
        let (base, _) = serve(tampered);
        let source = block_on(catalog::load(&Origin::repo(&base), None, false)).unwrap();
        let err = block_on(download_pack(&source, "blues", &no_progress)).unwrap_err();
        assert_eq!(err, "navy.png arrived damaged; try again");
        let err = block_on(download_pack(&source, "greens", &no_progress)).unwrap_err();
        assert!(err.contains("isn't listed"), "{err}");

        // A manifest that isn't the one the catalog names, however well formed, is refused too:
        // otherwise a host could point it at pictures of its own choosing.
        let (files, _, hashes) = tree_files(&packs);
        let manifest = format!("/v2/{}", tree::manifest_path("blues", &hashes["blues"]));
        let swapped: Served = files
            .into_iter()
            .map(|(path, body, delay)| {
                if path == manifest {
                    let text = String::from_utf8(body)
                        .unwrap()
                        .replace("\"Navy\"", "\"Ink\"");
                    (path, text.into_bytes(), delay)
                } else {
                    (path, body, delay)
                }
            })
            .collect();
        let (base, _) = serve(swapped);
        let source = block_on(catalog::load(&Origin::repo(&base), None, false)).unwrap();
        let err = block_on(download_pack(&source, "blues", &no_progress)).unwrap_err();
        assert_eq!(err, "that pack's list arrived damaged; try again");
    }

    #[test]
    fn a_published_pack_is_looked_through_by_its_thumbnails() {
        let (files, _, hashes) = tree_files(&colour_packs());
        let (base, _) = serve(files);
        let source = block_on(catalog::load(&Origin::repo(&base), None, false)).unwrap();
        let published = block_on(published_pack(&source, None, "blues")).unwrap();
        assert_eq!(published.hash, hashes["blues"]);
        let skins = published_skins(&published);
        assert_eq!(skins.len(), 2);
        assert_eq!(skins[1].name, "Sky");
        assert_eq!(skins[1].tags, ["cool"]);
        assert_eq!(
            skins[1].thumbnail,
            previews::thumb_url(&published.skins[1].sha256)
        );
    }

    #[test]
    fn previews_download_once_and_come_from_the_disk_after() {
        let (files, _, hashes) = tree_files(&colour_packs());
        let (base, _, asked) = serve_logged(files);
        let dir = temp_dir("previews");
        let community = Community::with_cache(Some(dir.clone()));
        let hash = hashes["blues"].clone();
        let strip = || Asked::Strip {
            id: "blues".into(),
            hash: Some(hash.clone()),
        };
        let sky = || Asked::Skin {
            id: "blues".into(),
            hash: hash.clone(),
            position: 1,
        };
        let picture = block_on(previews::serve(&community, &Origin::repo(&base), strip())).unwrap();
        assert_eq!(
            (picture.bytes, picture.mime, picture.lasting),
            (webp(&hash), "image/webp", true)
        );
        let sky_sha = tree::sha256_hex(&colour_packs()[1].3[1].2);
        let picture = block_on(previews::serve(&community, &Origin::repo(&base), sky())).unwrap();
        assert_eq!(
            picture.bytes,
            webp(&sky_sha),
            "the thumbnail the manifest names"
        );

        let before = lock(&asked).len();
        block_on(previews::serve(&community, &Origin::repo(&base), strip())).unwrap();
        block_on(previews::serve(&community, &Origin::repo(&base), sky())).unwrap();
        assert_eq!(lock(&asked).len(), before, "both came from the disk");

        let missing = Asked::Thumb {
            sha256: "0".repeat(64),
        };
        assert_eq!(
            block_on(previews::serve(&community, &Origin::repo(&base), missing)).unwrap_err(),
            StatusCode::NOT_FOUND
        );
        let past_the_end = Asked::Skin {
            id: "blues".into(),
            hash: hash.clone(),
            position: 5,
        };
        assert_eq!(
            block_on(previews::serve(
                &community,
                &Origin::repo(&base),
                past_the_end
            ))
            .unwrap_err(),
            StatusCode::NOT_FOUND
        );
        drop(community);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn searches_carry_on_while_refresh_downloads() {
        let (mut files, _, _) = tree_files(&colour_packs());
        let head = files
            .iter()
            .find(|(p, ..)| p == "/v2/head.json")
            .unwrap()
            .1
            .clone();
        // Refresh's head.json, asked for past the caches, takes its time.
        files.push(("/v2/head.json?".into(), head, Duration::from_millis(1500)));
        let (base, _) = serve(files);
        let community = Arc::new(Community::with_cache(None));
        block_on(community.current(&Origin::repo(&base))).unwrap();

        let refreshing = {
            let (community, base) = (community.clone(), base.clone());
            std::thread::spawn(move || {
                block_on(community.refresh(&Origin::repo(&base))).map(|_| ())
            })
        };
        std::thread::sleep(Duration::from_millis(200));
        let started = std::time::Instant::now();
        let source = block_on(community.current(&Origin::repo(&base))).unwrap();
        assert_eq!(
            search(&source, &HashMap::new(), &query("sky"))
                .unwrap()
                .total,
            1
        );
        assert!(
            started.elapsed() < Duration::from_millis(700),
            "a search waited {:?} for Refresh",
            started.elapsed()
        );
        assert!(!refreshing.is_finished(), "Refresh was still downloading");
        refreshing.join().unwrap().unwrap();
    }

    #[test]
    fn index_json_marks_official_packs_and_dates_the_packs() {
        let index = r#"{ "version": 1, "packs": [
  { "id": "colours", "name": "Colours", "author": "prajwal-svm", "license": "CC0-1.0",
    "tags": ["colour"], "count": 8, "added": 1780000000, "official": true },
  { "id": "greek-art", "name": "Greek Art", "author": "someone", "license": "CC0-1.0",
    "tags": ["classic art"], "count": 16, "added": 1790000000 } ] }"#;
        let (base, _) = serve(vec![(
            "/index.json".into(),
            index.as_bytes().to_vec(),
            Duration::ZERO,
        )]);
        let source = block_on(catalog::load(&Origin::repo(&base), None, false)).unwrap();
        let all = search(&source, &HashMap::new(), &query("")).unwrap();
        let marks: Vec<(&str, bool)> = all
            .packs
            .iter()
            .map(|p| (p.id.as_str(), p.official))
            .collect();
        assert_eq!(
            marks,
            [("greek-art", false), ("colours", true)],
            "the newest first, by the dates index.json gives"
        );
    }

    #[test]
    fn a_link_finds_its_pack_and_looks_again_for_one_published_since() {
        // Reds and Blues now. Asked again past the caches, the host has Greens too, and marks
        // Blues official.
        let (mut files, _, _) = tree_files(&colour_packs());
        let mut later = colour_packs();
        later.push((
            "greens",
            "Greens",
            &["fresh"],
            vec![("lime.png", "Lime", png(256, 256, [60, 200, 60, 255]))],
        ));
        let (newer, _, _) = tree_files(&later);
        for (path, body, delay) in newer {
            if path == "/v2/head.json" {
                let mut head = Head::parse(&body).unwrap();
                head.official = vec!["blues".into()];
                let body = serde_json::to_vec(&head).unwrap();
                files.push(("/v2/head.json?".into(), body, delay));
            } else if !files.iter().any(|(p, ..)| *p == path) {
                files.push((path, body, delay));
            }
        }
        let (base, _, asked) = serve_logged(files);
        let community = Community::with_cache(None);
        let installed = HashMap::from([("reds".to_string(), None)]);
        let origin = Origin::repo(&base);
        let look =
            |id: &str| block_on(look_up(&community, &origin, id, |_| installed.clone())).unwrap();

        let reds = look("reds").unwrap();
        assert!(reds.added && !reds.official);
        assert!(
            lock(&asked).iter().all(|p| !p.contains('?')),
            "found in the list already: {:?}",
            lock(&asked)
        );

        let greens = look("greens").unwrap();
        assert_eq!((greens.name.as_str(), greens.added), ("Greens", false));
        assert!(
            lock(&asked)
                .iter()
                .any(|p| p.starts_with("/v2/head.json?t=")),
            "asked again past the caches: {:?}",
            lock(&asked)
        );
        assert!(look("blues").unwrap().official);
        assert!(look("nope").is_none());
        let before = lock(&asked).len();
        assert!(look("../escape").is_none());
        assert_eq!(lock(&asked).len(), before, "not a pack id: nothing asked");
    }

    /// `packs` published with `moved` in their head.json, served, and the catalog loaded from
    /// them, with the community it's in use in.
    fn moved_catalog(
        packs: &[Published],
        moved: &[(&str, &str)],
    ) -> (Community, Origin, Arc<Source>, HashMap<String, String>) {
        let (files, _, hashes) = tree_files(packs);
        let moved: BTreeMap<String, String> = moved
            .iter()
            .map(|(old, now)| (old.to_string(), now.to_string()))
            .collect();
        let files = changed_head(files, |head| head.moved = moved.clone());
        let (base, _) = serve(files);
        let community = Community::with_cache(None);
        let origin = Origin::repo(&base);
        let source = block_on(community.current(&origin)).unwrap();
        (community, origin, source, hashes)
    }

    #[test]
    fn a_pack_added_under_its_old_id_shows_as_added_under_its_new_one_and_gets_its_update() {
        // Reds was published as "rubies", with a garnet as well as the ruby, and added then.
        // It has moved to "reds" since, and dropped the garnet.
        let store = temp_dir("moved-update");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
        let packs = colour_packs();
        let ruby = packs[0].3[0].2.clone();
        let garnet = png(256, 256, [120, 10, 40, 255]);
        let old = listing(&[("ruby.png", "Ruby"), ("garnet.png", "Garnet")]);
        let old = Pack::parse(old.as_bytes()).unwrap();
        let before = save_pack(
            &state,
            "rubies",
            &old,
            &[ruby, garnet],
            Some("0000000000000001".into()),
            &no_progress,
        )
        .unwrap();
        let (community, origin, source, hashes) = moved_catalog(&packs, &[("rubies", "reds")]);

        // Listed, it's added and has an update, under the id it has now.
        let found = search(&source, &library(&state, &source), &query("")).unwrap();
        let reds = found.packs.iter().find(|p| p.name == "Reds").unwrap();
        assert_eq!(reds.id, "reds");
        assert!(
            reds.added && reds.update,
            "added before it moved, changed since"
        );
        assert!(found.packs.iter().all(|p| p.id != "rubies"));
        assert_eq!(
            state.installed_packs(),
            HashMap::from([("reds".to_string(), Some("0000000000000001".to_string()))]),
            "the same version it was added at"
        );

        // A link with the old id finds it too, as the pack it is now.
        let linked = block_on(look_up(&community, &origin, "rubies", |source| {
            library(&state, source)
        }))
        .unwrap()
        .unwrap();
        assert_eq!(
            (linked.id.as_str(), linked.added, linked.update),
            ("reds", true, true)
        );

        // The update swaps the skins under the new id, and keeps the one both versions share.
        let update = block_on(update_pack(&state, source, "reds", no_progress)).unwrap();
        assert_eq!(update.removed, [before[1].id.clone()], "the garnet goes");
        assert_eq!(update.skins.len(), 1);
        assert_eq!(update.skins[0].id, before[0].id, "the ruby stays itself");
        assert_eq!(
            state.installed_packs(),
            HashMap::from([("reds".to_string(), Some(hashes["reds"].clone()))])
        );
        assert!(state
            .saved_skins()
            .iter()
            .all(|(entry, _)| entry.pack.as_deref() == Some("reds")));
    }

    #[test]
    fn an_old_id_adds_the_pack_it_moved_to_and_is_counted_under_the_new_one() {
        let store = temp_dir("moved-add");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
        let (_community, _, source, hashes) =
            moved_catalog(&colour_packs(), &[("sky-blues", "blues")]);
        // As a folderskin://install link with the old id asks for it.
        let (id, skins) = block_on(add_pack(&state, source, "sky-blues", no_progress)).unwrap();
        assert_eq!(
            id, "blues",
            "the id community_add reports the install under"
        );
        assert_eq!(skins.len(), 2);
        assert!(skins.iter().all(|s| s.pack.as_deref() == Some("blues")));
        assert_eq!(
            state.installed_packs(),
            HashMap::from([("blues".to_string(), Some(hashes["blues"].clone()))])
        );
    }

    #[test]
    fn the_first_launch_hears_where_old_ids_moved_among_its_packs() {
        let state = AppState::default();
        let (_community, _, source, _) = moved_catalog(
            &colour_packs(),
            &[
                ("rubies", "reds"),
                ("scarlets", "reds"),
                ("emeralds", "greens"),
            ],
        );
        let first = first_packs(&state, &source).unwrap();
        let ids: Vec<&str> = first.packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(
            first.moved,
            BTreeMap::from([
                ("rubies".to_string(), "reds".to_string()),
                ("scarlets".to_string(), "reds".to_string()),
            ]),
            "only moves to the packs it offers"
        );
    }

    #[test]
    fn a_pack_saved_as_a_folder_gets_a_new_id_whatever_its_name() {
        let store = temp_dir("build");
        let state = AppState::default();
        state.open_store(store.to_path_buf());
        let picture = png(512, 480, [30, 90, 160, 255]);
        let (entry, _) = state
            .save(
                NewSkin {
                    id: store::skin_id(&picture),
                    name: "Harbour at night".into(),
                    source: SkinSource::Import,
                    provider: None,
                    model: None,
                    idea: None,
                    tags: vec!["night".into()],
                    pack: None,
                    pack_name: None,
                    author: None,
                    license: None,
                    pack_hash: None,
                },
                SkinImage::Artwork(Arc::new(folderskin_core::compositor::Artwork {
                    rgba: image::load_from_memory(&picture).unwrap().to_rgba8(),
                    focus: (0.5, 0.5),
                })),
            )
            .unwrap();
        let heard = Mutex::new(Vec::new());
        let build = |name: &str, taken: &dyn Fn(&str) -> bool| {
            build_pack(
                &state,
                name,
                "sunny-otter",
                "CC0-1.0",
                &["woodblock".into()],
                std::slice::from_ref(&entry.id),
                taken,
                &|p| heard.lock().unwrap().push(p),
            )
            .map(|built| (built.id, built.files))
        };

        let (id, files) = build("Night prints", &|_| false).unwrap();
        assert_eq!(
            *heard.lock().unwrap(),
            [
                MakeProgress { done: 0, total: 1 },
                MakeProgress { done: 1, total: 1 }
            ]
        );
        let names: Vec<&str> = files.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(names, ["harbour-at-night.webp", pack::MANIFEST_FILE]);
        assert!(pack::check_new_picture(&files[0].1).is_ok());
        assert!(
            id.starts_with("night-prints-") && pack::is_generated_id(&id),
            "{id}"
        );
        let manifest = files
            .iter()
            .find(|(f, _)| f == pack::MANIFEST_FILE)
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&manifest.1).contains(&id),
            "the id isn't in pack.json"
        );
        // Two packs with one name are two packs.
        assert_ne!(build("Night prints", &|_| false).unwrap().0, id);
        // An id that's taken (a folder of that name already there) is drawn again.
        let first = std::cell::Cell::new(true);
        let again = build("Night prints", &|_| first.replace(false)).unwrap().0;
        assert!(pack::is_generated_id(&again) && !first.get());
        // A name with nothing an address can use still makes a pack.
        let (id, _) = build("東京の夜", &|_| false).unwrap();
        assert!(
            id.starts_with("pack-") && pack::is_generated_id(&id),
            "{id}"
        );
    }

    #[test]
    fn a_catalog_that_doesnt_arrive_leaves_the_last_visits_packs() {
        let (files, _, _) = tree_files(&colour_packs());
        let (base, _) = serve(files);
        let dir = temp_dir("behind");
        drop(block_on(catalog::load(&Origin::repo(&base), Some(&dir), false)).unwrap());

        // A newer head.json whose catalog isn't there (a mirror or CDN a moment behind).
        let (newer, _, _) = tree_files(&colour_packs()[..1]);
        let newer: Served = newer
            .into_iter()
            .filter(|(path, ..)| !path.ends_with(".sqlite.gz"))
            .collect();
        let (behind, _) = serve(newer);
        let source = block_on(catalog::load(&Origin::repo(&behind), Some(&dir), false)).unwrap();
        assert_eq!(source.last_visit, Some(LastVisit::Behind));
        let found = search(&source, &HashMap::new(), &query("sky")).unwrap();
        assert_eq!(found.total, 1, "Blues, from the last visit");
        assert_eq!(
            found.last_visit.as_deref(),
            Some("the newest list of packs didn't arrive")
        );
        drop(source);

        // Refresh, or a first visit with nothing kept, says what happened in a sentence.
        let err = block_on(catalog::load(&Origin::repo(&behind), Some(&dir), true))
            .err()
            .unwrap();
        assert_eq!(
            err,
            "the newest list of packs isn't there yet; try again in a minute"
        );
        let err = block_on(catalog::load(&Origin::repo(&behind), None, false))
            .err()
            .unwrap();
        assert_eq!(
            err,
            "the newest list of packs isn't there yet; try again in a minute"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_last_visits_packs_are_asked_about_again_after_a_while() {
        let (files, _, _) = tree_files(&colour_packs());
        let (base, _) = serve(files);
        let dir = temp_dir("retry");
        drop(block_on(catalog::load(&Origin::repo(&base), Some(&dir), false)).unwrap());
        let nowhere = "http://127.0.0.1:9";

        // Within the minute, the last visit's packs stay.
        let patient = Community::with_cache(Some(dir.clone()));
        let first = block_on(patient.current(&Origin::repo(nowhere))).unwrap();
        assert_eq!(first.last_visit, Some(LastVisit::Offline));
        let still = block_on(patient.current(&Origin::repo(&base))).unwrap();
        assert!(Arc::ptr_eq(&first, &still));
        drop((first, still, patient));

        // After it, the next request asks again, and the packs published now take over.
        let eager = Community::with_cache(Some(dir.clone())).retrying_after(Duration::ZERO);
        let first = block_on(eager.current(&Origin::repo(nowhere))).unwrap();
        assert_eq!(first.last_visit, Some(LastVisit::Offline));
        let failed_again = block_on(eager.current(&Origin::repo(nowhere))).unwrap();
        assert!(Arc::ptr_eq(&first, &failed_again), "still offline");
        let back = block_on(eager.current(&Origin::repo(&base))).unwrap();
        assert_eq!(back.last_visit, None);
        assert!(Arc::ptr_eq(
            &back,
            &block_on(eager.current(&Origin::repo(&base))).unwrap()
        ));
        drop((first, failed_again, back, eager));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn index_json_strips_are_kept_for_that_list_alone() {
        let index = r#"{ "version": 1, "packs": [
  { "id": "x", "name": "X", "author": "a", "license": "CC0-1.0", "tags": [], "count": 1,
    "hash": "00000000000000b2" } ] }"#;
        let (base, _, asked) = serve_logged(vec![
            (
                "/index.json".into(),
                index.as_bytes().to_vec(),
                Duration::ZERO,
            ),
            (
                "/previews/x.png".into(),
                png(8, 8, [1, 2, 3, 255]),
                Duration::ZERO,
            ),
        ]);
        let dir = temp_dir("index-strips");
        let community = Community::with_cache(Some(dir.clone()));
        let strip = || Asked::Strip {
            id: "x".into(),
            hash: Some("00000000000000b2".into()),
        };
        let strips_asked = || {
            lock(&asked)
                .iter()
                .filter(|p| p.starts_with("/previews/"))
                .count()
        };
        let picture = block_on(previews::serve(&community, &Origin::repo(&base), strip())).unwrap();
        assert_eq!(picture.mime, "image/png");
        assert!(!picture.lasting, "nothing ties previews/x.png to a version");
        block_on(previews::serve(&community, &Origin::repo(&base), strip())).unwrap();
        assert_eq!(strips_asked(), 1, "kept while this list is in use");
        let on_disk: Vec<_> = std::fs::read_dir(dir.join("files"))
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        assert!(on_disk.is_empty(), "never kept on disk: {on_disk:?}");

        // Refresh makes a new list, which asks for its strips again.
        block_on(community.refresh(&Origin::repo(&base))).unwrap();
        block_on(previews::serve(&community, &Origin::repo(&base), strip())).unwrap();
        assert_eq!(strips_asked(), 2);
        drop(community);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn packs_come_from_packs_folderskin_app_then_github_unless_another_copy_is_named() {
        let release = origin_for(None);
        assert_eq!(release.trees, ["https://packs.folderskin.app"]);
        assert_eq!(
            release.repo,
            "https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main"
        );
        // A copy served for testing is read on its own, so nothing goes to the real packs.
        assert_eq!(
            origin_for(Some(" http://127.0.0.1:8000/ ")),
            Origin::repo("http://127.0.0.1:8000")
        );
        assert_eq!(origin_for(Some("  ")), release);
    }

    #[test]
    fn errors_name_whoever_serves_the_packs() {
        for github in [
            "https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main/index.json",
            "https://github.com/prajwal-svm/folderskin/releases/download/x/y",
            "https://objects.githubusercontent.com/x",
        ] {
            assert_eq!(host(github), "GitHub", "{github}");
        }
        assert_eq!(
            host("https://packs.folderskin.app/v2/head.json"),
            "packs.folderskin.app"
        );
        assert_eq!(host("https://notgithub.com/x"), "notgithub.com");
        assert_eq!(
            Fetch::Unreachable(host("https://cdn.example.org/v2/head.json")).to_string(),
            "couldn't reach cdn.example.org. Check your connection and try again"
        );
        // No bare "not found": every reason is a sentence.
        assert_eq!(
            Fetch::NotFound("GitHub".into()).to_string(),
            "it isn't on GitHub any more; try Refresh"
        );
        assert_eq!(
            Fetch::Refused("GitHub".into(), "503 Service Unavailable".into()).to_string(),
            "GitHub answered 503 Service Unavailable; try again in a minute"
        );
    }
}
