//! The layers the composer draws a design between, as the files `composer-layers` writes and
//! `docs/images/composer/` keeps: FolderSkin's own folder at the top, Windows' in `windows/` and
//! Linux's in `linux/`. Beside them, for the browser preview, every drive's layers in
//! `drives/<id>/` with where each drive's face is in `drives/parts.json` and the strip its pack
//! of drives shows in `drives/strip.webp`, and every base's bare shape in `bases/<id>.webp`
//! ([`folderskin_core::base`]).

use folderskin_core::base::{self, BASES};
use folderskin_core::compositor::{template_layers_in, Style};
use folderskin_core::drive::{self, DriveShape};
use image::RgbaImage;

/// Where a style's layers go, under the folder `composer-layers` writes into.
pub fn style_dir(style: Style) -> &'static str {
    match style {
        Style::Mac => "",
        Style::Windows => "windows/",
        Style::Linux => "linux/",
    }
}

/// Each layer's file name and picture at `size` px, bottom to top the way the composer stacks
/// them around a design: the design masked by `back.png`, then `middle.png`, then the design
/// masked by `front.png`, then `top.png`, with `outline.png` last, for showing the folder's edges.
pub fn layer_files(size: u32, style: Style) -> [(&'static str, RgbaImage); 5] {
    let layers = template_layers_in(size, style);
    [
        ("back.png", layers.back),
        ("middle.png", layers.middle),
        ("front.png", layers.front),
        ("top.png", layers.top),
        ("outline.png", layers.outline),
    ]
}

/// A drive's layers at `size` px, named as `drives/<id>/` keeps them and stacked the way a folder's
/// are: the plain drive (`middle.webp`), the design masked by its face (`front.webp`), what goes
/// over the face (`top.webp`), and its edges (`outline.webp`). A drive's `back` is always empty, so
/// it isn't kept.
pub fn drive_layer_files(shape: DriveShape, size: u32) -> [(&'static str, RgbaImage); 4] {
    let layers = drive::layers(shape, size);
    [
        ("middle.webp", layers.middle),
        ("front.webp", layers.front),
        ("top.webp", layers.top),
        ("outline.webp", layers.outline),
    ]
}

/// Where every drive's face is, in canvas units, by drive id: its box, a point on it nothing hides,
/// and the drive's whole extent. What `drives/parts.json` keeps, for the browser preview to place
/// layers as the app does.
pub fn drive_parts() -> serde_json::Value {
    let parts: serde_json::Map<String, serde_json::Value> = DriveShape::all()
        .into_iter()
        .map(|shape| {
            let face = shape.face_box();
            let (x, y) = shape.face_point();
            (
                shape.id(),
                serde_json::json!({
                    "face": [face.x0, face.y0, face.x1, face.y1],
                    "point": [x, y],
                    "extent": shape.extent(),
                }),
            )
        })
        .collect();
    serde_json::Value::Object(parts)
}

/// Every base's bare shape at `size` px, by id, as `bases/<id>.webp` keeps them: the folders and
/// the drives. A free icon has no picture.
pub fn base_pictures(size: u32) -> Vec<(String, RgbaImage)> {
    BASES
        .iter()
        .filter_map(|b| b.bare(size).map(|bare| (b.id.to_string(), bare)))
        .collect()
}

/// The drives the browser preview's pack of drives starts with, in its order (`PLAIN_DRIVES` in
/// src/lib/devMock.ts).
const STRIP_DRIVES: [&str; 4] = [
    "mac-external",
    "mac-removable",
    "mac-network",
    "windows-internal",
];

/// The strip the browser preview's pack of drives shows in Community, as `drives/strip.webp`
/// keeps it: its first drives side by side, each [`crate::packs::PREVIEW_SIDE`] px square, as
/// [`crate::packs::preview_strip`] draws a pack's.
pub fn drive_strip() -> RgbaImage {
    let side = crate::packs::PREVIEW_SIDE;
    let mut strip = RgbaImage::new(side * STRIP_DRIVES.len() as u32, side);
    for (i, id) in STRIP_DRIVES.iter().enumerate() {
        let shape = DriveShape::from_id(id).expect("a drive FolderSkin draws");
        let tile = base::drive_of(shape)
            .bare(side)
            .expect("a drive has a picture");
        image::imageops::replace(&mut strip, &tile, i64::from(i as u32 * side), 0);
    }
    strip
}

/// Whether two pictures show the same pixels: every pixel's alpha, and the colour of every pixel
/// that shows. A lossless WebP may change the colour under a fully transparent pixel, which
/// nothing ever shows.
pub fn same_pixels(a: &RgbaImage, b: &RgbaImage) -> bool {
    a.dimensions() == b.dimensions()
        && a.pixels()
            .zip(b.pixels())
            .all(|(p, q)| p.0[3] == q.0[3] && (p.0[3] == 0 || p.0 == q.0))
}

/// Whether `a` and `b` are one picture, give or take the rounding another processor draws it
/// with. The same drives drawn on an x86 computer and an Arm Mac came out a few pixels apart in each
/// picture, by one colour level mostly and four at the most. So coverage within [`NEAR_ALPHA`]
/// levels and colour within [`NEAR_COLOUR`] where it shows, and no more than one pixel in
/// [`NEAR_SHARE`] off at all: a change to a drawing moves thousands.
pub fn near_pixels(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.dimensions() != b.dimensions() {
        return false;
    }
    let mut off = 0usize;
    for (p, q) in a.pixels().zip(b.pixels()) {
        // A pixel nobody sees may keep any colour: WebP keeps what it likes under no coverage.
        if p == q || (p.0[3] == 0 && q.0[3] == 0) {
            continue;
        }
        off += 1;
        let coverage = p.0[3].abs_diff(q.0[3]);
        let colour = (0..3).map(|c| p.0[c].abs_diff(q.0[c])).max().unwrap_or(0);
        if coverage > NEAR_ALPHA || (p.0[3].min(q.0[3]) >= 16 && colour > NEAR_COLOUR) {
            return false;
        }
    }
    off <= (a.width() as usize * a.height() as usize / NEAR_SHARE).max(1)
}

/// How far a pixel's coverage may be from the other picture's in [`near_pixels`].
const NEAR_ALPHA: u8 = 4;
/// How far a pixel's colour may be from the other picture's in [`near_pixels`].
const NEAR_COLOUR: u8 = 8;
/// One pixel in this many may differ at all in [`near_pixels`].
const NEAR_SHARE: usize = 200;

#[cfg(test)]
mod tests {
    use super::*;

    /// The pictures in the docs are the layers the compositor draws now, pixel for pixel, so they
    /// can't go stale without this failing.
    #[test]
    fn the_layers_in_the_docs_are_the_ones_the_compositor_draws() {
        for style in Style::ALL {
            let dir = format!(
                "{}/../../docs/images/composer/{}",
                env!("CARGO_MANIFEST_DIR"),
                style_dir(style)
            );
            for (file, want) in layer_files(1024, style) {
                let path = format!("{dir}{file}");
                let got = image::open(&path)
                    .unwrap_or_else(|e| panic!("couldn't read {path}: {e}"))
                    .to_rgba8();
                assert_eq!(got.dimensions(), want.dimensions(), "{path}");
                let differ = got
                    .pixels()
                    .zip(want.pixels())
                    .filter(|(a, b)| a != b)
                    .count();
                assert_eq!(
                    differ, 0,
                    "{differ} pixels of {path} differ from the compositor's; write them again with \
                     `cargo run -p folderskin-tools -- composer-layers --out docs/images/composer`"
                );
            }
        }
    }

    /// The browser preview's drives and bases are the ones the compositor draws now, so they can't
    /// go stale without this failing either.
    #[test]
    fn the_drives_and_bases_in_the_docs_are_the_ones_the_compositor_draws() {
        let dir = format!("{}/../../docs/images/composer", env!("CARGO_MANIFEST_DIR"));
        let again = "write them again with \
                     `cargo run -p folderskin-tools -- composer-layers --out docs/images/composer`";
        let read = |path: &str| {
            image::open(path)
                .unwrap_or_else(|e| panic!("couldn't read {path}: {e}"))
                .to_rgba8()
        };
        for shape in DriveShape::all() {
            for (file, want) in drive_layer_files(shape, 512) {
                let path = format!("{dir}/drives/{}/{file}", shape.id());
                assert!(near_pixels(&read(&path), &want), "{path} differs; {again}");
            }
        }
        let parts: serde_json::Value =
            serde_json::from_slice(&std::fs::read(format!("{dir}/drives/parts.json")).unwrap())
                .unwrap();
        assert!(
            near_json(&parts, &drive_parts()),
            "drives/parts.json differs; {again}"
        );
        for (id, want) in base_pictures(256) {
            let path = format!("{dir}/bases/{id}.webp");
            assert!(near_pixels(&read(&path), &want), "{path} differs; {again}");
        }
        let strip = format!("{dir}/drives/strip.webp");
        assert!(
            near_pixels(&read(&strip), &drive_strip()),
            "{strip} differs; {again}"
        );
    }

    /// Whether two JSON values say the same, numbers within a hundredth of a unit: the drives'
    /// parts come from their drawings' `f32` geometry, which an x86 computer rounds differently in
    /// the last digits (93.2515335 there, 93.2515182 on an Arm Mac).
    fn near_json(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        use serde_json::Value;
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => (x - y).abs() <= 0.01,
                _ => x == y,
            },
            (Value::Array(x), Value::Array(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(p, q)| near_json(p, q))
            }
            (Value::Object(x), Value::Object(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .all(|(k, v)| y.get(k).is_some_and(|w| near_json(v, w)))
            }
            _ => a == b,
        }
    }
}
