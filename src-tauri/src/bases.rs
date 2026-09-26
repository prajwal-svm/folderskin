//! Every base a skin can be drawn on, for the webview: FolderSkin's folders in their three looks,
//! every drive shape and none at all ([`folderskin_core::base`]). The composer starts designs on
//! them, and anything that lets someone pick what a picture is painted on reads the same list.
//!
//! An entry is plain data that stays the same from one version to the next: its id, the message
//! that names it, whether it's a folder, a drive or nothing, the system whose look it's drawn in,
//! the drive's kind, and the bare shape drawn at the size asked for.

use crate::commands::data_url;
use folderskin_core::bases::Base;
use folderskin_core::raster;
use serde::Serialize;
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Mutex;

/// One base, as the webview reads it:
///
/// ```json
/// {"id": "drive-mac-external", "label": "common.bases.drive-mac-external", "base": "drive",
///  "style": "mac", "kind": "external", "picture": "data:image/png;base64,…"}
/// ```
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct BaseDto {
    /// `folder-mac`, `drive-linux-solid-state` or `free`. It never changes, so it can be kept.
    pub id: String,
    /// The message that names it, in every language: `common.bases.<id>`.
    pub label: String,
    /// `folder`, `drive` or `free`.
    pub base: &'static str,
    /// `mac`, `windows` or `linux`: whose look it's drawn in. None for `free`.
    pub style: Option<&'static str>,
    /// A drive's kind, as `docs/DRIVES.md` names them: `external`, `solid-state`, `network`.
    pub kind: Option<&'static str>,
    /// The bare shape as a PNG data URL: the folder in its own colour, the drive with nothing on
    /// its face, or for `free` an empty square.
    pub picture: String,
}

/// The sizes the bare shapes can be drawn at, in pixels.
const SIZES: RangeInclusive<u32> = 16..=512;
/// What a list with no size is drawn at: a card's picture on a screen twice as sharp as its size.
const DEFAULT_SIZE: u32 = 128;

/// Each size's list, once drawn: the bases never change while the app runs.
static DRAWN: Mutex<BTreeMap<u32, Vec<BaseDto>>> = Mutex::new(BTreeMap::new());

/// Every base, folders first, then the drives system by system, then `free`, each drawn at
/// `size` px (128 when not given, and 16 to 512).
#[tauri::command]
pub async fn base_shapes(size: Option<u32>) -> Result<Vec<BaseDto>, String> {
    let size = size.unwrap_or(DEFAULT_SIZE);
    if !SIZES.contains(&size) {
        return Err(format!(
            "a base can be drawn at {} to {} px, not {size}",
            SIZES.start(),
            SIZES.end()
        ));
    }
    tauri::async_runtime::spawn_blocking(move || list(size))
        .await
        .map_err(|e| e.to_string())
}

/// The list at `size`, drawn on every core the first time it's asked for.
pub(crate) fn list(size: u32) -> Vec<BaseDto> {
    let drawn = DRAWN
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&size)
        .cloned();
    if let Some(list) = drawn {
        return list;
    }
    let list = crate::state::parallel_map(&Base::all(), |base| BaseDto {
        id: base.id(),
        label: base.label_key(),
        base: base.kind_id(),
        style: base.style_id(),
        kind: base.drive_kind().map(|k| k.id()),
        picture: data_url(&raster::encode_png(&base.render(size))),
    });
    DRAWN
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(size, list.clone());
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_comes_with_its_bare_shape() {
        let list = list(32);
        assert_eq!(list.len(), Base::all().len());
        assert_eq!(list[0].id, "folder-mac");
        assert_eq!(list.last().map(|b| b.base), Some("free"));
        let external = list.iter().find(|b| b.id == "drive-mac-external").unwrap();
        assert_eq!(external.label, "common.bases.drive-mac-external");
        assert_eq!(
            (external.base, external.style, external.kind),
            ("drive", Some("mac"), Some("external"))
        );
        for base in &list {
            assert!(
                base.picture.starts_with("data:image/png;base64,"),
                "{}",
                base.id
            );
        }
        // Asked again, it's the list drawn before.
        assert_eq!(super::list(32), list);
    }

    #[test]
    fn the_list_is_the_shape_the_webview_reads() {
        let json = serde_json::to_value(&list(16)[2]).unwrap();
        assert_eq!(json["id"], "folder-linux");
        assert_eq!(json["base"], "folder");
        assert_eq!(json["style"], "linux");
        assert!(json["kind"].is_null());
    }
}
