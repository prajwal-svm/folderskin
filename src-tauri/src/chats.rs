//! The AI assistant's conversations, kept between runs.
//!
//! ```text
//! <app data>/chats/index.json       [{id, title, created, updated, turns, cover}], newest first
//! <app data>/chats/<id>.json        one chat, as the webview wrote it
//! <app data>/chats/<id>/<hash>.png  the reference pictures it was given, copied and shrunk
//! ```
//!
//! What a chat holds is the webview's to decide (src/state/chats.ts); this side checks that it is
//! a chat of a sane size under a name that can only be a file in this folder, keeps the index the
//! history list reads, and copies reference pictures in so a chat outlives the files it was shown.
//! A chat is written before the index that lists it, each atomically, so a crash leaves either the
//! old chat or the new one; an index that's missing or can't be read is rebuilt from the chats.

use crate::commands::data_url;
use folderskin_core::apply::paths::write_atomic;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

const INDEX: &str = "index.json";
/// The most a chat file may be: text and small thumbnails only, never a whole picture.
const MAX_CHAT_BYTES: usize = 4 * 1024 * 1024;
const MAX_TURNS: usize = 2000;
const MAX_TITLE_CHARS: usize = 200;
/// A reference picture is read up to this size, and kept at most this many pixels on its long side.
const MAX_REFERENCE_BYTES: u64 = 64 * 1024 * 1024;
const REFERENCE_SIDE: u32 = 1536;
const THUMB_SIDE: u32 = 160;

/// Where the chats live; `None` until setup opens it (or when there's no app data folder).
#[derive(Default)]
pub struct Chats(Mutex<Option<PathBuf>>);

impl Chats {
    pub fn open(&self, dir: PathBuf) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir);
    }

    fn dir(&self) -> Result<PathBuf, String> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| {
                "chats can't be kept on this computer: FolderSkin has no data folder here".into()
            })
    }
}

/// A chat as the history list shows it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub created: u64,
    pub updated: u64,
    pub turns: u32,
    /// The skin its latest picture became, for the list's thumbnail.
    pub cover: Option<String>,
}

/// A reference picture kept for a chat.
#[derive(Serialize, Debug)]
pub struct ChatRefDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub thumb: String,
}

/// `c` and up to 39 lower-case letters and digits (chatId in chats.ts): a name that can only be a
/// file in the chats folder.
pub fn is_chat_id(id: &str) -> bool {
    (2..=40).contains(&id.len())
        && id.starts_with('c')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn check_id(id: &str) -> Result<(), String> {
    if is_chat_id(id) {
        Ok(())
    } else {
        Err(format!("{id:?} isn't a chat FolderSkin made"))
    }
}

/// What the history list needs from a chat, read from the chat itself.
pub fn summarise(chat: &Value) -> Result<ChatSummary, String> {
    let id = chat
        .get("id")
        .and_then(Value::as_str)
        .ok_or("a chat without an id")?;
    check_id(id)?;
    let turns = chat
        .get("turns")
        .and_then(Value::as_array)
        .ok_or("a chat without its turns")?;
    if turns.len() > MAX_TURNS {
        return Err(format!(
            "this chat has more than {MAX_TURNS} requests; start a new one"
        ));
    }
    let title: String = chat
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("New chat")
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    let when = |key: &str| chat.get(key).and_then(Value::as_u64).unwrap_or(0);
    let cover = turns
        .iter()
        .rev()
        .find_map(|t| t.get("skinId").and_then(Value::as_str))
        .map(str::to_string);
    Ok(ChatSummary {
        id: id.to_string(),
        title,
        created: when("created"),
        updated: when("updated"),
        turns: turns.len() as u32,
        cover,
    })
}

fn read_index(dir: &Path) -> Option<Vec<ChatSummary>> {
    serde_json::from_slice(&std::fs::read(dir.join(INDEX)).ok()?).ok()
}

/// The index, rebuilt from the chats themselves when it's missing or can't be read.
fn index(dir: &Path) -> Vec<ChatSummary> {
    if let Some(list) = read_index(dir) {
        return list;
    }
    let mut list: Vec<ChatSummary> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            if !is_chat_id(id) {
                return None;
            }
            let chat: Value = serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok()?;
            summarise(&chat).ok().filter(|s| s.id == id)
        })
        .collect();
    list.sort_by_key(|s| std::cmp::Reverse(s.updated));
    list
}

fn write_index(dir: &Path, list: &[ChatSummary]) -> Result<(), String> {
    let bytes = serde_json::to_vec(list).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(INDEX), &bytes)
        .map_err(|e| format!("couldn't update the list of chats: {e}"))
}

/// Saves `chat`, returning how the history list now shows it.
pub fn save(dir: &Path, chat: &Value) -> Result<ChatSummary, String> {
    let summary = summarise(chat)?;
    let bytes = serde_json::to_vec(chat).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_CHAT_BYTES {
        return Err("this chat has grown too big to keep; start a new one".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't make the chats folder: {e}"))?;
    write_atomic(&dir.join(format!("{}.json", summary.id)), &bytes)
        .map_err(|e| format!("couldn't save the chat: {e}"))?;
    let mut list: Vec<ChatSummary> = index(dir)
        .into_iter()
        .filter(|s| s.id != summary.id)
        .collect();
    list.push(summary.clone());
    list.sort_by_key(|s| std::cmp::Reverse(s.updated));
    write_index(dir, &list)?;
    Ok(summary)
}

pub fn read(dir: &Path, id: &str) -> Result<Value, String> {
    check_id(id)?;
    let bytes = std::fs::read(dir.join(format!("{id}.json"))).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "that chat isn't on this computer any more".to_string()
        } else {
            format!("couldn't open the chat: {e}")
        }
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "that chat's file is damaged and can't be opened".into())
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    check_id(id)?;
    match std::fs::remove_file(dir.join(format!("{id}.json"))) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("couldn't delete the chat: {e}")),
    }
    let _ = std::fs::remove_dir_all(dir.join(id));
    let list: Vec<ChatSummary> = index(dir).into_iter().filter(|s| s.id != id).collect();
    write_index(dir, &list)
}

/// Copies the picture at `source` into chat `id`'s folder, shrunk to a size a model takes, with a
/// small thumbnail for the chat to show. The same picture twice is the same copy.
pub fn keep_reference(dir: &Path, id: &str, source: &Path) -> Result<ChatRefDto, String> {
    use sha2::{Digest, Sha256};
    check_id(id)?;
    let meta = std::fs::metadata(source).map_err(|_| "couldn't find that picture".to_string())?;
    if !meta.is_file() {
        return Err("that isn't a picture".into());
    }
    if meta.len() > MAX_REFERENCE_BYTES {
        return Err("that picture is too big to use as a reference (over 64 MB)".into());
    }
    let bytes = std::fs::read(source).map_err(|e| format!("couldn't read that picture: {e}"))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let img = image::load_from_memory(&bytes)
        .map_err(|_| {
            "that file isn't a picture FolderSkin can read (try a PNG, JPEG or WebP)".to_string()
        })?
        .to_rgba8();
    let folder = dir.join(id);
    std::fs::create_dir_all(&folder).map_err(|e| format!("couldn't keep the picture: {e}"))?;
    let path = folder.join(format!("{}.png", &hash[..12]));
    if !path.is_file() {
        let png = folderskin_core::raster::encode_png(&crate::store::shrink_to(
            img.clone(),
            REFERENCE_SIDE,
        ));
        write_atomic(&path, &png).map_err(|e| format!("couldn't keep the picture: {e}"))?;
    }
    let thumb = folderskin_core::raster::encode_png(&crate::store::shrink_to(img, THUMB_SIDE));
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "picture".into());
    Ok(ChatRefDto {
        id: hash[..12].to_string(),
        name,
        path: path.to_string_lossy().into_owned(),
        thumb: data_url(&thumb),
    })
}

// ---------- commands ----------

#[tauri::command]
pub fn chats_list(chats: State<'_, Chats>) -> Result<Vec<ChatSummary>, String> {
    Ok(index(&chats.dir()?))
}

#[tauri::command]
pub fn chat_read(chats: State<'_, Chats>, id: String) -> Result<Value, String> {
    read(&chats.dir()?, &id)
}

#[tauri::command]
pub fn chat_save(chats: State<'_, Chats>, chat: Value) -> Result<ChatSummary, String> {
    let dir = chats.dir()?;
    // Chats are saved as they change; one at a time, so two saves never race over the index.
    static WRITING: Mutex<()> = Mutex::new(());
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    save(&dir, &chat)
}

#[tauri::command]
pub fn chat_delete(chats: State<'_, Chats>, id: String) -> Result<(), String> {
    delete(&chats.dir()?, &id)
}

#[tauri::command]
pub async fn chat_keep_reference(
    chats: State<'_, Chats>,
    id: String,
    path: String,
) -> Result<ChatRefDto, String> {
    let dir = chats.dir()?;
    tauri::async_runtime::spawn_blocking(move || keep_reference(&dir, &id, Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "folderskin-chats-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn chat(id: &str, updated: u64, skin: Option<&str>) -> Value {
        json!({
            "version": 1, "id": id, "title": "A lighthouse", "named": false,
            "created": 1, "updated": updated, "folder": null,
            "turns": [{"id": "t1", "idea": "a lighthouse", "skinId": skin}]
        })
    }

    #[test]
    fn only_its_own_names_are_chats() {
        assert!(is_chat_id("cmfd3k2x0a1b2c"));
        for bad in [
            "",
            "c",
            "C1",
            "x123",
            "c../x",
            "c1/2",
            "c1.json",
            "c\u{e9}",
            &"c".repeat(41),
        ] {
            assert!(!is_chat_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn saves_lists_reads_and_deletes() {
        let dir = temp();
        save(&dir, &chat("ca1", 5, None)).unwrap();
        let s = save(&dir, &chat("cb2", 9, Some("user:abc"))).unwrap();
        assert_eq!(s.cover.as_deref(), Some("user:abc"));
        assert_eq!(s.turns, 1);
        assert_eq!(
            index(&dir)
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["cb2", "ca1"]
        );
        // Saving again moves it to the top, once.
        save(&dir, &chat("ca1", 12, None)).unwrap();
        assert_eq!(
            index(&dir)
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["ca1", "cb2"]
        );
        assert_eq!(
            read(&dir, "cb2").unwrap()["turns"][0]["idea"],
            "a lighthouse"
        );
        delete(&dir, "cb2").unwrap();
        assert_eq!(index(&dir).len(), 1);
        assert!(read(&dir, "cb2")
            .unwrap_err()
            .contains("isn't on this computer"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn refuses_what_isnt_a_chat() {
        let dir = temp();
        assert!(save(&dir, &json!({"id": "../evil", "turns": []})).is_err());
        assert!(save(&dir, &json!({"id": "ca1"})).is_err());
        let huge = json!({"id": "ca1", "turns": [], "title": "x".repeat(MAX_CHAT_BYTES)});
        assert!(save(&dir, &huge).unwrap_err().contains("too big"));
        assert!(read(&dir, "../index").is_err());
        assert!(delete(&dir, "..").is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_lost_index_is_rebuilt_from_the_chats() {
        let dir = temp();
        save(&dir, &chat("ca1", 5, None)).unwrap();
        save(&dir, &chat("cb2", 9, None)).unwrap();
        std::fs::write(dir.join(INDEX), b"{not json").unwrap();
        assert_eq!(
            index(&dir)
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["cb2", "ca1"]
        );
        std::fs::remove_file(dir.join(INDEX)).unwrap();
        assert_eq!(index(&dir).len(), 2);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn keeps_a_shrunk_copy_of_a_reference_with_a_thumbnail() {
        let dir = temp();
        let src = dir.join("photo.png");
        let img = image::RgbaImage::from_pixel(3000, 1500, image::Rgba([200, 30, 40, 255]));
        std::fs::write(&src, folderskin_core::raster::encode_png(&img)).unwrap();
        let kept = keep_reference(&dir, "ca1", &src).unwrap();
        assert_eq!(kept.name, "photo.png");
        assert!(kept.thumb.starts_with("data:image/png;base64,"));
        let copy = image::open(&kept.path).unwrap();
        assert_eq!(
            (copy.width(), copy.height()),
            (REFERENCE_SIDE, REFERENCE_SIDE / 2)
        );
        // The same picture again is the same copy.
        assert_eq!(keep_reference(&dir, "ca1", &src).unwrap().path, kept.path);
        // Deleting the chat takes its pictures too.
        save(&dir, &chat("ca1", 1, None)).unwrap();
        delete(&dir, "ca1").unwrap();
        assert!(!Path::new(&kept.path).exists());
        assert!(keep_reference(&dir, "ca1", &dir.join("missing.png")).is_err());
        std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
        assert!(keep_reference(&dir, "ca1", &dir.join("notes.txt"))
            .unwrap_err()
            .contains("isn't a picture"));
        std::fs::remove_dir_all(dir).ok();
    }
}
