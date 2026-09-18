//! The gallery, import, apply and delete commands the webview calls (the AI assistant's are in
//! `ai.rs`). Heavy work runs off the async runtime's threads; errors are plain sentences the
//! drop zone shows as-is.

use crate::state::AppState;
use crate::store::{self, NewSkin, SavedSkin, SkinImage, SkinKind, SkinSource, MAX_STORED_SIDE};
use base64::Engine;
use folderskin_core::apply::{apply_icon, revert_icon, validate_folder};
use folderskin_core::compositor::{self, Artwork, ICON_SIZES};
use folderskin_core::matte;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Bump when the compositor's output changes so cached thumbnails, the built-in ones and the
/// saved skins' alike, are re-rendered.
pub const THUMB_CACHE_VERSION: u32 = 2;
const THUMB_SIZE: u32 = 512;
const DEFAULT_ID: &str = "__default__";
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif",
];

#[derive(Serialize, Clone, Debug)]
pub struct SkinDto {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub thumbnail: String,
    pub custom: bool,
    /// "artwork" (wrapped onto the folder template) or "folder" (a finished folder image).
    pub kind: SkinKind,
    /// "builtin", "import" or "ai".
    pub source: SkinSource,
    /// When a saved skin was added, in Unix milliseconds; `null` for the built-in skins.
    pub created_at: Option<u64>,
    /// What the gallery filters it by.
    pub tags: Vec<String>,
    /// For a community skin, the pack it came from.
    pub pack: Option<String>,
    /// AI results: the provider and model that made it, as people call them.
    pub made_with: Option<String>,
    /// AI results: the description it was made from.
    pub idea: Option<String>,
    /// Community skins: the pack's name, its author's GitHub name and its licence.
    pub pack_name: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
}

impl SkinDto {
    /// A saved skin, as the gallery's "yours" collection shows it.
    pub fn saved(entry: &SavedSkin, thumbnail_png: &[u8]) -> SkinDto {
        SkinDto {
            id: entry.id.clone(),
            name: entry.name.clone(),
            collection: "yours".into(),
            thumbnail: data_url(thumbnail_png),
            custom: true,
            kind: entry.kind,
            source: entry.source,
            created_at: Some(entry.created_at),
            tags: entry.tags.clone(),
            pack: entry.pack.clone(),
            made_with: made_with(entry),
            idea: entry.idea.clone(),
            pack_name: entry.pack_name.clone(),
            author: entry.author.clone(),
            license: entry.license.clone(),
        }
    }
}

/// "OpenAI · GPT Image 2.5 Flare" for an AI result: the names the studio shows, or the ids when
/// the catalogue no longer lists them.
fn made_with(entry: &SavedSkin) -> Option<String> {
    let provider = entry.provider.as_deref()?;
    let model = entry.model.as_deref()?;
    let provider_label = folderskin_ai::catalogue::provider(provider).map_or(provider, |p| p.label);
    let model_label = folderskin_ai::model(provider, model).map_or(model, |m| m.label);
    Some(format!("{provider_label} · {model_label}"))
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

/// True for the shipped skins and the plain default folder, which cannot be deleted.
pub fn is_builtin_id(id: &str) -> bool {
    id == DEFAULT_ID || crate::skins::SKINS.iter().any(|s| s.id == id)
}

/// Decides what an imported picture is: a finished folder image, used as the icon as it is
/// (trimmed, with a magenta backdrop keyed out), or artwork for the folder template.
///
/// See [`matte::surround`] for how the two are told apart.
pub fn prepare_import(rgba: image::RgbaImage) -> Result<SkinImage, String> {
    if matte::alpha_bounds(&rgba, 8).is_none() {
        return Err("that picture is completely transparent".into());
    }
    Ok(match matte::finished_cutout(&rgba, matte::MAGENTA) {
        Some(cut) => SkinImage::Folder(Arc::new(cut)),
        None => SkinImage::Artwork(Arc::new(Artwork {
            rgba,
            focus: (0.5, 0.5),
        })),
    })
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

/// Decodes a picture the user picked, `bytes` being the file as read, downscaled so its longer
/// side is at most [`MAX_STORED_SIDE`]. HEIC is converted by the OS first.
fn decode_picture(path: &Path, bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let converted;
    let bytes = if ext == "heic" || ext == "heif" {
        converted = heic_to_png(path)?;
        &converted[..]
    } else {
        bytes
    };
    let img =
        image::load_from_memory(bytes).map_err(|_| "couldn't read that picture".to_string())?;
    Ok(store::shrink_to(img.to_rgba8(), MAX_STORED_SIDE))
}

#[cfg(target_os = "macos")]
fn heic_to_png(path: &Path) -> Result<Vec<u8>, String> {
    // One file per conversion, so two pictures dropped together do not overwrite each other.
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let out = std::env::temp_dir().join(format!(
        "folderskin-{}-{}.png",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
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

/// The built-in skins first, then the user's saved skins, newest first.
#[tauri::command]
pub async fn list_skins(app: AppHandle, state: State<'_, AppState>) -> Result<SkinListDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut skins: Vec<SkinDto> = state
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
                    kind: SkinKind::Artwork,
                    source: SkinSource::Builtin,
                    created_at: None,
                    tags: s.tags.clone(),
                    pack: None,
                    made_with: None,
                    idea: None,
                    pack_name: None,
                    author: None,
                    license: None,
                }
            })
            .collect();
        skins.extend(
            state
                .saved_skins()
                .iter()
                .map(|(entry, png)| SkinDto::saved(entry, png)),
        );
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

/// Imports a picture and saves it as a skin. A finished folder image (on real transparency or
/// on the magenta key) becomes a `folder` skin used as the icon as it is; anything else becomes
/// `artwork` for the folder template. The same picture imported twice is the same skin.
#[tauri::command]
pub async fn import_image(state: State<'_, AppState>, path: String) -> Result<SkinDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        let bytes = std::fs::read(&p).map_err(|_| "couldn't read that picture".to_string())?;
        let id = store::skin_id(&bytes);
        if let Some((entry, thumb)) = state.find_saved(&id) {
            return Ok(SkinDto::saved(&entry, &thumb));
        }
        let image = prepare_import(decode_picture(&p, &bytes)?)?;
        let new = NewSkin {
            id,
            name: display_name(&p),
            source: SkinSource::Import,
            provider: None,
            model: None,
            idea: None,
            tags: Vec::new(),
            pack: None,
            pack_name: None,
            author: None,
            license: None,
        };
        let (entry, thumb) = state.save(new, image)?;
        Ok(SkinDto::saved(&entry, &thumb))
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
    // A finished folder image is already the icon; artwork goes through the compositor. A saved
    // skin that is not in memory is read back from disk here, off the async threads.
    let icons = tauri::async_runtime::spawn_blocking(move || {
        state
            .resolve(&skin_id)
            .map(|skin| skin.icon_set(&ICON_SIZES))
    })
    .await
    .map_err(|e| e.to_string())??;
    // NSWorkspace.setIcon is thread-safe and the PNG encodes are slow, so this stays off the
    // main thread; the window keeps painting the "Applying…" state.
    tauri::async_runtime::spawn_blocking(move || {
        apply_icon(&folder, &icons).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Where the skins are saved, for Settings to show.
#[tauri::command]
pub fn skins_folder(state: State<'_, AppState>) -> Result<String, String> {
    state
        .store()
        .map(|store| store.dir().display().to_string())
        .ok_or_else(|| "skins are only kept until you quit, since there is no data folder".into())
}

/// Removes a saved skin: its index entry, its files and anything cached for it. The built-in
/// skins cannot be deleted; a saved skin that is already gone is not an error.
#[tauri::command]
pub async fn delete_skin(state: State<'_, AppState>, skin_id: String) -> Result<(), String> {
    if is_builtin_id(&skin_id) {
        return Err("the built-in skins can't be deleted".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete(&skin_id))
        .await
        .map_err(|e| e.to_string())?
}

/// A skin's name and tags as saved.
#[derive(Serialize)]
pub struct SkinEditDto {
    pub name: String,
    pub tags: Vec<String>,
}

/// Gives one of the user's skins a new name and tags and returns them as saved: the name on one
/// line and at most `MAX_NAME_CHARS` long, the tags cleaned and at most eight. The built-in skins
/// keep theirs.
#[tauri::command]
pub async fn edit_skin(
    state: State<'_, AppState>,
    skin_id: String,
    name: String,
    tags: Vec<String>,
) -> Result<SkinEditDto, String> {
    if is_builtin_id(&skin_id) {
        return Err("the built-in skins can't be changed".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.edit(&skin_id, &name, &tags).map(|e| SkinEditDto {
            name: e.name,
            tags: e.tags,
        })
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

    /// A folder-ish block: an opaque rectangle with a tab, `fill` inside, `around` outside.
    fn picture(around: [u8; 4]) -> image::RgbaImage {
        image::RgbaImage::from_fn(120, 100, |x, y| {
            let body = (10..110).contains(&x) && (22..92).contains(&y);
            let tab = (16..50).contains(&x) && (10..22).contains(&y);
            if body || tab {
                image::Rgba([40, 120, 200, 255])
            } else {
                image::Rgba(around)
            }
        })
    }

    #[test]
    fn an_ordinary_picture_imports_as_artwork() {
        let photo = image::RgbaImage::from_fn(120, 90, |x, y| {
            image::Rgba([(x * 2) as u8, 120, (y * 2) as u8, 255])
        });
        match prepare_import(photo.clone()).unwrap() {
            SkinImage::Artwork(art) => {
                assert_eq!(art.rgba, photo, "artwork is kept whole");
                assert_eq!(art.focus, (0.5, 0.5));
            }
            SkinImage::Folder(_) => panic!("a photo must not be used as a finished folder"),
        }
    }

    #[test]
    fn a_finished_folder_with_transparency_imports_trimmed() {
        match prepare_import(picture([0, 0, 0, 0])).unwrap() {
            SkinImage::Folder(img) => assert_eq!(img.dimensions(), (100, 82)),
            SkinImage::Artwork(_) => panic!("a cut-out folder must be used as it is"),
        }
    }

    #[test]
    fn a_finished_folder_on_magenta_imports_keyed_out() {
        match prepare_import(picture([255, 0, 255, 255])).unwrap() {
            SkinImage::Folder(img) => {
                assert_eq!(img.dimensions(), (100, 82));
                let (w, _) = img.dimensions();
                assert_eq!(img.get_pixel(w - 1, 0).0[3], 0, "the magenta is gone");
                assert_eq!(img.get_pixel(50, 50).0[3], 255, "the folder stays");
            }
            SkinImage::Artwork(_) => panic!("a keyed folder must be used as it is"),
        }
    }

    #[test]
    fn a_blank_picture_is_refused() {
        let blank = image::RgbaImage::from_pixel(32, 32, image::Rgba([0, 0, 0, 0]));
        assert!(prepare_import(blank).is_err());
    }

    #[test]
    fn only_shipped_skins_count_as_built_in() {
        assert!(is_builtin_id(DEFAULT_ID));
        let first = crate::skins::SKINS.first().expect("the app ships skins");
        assert!(is_builtin_id(first.id));
        assert!(!is_builtin_id(&store::skin_id(b"mine")));
        assert!(!is_builtin_id(""));
    }

    #[test]
    fn skin_dtos_serialise_the_shape_the_webview_expects() {
        let entry = SavedSkin {
            id: store::skin_id(b"x"),
            name: "Mine".into(),
            kind: SkinKind::Folder,
            source: SkinSource::Ai,
            created_at: 1_790_000_000_000,
            focus: None,
            provider: Some("xai".into()),
            model: Some("grok-imagine-image".into()),
            idea: Some("a fox".into()),
            tags: vec!["woodblock".into()],
            pack: None,
            pack_name: None,
            author: None,
            license: None,
        };
        let json = serde_json::to_value(SkinDto::saved(&entry, b"png")).unwrap();
        assert_eq!(json["collection"], "yours");
        assert_eq!(json["custom"], true);
        assert_eq!(json["kind"], "folder");
        assert_eq!(json["source"], "ai");
        assert_eq!(json["created_at"], 1_790_000_000_000u64);
        assert_eq!(json["tags"], serde_json::json!(["woodblock"]));
        assert!(json["pack"].is_null());
        assert!(json["thumbnail"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert_eq!(json["made_with"], "xAI Grok · Grok Imagine");
        assert_eq!(json["idea"], "a fox");
        assert!(json["author"].is_null());

        let builtin = SkinDto {
            id: "aurora".into(),
            name: "Aurora".into(),
            collection: "glow".into(),
            thumbnail: String::new(),
            custom: false,
            kind: SkinKind::Artwork,
            source: SkinSource::Builtin,
            created_at: None,
            tags: vec!["glow".into()],
            pack: None,
            made_with: None,
            idea: None,
            pack_name: None,
            author: None,
            license: None,
        };
        let json = serde_json::to_value(builtin).unwrap();
        assert_eq!(json["kind"], "artwork");
        assert_eq!(json["source"], "builtin");
        assert!(json["created_at"].is_null());
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
