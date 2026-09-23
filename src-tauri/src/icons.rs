//! The composer's icon packs beyond the built-in one: downloaded once from the `icons-v*`
//! release, checked against the SHA-256 the app itself carries (src/composer/icons/catalog.ts),
//! and kept in the app's data folder.
//!
//! The webview can't fetch them itself: the content security policy allows no remote
//! connections, and it shouldn't. It names a pack and the hash it expects; the URL is built here,
//! from a fixed base, so a webview can't point this at anything else.

use folderskin_core::apply::paths::write_atomic;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::{ipc::Channel, AppHandle, Manager};

/// Where the packs are published. `FOLDERSKIN_ICONS_URL` points at another copy, such as a local
/// folder served while trying a new pack.
const RELEASES: &str = "https://github.com/prajwal-svm/folderskin/releases/download";
/// The biggest pack today is about 5 MB; anything far past that isn't one of ours.
const MAX_PACK: u64 = 24 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct IconPackProgress {
    pub done: u64,
    pub total: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct InstalledIconPack {
    pub id: String,
    pub sha256: String,
    pub bytes: u64,
}

/// A pack id as the catalog writes them: lower-case words joined by dashes.
fn is_pack_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_release(s: &str) -> bool {
    s.strip_prefix("icons-v")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn pack_url(release: &str, id: &str) -> String {
    let base = std::env::var("FOLDERSKIN_ICONS_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .map(|u| u.trim_end_matches('/').to_string())
        .unwrap_or_else(|| format!("{RELEASES}/{release}"));
    format!("{base}/{id}.json")
}

fn packs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("icon-packs"))
        .map_err(|_| "FolderSkin has no data folder to keep icon packs in".to_string())
}

fn pack_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn hash_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.sha256"))
}

fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    let c = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("FolderSkin/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(CLIENT.get_or_init(|| c))
}

/// Downloads `url`, expecting exactly `bytes` of it whose SHA-256 is `sha256`.
async fn download(
    url: &str,
    bytes: u64,
    sha256: &str,
    progress: &(dyn Fn(IconPackProgress) + Sync),
) -> Result<Vec<u8>, String> {
    const OFFLINE: &str = "couldn't download the icon pack. Check your connection and try again";
    let mut res = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| OFFLINE.to_string())?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("that icon pack isn't published yet. Try again after the next update".into());
    }
    if !res.status().is_success() {
        return Err(format!(
            "the icon pack's server answered {}. Try again later",
            res.status()
        ));
    }
    let mut body = Vec::with_capacity(bytes as usize);
    progress(IconPackProgress {
        done: 0,
        total: bytes,
    });
    while let Some(chunk) = res.chunk().await.map_err(|_| OFFLINE.to_string())? {
        body.extend_from_slice(&chunk);
        if body.len() as u64 > bytes {
            return Err(
                "the icon pack that arrived isn't the one this version of FolderSkin expects"
                    .into(),
            );
        }
        progress(IconPackProgress {
            done: body.len() as u64,
            total: bytes,
        });
    }
    check(&body, bytes, sha256)?;
    Ok(body)
}

/// Whether `body` is exactly the pack the catalog describes.
fn check(body: &[u8], bytes: u64, sha256: &str) -> Result<(), String> {
    let hash = format!("{:x}", Sha256::digest(body));
    if body.len() as u64 != bytes || hash != sha256 {
        return Err(
            "the icon pack that arrived isn't the one this version of FolderSkin expects".into(),
        );
    }
    Ok(())
}

/// Keeps a checked pack, with its hash beside it so the list below doesn't re-read every file.
fn keep(dir: &Path, id: &str, body: &[u8], sha256: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't make the icon pack folder: {e}"))?;
    write_atomic(&pack_path(dir, id), body)
        .map_err(|e| format!("couldn't save the icon pack: {e}"))?;
    write_atomic(&hash_path(dir, id), sha256.as_bytes())
        .map_err(|e| format!("couldn't save the icon pack: {e}"))
}

fn installed(dir: &Path) -> Vec<InstalledIconPack> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<InstalledIconPack> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_stem()?.to_str()?.to_string();
            if path.extension()? != "json" || !is_pack_id(&id) {
                return None;
            }
            let sha256 = std::fs::read_to_string(hash_path(dir, &id))
                .ok()?
                .trim()
                .to_string();
            if !is_sha256(&sha256) {
                return None;
            }
            Some(InstalledIconPack {
                bytes: e.metadata().ok()?.len(),
                id,
                sha256,
            })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Downloads icon pack `id` from `release`, checks it is exactly `bytes` long with SHA-256
/// `sha256`, and keeps it.
#[tauri::command]
pub async fn icon_pack_download(
    app: AppHandle,
    id: String,
    release: String,
    sha256: String,
    bytes: u64,
    on_progress: Channel<IconPackProgress>,
) -> Result<(), String> {
    if !is_pack_id(&id)
        || !is_release(&release)
        || !is_sha256(&sha256)
        || bytes == 0
        || bytes > MAX_PACK
    {
        return Err("that isn't an icon pack FolderSkin knows".into());
    }
    let dir = packs_dir(&app)?;
    let body = download(&pack_url(&release, &id), bytes, &sha256, &|p| {
        // The window may have gone; the download is still worth finishing.
        let _ = on_progress.send(p);
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || keep(&dir, &id, &body, &sha256))
        .await
        .map_err(|e| e.to_string())?
}

/// The icon packs downloaded on this computer.
#[tauri::command]
pub fn icon_packs_installed(app: AppHandle) -> Result<Vec<InstalledIconPack>, String> {
    Ok(installed(&packs_dir(&app)?))
}

/// A downloaded pack's JSON, as the composer reads it.
#[tauri::command]
pub async fn icon_pack_read(app: AppHandle, id: String) -> Result<String, String> {
    if !is_pack_id(&id) {
        return Err("that isn't an icon pack FolderSkin knows".into());
    }
    let path = pack_path(&packs_dir(&app)?, &id);
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::read_to_string(&path).map_err(|_| "that icon pack isn't downloaded".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Forgets a downloaded pack. Designs that use its icons keep them: a layer carries its drawing.
#[tauri::command]
pub fn icon_pack_remove(app: AppHandle, id: String) -> Result<(), String> {
    if !is_pack_id(&id) {
        return Err("that isn't an icon pack FolderSkin knows".into());
    }
    let dir = packs_dir(&app)?;
    for path in [pack_path(&dir, &id), hash_path(&dir, &id)] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("couldn't remove the icon pack: {e}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-icons-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn only_well_formed_names_reach_the_disk_or_the_network() {
        for good in ["tabler", "phosphor-fill", "simple-icons", "a1"] {
            assert!(is_pack_id(good), "{good}");
        }
        for bad in [
            "",
            "../keys",
            "Tabler",
            "a/b",
            "-x",
            "x-",
            "a.json",
            &"x".repeat(41),
        ] {
            assert!(!is_pack_id(bad), "{bad}");
        }
        assert!(is_release("icons-v1") && is_release("icons-v12"));
        for bad in ["icons-v", "v1", "icons-v1/../x", "icons-vx"] {
            assert!(!is_release(bad), "{bad}");
        }
        assert!(is_sha256(&"a".repeat(64)));
        assert!(!is_sha256(&"A".repeat(64)) && !is_sha256("abc"));
    }

    #[test]
    fn a_pack_is_kept_only_when_it_is_exactly_the_one_expected() {
        let body = br#"{"format":1}"#;
        let hash = format!("{:x}", Sha256::digest(body));
        assert!(check(body, body.len() as u64, &hash).is_ok());
        assert!(
            check(body, body.len() as u64 + 1, &hash).is_err(),
            "wrong size"
        );
        assert!(check(b"{}", 2, &hash).is_err(), "wrong content");
    }

    #[test]
    fn kept_packs_are_listed_with_their_hash_and_removed_cleanly() {
        let dir = scratch("keep");
        let body = br#"{"format":1,"id":"tabler"}"#;
        let hash = format!("{:x}", Sha256::digest(body));
        keep(&dir, "tabler", body, &hash).unwrap();
        // Something that isn't a pack sits beside it and is left out.
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        std::fs::write(dir.join("stray.json"), "{}").unwrap();
        let list = installed(&dir);
        assert_eq!(
            list,
            vec![InstalledIconPack {
                id: "tabler".into(),
                sha256: hash,
                bytes: body.len() as u64
            }]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_download_is_checked_before_anything_is_kept() {
        let body = br#"{"format":1,"id":"heroicons"}"#.to_vec();
        let hash = format!("{:x}", Sha256::digest(&body));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let served = body.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut s = stream.unwrap();
                let mut req = [0u8; 1024];
                let n = s.read(&mut req).unwrap();
                let path = String::from_utf8_lossy(&req[..n]);
                let (status, payload): (&str, &[u8]) = if path.contains("/missing.json") {
                    ("404 Not Found", b"")
                } else {
                    ("200 OK", &served)
                };
                write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                )
                .unwrap();
                s.write_all(payload).unwrap();
            }
        });
        // The app's tokio has only the timer; Tauri's runtime drives the future.
        let seen = std::sync::Mutex::new(Vec::new());
        let got = tauri::async_runtime::block_on(download(
            &format!("http://{addr}/heroicons.json"),
            body.len() as u64,
            &hash,
            &|p| seen.lock().unwrap().push(p),
        ))
        .unwrap();
        assert_eq!(got, body);
        let seen = seen.into_inner().unwrap();
        assert_eq!(seen.first().map(|p| p.done), Some(0));
        assert_eq!(seen.last().map(|p| p.done), Some(body.len() as u64));
        let missing = tauri::async_runtime::block_on(download(
            &format!("http://{addr}/missing.json"),
            4,
            &hash,
            &|_| {},
        ));
        assert!(missing.unwrap_err().contains("isn't published yet"));
    }
}
