//! Community pictures handed to the webview as files rather than data URLs: each pack's preview
//! strip and each skin's thumbnail, served under the `fscommunity:` scheme.
//!
//! A search can list thousands of packs; the webview asks only for the pictures of the cards on
//! screen, as `<img>` loads, and gets bytes straight from here. Each picture is downloaded once,
//! at most [`crate::catalog::PREVIEW_DOWNLOADS`] at a time so a fast scroll can't queue a hundred
//! requests, and kept in a folder of the app cache that is held under [`CACHE_BYTES`]. The files
//! are named after their contents, so the webview is told it may keep them forever too.
//!
//! The webview reaches the scheme as `fscommunity://localhost/<path>` on macOS and Linux and as
//! `http://fscommunity.localhost/<path>` on Windows, which is why [`url`] builds both and the
//! Content-Security-Policy in tauri.conf.json allows both.

use crate::catalog::{Community, Source};
use folderskin_catalog::tree::{self, PublishedPack};
use folderskin_core::apply::paths::write_atomic;
use folderskin_core::pack;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

/// The scheme's name.
pub const SCHEME: &str = "fscommunity";
/// The most the kept pictures and manifests may take on disk. At about 20 KB a thumbnail that is
/// some thirteen thousand of them: more than anyone scrolls through, and small next to the skins.
pub const CACHE_BYTES: u64 = 256 * 1024 * 1024;

/// The address the webview loads `path` from.
pub fn url(path: &str) -> String {
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{SCHEME}.localhost/{path}")
    } else {
        format!("{SCHEME}://localhost/{path}")
    }
}

/// A pack's preview strip, at version `hash` ("-" when the list has no version for it).
pub fn strip_url(id: &str, hash: &str) -> String {
    url(&format!(
        "strip/{id}/{}",
        if hash.is_empty() { "-" } else { hash }
    ))
}

/// A skin's thumbnail, by its picture's SHA-256.
pub fn thumb_url(sha256: &str) -> String {
    url(&format!("thumb/{sha256}"))
}

/// The thumbnail of skin `position` of pack `id` at version `hash`, found through the pack's
/// manifest: what the strip of matching skins shows.
pub fn skin_url(id: &str, hash: &str, position: usize) -> String {
    url(&format!("skin/{id}/{hash}/{position}"))
}

/// What the webview asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Strip {
        id: String,
        hash: Option<String>,
    },
    Thumb {
        sha256: String,
    },
    Skin {
        id: String,
        hash: String,
        position: usize,
    },
}

impl Asked {
    /// Reads a request's path. Every part is checked to be what it claims, so none can name a
    /// file anywhere else.
    pub fn parse(path: &str) -> Option<Asked> {
        let path = path.trim_start_matches('/');
        // convertFileSrc-style paths arrive percent-encoded; ours never need to be.
        let parts: Vec<&str> = path.split('?').next()?.split('/').collect();
        match parts.as_slice() {
            ["strip", id, hash] if pack::is_pack_id(id) => Some(Asked::Strip {
                id: id.to_string(),
                hash: match *hash {
                    "-" => None,
                    h if tree::is_hex(h, 16) => Some(h.to_string()),
                    _ => return None,
                },
            }),
            ["thumb", sha] if tree::is_hex(sha, 64) => Some(Asked::Thumb {
                sha256: sha.to_string(),
            }),
            ["skin", id, hash, position] if pack::is_pack_id(id) && tree::is_hex(hash, 16) => {
                Some(Asked::Skin {
                    id: id.to_string(),
                    hash: hash.to_string(),
                    position: position.parse().ok().filter(|p| *p < pack::MAX_SKINS)?,
                })
            }
            _ => None,
        }
    }

    /// Its name in the disk cache; `None` for a strip with no version, which can't be kept
    /// without risking an old one forever.
    fn key(&self) -> Option<String> {
        match self {
            Asked::Strip { id, hash: Some(h) } => Some(format!("strip-{id}-{h}")),
            Asked::Strip { hash: None, .. } => None,
            Asked::Thumb { sha256 } => Some(format!("thumb-{sha256}")),
            Asked::Skin { id, hash, position } => Some(format!("skin-{id}-{hash}-{position}")),
        }
    }
}

/// The scheme's handler: answers on the async runtime, so a download never holds up the webview.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn(async move {
        let community = app.state::<Community>();
        community.init_cache(&app);
        let answer = match Asked::parse(&path) {
            Some(asked) => serve(&community, &crate::community::base_url(), asked).await,
            None => Err(StatusCode::BAD_REQUEST),
        };
        let response = match answer {
            Ok((bytes, mime)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                // Named after their contents: nothing under this name ever changes.
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(bytes),
            Err(status) => Response::builder().status(status).body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
    });
}

/// The picture `asked` names, from the disk cache or downloaded into it.
pub async fn serve(
    community: &Community,
    base: &str,
    asked: Asked,
) -> Result<(Vec<u8>, &'static str), StatusCode> {
    let files = community.files();
    let key = asked.key();
    if let Some(bytes) = cached(files, key.as_deref()).await {
        return picture(bytes);
    }
    // Waiting here, rather than in the webview, keeps a fast scroll from opening a hundred
    // connections: the pictures that scrolled away still arrive, but a few at a time.
    let _turn = community
        .downloads()
        .acquire()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    // Another request for the same picture may have fetched it while this one waited.
    if let Some(bytes) = cached(files, key.as_deref()).await {
        return picture(bytes);
    }
    let source = community
        .current(base)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let bytes = download(&source, files, &asked).await?;
    let answer = picture(bytes.clone())?;
    if let (Some(files), Some(key)) = (files, key) {
        let files = files.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || files.put(&key, &bytes)).await;
    }
    Ok(answer)
}

async fn download(
    source: &Source,
    files: Option<&DiskCache>,
    asked: &Asked,
) -> Result<Vec<u8>, StatusCode> {
    let path = match asked {
        Asked::Strip { id, hash } => match (source.is_tree(), hash) {
            (true, Some(hash)) => tree::strip_path(hash),
            (true, None) => return Err(StatusCode::NOT_FOUND),
            (false, _) => format!("previews/{id}.png"),
        },
        Asked::Thumb { sha256 } if source.is_tree() => tree::thumb_path(sha256),
        Asked::Skin { id, hash, position } if source.is_tree() => {
            let published = manifest(source, files, id, hash)
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?;
            let skin = published
                .skins
                .get(*position)
                .ok_or(StatusCode::NOT_FOUND)?;
            tree::thumb_path(&skin.sha256)
        }
        // A list made from index.json has no thumbnails.
        _ => return Err(StatusCode::NOT_FOUND),
    };
    source
        .get(&path, tree::MAX_PREVIEW_BYTES)
        .await
        .map_err(|e| {
            if e == crate::community::NOT_FOUND {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_GATEWAY
            }
        })
}

/// Pack `id`'s manifest at version `hash`, kept in the disk cache: it is named after the pack's
/// contents, so a kept one is never out of date.
pub async fn manifest(
    source: &Source,
    files: Option<&DiskCache>,
    id: &str,
    hash: &str,
) -> Result<PublishedPack, String> {
    let key = format!("pack-{id}-{hash}");
    let bytes = match cached(files, Some(&key)).await {
        Some(bytes) => bytes,
        None => {
            let bytes = source
                .get(&tree::manifest_path(id, hash), tree::MAX_MANIFEST_BYTES)
                .await?;
            if let Some(files) = files {
                let (files, bytes) = (files.clone(), bytes.clone());
                let _ = tauri::async_runtime::spawn_blocking(move || files.put(&key, &bytes)).await;
            }
            bytes
        }
    };
    let published = PublishedPack::parse(&bytes)?;
    if published.id != id || published.hash != hash {
        return Err("that pack's list doesn't match the catalog; try Refresh".into());
    }
    Ok(published)
}

async fn cached(files: Option<&DiskCache>, key: Option<&str>) -> Option<Vec<u8>> {
    let (files, key) = (files?.clone(), key?.to_string());
    tauri::async_runtime::spawn_blocking(move || files.get(&key))
        .await
        .ok()
        .flatten()
}

/// `bytes` with its type, when it is a PNG or a WebP; anything else is refused.
fn picture(bytes: Vec<u8>) -> Result<(Vec<u8>, &'static str), StatusCode> {
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return Err(StatusCode::BAD_GATEWAY);
    };
    Ok((bytes, mime))
}

/// A folder of downloaded files kept under a size limit. When a new file takes it over the
/// limit, the files used longest ago go until it is back to four fifths of it. Every step may
/// fail: a file that can't be kept is only downloaded again.
#[derive(Clone)]
pub struct DiskCache {
    dir: PathBuf,
    limit: u64,
    /// How much it holds, once counted; counted on the first write.
    used: std::sync::Arc<Mutex<Option<u64>>>,
}

/// A file read from the cache is marked used again at most this often, so looking at the same
/// pictures over and over isn't a write each time.
const TOUCH_AFTER: Duration = Duration::from_secs(60 * 60);

impl DiskCache {
    pub fn new(dir: PathBuf, limit: u64) -> DiskCache {
        DiskCache {
            dir,
            limit,
            used: Default::default(),
        }
    }

    fn path(&self, key: &str) -> Option<PathBuf> {
        let safe = !key.is_empty()
            && key.len() <= 128
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        safe.then(|| self.dir.join(key))
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.path(key)?;
        let bytes = std::fs::read(&path).ok()?;
        let now = SystemTime::now();
        let stale = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| now.duration_since(t).unwrap_or_default() > TOUCH_AFTER);
        if stale {
            let _ = std::fs::File::options()
                .append(true)
                .open(&path)
                .and_then(|f| f.set_modified(now));
        }
        Some(bytes)
    }

    pub fn put(&self, key: &str, bytes: &[u8]) {
        let Some(path) = self.path(key) else {
            return;
        };
        if std::fs::create_dir_all(&self.dir).is_err() || write_atomic(&path, bytes).is_err() {
            return;
        }
        let mut used = lock(&self.used);
        let total = match *used {
            Some(n) => n + bytes.len() as u64,
            None => self.entries().iter().map(|(_, len, _)| len).sum(),
        };
        *used = Some(if total > self.limit {
            self.shrink(self.limit / 5 * 4)
        } else {
            total
        });
    }

    /// Every file with its size and when it was last used.
    fn entries(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        let Ok(dir) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        dir.flatten()
            .filter_map(|e| {
                let meta = e.metadata().ok().filter(|m| m.is_file())?;
                Some((
                    e.path(),
                    meta.len(),
                    meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ))
            })
            .collect()
    }

    /// Deletes the files used longest ago until what is left fits in `target`; returns what is
    /// left.
    fn shrink(&self, target: u64) -> u64 {
        let mut entries = self.entries();
        entries.sort_by_key(|(_, _, used)| *used);
        let mut total: u64 = entries.iter().map(|(_, len, _)| len).sum();
        for (path, len, _) in entries {
            if total <= target {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
        total
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_the_app_hands_out_is_served() {
        let sha = "a".repeat(64);
        assert_eq!(
            Asked::parse(&format!("/thumb/{sha}")),
            Some(Asked::Thumb {
                sha256: sha.clone()
            })
        );
        assert_eq!(
            Asked::parse("/strip/classic-art/0123456789abcdef"),
            Some(Asked::Strip {
                id: "classic-art".into(),
                hash: Some("0123456789abcdef".into())
            })
        );
        assert_eq!(
            Asked::parse("strip/colours/-"),
            Some(Asked::Strip {
                id: "colours".into(),
                hash: None
            })
        );
        assert_eq!(
            Asked::parse("/skin/colours/0123456789abcdef/3"),
            Some(Asked::Skin {
                id: "colours".into(),
                hash: "0123456789abcdef".into(),
                position: 3
            })
        );
        for bad in [
            "/thumb/../../etc/passwd",
            "/thumb/ABC",
            "/strip/../x/0123456789abcdef",
            "/strip/colours/..",
            "/skin/colours/0123456789abcdef/999",
            "/skin/colours/0123456789abcdef/-1",
            "/pictures/x.png",
            "",
        ] {
            assert_eq!(Asked::parse(bad), None, "{bad}");
        }
        assert!(strip_url("colours", "").ends_with("/strip/colours/-"));
        assert!(thumb_url(&sha).contains(SCHEME));
    }

    #[test]
    fn only_pictures_are_handed_on() {
        assert_eq!(
            picture(b"\x89PNG\r\n\x1a\nrest".to_vec()).unwrap().1,
            "image/png"
        );
        assert_eq!(
            picture(b"RIFF\x10\0\0\0WEBPVP8 ".to_vec()).unwrap().1,
            "image/webp"
        );
        assert!(picture(b"<html>".to_vec()).is_err());
    }

    #[test]
    fn the_cache_keeps_what_it_can_and_lets_the_least_used_go() {
        let dir = std::env::temp_dir().join(format!("folderskin-previews-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = DiskCache::new(dir.clone(), 1000);
        let used_hours_ago = |key: &str, hours: u64| {
            let when = SystemTime::now() - Duration::from_secs(hours * 60 * 60);
            std::fs::File::options()
                .append(true)
                .open(dir.join(key))
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        assert_eq!(cache.get("thumb-a"), None);
        cache.put("thumb-a", &[1; 400]);
        cache.put("thumb-b", &[2; 400]);
        used_hours_ago("thumb-a", 3);
        used_hours_ago("thumb-b", 2);
        // Reading a marks it used now, so b is the one used longest ago.
        assert_eq!(cache.get("thumb-a").unwrap().len(), 400);
        cache.put("thumb-c", &[3; 400]);
        assert_eq!(cache.get("thumb-b"), None, "over the limit, b went");
        assert!(cache.get("thumb-a").is_some() && cache.get("thumb-c").is_some());

        // Names that could reach outside the folder are never used.
        cache.put("../escape", b"x");
        assert!(!dir.parent().unwrap().join("escape").exists());
        assert_eq!(cache.get("../escape"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
