//! Community skin packs: the list on GitHub, each pack's preview, adding and removing a pack,
//! adding one from a folder on this computer, and saving your own skins as a pack to share.
//!
//! Packs live in the FolderSkin repository under `community/`, so reading them needs no account
//! and no server: `index.json` lists them, `previews/<id>.png` shows a few skins of each, and
//! `packs/<id>/` is the pack itself. The rules every pack follows are in
//! `folderskin_core::pack`, and docs/PACKS.md says the same in prose.

use crate::commands::{data_url, prepare_import, SkinDto};
use crate::state::AppState;
use crate::store::{self, NewSkin, SkinImage, SkinSource};
use folderskin_core::pack::{self, Index, Pack, PackSkin};
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::State;

/// The `community/` folder of the repository on GitHub. Set `FOLDERSKIN_COMMUNITY_URL` to read
/// another copy of it instead, such as a checkout served locally while testing.
const COMMUNITY_URL: &str =
    "https://raw.githubusercontent.com/prajwal-svm/folderskin/main/community";
/// The largest `index.json` and preview the app downloads.
const MAX_INDEX_BYTES: usize = 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 1024 * 1024;
/// Shown when GitHub cannot be reached at all.
const OFFLINE: &str = "couldn't reach GitHub. Check your connection and try again";

/// One pack in the Community list.
#[derive(Serialize)]
pub struct PackDto {
    pub id: String,
    pub name: String,
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub count: usize,
    /// True when the pack's skins are in the library.
    pub added: bool,
}

/// The packs on GitHub, each marked with whether it has been added.
#[tauri::command]
pub async fn community_packs(state: State<'_, AppState>) -> Result<Vec<PackDto>, String> {
    let bytes = fetch(&format!("{}/index.json", base_url()), MAX_INDEX_BYTES)
        .await
        .map_err(|e| {
            if e == NOT_FOUND {
                "the community packs aren't published yet".to_string()
            } else {
                e
            }
        })?;
    let index = Index::parse(&bytes)?;
    let added = state.added_packs();
    Ok(index
        .packs
        .into_iter()
        .filter(|p| pack::is_pack_id(&p.id))
        .map(|p| PackDto {
            added: added.contains(&p.id),
            id: p.id,
            name: p.name,
            author: p.author,
            license: p.license,
            tags: pack::clean_tags(&p.tags, usize::MAX),
            count: p.count,
        })
        .collect())
}

/// A pack's preview (a few of its skins as folders, side by side) as a PNG data URL. Each is
/// downloaded once per session.
#[tauri::command]
pub async fn community_preview(pack_id: String) -> Result<String, String> {
    static PREVIEWS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let previews = PREVIEWS.get_or_init(Default::default);
    if !pack::is_pack_id(&pack_id) {
        return Err("that isn't a pack".into());
    }
    if let Some(url) = lock(previews).get(&pack_id) {
        return Ok(url.clone());
    }
    let png = fetch(
        &format!("{}/previews/{pack_id}.png", base_url()),
        MAX_PREVIEW_BYTES,
    )
    .await?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("that preview isn't a PNG".into());
    }
    let url = data_url(&png);
    lock(previews).insert(pack_id, url.clone());
    Ok(url)
}

/// Downloads a pack and saves its skins. Every picture is downloaded and checked before any is
/// saved, so a dropped connection or a bad file never leaves half a pack behind.
#[tauri::command]
pub async fn community_add(
    state: State<'_, AppState>,
    pack_id: String,
) -> Result<Vec<SkinDto>, String> {
    if !pack::is_pack_id(&pack_id) {
        return Err("that isn't a pack".into());
    }
    let base = format!("{}/packs/{pack_id}", base_url());
    let manifest = fetch(
        &format!("{base}/{}", pack::MANIFEST_FILE),
        pack::MAX_MANIFEST_BYTES,
    )
    .await?;
    let pack = Pack::parse(&manifest).map_err(|problems| problems.join("; "))?;
    let mut pictures = Vec::with_capacity(pack.skins.len());
    for skin in &pack.skins {
        // `file` passed `is_picture_file_name`, so it cannot leave the pack's folder.
        let bytes = fetch(&format!("{base}/{}", skin.file), pack::MAX_PICTURE_BYTES)
            .await
            .map_err(|e| format!("{}: {e}", skin.file))?;
        pictures.push(bytes);
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || save_pack(&state, &pack_id, &pack, &pictures))
        .await
        .map_err(|e| e.to_string())?
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
        save_pack(&state, &id, &pack, &pictures)
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
            if !matches!(entry.source, SkinSource::Import | SkinSource::Ai) {
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

/// Checks and decodes every picture of a pack, and only then saves each as a community skin
/// carrying the pack's tags and id.
fn save_pack(
    state: &AppState,
    pack_id: &str,
    pack: &Pack,
    pictures: &[Vec<u8>],
) -> Result<Vec<SkinDto>, String> {
    let mut ready = Vec::with_capacity(pictures.len());
    for (skin, bytes) in pack.skins.iter().zip(pictures) {
        let rgba = pack::decode_picture(bytes).map_err(|e| format!("{} {e}", skin.file))?;
        let image = prepare_import(rgba).map_err(|e| format!("{}: {e}", skin.file))?;
        ready.push((skin, store::skin_id(bytes), image));
    }
    let mut saved = Vec::with_capacity(ready.len());
    for (skin, id, image) in ready {
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
        };
        let (entry, thumb) = state.save(new, image)?;
        saved.push(SkinDto::saved(&entry, &thumb));
    }
    Ok(saved)
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
        let saved = save_pack(&state, "test-colours", &pack, &pictures).unwrap();
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
        assert!(state.added_packs().contains("test-colours"));

        let removed = state.remove_pack("test-colours").unwrap();
        assert_eq!(removed.len(), 2);
        assert!(state.added_packs().is_empty());
    }

    #[test]
    fn a_bad_picture_stops_the_whole_pack_before_anything_is_saved() {
        let pack = Pack::parse(PACK.as_bytes()).unwrap();
        let state = AppState::default();
        state.open_store(temp_dir("bad-store"));
        let pictures = vec![png(512, 480, [1, 2, 3, 255]), png(100, 100, [1, 2, 3, 255])];
        let err = save_pack(&state, "test-colours", &pack, &pictures).unwrap_err();
        assert!(err.contains("rust.png"), "{err}");
        assert!(state.saved_skins().is_empty(), "nothing was saved");
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
}
