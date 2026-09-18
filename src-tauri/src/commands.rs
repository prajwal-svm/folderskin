//! The six commands the webview calls. Heavy work runs off the async runtime's threads;
//! errors are plain sentences the drop zone shows as-is.

use crate::state::AppState;
use base64::Engine;
use folderskin_core::apply::{apply_icon, revert_icon, validate_folder};
use folderskin_core::compositor::{self, Artwork, ICON_SIZES};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Bump when the compositor's output changes so cached thumbnails are re-rendered.
const THUMB_CACHE_VERSION: u32 = 1;
const THUMB_SIZE: u32 = 512;
const DEFAULT_ID: &str = "__default__";
const MAX_IMPORT_SIDE: u32 = 2048;
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif",
];

#[derive(Serialize, Clone)]
pub struct SkinDto {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub thumbnail: String,
    pub custom: bool,
}

#[derive(Serialize)]
pub struct SkinListDto {
    pub skins: Vec<SkinDto>,
    pub default_thumbnail: String,
}

#[derive(Serialize)]
pub struct PathInfo {
    pub kind: String,
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub browse_label: String,
    pub note: String,
}

// ---------- pure helpers (unit-tested below) ----------

/// "folder" | "image" | "other" from what is on disk and the file name.
pub fn classify(path: &Path) -> &'static str {
    if path.is_dir() {
        "folder"
    } else if path.is_file() && has_image_extension(path) {
        "image"
    } else {
        "other"
    }
}

pub fn has_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Stable id for an imported picture: the same bytes always map to the same id.
pub fn custom_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("custom:{:x}", digest)[..19].to_string()
}

/// Display name for a path: the file stem, or the last component for folders.
pub fn display_name(path: &Path) -> String {
    let name = if path.is_dir() {
        path.file_name().map(|n| n.to_string_lossy().into_owned())
    } else {
        path.file_stem().map(|n| n.to_string_lossy().into_owned())
    };
    name.filter(|n| !n.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

pub fn data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

fn thumb_cache_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?.join("thumbs");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn cache_key(id: &str, bytes: &[u8], focus: (f32, f32)) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.update(focus.0.to_le_bytes());
    h.update(focus.1.to_le_bytes());
    h.update(THUMB_CACHE_VERSION.to_le_bytes());
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}-{:x}.png", h.finalize())[..(safe.len() + 1 + 16)].to_string() + ".png"
}

/// Renders (or loads from the disk cache) a thumbnail and returns it as a data URL.
fn thumbnail(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    art: &Artwork,
    source_bytes: Option<&[u8]>,
) -> String {
    if let Some(t) = state.cached_thumb(id) {
        return t;
    }
    let cache_dir = source_bytes.and_then(|_| thumb_cache_dir(app));
    let cache_path =
        cache_dir.map(|d| d.join(cache_key(id, source_bytes.unwrap_or_default(), art.focus)));
    let png = cache_path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_else(|| {
            let png = compositor::render_preview_png(art, THUMB_SIZE);
            if let Some(p) = &cache_path {
                let _ = std::fs::write(p, &png);
            }
            png
        });
    let url = data_url(&png);
    state.remember_thumb(id.to_string(), url.clone());
    url
}

fn decode_picture(path: &Path) -> Result<image::RgbaImage, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let bytes = if ext == "heic" || ext == "heif" {
        heic_to_png(path)?
    } else {
        std::fs::read(path).map_err(|_| "couldn't read that picture".to_string())?
    };
    let img =
        image::load_from_memory(&bytes).map_err(|_| "couldn't read that picture".to_string())?;
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let longest = w.max(h);
    if longest > MAX_IMPORT_SIDE {
        let s = MAX_IMPORT_SIDE as f32 / longest as f32;
        rgba = image::imageops::resize(
            &rgba,
            ((w as f32 * s).round() as u32).max(1),
            ((h as f32 * s).round() as u32).max(1),
            image::imageops::FilterType::Lanczos3,
        );
    }
    Ok(rgba)
}

#[cfg(target_os = "macos")]
fn heic_to_png(path: &Path) -> Result<Vec<u8>, String> {
    let out = std::env::temp_dir().join(format!("folderskin-{}.png", std::process::id()));
    let status = std::process::Command::new("sips")
        .args(["-s", "format", "png"])
        .arg(path)
        .arg("--out")
        .arg(&out)
        .output()
        .map_err(|_| "couldn't convert that HEIC picture".to_string())?;
    if !status.status.success() {
        return Err("couldn't convert that HEIC picture".into());
    }
    let bytes =
        std::fs::read(&out).map_err(|_| "couldn't convert that HEIC picture".to_string())?;
    let _ = std::fs::remove_file(&out);
    Ok(bytes)
}

#[cfg(not(target_os = "macos"))]
fn heic_to_png(_path: &Path) -> Result<Vec<u8>, String> {
    Err("HEIC pictures aren't supported here yet — export it as PNG or JPEG first".into())
}

// ---------- commands ----------

#[tauri::command]
pub async fn list_skins(app: AppHandle, state: State<'_, AppState>) -> Result<SkinListDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let skins = state
            .builtin()
            .iter()
            .map(|s| {
                let bytes = crate::skins::SKINS
                    .iter()
                    .find(|b| b.id == s.id)
                    .map(|b| b.bytes);
                SkinDto {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    collection: s.collection.clone(),
                    thumbnail: thumbnail(&app, &state, &s.id, &s.art, bytes),
                    custom: false,
                }
            })
            .collect();
        let default_art = compositor::default_folder_artwork();
        let default_thumbnail = thumbnail(&app, &state, DEFAULT_ID, &default_art, Some(b"default"));
        SkinListDto {
            skins,
            default_thumbnail,
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inspect_path(path: String) -> Result<PathInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err("that path doesn't exist any more".to_string());
        }
        Ok(PathInfo {
            kind: classify(&p).to_string(),
            name: display_name(&p),
            path,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn import_image(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<SkinDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        let bytes = std::fs::read(&p).map_err(|_| "couldn't read that picture".to_string())?;
        let id = custom_id(&bytes);
        let art = Arc::new(Artwork {
            rgba: decode_picture(&p)?,
            focus: (0.5, 0.5),
        });
        state.remember_custom(id.clone(), art.clone());
        let thumb = thumbnail(&app, &state, &id, &art, None);
        Ok(SkinDto {
            id,
            name: display_name(&p),
            collection: "yours".into(),
            thumbnail: thumb,
            custom: true,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_skin(
    state: State<'_, AppState>,
    folder: String,
    skin_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let folder = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
    // A whole-folder render from the AI assistant is already the icon; everything else is
    // artwork that goes through the compositor.
    let prerendered = state.prerendered(&skin_id);
    let art = match &prerendered {
        Some(_) => None,
        None => Some(
            state
                .artwork(&skin_id)
                .ok_or_else(|| "that skin isn't available any more".to_string())?,
        ),
    };
    let icons = tauri::async_runtime::spawn_blocking(move || match (prerendered, art) {
        (Some(img), _) => compositor::icon_set_from_image(&img, &ICON_SIZES),
        (None, Some(art)) => compositor::render_icon_set(&art, &ICON_SIZES),
        (None, None) => unreachable!("one of the two is always set"),
    })
    .await
    .map_err(|e| e.to_string())?;
    // NSWorkspace.setIcon is thread-safe and the PNG encodes are slow, so this stays off the
    // main thread; the window keeps painting the "Applying…" state.
    tauri::async_runtime::spawn_blocking(move || {
        apply_icon(&folder, &icons).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn revert_skin(folder: String) -> Result<(), String> {
    let folder = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || revert_icon(&folder).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// The folder's current icon as a data URL: the real OS icon on macOS, the plain rendered
/// folder elsewhere (or when the OS cannot provide one).
#[tauri::command]
pub async fn folder_icon(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: String,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&folder);
        if let Some(png) = crate::folder_icon::current_icon_png(&p, THUMB_SIZE) {
            return data_url(&png);
        }
        let default_art = compositor::default_folder_artwork();
        thumbnail(&app, &state, DEFAULT_ID, &default_art, Some(b"default"))
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn platform_info() -> PlatformInfo {
    let os = std::env::consts::OS.to_string();
    let (browse_label, note) = match os.as_str() {
        "macos" => ("your Mac", "macOS keeps the icon inside the folder itself (a hidden Icon file). Revert removes it."),
        "windows" => (
            "your PC",
            "Writes desktop.ini and folderskin.ico inside the folder. If Explorer keeps showing the old icon, press F5.",
        ),
        _ => (
            "your computer",
            "Writes .directory and .folderskin.png inside the folder (KDE, Dolphin) and sets GIO metadata for Nautilus, Nemo and Caja.",
        ),
    };
    PlatformInfo {
        os,
        browse_label: browse_label.into(),
        note: note.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension_and_kind() {
        let dir = std::env::temp_dir().join(format!("folderskin-cmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo.JPG"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert_eq!(classify(&dir), "folder");
        assert_eq!(classify(&dir.join("photo.JPG")), "image");
        assert_eq!(classify(&dir.join("notes.txt")), "other");
        assert_eq!(classify(&dir.join("missing.png")), "other");
        assert_eq!(display_name(&dir.join("photo.JPG")), "photo");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn custom_ids_are_stable_short_and_prefixed() {
        let a = custom_id(b"hello");
        assert_eq!(a, custom_id(b"hello"));
        assert_ne!(a, custom_id(b"hello!"));
        assert!(a.starts_with("custom:"));
        assert_eq!(a.len(), 19);
    }

    #[test]
    fn cache_keys_change_with_focus_and_are_filesystem_safe() {
        let a = cache_key("aurora", b"img", (0.5, 0.5));
        let b = cache_key("aurora", b"img", (0.4, 0.5));
        assert_ne!(a, b);
        assert!(a.starts_with("aurora-") && a.ends_with(".png"));
        let c = cache_key("custom:ab/cd", b"img", (0.5, 0.5));
        assert!(!c.contains(':') && !c.contains('/'));
    }
}
