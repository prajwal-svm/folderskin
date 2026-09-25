//! Sharing a pack, through FolderSkin's community service (services/community).
//!
//! Sharing needs no account anywhere: the computer verifies itself once, and packs go to a review
//! queue the maintainer works through. Nothing is public until a person has approved it; approved
//! packs join the community packs like any other, under an id of their own, and are credited to
//! the name chosen here. It is the only way a pack is shared from the app: the service is what
//! keeps automated uploads out and gives every pack its id, and it takes packs from people with
//! no GitHub account as readily as from people with one.
//!
//! **The computer's key.** An Ed25519 key (folderskin_share::DeviceKey) signs every request. It is
//! made the first time it is needed and kept sealed in the same store as the API keys
//! ([`crate::keys`]), under a slot of its own. That store only opens on the computer that saved
//! it, so a recovery file (the key itself, to keep privately) is how it moves to a new computer.
//!
//! **Verifying.** The service's person check (Turnstile) can't run inside the app, so the app signs
//! a link and the page opens in the browser; the app then asks the service until it says the
//! computer is verified. See services/community/src/account.ts.
//!
//! **Sending a pack.** The pack is built and checked exactly as it is for a folder
//! ([`crate::community::build_pack`]), with the handle as its author: every picture a lossless
//! WebP, so the pack looks exactly as it does here. The service is told what every picture is
//! (hash, size, dimensions) first and answers with the ones it still needs; they go up one at a
//! time, then contact sheets for the review, and the pack is sent for review.
//!
//! **Trying again.** A request that fails in a way that passes by itself (no answer, the service
//! failing, its limit on bursts of requests) is tried again after a pause: up to [`TRIES`] tries,
//! the pauses doubling from a second with full jitter, and never shorter than the service asked
//! for ([`pause`]). A cooldown or a ban is never tried again by itself: every refused request
//! counts against the computer, so the dialog shows the service's sentence and leaves it there.

use crate::community::{MakeProgress, Scaled};
use crate::keys::Keys;
use crate::state::AppState;
use folderskin_core::pack::{self, Pack};
use folderskin_core::raster;
use folderskin_share::api::{Item, Manifest, ManifestSkin, NewSubmission, Submission};
use folderskin_share::{Client, DeviceKey, Error};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::State;

/// The service's address, for release builds. `FOLDERSKIN_COMMUNITY_API`, set when FolderSkin
/// runs or when it is built, points at another, such as `wrangler dev` on this computer; a
/// development build uses a service only when that names one, so trying things out never sends a
/// pack to the real one.
const COMMUNITY_API: &str = "https://community.folderskin.app";

/// Where the computer's key is kept in [`Keys`], beside the AI keys.
const SLOT: &str = "community-key";

/// How often the app asks whether the browser check is done, and for how long.
const POLL: Duration = Duration::from_secs(3);
const VERIFY_FOR: Duration = Duration::from_secs(15 * 60);

/// Pictures per contact sheet (the service expects the same), and each one's side on it.
const PER_SHEET: usize = 16;
const TILE: u32 = 192;
const GAP: u32 = 8;
/// The most a contact sheet may weigh at the service.
const MAX_SHEET_BYTES: usize = 300 * 1024;

const NOT_YET: &str = "This build of FolderSkin can't share packs. You can still save the pack \
                       as a folder.";
const UNREACHABLE: &str = "FolderSkin's sharing service can't be reached right now. Check your \
                           connection and try again.";
const VERIFY_FIRST: &str = "Verify this computer first, so the service knows the pack is yours.";

/// Most tries of one request, the first included.
const TRIES: u32 = 5;
/// The most the first pause before trying again may be. Each pause after it may be twice as long
/// as the one before, up to [`BACKOFF_CAP`], and is a random part of that ("full jitter"), so
/// computers that failed together don't all try again together.
const BACKOFF_BASE: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);
/// The longest the service can ask to be left before a try. Its limit on bursts of requests asks
/// for a minute. Asked for longer, as a full review queue or a pause does, the dialog says so and
/// stops rather than sit there.
const LONGEST_WAIT: Duration = Duration::from_secs(120);

/// The service `FOLDERSKIN_COMMUNITY_API` names as FolderSkin runs, then as it was built: one
/// chosen on purpose. Install counts ([`crate::installs`]) follow the same choice.
pub(crate) fn api_override() -> Option<String> {
    let running = std::env::var("FOLDERSKIN_COMMUNITY_API").ok();
    let built = option_env!("FOLDERSKIN_COMMUNITY_API").map(str::to_string);
    [running, built]
        .into_iter()
        .flatten()
        .map(|url| url.trim().to_string())
        .find(|url| !url.is_empty())
}

/// The service this build or run shares with: [`api_override`], or [`COMMUNITY_API`] in a
/// release build.
pub(crate) fn api_base() -> Option<String> {
    api_override().or_else(|| default_api(cfg!(debug_assertions)))
}

fn default_api(development: bool) -> Option<String> {
    (!development && !COMMUNITY_API.is_empty()).then(|| COMMUNITY_API.to_string())
}

/// Where FolderSkin 0.1.6 and before kept a GitHub sign-in, to share packs through GitHub.
const OLD_GITHUB_SLOT: &str = "github";

/// Forgets the GitHub sign-in an earlier FolderSkin kept to share packs through GitHub: nothing
/// uses it now, and a token nothing uses is better not kept. Writes nothing when there's none.
pub fn forget_github_sign_in(keys: &Keys) {
    if let Err(e) = keys.clear(OLD_GITHUB_SLOT) {
        eprintln!("folderskin: couldn't forget the old GitHub sign-in: {e}");
    }
}

/// The computer's key, if it has one.
fn saved_key(keys: &Keys) -> Result<Option<DeviceKey>, String> {
    keys.get(SLOT)
        .map(|secret| DeviceKey::from_secret(&secret).map_err(|e| e.to_string()))
        .transpose()
}

/// The computer's key, made and saved the first time.
fn key(keys: &Keys) -> Result<DeviceKey, String> {
    if let Some(key) = saved_key(keys)? {
        return Ok(key);
    }
    let key = DeviceKey::generate().map_err(|e| e.to_string())?;
    keys.set(SLOT, &key.to_secret())?;
    Ok(key)
}

fn client(key: DeviceKey) -> Result<Client, String> {
    let base = api_base().ok_or_else(|| NOT_YET.to_string())?;
    Client::new(&base, key).map_err(|e| e.to_string())
}

/// The error as the dialog shows it: the service's own sentence, or one about being offline.
fn said(e: Error) -> String {
    match e {
        Error::Offline => UNREACHABLE.into(),
        other => other.to_string(),
    }
}

/// Whether sharing can be used here, and who this computer is to the service.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ShareStatus {
    /// False when the service isn't set up in this build, can't be reached, is paused, or won't
    /// take packs from this computer.
    pub available: bool,
    /// Why not, as a sentence.
    pub reason: Option<String>,
    pub verified: bool,
    /// The name packs from this computer are credited to, once verified.
    pub handle: Option<String>,
    /// Whether this computer has a key yet, which is what a recovery file saves.
    pub has_key: bool,
    /// The version of the pack terms packs are sent under now, as the service says: what a pack
    /// records the person agreed to, so it's taken from the service rather than built in, and a
    /// new version of the terms needs no new FolderSkin. `None` while sharing can't be used.
    pub terms_version: Option<u32>,
}

impl ShareStatus {
    fn unavailable(reason: &str, has_key: bool) -> ShareStatus {
        ShareStatus {
            available: false,
            reason: Some(reason.into()),
            verified: false,
            handle: None,
            has_key,
            terms_version: None,
        }
    }
}

async fn status_of(keys: &Keys) -> Result<ShareStatus, String> {
    status_at(keys, api_base().as_deref()).await
}

/// [`status_of`] with the service at `base`, or with none. What the service tells anyone
/// (`/v1/status`: whether it takes packs, and the version of the pack terms they're sent under)
/// is asked every time; who this computer is (`/v1/me`) only once it has a key, since opening
/// the dialog shouldn't make one. The two are asked together.
async fn status_at(keys: &Keys, base: Option<&str>) -> Result<ShareStatus, String> {
    let Some(base) = base else {
        return Ok(ShareStatus::unavailable(NOT_YET, false));
    };
    let saved = saved_key(keys)?;
    let has_key = saved.is_some();
    let key = match saved {
        Some(key) => key,
        // Signs nothing: the status is public.
        None => DeviceKey::generate().map_err(|e| e.to_string())?,
    };
    let client = Client::new(base, key).map_err(|e| e.to_string())?;
    let (status, me) = if has_key {
        let (status, me) = futures_util::future::join(client.status(), client.me()).await;
        (status, Some(me))
    } else {
        (client.status().await, None)
    };
    let status = match status {
        Ok(status) => status,
        Err(e) => return Ok(ShareStatus::unavailable(&said(e), has_key)),
    };
    let me = match me.transpose() {
        Ok(me) => me,
        Err(e) => return Ok(ShareStatus::unavailable(&said(e), has_key)),
    };
    if me
        .as_ref()
        .is_some_and(|me| me.tier.as_deref() == Some("banned"))
    {
        return Ok(ShareStatus::unavailable(
            "This computer can't share packs any more, because a pack from it broke the pack terms.",
            has_key,
        ));
    }
    let accepting = status.accepting && me.as_ref().is_none_or(|me| me.accepting);
    Ok(ShareStatus {
        available: accepting,
        reason: (!accepting).then(|| paused(&status.message)),
        verified: me.as_ref().is_some_and(|me| me.verified),
        handle: me.and_then(|me| me.handle),
        has_key,
        terms_version: accepting.then_some(status.terms_version),
    })
}

fn paused(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        "Sharing is paused for now. Please try again later.".into()
    } else {
        message.into()
    }
}

/// Whether sharing can be used here. A build with no service says so without asking anything
/// over the network; one with a service asks it.
#[tauri::command]
pub async fn share_status(keys: State<'_, Keys>) -> Result<ShareStatus, String> {
    status_of(&keys).await
}

// ---------- verifying ----------

/// Set while the app waits for the browser check, so closing the dialog stops the waiting.
#[derive(Default)]
pub struct Waiting(AtomicBool);

/// The signed page that verifies this computer under `handle`, for the dialog to open in the
/// browser. Makes the computer's key the first time. The link is signed by the service's clock,
/// so a computer whose own clock is out can still be verified.
#[tauri::command]
pub async fn share_verify(keys: State<'_, Keys>, handle: String) -> Result<String, String> {
    let handle = handle.trim();
    if !folderskin_share::is_handle(handle) {
        return Err(
            "A name is 3 to 39 letters, digits and single dashes, not starting or ending with a dash."
                .into(),
        );
    }
    client(key(&keys)?)?.verify_link(handle).await.map_err(said)
}

/// Waits until the service says this computer is verified, asking every few seconds. Resolves
/// with an error when stopped, or when the check wasn't finished within a quarter of an hour.
#[tauri::command]
pub async fn share_wait(
    keys: State<'_, Keys>,
    waiting: State<'_, Waiting>,
) -> Result<ShareStatus, String> {
    let client = client(saved_key(&keys)?.ok_or_else(|| VERIFY_FIRST.to_string())?)?;
    waiting.0.store(true, Ordering::SeqCst);
    let started = Instant::now();
    loop {
        tokio::time::sleep(POLL).await;
        if !waiting.0.load(Ordering::SeqCst) {
            return Err("Verifying was stopped.".into());
        }
        match client.me().await {
            Ok(me) if me.verified => {
                waiting.0.store(false, Ordering::SeqCst);
                return status_of(&keys).await;
            }
            // Not yet, or a moment offline: keep asking until the time is up.
            Ok(_) | Err(Error::Offline) => {}
            Err(e) => {
                waiting.0.store(false, Ordering::SeqCst);
                return Err(said(e));
            }
        }
        if started.elapsed() > VERIFY_FOR {
            waiting.0.store(false, Ordering::SeqCst);
            return Err(
                "The check in your browser wasn't finished in time. Start again when you're ready."
                    .into(),
            );
        }
    }
}

#[tauri::command]
pub fn share_cancel(waiting: State<'_, Waiting>) {
    waiting.0.store(false, Ordering::SeqCst);
}

// ---------- the recovery file ----------

/// Saves the computer's key to `path`, readable by its owner only where the system allows.
#[tauri::command]
pub async fn share_save_key(keys: State<'_, Keys>, path: String) -> Result<(), String> {
    let key =
        saved_key(&keys)?.ok_or_else(|| "This computer has no sharing key yet.".to_string())?;
    let text = key.recovery_file();
    tauri::async_runtime::spawn_blocking(move || {
        write_private(std::path::Path::new(&path), text.as_bytes())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("The recovery file couldn't be saved: {e}"))
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Takes the key in a recovery file as this computer's, and says who that makes it.
#[tauri::command]
pub async fn share_load_key(keys: State<'_, Keys>, path: String) -> Result<ShareStatus, String> {
    let text = tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(&path)
            .and_then(|f| f.take(16 * 1024).read_to_string(&mut text))
            .map(|_| text)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|_| "That file couldn't be read.".to_string())?;
    let key = DeviceKey::from_recovery_file(&text).map_err(|e| e.to_string())?;
    keys.set(SLOT, &key.to_secret())?;
    status_of(&keys).await
}

// ---------- sending a pack ----------

/// A pack to send for review, and what the person answered about it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareRequest {
    pub name: String,
    pub license: String,
    pub tags: Vec<String>,
    pub skin_ids: Vec<String>,
    /// Credits, in their words.
    pub notes: String,
    /// Where the pictures came from: own, ai, mixed or licensed.
    pub source: String,
    /// The version of the pack terms they agreed to: the one [`ShareStatus`] said the service
    /// sends packs under.
    pub terms_version: u32,
}

/// How far sending has got, for the dialog to show.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum ShareProgress {
    /// Making sure the computer is verified, before the pack is built.
    Preparing,
    /// Making the pictures ready (lossless WebP, several at once): `done` of `total` are.
    Encoding { done: usize, total: usize },
    /// Telling the service what is coming.
    Checking,
    /// Sending the pictures, one at a time.
    Uploading { done: usize, total: usize },
    /// The last checks, and into the review queue.
    Finishing,
    /// A request failed in a way that may pass, and is tried again in `seconds`. The stage it
    /// was part of is sent again when it is.
    Waiting { seconds: u64 },
}

/// A pack that is in the review queue.
#[derive(Serialize, Clone, Debug)]
pub struct Shared {
    pub submission_id: String,
    pub name: String,
    pub pictures: usize,
    /// The pictures made smaller than 1024 px to fit the size a pack's picture can be.
    pub scaled: Vec<Scaled>,
}

/// A pack ready to send: the submission, each picture's bytes, and the contact sheets.
struct Prepared {
    submission: NewSubmission,
    pictures: Vec<(String, Vec<u8>)>,
    sheets: Vec<Vec<u8>>,
}

/// Describes a built pack for the service: every picture's hash, size and dimensions, and
/// pack.json without the fields the service decides itself.
fn prepare(files: crate::community::PackFiles, request: &ShareRequest) -> Result<Prepared, String> {
    let (manifest, pictures): (Vec<_>, Vec<_>) = files
        .into_iter()
        .partition(|(file, _)| file == pack::MANIFEST_FILE);
    let manifest = manifest
        .first()
        .ok_or_else(|| "the pack has no pack.json".to_string())?;
    let pack = Pack::parse(&manifest.1).map_err(|problems| problems.join("; "))?;
    let mut items = Vec::with_capacity(pictures.len());
    for (file, bytes) in &pictures {
        let (width, height) = pack::check_new_picture(bytes).map_err(|e| format!("{file} {e}"))?;
        items.push(Item {
            file: file.clone(),
            sha256: folderskin_share::sign::sha256_hex(bytes),
            bytes: bytes.len(),
            width,
            height,
        });
    }
    let sheets = pictures
        .chunks(PER_SHEET)
        .map(|chunk| contact_sheet(chunk.iter().map(|(_, bytes)| bytes.as_slice())))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Prepared {
        submission: NewSubmission {
            manifest: Manifest {
                name: pack.name,
                tags: pack.tags,
                skins: pack
                    .skins
                    .into_iter()
                    .map(|s| ManifestSkin {
                        file: s.file,
                        name: s.name,
                        tags: s.tags,
                    })
                    .collect(),
            },
            license: request.license.clone(),
            source: request.source.clone(),
            // Credits can run over a few lines; the service keeps the breaks as \n.
            notes: request.notes.replace("\r\n", "\n").trim().to_string(),
            terms_version: request.terms_version,
            items,
        },
        pictures,
        sheets,
    })
}

/// Up to sixteen pictures small, four to a row on a light ground, as a lossy WebP: what the
/// service's triage and the maintainer's phone look at before the pictures themselves. Only the
/// pack's own pictures are lossless; a sheet is a preview, and has to be small.
fn contact_sheet<'a>(pictures: impl Iterator<Item = &'a [u8]>) -> Result<Vec<u8>, String> {
    let pictures: Vec<RgbaImage> = pictures
        .map(pack::decode_picture)
        .collect::<Result<_, _>>()?;
    let columns = pictures.len().clamp(1, 4) as u32;
    let rows = pictures.len().div_ceil(4).max(1) as u32;
    let side = |n: u32| n * TILE + (n + 1) * GAP;
    let mut sheet = RgbaImage::from_pixel(side(columns), side(rows), Rgba([242, 240, 236, 255]));
    for (i, picture) in pictures.iter().enumerate() {
        let (w, h) = picture.dimensions();
        let scale = TILE as f32 / w.max(h) as f32;
        let (tw, th) = (
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
        );
        let small = image::imageops::resize(picture, tw, th, image::imageops::FilterType::Triangle);
        let (col, row) = (i as u32 % 4, i as u32 / 4);
        let x = GAP + col * (TILE + GAP) + (TILE - tw) / 2;
        let y = GAP + row * (TILE + GAP) + (TILE - th) / 2;
        image::imageops::overlay(&mut sheet, &small, i64::from(x), i64::from(y));
    }
    for quality in [82.0, 70.0, 55.0] {
        let webp = raster::encode_webp_lossy(&sheet, quality);
        if webp.len() <= MAX_SHEET_BYTES {
            return Ok(webp);
        }
    }
    Err("the contact sheet came out too big to send".into())
}

/// Sends a pack for review. Resolves once it is in the queue; nothing is public until the
/// maintainer approves it. Every request is tried again after a pause while it fails in a way
/// that may pass ([`with_retries`]), and the dialog hears of each pause.
#[tauri::command]
pub async fn share_submit(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    pack: ShareRequest,
    on_progress: Channel<ShareProgress>,
) -> Result<Shared, String> {
    let progress = |p: ShareProgress| {
        let _ = on_progress.send(p);
    };
    progress(ShareProgress::Preparing);
    let client = client(saved_key(&keys)?.ok_or_else(|| VERIFY_FIRST.to_string())?)?;
    // Borrowed, so every request's future can hold it however often it's tried.
    let client = &client;
    let me = send(&progress, ShareProgress::Preparing, |_| client.me())
        .await
        .map_err(said)?;
    let handle = match (me.verified, me.handle) {
        (true, Some(handle)) => handle,
        _ => return Err(VERIFY_FIRST.into()),
    };

    // Built and checked before anything is sent, exactly as a pack saved as a folder is. The
    // service gives the pack its id once it's approved, so the one drawn here goes unused.
    let app = state.inner().clone();
    let prepared = {
        let (name, license, tags, skin_ids) = (
            pack.name.clone(),
            pack.license.clone(),
            pack.tags.clone(),
            pack.skin_ids.clone(),
        );
        let channel = on_progress.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let built = crate::community::build_pack(
                &app,
                &name,
                &handle,
                &license,
                &tags,
                &skin_ids,
                |_| false,
                &|p: MakeProgress| {
                    let _ = channel.send(ShareProgress::Encoding {
                        done: p.done,
                        total: p.total,
                    });
                },
            )?;
            Ok::<_, String>((prepare(built.files, &pack)?, built.scaled))
        })
        .await
        .map_err(|e| e.to_string())??
    };
    let (
        Prepared {
            submission,
            pictures,
            sheets,
        },
        scaled,
    ) = prepared;
    let name = submission.manifest.name.clone();

    progress(ShareProgress::Checking);
    let created = send(&progress, ShareProgress::Checking, |_| {
        client.create(&submission)
    })
    .await
    .map_err(said)?;
    let id = created.submission_id.as_str();
    let need: Vec<&Vec<u8>> = pictures
        .iter()
        .zip(&submission.items)
        .filter(|(_, item)| created.need.contains(&item.sha256))
        .map(|((_, bytes), _)| bytes)
        .collect();
    let total = need.len();
    for (done, bytes) in need.into_iter().enumerate() {
        let stage = ShareProgress::Uploading { done, total };
        progress(stage.clone());
        let sha = folderskin_share::sign::sha256_hex(bytes);
        // The service takes the same picture twice happily, so a try that got through before its
        // answer was lost does no harm.
        send(&progress, stage, |_| {
            client.put_item(id, &sha, bytes.clone())
        })
        .await
        .map_err(said)?;
    }
    progress(ShareProgress::Uploading { done: total, total });

    progress(ShareProgress::Finishing);
    for (n, sheet) in sheets.iter().enumerate().take(created.sheets) {
        send(&progress, ShareProgress::Finishing, |_| {
            client.put_sheet(id, n, sheet.clone())
        })
        .await
        .map_err(said)?;
    }
    send(&progress, ShareProgress::Finishing, |tried| async move {
        match client.finalize(id).await {
            // Tried again after an answer that never came: the first try got it into the queue.
            Err(e) if tried > 1 && e.code() == Some("closed") => Ok(()),
            answer => answer,
        }
    })
    .await
    .map_err(said)?;
    Ok(Shared {
        submission_id: created.submission_id.clone(),
        name,
        pictures: pictures.len(),
        scaled,
    })
}

// ---------- trying again ----------

/// Whether a request that failed with `e` may work if it's tried again in a moment: nothing
/// answered, the service itself failed (5xx), or it turned down a burst of requests (429
/// `slow_down`). A cooldown (`cooling_down`), a ban (`banned`) and every other answer say what to
/// do instead, and a try that's refused counts against the computer, so those are shown as they
/// are and never tried again by themselves.
fn transient(e: &Error) -> bool {
    match (e.status(), e.code()) {
        (_, Some("cooling_down" | "banned")) => false,
        (Some(429), Some(code)) => code == "slow_down",
        (Some(status), _) => (500..=599).contains(&status),
        _ => *e == Error::Offline,
    }
}

/// The pause before trying again, after try number `tried` (from 1) failed with `e`; `None` to
/// stop and say why. The pause is a random part (`jitter`, from 0 to 1) of a bound that starts at
/// [`BACKOFF_BASE`] and doubles with each try, to at most [`BACKOFF_CAP`]; and it's never shorter
/// than the service asked for (its `Retry-After`), unless that's longer than [`LONGEST_WAIT`],
/// when it stops instead. There are no more after [`TRIES`] tries.
fn pause(e: &Error, tried: u32, jitter: f64) -> Option<Duration> {
    if tried >= TRIES || !transient(e) {
        return None;
    }
    let bound = BACKOFF_BASE
        .saturating_mul(1 << tried.saturating_sub(1).min(16))
        .min(BACKOFF_CAP);
    let backoff = bound.mul_f64(jitter.clamp(0.0, 1.0));
    match e.retry_after() {
        Some(asked) if asked > LONGEST_WAIT => None,
        Some(asked) => Some(backoff.max(asked)),
        None => Some(backoff),
    }
}

/// Runs `request` (which is told the try it's on, from 1) until it works, fails in a way waiting
/// won't mend, or has been tried [`TRIES`] times, sitting out the pause [`pause`] gives between
/// tries with `sit`. `jitter` is the dice [`pause`] rolls.
async fn with_retries<T, Fut, Sat>(
    mut request: impl FnMut(u32) -> Fut,
    mut sit: impl FnMut(Duration) -> Sat,
    mut jitter: impl FnMut() -> f64,
) -> Result<T, Error>
where
    Fut: Future<Output = Result<T, Error>>,
    Sat: Future<Output = ()>,
{
    let mut tried = 0;
    loop {
        tried += 1;
        let e = match request(tried).await {
            Ok(answer) => return Ok(answer),
            Err(e) => e,
        };
        match pause(&e, tried, jitter()) {
            Some(wait) => sit(wait).await,
            None => return Err(e),
        }
    }
}

/// One request of sending a pack, tried as [`with_retries`] tries it. `progress` hears of each
/// pause, and then of `stage`, the part of sending it belongs to, as the next try starts.
async fn send<T, Fut>(
    progress: &impl Fn(ShareProgress),
    stage: ShareProgress,
    request: impl FnMut(u32) -> Fut,
) -> Result<T, Error>
where
    Fut: Future<Output = Result<T, Error>>,
{
    let sit = |wait: Duration| {
        progress(ShareProgress::Waiting {
            seconds: wait.as_secs_f64().ceil() as u64,
        });
        let stage = stage.clone();
        async move {
            tokio::time::sleep(wait).await;
            progress(stage);
        }
    };
    with_retries(request, sit, jitter).await
}

/// A number from 0 to 1 from the system's random source, or the middle of the range when it has
/// none to give.
fn jitter() -> f64 {
    let mut bytes = [0u8; 8];
    match getrandom::fill(&mut bytes) {
        // The top 53 bits: every value a double between 0 and 1 can hold evenly.
        Ok(()) => (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64,
        Err(_) => 0.5,
    }
}

// ---------- what happened to them ----------

/// The packs this computer has sent, newest first, with where each one is and why.
#[tauri::command]
pub async fn share_submissions(keys: State<'_, Keys>) -> Result<Vec<Submission>, String> {
    let Some(key) = saved_key(&keys)? else {
        return Ok(Vec::new());
    };
    if api_base().is_none() {
        return Ok(Vec::new());
    }
    client(key)?.submissions().await.map_err(said)
}

/// Takes one of this computer's packs back: out of the queue, or out of the community if it was
/// published.
#[tauri::command]
pub async fn share_withdraw(keys: State<'_, Keys>, id: String) -> Result<(), String> {
    let key = saved_key(&keys)?.ok_or_else(|| VERIFY_FIRST.to_string())?;
    client(key)?.withdraw(&id).await.map_err(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_build_shares_with_the_real_service_and_a_development_build_only_when_told() {
        assert_eq!(
            default_api(false).as_deref(),
            Some("https://community.folderskin.app")
        );
        assert_eq!(default_api(true), None);
    }

    fn picture(w: u32, h: u32, shade: u8) -> Vec<u8> {
        folderskin_core::raster::encode_png(&RgbaImage::from_pixel(
            w,
            h,
            Rgba([shade, 120, 80, 255]),
        ))
    }

    fn request() -> ShareRequest {
        ShareRequest {
            name: "Night prints".into(),
            license: "CC-BY-4.0".into(),
            tags: vec!["woodblock".into()],
            skin_ids: vec![],
            notes: "  Drawn by me.\r\nFrame by Jane Doe, CC0.  ".into(),
            source: "own".into(),
            terms_version: 1,
        }
    }

    #[test]
    fn a_built_pack_is_described_the_way_the_service_reads_it() {
        let koi = raster::encode_webp_lossless(&RgbaImage::from_pixel(
            512,
            400,
            Rgba([10, 120, 80, 255]),
        ));
        let fox = picture(300, 300, 200);
        let manifest = br#"{
  "version": 1,
  "name": "Night prints",
  "author": "sunny-otter",
  "license": "CC-BY-4.0",
  "tags": ["woodblock"],
  "skins": [
    { "file": "koi.webp", "name": "Koi", "tags": ["fish"] },
    { "file": "fox.png", "name": "Fox" }
  ]
}"#;
        let files = |koi: &[u8]| {
            vec![
                ("koi.webp".to_string(), koi.to_vec()),
                ("fox.png".to_string(), fox.clone()),
                (pack::MANIFEST_FILE.to_string(), manifest.to_vec()),
            ]
        };
        let prepared = prepare(files(&koi), &request()).unwrap();
        let s = &prepared.submission;
        assert_eq!(s.manifest.name, "Night prints");
        assert_eq!(s.manifest.skins[0].tags, ["fish"]);
        assert_eq!(s.license, "CC-BY-4.0");
        assert_eq!(s.notes, "Drawn by me.\nFrame by Jane Doe, CC0.");
        assert_eq!(
            s.items[0],
            Item {
                file: "koi.webp".into(),
                sha256: folderskin_share::sign::sha256_hex(&koi),
                bytes: koi.len(),
                width: 512,
                height: 400,
            }
        );
        assert_eq!(prepared.pictures.len(), 2, "pack.json isn't a picture");
        assert_eq!(prepared.sheets.len(), 1);

        // Nothing lossy is ever sent: the pack would no longer look as it was made.
        let lossy = raster::encode_webp_lossy(
            &RgbaImage::from_pixel(512, 400, Rgba([10, 120, 80, 255])),
            90.0,
        );
        assert_eq!(
            prepare(files(&lossy), &request()).err().unwrap(),
            "koi.webp isn't lossless: a pack's pictures are PNG or lossless WebP"
        );
    }

    #[test]
    fn contact_sheets_hold_sixteen_pictures_and_stay_small() {
        let pictures: Vec<Vec<u8>> = (0..17).map(|i| picture(1024, 1024, i * 12)).collect();
        let sheets: Vec<Vec<u8>> = pictures
            .chunks(PER_SHEET)
            .map(|chunk| contact_sheet(chunk.iter().map(Vec::as_slice)).unwrap())
            .collect();
        assert_eq!(sheets.len(), 2);
        for sheet in &sheets {
            assert!(sheet.len() <= MAX_SHEET_BYTES, "{}", sheet.len());
            assert!(
                sheet.starts_with(b"RIFF") && &sheet[8..12] == b"WEBP",
                "a WebP"
            );
            assert!(!pack::is_lossless_picture(sheet), "a preview, so lossy");
        }
        let full = image::load_from_memory(&sheets[0]).unwrap();
        assert_eq!(
            (full.width(), full.height()),
            (4 * TILE + 5 * GAP, 4 * TILE + 5 * GAP)
        );
        let one = image::load_from_memory(&sheets[1]).unwrap();
        assert_eq!(
            (one.width(), one.height()),
            (TILE + 2 * GAP, TILE + 2 * GAP)
        );
    }

    #[test]
    fn without_an_address_it_says_it_isnt_available_yet() {
        if api_base().is_some() {
            return; // pointed at a service by the environment
        }
        let status = tauri::async_runtime::block_on(status_of(&Keys::default())).unwrap();
        assert_eq!(status, ShareStatus::unavailable(NOT_YET, false));
        assert!(NOT_YET.ends_with("You can still save the pack as a folder."));
    }

    // ---------- trying again ----------

    fn service(status: u16, code: &str, retry_after: Option<u64>) -> Error {
        Error::Service {
            status,
            code: code.into(),
            message: format!("The service said {code}."),
            retry_after: retry_after.map(Duration::from_secs),
        }
    }

    #[test]
    fn only_what_passes_by_itself_is_tried_again() {
        for passing in [
            Error::Offline,
            service(500, "internal", None),
            service(502, "http", None),
            service(503, "http", None),
            service(504, "http", None),
            service(429, "slow_down", Some(60)),
        ] {
            assert!(transient(&passing), "{passing:?}");
        }
        for staying in [
            // A cooldown and a ban, however they're sent, are never tried again by themselves.
            service(429, "cooling_down", Some(7200)),
            service(403, "banned", Some(86400)),
            service(403, "banned", None),
            service(503, "cooling_down", None),
            // Every other refusal says what to do instead.
            service(429, "quota", None),
            service(429, "waiting", None),
            service(400, "bad_pack", None),
            service(401, "clock", None),
            service(404, "not_found", None),
            service(409, "closed", None),
            Error::Unreadable,
            Error::BadAddress,
        ] {
            assert!(!transient(&staying), "{staying:?}");
            assert_eq!(pause(&staying, 1, 0.5), None, "{staying:?}");
        }
    }

    #[test]
    fn the_pauses_double_from_a_second_at_random_and_stop_after_five_tries() {
        let offline = Error::Offline;
        let most = |tried| pause(&offline, tried, 1.0).unwrap();
        // The most each pause can be: 1 s, then twice as long each time.
        assert_eq!(
            [most(1), most(2), most(3), most(4)],
            [1, 2, 4, 8].map(Duration::from_secs)
        );
        // Full jitter: anywhere from nothing to that bound, in proportion to the roll.
        assert_eq!(pause(&offline, 1, 0.0), Some(Duration::ZERO));
        assert_eq!(pause(&offline, 3, 0.25), Some(Duration::from_secs(1)));
        assert_eq!(
            pause(&offline, 2, 7.0),
            Some(Duration::from_secs(2)),
            "a roll past 1"
        );
        // The fifth try is the last.
        assert_eq!(pause(&offline, 5, 1.0), None);
        assert_eq!(pause(&offline, 9, 1.0), None);
        // However many tries there were, no bound is over 30 s.
        for tried in 1..=64 {
            let bound = BACKOFF_BASE
                .saturating_mul(1 << (tried - 1).min(16))
                .min(BACKOFF_CAP);
            assert!(bound <= Duration::from_secs(30), "{tried}");
        }
    }

    #[test]
    fn a_pause_is_never_shorter_than_the_service_asked_for() {
        let burst = service(429, "slow_down", Some(60));
        assert_eq!(pause(&burst, 1, 1.0), Some(Duration::from_secs(60)));
        assert_eq!(pause(&burst, 4, 0.0), Some(Duration::from_secs(60)));
        // Asked for less than the backoff: the backoff.
        let short = service(503, "http", Some(1));
        assert_eq!(pause(&short, 4, 1.0), Some(Duration::from_secs(8)));
        // Asked to come back in an hour (a full queue, a pause): that's for later, not this dialog.
        assert_eq!(pause(&service(503, "queue_full", Some(3600)), 1, 1.0), None);
        assert_eq!(pause(&service(503, "http", Some(121)), 1, 1.0), None);
        assert!(pause(&service(503, "http", Some(120)), 1, 1.0).is_some());
    }

    /// Runs `answers` through [`with_retries`], one a try, with the pauses kept rather than sat
    /// out; says what came of it and the pauses.
    fn tried(
        answers: Vec<Result<&'static str, Error>>,
    ) -> (Result<&'static str, Error>, Vec<Duration>) {
        let answers = std::sync::Mutex::new(answers.into_iter());
        let paused = std::sync::Mutex::new(Vec::new());
        let answer = tauri::async_runtime::block_on(with_retries(
            |_| {
                let next = answers
                    .lock()
                    .unwrap()
                    .next()
                    .expect("no more tries than answers");
                std::future::ready(next)
            },
            |wait| {
                paused.lock().unwrap().push(wait);
                std::future::ready(())
            },
            || 1.0,
        ));
        (answer, paused.into_inner().unwrap())
    }

    #[test]
    fn a_request_is_tried_again_until_it_works_five_tries_at_most() {
        let (answer, paused) = tried(vec![
            Err(Error::Offline),
            Err(service(502, "http", None)),
            Ok("sent"),
        ]);
        assert_eq!(answer, Ok("sent"));
        assert_eq!(paused, [1, 2].map(Duration::from_secs));

        let (answer, paused) = tried(vec![
            Err(service(429, "slow_down", Some(60))),
            Err(Error::Offline),
            Err(Error::Offline),
            Err(Error::Offline),
            Err(service(503, "http", None)),
        ]);
        assert_eq!(
            answer,
            Err(service(503, "http", None)),
            "the last try's error"
        );
        assert_eq!(paused, [60, 2, 4, 8].map(Duration::from_secs));
    }

    #[test]
    fn a_cooldown_or_a_ban_ends_it_at_once_with_the_services_own_words() {
        for refused in [
            service(429, "cooling_down", Some(7200)),
            service(403, "banned", None),
        ] {
            let (answer, paused) = tried(vec![Err(Error::Offline), Err(refused.clone())]);
            assert_eq!(answer, Err(refused.clone()));
            assert_eq!(
                paused.len(),
                1,
                "only the dropped connection was waited out"
            );
            assert_eq!(said(refused.clone()), refused.to_string());
        }
        assert_eq!(said(Error::Offline), UNREACHABLE);
    }

    #[test]
    fn waiting_is_said_to_the_dialog_in_whole_seconds() {
        assert_eq!(
            serde_json::to_value(ShareProgress::Waiting { seconds: 4 }).unwrap(),
            serde_json::json!({"stage": "waiting", "seconds": 4})
        );
        assert_eq!(
            serde_json::to_value(ShareProgress::Encoding { done: 3, total: 16 }).unwrap(),
            serde_json::json!({"stage": "encoding", "done": 3, "total": 16})
        );
    }

    #[test]
    fn a_pack_is_sent_under_the_terms_the_service_asks_for_now() {
        use crate::community::tests::serve;
        use tauri::async_runtime::block_on;
        let answer = |body: &str| body.as_bytes().to_vec();
        let (base, _) = serve(vec![
            (
                "/v1/status".into(),
                answer(
                    r#"{"accepting": true, "message": "", "terms_version": 2, "max_pictures": 50}"#,
                ),
                Duration::ZERO,
            ),
            (
                "/v1/me".into(),
                answer(
                    r#"{"verified": true, "handle": "sunny-otter", "tier": "active", "accepting": true}"#,
                ),
                Duration::ZERO,
            ),
        ]);
        let keys = Keys::default();
        // Before there's a key, from what the service tells anyone, and no key is made.
        let fresh = block_on(status_at(&keys, Some(&base))).unwrap();
        assert!(fresh.available && !fresh.verified && !fresh.has_key);
        assert_eq!(fresh.terms_version, Some(2));
        assert!(!keys.has(SLOT));
        // With one, who the computer is as well.
        key(&keys).unwrap();
        let known = block_on(status_at(&keys, Some(&base))).unwrap();
        assert!(known.available && known.verified && known.has_key);
        assert_eq!(known.handle.as_deref(), Some("sunny-otter"));
        assert_eq!(known.terms_version, Some(2));

        // Paused, the service's own words, and no terms to send anything under.
        let (paused, _) = serve(vec![(
            "/v1/status".into(),
            answer(
                r#"{"accepting": false, "message": "Sharing is paused for today.", "terms_version": 2}"#,
            ),
            Duration::ZERO,
        )]);
        let status = block_on(status_at(&Keys::default(), Some(&paused))).unwrap();
        assert!(!status.available);
        assert_eq!(
            status.reason.as_deref(),
            Some("Sharing is paused for today.")
        );
        assert_eq!(status.terms_version, None);
        // No service at all.
        let none = block_on(status_at(&Keys::default(), None)).unwrap();
        assert_eq!(none, ShareStatus::unavailable(NOT_YET, false));
    }

    #[test]
    fn an_old_github_sign_in_is_forgotten_and_nothing_else() {
        let keys = Keys::default();
        keys.set(OLD_GITHUB_SLOT, "gho_left_by_0_1_6").unwrap();
        let computer = key(&keys).unwrap();
        forget_github_sign_in(&keys);
        assert!(!keys.has(OLD_GITHUB_SLOT));
        assert_eq!(
            saved_key(&keys).unwrap().unwrap().public(),
            computer.public()
        );
        // With none to forget, nothing happens.
        forget_github_sign_in(&keys);
        assert!(keys.has(SLOT));
    }

    #[test]
    fn the_key_is_made_once_and_kept() {
        let keys = Keys::default();
        assert!(saved_key(&keys).unwrap().is_none());
        let first = key(&keys).unwrap();
        let again = key(&keys).unwrap();
        assert_eq!(first.public(), again.public());
        assert!(keys.has(SLOT));
    }
}
