//! Sharing a pack without a GitHub account, through FolderSkin's community service
//! (services/community).
//!
//! Not everyone has a GitHub account, and signing up for one to share a few folder pictures is a
//! lot to ask. This path needs none: the computer verifies itself once, and packs go to a review
//! queue the maintainer works through. Nothing is public until a person has approved it; approved
//! packs join community/packs like any other and are credited to the name chosen here.
//!
//! **The computer's key.** An Ed25519 key (folderskin_share::DeviceKey) signs every request. It is
//! made the first time it is needed and kept sealed in the same store as the API keys and the
//! GitHub sign-in ([`crate::keys`]), under a slot of its own. That store only opens on the computer
//! that saved it, so a recovery file (the key itself, to keep privately) is how it moves to a new
//! computer.
//!
//! **Verifying.** The service's person check (Turnstile) can't run inside the app, so the app signs
//! a link and the page opens in the browser; the app then asks the service until it says the
//! computer is verified. See services/community/src/account.ts.
//!
//! **Sending a pack.** The pack is built and checked exactly as it is for GitHub
//! ([`crate::community::build_pack`]), with the handle as its author. The service is told what
//! every picture is (hash, size, dimensions) first and answers with the ones it still needs; they
//! go up one at a time, then contact sheets for the review, and the pack is sent for review.

use crate::keys::Keys;
use crate::state::AppState;
use folderskin_core::pack::{self, Pack};
use folderskin_share::api::{Item, Manifest, ManifestSkin, NewSubmission, Submission};
use folderskin_share::{Client, DeviceKey, Error};
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::State;

/// The service's address. Empty until it is deployed, which the share dialog says rather than
/// failing. `FOLDERSKIN_COMMUNITY_API`, set when FolderSkin runs or when it is built, points at
/// another, such as `wrangler dev` on this computer.
const COMMUNITY_API: &str = "";

/// Where the computer's key is kept in [`Keys`], beside the AI keys and the GitHub sign-in.
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

const NOT_YET: &str = "Sharing without GitHub isn't available yet. It will be in a later version \
                       of FolderSkin; until then, share through GitHub or save a folder.";
const UNREACHABLE: &str = "FolderSkin's sharing service can't be reached right now. Check your \
                           connection, or share through GitHub instead.";
const VERIFY_FIRST: &str = "Verify this computer first, so the service knows the pack is yours.";

fn api_base() -> Option<String> {
    let running = std::env::var("FOLDERSKIN_COMMUNITY_API").ok();
    let built = option_env!("FOLDERSKIN_COMMUNITY_API").map(str::to_string);
    [running, built, Some(COMMUNITY_API.to_string())]
        .into_iter()
        .flatten()
        .map(|url| url.trim().to_string())
        .find(|url| !url.is_empty())
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

/// Whether sharing without GitHub can be used here, and who this computer is to the service.
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
}

impl ShareStatus {
    fn unavailable(reason: &str, has_key: bool) -> ShareStatus {
        ShareStatus {
            available: false,
            reason: Some(reason.into()),
            verified: false,
            handle: None,
            has_key,
        }
    }
}

async fn status_of(keys: &Keys) -> Result<ShareStatus, String> {
    if api_base().is_none() {
        return Ok(ShareStatus::unavailable(NOT_YET, false));
    }
    let saved = saved_key(keys)?;
    let has_key = saved.is_some();
    let Some(key) = saved else {
        // Asked before there is a key, and without making one: opening the dialog shouldn't.
        let throwaway = DeviceKey::generate().map_err(|e| e.to_string())?;
        return Ok(match client(throwaway)?.status().await {
            Ok(status) if status.accepting => ShareStatus {
                available: true,
                reason: None,
                verified: false,
                handle: None,
                has_key,
            },
            Ok(status) => ShareStatus::unavailable(&paused(&status.message), has_key),
            Err(e) => ShareStatus::unavailable(&said(e), has_key),
        });
    };
    Ok(match client(key)?.me().await {
        Ok(me) if me.tier.as_deref() == Some("banned") => ShareStatus::unavailable(
            "This computer can't share packs any more, because a pack from it broke the pack terms.",
            has_key,
        ),
        Ok(me) => ShareStatus {
            available: me.accepting,
            reason: (!me.accepting).then(|| paused("")),
            verified: me.verified,
            handle: me.handle,
            has_key,
        },
        Err(e) => ShareStatus::unavailable(&said(e), has_key),
    })
}

fn paused(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        "Sharing without GitHub is paused for now. Please try again later.".into()
    } else {
        message.into()
    }
}

#[tauri::command]
pub async fn share_status(keys: State<'_, Keys>) -> Result<ShareStatus, String> {
    status_of(&keys).await
}

/// Whether this build knows a sharing service at all, without asking it anything. A build with
/// none offers only GitHub: sharing a pack forks folderskin-community and opens a pull request
/// there, and the dialog doesn't show a way it can't use.
#[tauri::command]
pub fn share_offered() -> bool {
    api_base().is_some()
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
    /// The version of the pack terms they agreed to.
    pub terms_version: u32,
}

/// How far sending has got, for the dialog to show.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum ShareProgress {
    /// Building the pack and its contact sheets.
    Preparing,
    /// Telling the service what is coming.
    Checking,
    /// Sending the pictures, one at a time.
    Uploading { done: usize, total: usize },
    /// The last checks, and into the review queue.
    Finishing,
}

/// A pack that is in the review queue.
#[derive(Serialize, Clone, Debug)]
pub struct Shared {
    pub submission_id: String,
    pub name: String,
    pub pictures: usize,
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
        let (width, height) = pack::check_picture(bytes).map_err(|e| format!("{file} {e}"))?;
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

/// Up to sixteen pictures small, four to a row on a light ground, as a JPEG: what the service's
/// triage and the maintainer's phone look at before the pictures themselves.
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
    let rgb = image::DynamicImage::ImageRgba8(sheet).to_rgb8();
    for quality in [82, 70, 55] {
        let mut jpg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpg, quality)
            .write_image(&rgb, rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            .map_err(|e| e.to_string())?;
        if jpg.len() <= MAX_SHEET_BYTES {
            return Ok(jpg);
        }
    }
    Err("the contact sheet came out too big to send".into())
}

/// Sends a pack for review. Resolves once it is in the queue; nothing is public until the
/// maintainer approves it.
#[tauri::command]
pub async fn share_submit(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    pack: ShareRequest,
    on_progress: Channel<ShareProgress>,
) -> Result<Shared, String> {
    let _ = on_progress.send(ShareProgress::Preparing);
    let client = client(saved_key(&keys)?.ok_or_else(|| VERIFY_FIRST.to_string())?)?;
    let me = client.me().await.map_err(said)?;
    let handle = match (me.verified, me.handle) {
        (true, Some(handle)) => handle,
        _ => return Err(VERIFY_FIRST.into()),
    };

    // Built and checked before anything is sent, exactly as a pack for GitHub is.
    let app = state.inner().clone();
    let prepared = {
        let (name, license, tags, skin_ids) = (
            pack.name.clone(),
            pack.license.clone(),
            pack.tags.clone(),
            pack.skin_ids.clone(),
        );
        tauri::async_runtime::spawn_blocking(move || {
            let (_, files) =
                crate::community::build_pack(&app, &name, &handle, &license, &tags, &skin_ids)?;
            prepare(files, &pack)
        })
        .await
        .map_err(|e| e.to_string())??
    };
    let Prepared {
        submission,
        pictures,
        sheets,
    } = prepared;
    let name = submission.manifest.name.clone();

    let _ = on_progress.send(ShareProgress::Checking);
    let created = client.create(&submission).await.map_err(said)?;
    let need: Vec<&(String, Vec<u8>)> = pictures
        .iter()
        .zip(&submission.items)
        .filter(|(_, item)| created.need.contains(&item.sha256))
        .map(|(picture, _)| picture)
        .collect();
    let total = need.len();
    for (done, (file, bytes)) in need.into_iter().enumerate() {
        let _ = on_progress.send(ShareProgress::Uploading { done, total });
        let sha = folderskin_share::sign::sha256_hex(bytes);
        // One more try after a dropped connection; the service takes the same picture twice happily.
        let sent = match client
            .put_item(&created.submission_id, &sha, bytes.clone())
            .await
        {
            Err(Error::Offline) => {
                client
                    .put_item(&created.submission_id, &sha, bytes.clone())
                    .await
            }
            other => other,
        };
        sent.map_err(|e| match e {
            Error::Offline => said(e),
            other => format!("{file}: {other}"),
        })?;
    }
    let _ = on_progress.send(ShareProgress::Uploading { done: total, total });

    let _ = on_progress.send(ShareProgress::Finishing);
    for (n, sheet) in sheets.into_iter().enumerate().take(created.sheets) {
        client
            .put_sheet(&created.submission_id, n, sheet)
            .await
            .map_err(said)?;
    }
    client
        .finalize(&created.submission_id)
        .await
        .map_err(said)?;
    Ok(Shared {
        submission_id: created.submission_id,
        name,
        pictures: pictures.len(),
    })
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
        let koi = picture(512, 400, 10);
        let fox = picture(300, 300, 200);
        let manifest = br#"{
  "version": 1,
  "name": "Night prints",
  "author": "sunny-otter",
  "license": "CC-BY-4.0",
  "tags": ["woodblock"],
  "skins": [
    { "file": "koi.png", "name": "Koi", "tags": ["fish"] },
    { "file": "fox.png", "name": "Fox" }
  ]
}"#;
        let files = vec![
            ("koi.png".to_string(), koi.clone()),
            ("fox.png".to_string(), fox.clone()),
            (pack::MANIFEST_FILE.to_string(), manifest.to_vec()),
        ];
        let prepared = prepare(files, &request()).unwrap();
        let s = &prepared.submission;
        assert_eq!(s.manifest.name, "Night prints");
        assert_eq!(s.manifest.skins[0].tags, ["fish"]);
        assert_eq!(s.license, "CC-BY-4.0");
        assert_eq!(s.notes, "Drawn by me.\nFrame by Jane Doe, CC0.");
        assert_eq!(
            s.items[0],
            Item {
                file: "koi.png".into(),
                sha256: folderskin_share::sign::sha256_hex(&koi),
                bytes: koi.len(),
                width: 512,
                height: 400,
            }
        );
        assert_eq!(prepared.pictures.len(), 2, "pack.json isn't a picture");
        assert_eq!(prepared.sheets.len(), 1);
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
            assert!(sheet.starts_with(&[0xff, 0xd8]), "a JPEG");
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
        assert!(NOT_YET.starts_with("Sharing without GitHub isn't available yet."));
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
