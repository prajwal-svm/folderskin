//! Every shape a skin is made for, for the webview ([`folderskin_core::base`]): the folders in
//! their three looks, every drive shape each system shows, and a free icon. The AI view paints on
//! them, the composer starts designs on them, and a drive picked on the stage is one of them, so
//! all three read this one list.

use crate::commands::data_url;
use folderskin_core::base::{Base, BASES};
use folderskin_core::raster;
use serde::Serialize;
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Mutex;

/// One shape, as the webview reads it (src/lib/shapes.ts `ShapeInfo`):
///
/// ```json
/// {"id": "mac-external", "label": "Mac external drive", "system": "mac", "family": "drive",
///  "whole": true, "kind": "external", "thumbnail": "data:image/png;base64,…"}
/// ```
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ShapeDto {
    /// `mac-folder`, `linux-removable` or `free`. It never changes, so it can be kept.
    pub id: &'static str,
    /// Its name in English; the window names it in the language on show, by `id`.
    pub label: &'static str,
    /// "mac", "windows" or "linux", or "any" for a free icon.
    pub system: &'static str,
    /// "folder", "drive" or "free": where its skins go in the library.
    pub family: &'static str,
    /// Whether it can be painted whole, from its own template, as well as as artwork.
    pub whole: bool,
    /// A drive's kind, as docs/DRIVES.md names them: `external`, `solid-state`, `network`.
    pub kind: Option<&'static str>,
    /// The shape bare, as its system draws it, as a PNG data URL; none for a free icon.
    pub thumbnail: Option<String>,
}

/// The sizes the bare shapes can be drawn at, in pixels.
const SIZES: RangeInclusive<u32> = 16..=512;
/// What a list with no size is drawn at: the AI picker's picture, sharp at twice the size it's
/// shown at.
const DEFAULT_SIZE: u32 = 96;

/// Each size's list, once drawn: the shapes never change while the app runs.
static DRAWN: Mutex<BTreeMap<u32, Vec<ShapeDto>>> = Mutex::new(BTreeMap::new());

/// Every shape, in the order a picker lists them ([`BASES`]), each drawn at `size` px (96 when
/// not given, and 16 to 512).
#[tauri::command]
pub async fn shapes(size: Option<u32>) -> Result<Vec<ShapeDto>, String> {
    let size = size.unwrap_or(DEFAULT_SIZE);
    if !SIZES.contains(&size) {
        return Err(format!(
            "a shape can be drawn at {} to {} px, not {size}",
            SIZES.start(),
            SIZES.end()
        ));
    }
    tauri::async_runtime::spawn_blocking(move || list(size))
        .await
        .map_err(|e| e.to_string())
}

/// The list at `size`, drawn on every core the first time it's asked for.
pub(crate) fn list(size: u32) -> Vec<ShapeDto> {
    let drawn = DRAWN
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&size)
        .cloned();
    if let Some(list) = drawn {
        return list;
    }
    let bases: Vec<&'static Base> = BASES.iter().collect();
    let list = crate::state::parallel_map(&bases, |b| ShapeDto {
        id: b.id,
        label: b.label,
        system: b.system.id(),
        family: b.family.id(),
        whole: !b.is_free(),
        kind: b.drive().map(|shape| shape.kind().id()),
        thumbnail: b.bare(size).map(|img| data_url(&raster::encode_png(&img))),
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
    fn every_shape_is_listed_with_its_picture_and_where_it_goes() {
        let shapes = list(32);
        let ids: Vec<&str> = shapes.iter().map(|s| s.id).collect();
        assert_eq!(ids[..3], ["mac-folder", "windows-folder", "linux-folder"]);
        assert_eq!(ids.last(), Some(&"free"));
        assert_eq!(ids.len(), BASES.len());
        for s in &shapes {
            assert_eq!(s.whole, s.family != "free", "{}", s.id);
            assert_eq!(s.thumbnail.is_some(), s.family != "free", "{}", s.id);
            assert_eq!(s.kind.is_some(), s.family == "drive", "{}", s.id);
        }
        let mac = &shapes[0];
        assert_eq!(
            (mac.label, mac.system, mac.family),
            ("Mac folder", "mac", "folder")
        );
        assert!(mac
            .thumbnail
            .as_deref()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let stick = shapes.iter().find(|s| s.id == "linux-removable").unwrap();
        assert_eq!(
            (stick.system, stick.family, stick.kind),
            ("linux", "drive", Some("removable"))
        );
        let free = shapes.last().unwrap();
        assert_eq!((free.system, free.family), ("any", "free"));
        let json = serde_json::to_value(free).unwrap();
        assert!(
            json["thumbnail"].is_null() && json["kind"].is_null(),
            "{json}"
        );
        // Drawn once: the second list is the same pictures.
        assert_eq!(list(32), shapes);
    }
}
