//! The composer's commands. The user designs a skin on a canvas in the webview; everything around
//! their design comes from here: the folder template split into the layers the webview stacks the
//! design between for its live preview, the icon the design makes at any size, and the saved skin.
//! The pictures the user places in a design, and a saved design to edit again, are read here too.
//!
//! A design is a square picture in icon space: its pixels map one to one onto the 1024-unit canvas
//! the folder template is drawn on (`folderskin_core::geometry`). The webview never draws folder
//! geometry itself. The layers and every icon come from `folderskin_core::compositor`, so the
//! preview and the saved icon are the same pixels.
//!
//! `composer_save` and `composer_preview` take the design as raw bytes rather than JSON, framed as
//! `[u32 little-endian length of a JSON header][the header, UTF-8][PNG bytes]` ([`unframe`]).

use crate::commands::{self, SkinDto};
use crate::state::AppState;
use crate::store::{self, NewSkin, SkinImage, SkinSource};
use base64::Engine;
use folderskin_core::geometry as g;
use folderskin_core::{compositor, matte, raster};
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, ImageFormat, RgbaImage};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::ipc::{InvokeBody, Request};
use tauri::State;

/// Edge of the template layers, in pixels: the size the icon is rendered at, so the preview is as
/// sharp as the saved icon.
const LAYER_SIZE: u32 = compositor::RENDER_SIZE;
/// How big a design's picture can be, in pixels on a side.
const DESIGN_SIDES: RangeInclusive<u32> = 64..=4096;
/// Longest side a preview works from. Previews are 512 px at most, so more would only be slower.
const PREVIEW_DESIGN_SIDE: u32 = 1024;
/// The sizes a preview can be, in pixels, and how many one call can ask for.
const PREVIEW_SIZES: RangeInclusive<u32> = 16..=512;
const MAX_PREVIEWS: usize = 6;
/// What a design is called when it was given no name.
const UNNAMED: &str = "My design";
/// JPEG quality of an opaque picture handed to the composer.
const JPEG_QUALITY: u8 = 90;

const CUT_SHORT: &str = "the design didn't arrive whole";
const UNREADABLE: &str = "couldn't read the design's details";
const NO_PICTURE: &str = "the design came without its picture";

/// How a design becomes an icon.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Shape {
    /// On FolderSkin's folder: both panels filled with the design, the paper and rims over it.
    Folder,
    /// The design as it is: the whole icon.
    Free,
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Shape::Folder => "folder",
            Shape::Free => "free",
        }
    }
}

/// What `composer_save` is told about a design, ahead of its picture.
#[derive(Deserialize)]
struct SaveHeader {
    #[serde(default)]
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    shape: Shape,
    /// The design's document, kept as it came so the composer can open it again.
    #[serde(default)]
    design: serde_json::Value,
    /// The id of the saved design this one was made from, to save over it.
    #[serde(default)]
    replaces: Option<String>,
}

/// What `composer_preview` is asked for, ahead of the design's picture.
#[derive(Deserialize)]
struct PreviewHeader {
    shape: Shape,
    sizes: Vec<u32>,
}

/// The folder template as the composer stacks it around a design. The layers are PNG data URLs,
/// `size` px square, covering the whole canvas the design covers.
#[derive(Serialize, Clone)]
pub struct ComposerTemplateDto {
    pub size: u32,
    /// The back panel, tab included, and the front panel: white, each alpha the panel's
    /// coverage, to mask the design with.
    pub back: String,
    pub front: String,
    /// Goes between the design masked by `back` and the design masked by `front`: the back
    /// panel's rim light, the paper sheet and its highlight.
    pub middle: String,
    /// Goes over everything: the front panel's rim light and the shade along its bottom.
    pub top: String,
    /// The folder's visible edges as a white line, to show while the design is made. It is not
    /// part of the icon.
    pub outline: String,
    pub parts: PartsDto,
}

/// Where the template's parts are, in canvas units. The canvas is `canvas` units square and the
/// layers and the design cover it exactly. Boxes are `[x0, y0, x1, y1]`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PartsDto {
    pub canvas: f32,
    /// The whole folder: the front panel's sides and bottom, and the tab's top.
    pub folder: [f32; 4],
    /// The front panel, a rectangle with corners of `front_radius`.
    pub front: [f32; 4],
    pub front_radius: f32,
    /// The back panel, tab included.
    pub back: [f32; 4],
    /// The paper sheet between the panels.
    pub paper: [f32; 4],
    /// The tab: its left edge, its top, where its slanted right edge meets the top of the back
    /// panel's body, and that top.
    pub tab: [f32; 4],
}

impl PartsDto {
    fn new() -> PartsDto {
        let corners = |r: g::Rect| [r.x0, r.y0, r.x1, r.y1];
        PartsDto {
            canvas: g::CANVAS,
            folder: [g::FRONT.x0, g::TAB_TOP, g::FRONT.x1, g::FRONT.y1],
            front: corners(g::FRONT),
            front_radius: g::FRONT_RADIUS,
            back: corners(g::BACK_BBOX),
            paper: corners(g::PAPER),
            tab: [
                g::TAB_LEFT,
                g::TAB_TOP,
                g::TAB_TOP_RIGHT_X + g::TAB_SLANT * (g::BACK_BODY.y0 - g::TAB_TOP),
                g::BACK_BODY.y0,
            ],
        }
    }
}

/// A saved design: the skin it became and, when it was saved over a design, that design's id.
/// The library then has `skin` in that design's place, whether `skin` is new, the same design
/// renamed, or another saved design it has become the same as.
#[derive(Serialize)]
pub struct ComposerSavedDto {
    pub skin: SkinDto,
    pub replaced: Option<String>,
}

/// A picture to place in a design, at most 2048 px on its longer side.
#[derive(Serialize)]
pub struct ComposerImageDto {
    /// A PNG data URL when the picture has any transparency, a JPEG one when it has none.
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub name: String,
    /// Whether any pixel is less than opaque.
    pub alpha: bool,
}

// ---------- commands ----------

/// The template, drawn the first time the composer asks for it and kept until the app quits.
static TEMPLATE: OnceLock<ComposerTemplateDto> = OnceLock::new();

/// The folder template's layers at 2048 px, and where its parts are.
#[tauri::command]
pub async fn composer_template() -> Result<ComposerTemplateDto, String> {
    tauri::async_runtime::spawn_blocking(|| TEMPLATE.get_or_init(draw_template).clone())
        .await
        .map_err(|e| e.to_string())
}

/// Saves a design as a skin, or over the design it was made from, and returns the skin.
///
/// The body is raw bytes: `[u32 LE header length][header JSON][PNG]`, the header
/// `{name, tags, shape: "folder" | "free", design, replaces}`. The PNG is the design, square and
/// 64 to 4096 px across.
#[tauri::command]
pub async fn composer_save(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<ComposerSavedDto, String> {
    let body = raw_body(&request)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || save(&state, &body))
        .await
        .map_err(|e| e.to_string())?
}

/// The icon a design makes, as PNG data URLs, one for each size asked for, in that order.
///
/// The body is framed as `composer_save`'s, the header `{shape: "folder" | "free", sizes}`, with
/// at most six sizes of 16 to 512 px.
#[tauri::command]
pub async fn composer_preview(request: Request<'_>) -> Result<Vec<String>, String> {
    let body = raw_body(&request)?;
    tauri::async_runtime::spawn_blocking(move || preview(&body))
        .await
        .map_err(|e| e.to_string())?
}

/// A picture the user picked or dropped, to place in a design.
#[tauri::command]
pub async fn composer_image(path: String) -> Result<ComposerImageDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        let bytes = std::fs::read(&p).map_err(|_| "couldn't read that picture".to_string())?;
        let rgba = commands::decode_picture(&p, &bytes)?;
        image_dto(&rgba, commands::display_name(&p))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A saved skin's own picture, to place in a design: the artwork, or the finished folder.
#[tauri::command]
pub async fn composer_skin_image(
    state: State<'_, AppState>,
    skin_id: String,
) -> Result<ComposerImageDto, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let image = state.resolve(&skin_id)?;
        let name = state
            .entry(&skin_id)
            .map_or_else(|| "Skin".to_string(), |entry| entry.name);
        image_dto(image.rgba(), name)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The document a saved design was made from, to open it in the composer again. `null` for a
/// skin that wasn't made in the composer.
#[tauri::command]
pub async fn composer_design(
    state: State<'_, AppState>,
    skin_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || design_value(&state, &skin_id))
        .await
        .map_err(|e| e.to_string())?
}

// ---------- the work, off the async threads ----------

fn draw_template() -> ComposerTemplateDto {
    let layers = compositor::template_layers(LAYER_SIZE);
    let url = |layer: &RgbaImage| commands::data_url(&raster::encode_png(layer));
    ComposerTemplateDto {
        size: LAYER_SIZE,
        back: url(&layers.back),
        front: url(&layers.front),
        middle: url(&layers.middle),
        top: url(&layers.top),
        outline: url(&layers.outline),
        parts: PartsDto::new(),
    }
}

/// Saves the design framed in `body`; see `composer_save`.
fn save(state: &AppState, body: &[u8]) -> Result<ComposerSavedDto, String> {
    let (header, png) = unframe::<SaveHeader>(body)?;
    if let Some(old) = &header.replaces {
        if !store::is_skin_id(old) {
            return Err("FolderSkin doesn't know the design you were changing".into());
        }
    }
    let design = decode_design(png, store::MAX_STORED_SIDE)?;
    let document = serde_json::to_vec(&header.design).map_err(|_| UNREADABLE.to_string())?;
    let image = match header.shape {
        Shape::Folder => SkinImage::Folder(Arc::new(raster::to_straight_rgba(
            &compositor::render_master_placed(&design),
        ))),
        Shape::Free => {
            if matte::alpha_bounds(&design, 8).is_none() {
                return Err("that design is completely transparent".into());
            }
            SkinImage::Folder(Arc::new(design))
        }
    };
    // The shape is part of what was made: the same picture on the folder and on its own are two
    // different skins.
    let id = store::skin_id(&[png, &document, header.shape.name().as_bytes()].concat());
    let new = NewSkin {
        id,
        name: store::clean_name(&header.name).unwrap_or_else(|| UNNAMED.into()),
        source: SkinSource::Composer,
        provider: None,
        model: None,
        idea: None,
        tags: header.tags,
        pack: None,
        pack_name: None,
        author: None,
        license: None,
        pack_hash: None,
    };
    let (entry, thumb, replaced) = match header.replaces {
        Some(old) => {
            let (entry, thumb) = state.replace_design(&old, new, image, document)?;
            (entry, thumb, Some(old))
        }
        None => {
            let (entry, thumb) = state.save_design(new, image, document)?;
            (entry, thumb, None)
        }
    };
    Ok(ComposerSavedDto {
        skin: SkinDto::saved(&entry, &thumb),
        replaced,
    })
}

/// The previews asked for in `body`; see `composer_preview`.
fn preview(body: &[u8]) -> Result<Vec<String>, String> {
    let (header, png) = unframe::<PreviewHeader>(body)?;
    if header.sizes.len() > MAX_PREVIEWS {
        return Err(format!("ask for at most {MAX_PREVIEWS} previews at a time"));
    }
    if let Some(size) = header.sizes.iter().find(|s| !PREVIEW_SIZES.contains(s)) {
        return Err(format!(
            "a preview can be {} to {} px, not {size}",
            PREVIEW_SIZES.start(),
            PREVIEW_SIZES.end()
        ));
    }
    let design = decode_design(png, PREVIEW_DESIGN_SIDE)?;
    let icons = match header.shape {
        Shape::Folder => compositor::render_placed_icon_set(&design, &header.sizes),
        Shape::Free => compositor::icon_set_from_image(&design, &header.sizes),
    };
    Ok(icons
        .sizes
        .iter()
        .map(|(_, icon)| commands::data_url(&raster::encode_png(icon)))
        .collect())
}

/// A saved design's document, read back as JSON.
fn design_value(state: &AppState, skin_id: &str) -> Result<Option<serde_json::Value>, String> {
    let Some(bytes) = state.design(skin_id)? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "that design is damaged, so it can't be opened again".into())
}

/// The bytes of a request the webview sent as raw bytes, copied so they can go to another thread.
fn raw_body(request: &Request<'_>) -> Result<Vec<u8>, String> {
    body_bytes(request.body())
}

/// The bytes of a raw request body. Tauri's postMessage fallback, which it uses when its own
/// protocol is blocked, sends them as a JSON array of numbers instead.
fn body_bytes(body: &InvokeBody) -> Result<Vec<u8>, String> {
    match body {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(serde_json::Value::Array(items)) => items
            .iter()
            .map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| UNREADABLE.to_string()),
        InvokeBody::Json(_) => Err(UNREADABLE.into()),
    }
}

/// Splits a body framed as `[u32 little-endian length of a JSON header][the header][PNG]` into the
/// header, read as `H`, and the PNG's bytes.
fn unframe<H: DeserializeOwned>(body: &[u8]) -> Result<(H, &[u8]), String> {
    let (len, rest) = body.split_first_chunk::<4>().ok_or(CUT_SHORT)?;
    let (header, png) = rest
        .split_at_checked(u32::from_le_bytes(*len) as usize)
        .ok_or(CUT_SHORT)?;
    let header = serde_json::from_slice(header).map_err(|_| UNREADABLE.to_string())?;
    if png.is_empty() {
        return Err(NO_PICTURE.into());
    }
    Ok((header, png))
}

/// A design's picture: a PNG, square and 64 to 4096 px across, shrunk to at most `max_side`.
fn decode_design(png: &[u8], max_side: u32) -> Result<RgbaImage, String> {
    let img = image::load_from_memory_with_format(png, ImageFormat::Png)
        .map_err(|_| "couldn't read the design's picture".to_string())?;
    let (w, h) = (img.width(), img.height());
    if w != h {
        return Err(format!(
            "the design's picture is {w}×{h} px, and it has to be square"
        ));
    }
    if !DESIGN_SIDES.contains(&w) {
        return Err(format!(
            "the design's picture is {w} px across, and it has to be {} to {}",
            DESIGN_SIDES.start(),
            DESIGN_SIDES.end()
        ));
    }
    Ok(store::shrink_to(img.to_rgba8(), max_side))
}

/// A picture as the composer takes it: at most [`store::MAX_STORED_SIDE`] px on its longer side, as
/// a PNG when it has any transparency and as a JPEG, far smaller for a photo, when it has none.
fn image_dto(rgba: &RgbaImage, name: String) -> Result<ComposerImageDto, String> {
    let rgba = if rgba.width().max(rgba.height()) > store::MAX_STORED_SIDE {
        Cow::Owned(store::shrink_to(rgba.clone(), store::MAX_STORED_SIDE))
    } else {
        Cow::Borrowed(rgba)
    };
    let (width, height) = rgba.dimensions();
    let alpha = rgba.pixels().any(|p| p.0[3] < 255);
    let url = if alpha {
        commands::data_url(&store::encode_stored_png(&rgba))
    } else {
        let rgb = image::DynamicImage::ImageRgba8(rgba.into_owned()).to_rgb8();
        let mut jpg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpg, JPEG_QUALITY)
            .write_image(&rgb, width, height, ExtendedColorType::Rgb8)
            .map_err(|_| "couldn't prepare that picture".to_string())?;
        format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(jpg)
        )
    };
    Ok(ComposerImageDto {
        url,
        width,
        height,
        name,
        alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `header` and `png` framed the way the webview sends them.
    fn frame(header: &serde_json::Value, png: &[u8]) -> Vec<u8> {
        let header = serde_json::to_vec(header).unwrap();
        let mut body = (header.len() as u32).to_le_bytes().to_vec();
        body.extend_from_slice(&header);
        body.extend_from_slice(png);
        body
    }

    fn png_of(img: &RgbaImage) -> Vec<u8> {
        raster::encode_png(img)
    }

    /// A square design: red on the left half, blue on the right, `alpha` all over.
    fn design(side: u32, alpha: u8) -> RgbaImage {
        RgbaImage::from_fn(side, side, |x, _| {
            image::Rgba(if x < side / 2 {
                [220, 30, 40, alpha]
            } else {
                [30, 60, 220, alpha]
            })
        })
    }

    fn save_header(name: &str, shape: &str, replaces: Option<&str>) -> serde_json::Value {
        json!({
            "name": name,
            "tags": ["Taxes", "taxes", "work"],
            "shape": shape,
            "design": {"version": 1, "layers": [{"kind": "text", "text": name}]},
            "replaces": replaces,
        })
    }

    #[test]
    fn a_framed_body_splits_into_its_header_and_picture() {
        let body = frame(&json!({"shape": "free", "sizes": [16, 512]}), b"\x89PNG...");
        let (header, png) = unframe::<PreviewHeader>(&body).unwrap();
        assert_eq!(header.shape, Shape::Free);
        assert_eq!(header.sizes, [16, 512]);
        assert_eq!(png, b"\x89PNG...");
    }

    #[test]
    fn a_body_that_isnt_framed_right_is_refused() {
        let err = |body: &[u8]| unframe::<PreviewHeader>(body).err();
        // Shorter than the length itself.
        assert_eq!(err(b"").as_deref(), Some(CUT_SHORT));
        assert_eq!(err(&[5, 0, 0]).as_deref(), Some(CUT_SHORT));
        // A header longer than what came.
        let mut body = frame(&json!({"shape": "folder", "sizes": [64]}), b"png");
        let past_the_end = (body.len() as u32).to_le_bytes();
        body[..4].copy_from_slice(&past_the_end);
        assert_eq!(err(&body).as_deref(), Some(CUT_SHORT));
        body[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(err(&body).as_deref(), Some(CUT_SHORT));
        // A header that isn't JSON, or isn't what was asked for.
        let mut bad = 6u32.to_le_bytes().to_vec();
        bad.extend_from_slice(b"{shape\x89PNG");
        assert_eq!(err(&bad).as_deref(), Some(UNREADABLE));
        let wrong = frame(&json!({"shape": "square", "sizes": [64]}), b"png");
        assert_eq!(err(&wrong).as_deref(), Some(UNREADABLE));
        let missing = frame(&json!({"shape": "folder"}), b"png");
        assert_eq!(err(&missing).as_deref(), Some(UNREADABLE));
        // No picture after the header.
        let empty = frame(&json!({"shape": "folder", "sizes": [64]}), b"");
        assert_eq!(err(&empty).as_deref(), Some(NO_PICTURE));
    }

    #[test]
    fn the_body_is_read_as_raw_bytes_or_as_the_fallbacks_array_of_numbers() {
        let raw = InvokeBody::Raw(vec![1, 2, 255]);
        assert_eq!(body_bytes(&raw).unwrap(), [1, 2, 255]);
        let array = InvokeBody::Json(json!([1, 2, 255]));
        assert_eq!(body_bytes(&array).unwrap(), [1, 2, 255]);
        for not_bytes in [json!([1, 256]), json!([-1]), json!(["1"]), json!({"a": 1})] {
            let body = InvokeBody::Json(not_bytes.clone());
            assert_eq!(
                body_bytes(&body).err().as_deref(),
                Some(UNREADABLE),
                "{not_bytes}"
            );
        }
    }

    #[test]
    fn only_a_square_png_of_a_sensible_size_is_a_design() {
        let ok = decode_design(&png_of(&design(64, 255)), 2048).unwrap();
        assert_eq!(ok.dimensions(), (64, 64));
        let big = decode_design(&png_of(&design(3000, 255)), 2048).unwrap();
        assert_eq!(
            big.dimensions(),
            (2048, 2048),
            "shrunk to the size asked for"
        );

        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 90)
            .write_image(&[0; 64 * 64 * 3], 64, 64, ExtendedColorType::Rgb8)
            .unwrap();
        for (bytes, want) in [
            (jpeg, "couldn't read"),
            (b"not a picture".to_vec(), "couldn't read"),
            (png_of(&RgbaImage::new(128, 64)), "has to be square"),
            (png_of(&RgbaImage::new(32, 32)), "has to be 64 to 4096"),
            (png_of(&RgbaImage::new(4097, 4097)), "has to be 64 to 4096"),
        ] {
            let err = decode_design(&bytes, 2048).unwrap_err();
            assert!(err.contains(want), "{err}");
        }
    }

    #[test]
    fn previews_come_in_the_sizes_asked_for() {
        let png = png_of(&design(256, 255));
        let body = frame(&json!({"shape": "folder", "sizes": [128, 16, 64]}), &png);
        let urls = preview(&body).unwrap();
        let sizes: Vec<u32> = urls
            .iter()
            .map(|url| {
                let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .unwrap();
                image::load_from_memory(&bytes).unwrap().width()
            })
            .collect();
        assert_eq!(sizes, [128, 16, 64]);

        // On its own, the design is the whole icon; on the folder, the corners are clear.
        let corner = |shape: &str| {
            let body = frame(&json!({"shape": shape, "sizes": [64]}), &png);
            let url = preview(&body).unwrap().remove(0);
            let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap();
            image::load_from_memory(&bytes)
                .unwrap()
                .to_rgba8()
                .get_pixel(1, 1)
                .0
        };
        assert_eq!(corner("free")[3], 255);
        assert_eq!(corner("folder")[3], 0);

        for (sizes, want) in [
            (json!([16, 32, 64, 128, 256, 512, 16]), "at most 6"),
            (json!([15]), "not 15"),
            (json!([64, 513]), "not 513"),
        ] {
            let body = frame(&json!({"shape": "folder", "sizes": sizes}), &png);
            let err = preview(&body).unwrap_err();
            assert!(err.contains(want), "{err}");
        }
    }

    #[test]
    fn a_design_is_saved_and_saved_over() {
        let state = AppState::default();
        let png = png_of(&design(128, 255));
        let first = save(
            &state,
            &frame(&save_header("  Taxes\n2026 ", "folder", None), &png),
        )
        .unwrap();
        assert_eq!(first.replaced, None);
        let skin = serde_json::to_value(&first.skin).unwrap();
        assert_eq!(skin["source"], "composer");
        assert_eq!(skin["kind"], "folder");
        assert_eq!(skin["name"], "Taxes 2026");
        assert_eq!(skin["tags"], json!(["taxes", "work"]));
        assert!(skin["thumbnail"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        // The folder, 2048 px, with the design on it and nothing past its outline.
        let SkinImage::Folder(icon) = state.resolve(&first.skin.id).unwrap() else {
            panic!("a design is saved as a finished folder");
        };
        assert_eq!(icon.dimensions(), (2048, 2048));
        assert_eq!(icon.get_pixel(10, 10).0[3], 0);
        assert_eq!(icon.get_pixel(400, 1200).0, [220, 30, 40, 255]);
        assert_eq!(icon.get_pixel(1600, 1200).0, [30, 60, 220, 255]);
        // Its document comes back as it was sent.
        assert_eq!(
            design_value(&state, &first.skin.id).unwrap(),
            Some(json!({"version": 1, "layers": [{"kind": "text", "text": "  Taxes\n2026 "}]}))
        );

        // The same picture as a free-shaped icon is a different skin.
        let free = save(
            &state,
            &frame(&save_header("  Taxes\n2026 ", "free", None), &png),
        )
        .unwrap();
        assert_ne!(free.skin.id, first.skin.id);

        // Changed and saved over the first: it takes its place.
        let changed = png_of(&design(128, 200));
        let over = frame(
            &save_header("Taxes", "folder", Some(&first.skin.id)),
            &changed,
        );
        let second = save(&state, &over).unwrap();
        assert_eq!(second.replaced.as_deref(), Some(first.skin.id.as_str()));
        assert_eq!(second.skin.created_at, first.skin.created_at);
        assert!(state.entry(&first.skin.id).is_none());
        // Saved over again with only a new name: the same skin, renamed where it is.
        let mut header = save_header("Taxes", "folder", Some(&second.skin.id));
        header["name"] = json!("Taxes 2027");
        let renamed = save(&state, &frame(&header, &changed)).unwrap();
        assert_eq!(renamed.skin.id, second.skin.id);
        assert_eq!(renamed.skin.name, "Taxes 2027");
        assert_eq!(renamed.replaced.as_deref(), Some(second.skin.id.as_str()));
        assert_eq!(state.saved_skins().len(), 2, "this design and the free one");

        // What can't be saved over.
        for (replaces, want) in [
            ("user:../skins", "doesn't know the design"),
            ("__default__", "doesn't know the design"),
            (&*store::skin_id(b"never saved"), store::DESIGN_GONE),
        ] {
            let body = frame(&save_header("X", "folder", Some(replaces)), &changed);
            let err = save(&state, &body).err().unwrap();
            assert!(err.contains(want), "{replaces}: {err}");
        }
    }

    #[test]
    fn a_design_with_no_name_gets_one_and_a_clear_one_needs_the_folder() {
        let state = AppState::default();
        let body = frame(
            &save_header(" \n ", "folder", None),
            &png_of(&design(64, 0)),
        );
        let saved = save(&state, &body).unwrap();
        assert_eq!(saved.skin.name, UNNAMED);
        let body = frame(&save_header("Clear", "free", None), &png_of(&design(64, 0)));
        assert_eq!(
            save(&state, &body).err().as_deref(),
            Some("that design is completely transparent")
        );
    }

    #[test]
    fn a_damaged_design_is_an_error_and_a_missing_one_is_none() {
        let dir = std::env::temp_dir().join(format!("folderskin-composer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = AppState::default();
        state.open_store(dir.clone());
        let body = frame(
            &save_header("Mine", "folder", None),
            &png_of(&design(64, 255)),
        );
        let id = save(&state, &body).unwrap().skin.id;
        assert!(design_value(&state, &id).unwrap().is_some());

        let file = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().ends_with(".design.json"))
            .unwrap()
            .path();
        std::fs::write(&file, b"{\"layers\": [").unwrap();
        let err = design_value(&state, &id).unwrap_err();
        assert!(err.contains("damaged"), "{err}");
        std::fs::remove_file(&file).unwrap();
        assert_eq!(design_value(&state, &id).unwrap(), None);
        assert_eq!(
            design_value(&state, &store::skin_id(b"unknown")).unwrap(),
            None
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pictures_come_as_png_with_transparency_and_jpeg_without() {
        let opaque = image_dto(&design(300, 255), "Photo".into()).unwrap();
        assert!(opaque.url.starts_with("data:image/jpeg;base64,"));
        assert!(!opaque.alpha);
        assert_eq!(
            (opaque.width, opaque.height, opaque.name.as_str()),
            (300, 300, "Photo")
        );

        let mut cut = design(300, 255);
        cut.put_pixel(0, 0, image::Rgba([0, 0, 0, 254]));
        let cut = image_dto(&cut, "Cut-out".into()).unwrap();
        assert!(cut.url.starts_with("data:image/png;base64,"));
        assert!(cut.alpha);

        let wide = RgbaImage::from_pixel(4100, 1025, image::Rgba([1, 2, 3, 255]));
        let wide = image_dto(&wide, "Wide".into()).unwrap();
        assert_eq!((wide.width, wide.height), (2048, 512));
    }

    #[test]
    fn a_saved_skins_picture_is_handed_over_with_its_name() {
        let state = AppState::default();
        let body = frame(
            &save_header("Mine", "free", None),
            &png_of(&design(64, 255)),
        );
        let id = save(&state, &body).unwrap().skin.id;
        let image = state.resolve(&id).unwrap();
        let dto = image_dto(image.rgba(), state.entry(&id).unwrap().name).unwrap();
        assert_eq!(
            (dto.width, dto.name.as_str(), dto.alpha),
            (64, "Mine", false)
        );
    }

    #[test]
    fn the_parts_are_the_template_in_canvas_units() {
        let parts = serde_json::to_value(PartsDto::new()).unwrap();
        assert_eq!(parts["canvas"], 1024.0);
        assert_eq!(parts["folder"], json!([15.0, 36.5, 1009.0, 973.5]));
        assert_eq!(parts["front"], json!([15.0, 160.5, 1009.0, 973.5]));
        assert_eq!(parts["front_radius"], 55.0);
        assert_eq!(parts["back"], json!([29.0, 36.5, 995.0, 973.5]));
        assert_eq!(parts["paper"].as_array().unwrap().len(), 4);
        let tab: Vec<f64> = serde_json::from_value(parts["tab"].clone()).unwrap();
        assert_eq!(&tab[..2], [61.0, 36.5]);
        assert!((tab[2] - 441.739).abs() < 0.01, "{}", tab[2]);
        assert_eq!(tab[3], 97.0);
        // As the webview reads it: plain arrays, the numbers as short as they are in geometry.rs.
        let text = serde_json::to_string(&PartsDto::new()).unwrap();
        assert!(
            text.contains(r#""paper":[74.5,131.3,949.5,973.5]"#),
            "{text}"
        );
    }

    #[test]
    fn the_template_is_drawn_at_2048_with_its_parts() {
        let template = serde_json::to_value(draw_template()).unwrap();
        assert_eq!(template["size"], 2048);
        for layer in ["back", "front", "middle", "top", "outline"] {
            let url = template[layer].as_str().unwrap();
            let b64 = url.strip_prefix("data:image/png;base64,").unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            assert_eq!((img.width(), img.height()), (2048, 2048), "{layer}");
        }
        assert_eq!(template["parts"]["canvas"], 1024.0);
    }

    // Not on Windows: with tauri's `test` feature the lib's test binary imports a WebView2 entry
    // point the runner's loader can't resolve, so it dies with STATUS_ENTRYPOINT_NOT_FOUND before
    // a single test runs. The commands themselves are checked on macOS and Linux, and the Windows
    // writer has its own tests in folderskin-core.
    #[cfg(not(windows))]
    mod ipc {
        use super::*;
        /// The command as the webview calls it: raw bytes through Tauri's own IPC, into a mock app.
        fn invoke(
            webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
            cmd: &str,
            body: tauri::ipc::InvokeBody,
        ) -> Result<serde_json::Value, serde_json::Value> {
            tauri::test::get_ipc_response(
                webview,
                tauri::webview::InvokeRequest {
                    cmd: cmd.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "tauri://localhost".parse().unwrap(),
                    body,
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .map(|b| b.deserialize::<serde_json::Value>().unwrap())
        }

        #[test]
        fn a_design_sent_as_raw_bytes_through_the_ipc_is_previewed_and_saved() {
            let app = tauri::test::mock_builder()
                .manage(AppState::default())
                .invoke_handler(tauri::generate_handler![composer_preview, composer_save])
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            let png = png_of(&design(128, 255));

            let preview = frame(&json!({"shape": "folder", "sizes": [16, 64]}), &png);
            let urls = invoke(&webview, "composer_preview", InvokeBody::Raw(preview)).unwrap();
            let urls = urls.as_array().unwrap();
            assert_eq!(urls.len(), 2);
            assert!(urls
                .iter()
                .all(|u| u.as_str().unwrap().starts_with("data:image/png;base64,")));

            // With no data folder the design lasts for the session, which is enough to see it arrive.
            let save = frame(&save_header("Taxes 2026", "folder", None), &png);
            let saved = invoke(&webview, "composer_save", InvokeBody::Raw(save)).unwrap();
            assert_eq!(saved["skin"]["name"], "Taxes 2026");
            assert_eq!(saved["skin"]["source"], "composer");
            assert_eq!(saved["skin"]["kind"], "folder");
            assert!(saved["replaced"].is_null());

            // A body that isn't bytes at all is refused with a sentence, not a crash.
            let refused = invoke(
                &webview,
                "composer_preview",
                InvokeBody::Json(json!({"shape": "folder"})),
            );
            assert!(refused.unwrap_err().is_string());
        }
    }
}
