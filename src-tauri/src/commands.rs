//! The gallery, import, apply and delete commands the webview calls (the AI assistant's are in
//! `ai.rs`). Heavy work runs off the async runtime's threads; errors are plain sentences the
//! drop zone shows as-is.

use crate::state::AppState;
use crate::store::{self, NewSkin, SavedSkin, SkinImage, SkinKind, SkinSource, MAX_STORED_SIDE};
use base64::Engine;
use folderskin_core::apply::paths::write_atomic;
use folderskin_core::apply::{
    apply_icon, has_custom_icon, refresh_shell_icons, revert_icon, validate_folder,
};
use folderskin_core::compositor::{self, Artwork, ICON_SIZES};
use folderskin_core::matte;
use serde::Serialize;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Bump when the compositor's output changes so cached thumbnails, the default folder's and the
/// saved skins' alike, are re-rendered.
pub const THUMB_CACHE_VERSION: u32 = 2;

/// What a cached thumbnail's name carries: [`THUMB_CACHE_VERSION`], and the folder it's drawn on
/// when that isn't FolderSkin's own, so a Mac-shaped one cached earlier isn't shown on Windows.
pub fn thumb_tag() -> String {
    match compositor::Style::native() {
        compositor::Style::Mac => format!("v{THUMB_CACHE_VERSION}"),
        style => format!("v{THUMB_CACHE_VERSION}-{}", style.id()),
    }
}
const THUMB_SIZE: u32 = 512;
/// The id the webview can give the plain default folder, which is not a skin.
const DEFAULT_ID: &str = "__default__";
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif",
];

/// One skin in the library. Every skin is the user's own: a picture they added, an AI result, or
/// one from a community pack, saved on disk or kept for this session.
#[derive(Serialize, Clone, Debug)]
pub struct SkinDto {
    pub id: String,
    pub name: String,
    /// Always "yours".
    pub collection: String,
    pub thumbnail: String,
    /// Always true.
    pub custom: bool,
    /// "artwork" (wrapped onto the folder template) or "folder" (a finished folder image).
    pub kind: SkinKind,
    /// "import", "ai", "community" or "composer".
    pub source: SkinSource,
    /// When the skin was added, in Unix milliseconds.
    pub created_at: u64,
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
            created_at: entry.created_at,
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

/// The library: the user's skins, newest first, and the plain default folder.
#[derive(Serialize)]
pub struct SkinListDto {
    pub skins: Vec<SkinDto>,
    /// The plain default folder, drawn by the same compositor, as a PNG data URL.
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

/// True for the id of the plain default folder, which is not a skin, so it can't be deleted or
/// changed.
pub fn is_reserved_id(id: &str) -> bool {
    id == DEFAULT_ID
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

/// Where the default folder's thumbnail is cached: the app cache directory, so a later launch
/// skips the render. `None` when there is no cache directory to use.
fn default_thumb_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?.join("thumbs");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("default.thumb-{}.png", thumb_tag())))
}

/// The plain default folder's thumbnail as a data URL: kept in memory once drawn, and on disk
/// between launches.
fn default_thumbnail(app: &AppHandle, state: &AppState) -> String {
    state.default_thumbnail(|| {
        let png = cached_png(default_thumb_path(app).as_deref(), || {
            let style = compositor::Style::native();
            compositor::render_preview_png_in(
                &compositor::default_folder_artwork_in(style),
                THUMB_SIZE,
                style,
            )
        });
        data_url(&png)
    })
}

/// The PNG cached at `path`, or `render()`'s when there is none (or it is damaged), which is then
/// cached there. With no `path` it only renders.
fn cached_png(path: Option<&Path>, render: impl FnOnce() -> Vec<u8>) -> Vec<u8> {
    if let Some(bytes) = path.and_then(|p| std::fs::read(p).ok()) {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return bytes;
        }
    }
    let png = render();
    if let Some(path) = path {
        if let Err(e) = write_atomic(path, &png) {
            eprintln!("folderskin: couldn't cache {}: {e}", path.display());
        }
    }
    png
}

/// Decodes a picture the user picked, `bytes` being the file as read, downscaled so its longer
/// side is at most [`MAX_STORED_SIDE`]. HEIC is converted by the OS first.
pub(crate) fn decode_picture(path: &Path, bytes: &[u8]) -> Result<image::RgbaImage, String> {
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

/// The user's skins, newest first, and the plain default folder.
#[tauri::command]
pub async fn list_skins(app: AppHandle, state: State<'_, AppState>) -> Result<SkinListDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || SkinListDto {
        skins: state
            .saved_skins()
            .iter()
            .map(|(entry, png)| SkinDto::saved(entry, png))
            .collect(),
        default_thumbnail: default_thumbnail(&app, &state),
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
            pack_hash: None,
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
    // NSWorkspace.setIcon works off the main thread (one call at a time; the core takes a lock
    // for that) and the PNG encodes are slow, so this stays off it; the window keeps painting
    // the "Applying…" state.
    tauri::async_runtime::spawn_blocking(move || {
        apply_icon(&folder, &icons).map_err(|e| e.to_string())?;
        // Once the folder is written: on Windows the Desktop repaints for nothing narrower.
        refresh_shell_icons();
        Ok(())
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

/// Removes a saved skin: its index entry, its files and anything cached for it. A saved skin
/// that is already gone is not an error.
#[tauri::command]
pub async fn delete_skin(state: State<'_, AppState>, skin_id: String) -> Result<(), String> {
    if is_reserved_id(&skin_id) {
        return Err("the plain folder isn't a skin, so it can't be deleted".into());
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
/// line and at most `MAX_NAME_CHARS` long, the tags cleaned and at most eight.
#[tauri::command]
pub async fn edit_skin(
    state: State<'_, AppState>,
    skin_id: String,
    name: String,
    tags: Vec<String>,
) -> Result<SkinEditDto, String> {
    if is_reserved_id(&skin_id) {
        return Err("the plain folder isn't a skin, so it can't be changed".into());
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
    tauri::async_runtime::spawn_blocking(move || {
        revert_icon(&folder).map_err(|e| e.to_string())?;
        refresh_shell_icons();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A folder's icon as it looks now, and whether it's one of its own that a revert would take off.
#[derive(Serialize)]
pub struct FolderIconDto {
    /// A data URL: the real OS icon on macOS, the plain rendered folder elsewhere (or when the
    /// OS cannot provide one).
    pub url: String,
    pub custom: bool,
}

/// The folder's current icon, and whether it's a custom one FolderSkin can remove.
#[tauri::command]
pub async fn folder_icon(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: String,
) -> Result<FolderIconDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&folder);
        let url = match crate::folder_icon::current_icon_png(&p, THUMB_SIZE) {
            Some(png) => data_url(&png),
            None => default_thumbnail(&app, &state),
        };
        FolderIconDto {
            url,
            custom: has_custom_icon(&p),
        }
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
            "Writes desktop.ini and a folderskin icon file inside the folder, both hidden. Revert removes them.",
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
    fn only_the_default_folder_is_reserved() {
        assert!(is_reserved_id(DEFAULT_ID));
        assert!(!is_reserved_id(&store::skin_id(b"mine")));
        assert!(
            !is_reserved_id("aurora"),
            "no skin ships with the app any more"
        );
        assert!(!is_reserved_id(""));
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
            pack_hash: None,
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
    }

    #[test]
    fn every_source_is_named_the_way_the_webview_names_it() {
        for (source, name) in [
            (SkinSource::Import, "import"),
            (SkinSource::Ai, "ai"),
            (SkinSource::Community, "community"),
            (SkinSource::Composer, "composer"),
        ] {
            assert_eq!(serde_json::to_value(source).unwrap(), name);
            let read: SkinSource = serde_json::from_value(name.into()).unwrap();
            assert_eq!(read, source);
        }
    }

    #[test]
    fn a_cached_thumbnail_is_read_back_and_a_damaged_one_drawn_again() {
        let dir = std::env::temp_dir().join(format!("folderskin-thumb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("default.thumb-v2.png");
        let png = b"\x89PNG\r\n\x1a\nfirst".to_vec();

        assert_eq!(cached_png(Some(&path), || png.clone()), png);
        assert_eq!(std::fs::read(&path).unwrap(), png, "and cached");
        let again = cached_png(Some(&path), || panic!("drawn a second time"));
        assert_eq!(again, png, "read back instead");

        std::fs::write(&path, b"not a png").unwrap();
        let redrawn = b"\x89PNG\r\n\x1a\nsecond".to_vec();
        assert_eq!(cached_png(Some(&path), || redrawn.clone()), redrawn);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            redrawn,
            "a damaged file is replaced"
        );

        assert_eq!(cached_png(None, || png.clone()), png, "no cache folder");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
