//! Compositor: renders the folder template with cover-fitted artwork.
//!
//! The types below are the interface other modules and crates rely on; keep their names and
//! signatures stable.
//!
//! One render path, one master: everything is drawn once at [`RENDER_SIZE`] into a
//! premultiplied pixmap, and every icon size is a Lanczos3 downsample of that master. The
//! preview the user picks from and the icon written to disk are therefore the same pixels.

use crate::{fit, geometry as g, raster};
use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Mask, Paint, Path, Pattern, Pixmap, SpreadMode,
    Stroke, Transform,
};

/// Source artwork plus the focus point (0..1, 0..1) that cover-fit crops keep centred.
pub struct Artwork {
    pub rgba: image::RgbaImage,
    pub focus: (f32, f32),
}

/// Rendered icon at several sizes (straight alpha RGBA).
pub struct IconSet {
    pub sizes: Vec<(u32, image::RgbaImage)>,
}

impl IconSet {
    /// PNG bytes for one size, if that size was rendered.
    pub fn png(&self, size: u32) -> Option<Vec<u8>> {
        self.sizes
            .iter()
            .find(|(s, _)| *s == size)
            .map(|(_, img)| crate::raster::encode_png(img))
    }
}

/// Master render size; every smaller size is a Lanczos3 downsample of this.
pub const RENDER_SIZE: u32 = 2048;
/// Sizes the app renders for an applied icon.
pub const ICON_SIZES: [u32; 10] = [2048, 1024, 512, 256, 128, 64, 48, 32, 24, 16];
/// Width of artwork made for the template. At 1024 × 958 it is close to the back panel's own
/// aspect, so the folder crops little of it; artwork of any other size still works.
pub const SKIN_WIDTH: u32 = 1024;
/// Height of artwork made for the template; see [`SKIN_WIDTH`].
pub const SKIN_HEIGHT: u32 = 958;

/// Flat colour of the paper sheet.
const PAPER_FILL: [u8; 4] = [0xEB, 0xE6, 0xE0, 0xFF];
/// Highlight on the paper's top edge.
const PAPER_HIGHLIGHT: [u8; 4] = [0xF7, 0xF0, 0xE9, 0xFF];
/// How far the panel rims fade inward, in canvas units.
const RIM_BAND: f32 = 4.0;
/// How far the paper's highlight fades inward, in canvas units.
const PAPER_BAND: f32 = 1.0;
/// Rim light alpha. Tuned so a flat mid-grey skin gains ~14 luminance at the edge, which is
/// what the reference icon measures; see the `rim_magnitudes_match_the_reference` test.
const RIM_LIGHT: u8 = 19;
/// Bottom shade alpha, tuned the same way for ~-35 luminance.
const RIM_SHADE: u8 = 48;

/// The artwork as a premultiplied pixmap tiny-skia can use as a pattern.
fn artwork_pixmap(art: &Artwork) -> Pixmap {
    let p = raster::straight_to_premul(&art.rgba);
    let mut pm = Pixmap::new(p.width, p.height).expect("artwork size");
    pm.data_mut().copy_from_slice(&p.data);
    pm
}

/// Paint that draws `pm` cover-fitted to `target` (canvas units) at `scale` device px per unit.
fn artwork_paint<'a>(pm: &'a Pixmap, target: &g::Rect, focus: (f32, f32), scale: f32) -> Paint<'a> {
    let pl = fit::cover_fit(pm.width(), pm.height(), target, focus);
    let shader = Pattern::new(
        pm.as_ref(),
        SpreadMode::Pad,
        FilterQuality::Bicubic,
        1.0,
        Transform::from_row(
            pl.scale * scale,
            0.0,
            0.0,
            pl.scale * scale,
            pl.x * scale,
            pl.y * scale,
        ),
    );
    Paint {
        shader,
        anti_alias: true,
        ..Paint::default()
    }
}

/// Draws a rim light (or shade) just inside `edge`, fading inward over `band` canvas units.
///
/// The stroke sits *on* the shape's own outline and `mask` throws away its outer half, so what
/// remains is brightest at the edge. Three overlapping passes of decreasing width make the
/// falloff: widths are twice the band they should cover, since half of each is masked away.
fn rim(pixmap: &mut Pixmap, edge: &Path, mask: &Mask, rgba: [u8; 4], band: f32, scale: f32) {
    for (w, a) in [(2.0, 0.45), (1.25, 0.55), (0.625, 0.65)] {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], (rgba[3] as f32 * a) as u8);
        let stroke = Stroke {
            width: w * band * scale,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(edge, &paint, &stroke, Transform::identity(), Some(mask));
    }
}

/// Renders the template at [`RENDER_SIZE`], premultiplied.
pub fn render_master(art: &Artwork) -> raster::Premul {
    let s = RENDER_SIZE as f32 / g::CANVAS;
    let mut pm = Pixmap::new(RENDER_SIZE, RENDER_SIZE).expect("master pixmap");
    let art_pm = artwork_pixmap(art);
    let back = g::back_panel_path(s);
    let paper = g::paper_path(s);
    let front = g::front_panel_path(s);
    let mask_of = |p: &Path| {
        let mut m = Mask::new(RENDER_SIZE, RENDER_SIZE).expect("mask size");
        m.fill_path(p, FillRule::Winding, true, Transform::identity());
        m
    };

    // Back panel: the skin cover-fitted to the whole back bbox, so the tab shows the top of
    // the image.
    pm.fill_path(
        &back,
        &artwork_paint(&art_pm, &g::BACK_BBOX, art.focus, s),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    let back_mask = mask_of(&back);
    rim(
        &mut pm,
        &g::back_top_edge_path(s),
        &back_mask,
        [255, 255, 255, RIM_LIGHT],
        RIM_BAND,
        s,
    );

    // Paper sheet.
    let mut paper_paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paper_paint.set_color_rgba8(PAPER_FILL[0], PAPER_FILL[1], PAPER_FILL[2], PAPER_FILL[3]);
    pm.fill_path(
        &paper,
        &paper_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    let paper_mask = mask_of(&paper);
    rim(
        &mut pm,
        &g::paper_top_edge_path(s),
        &paper_mask,
        PAPER_HIGHLIGHT,
        PAPER_BAND,
        s,
    );

    // Front panel: the same skin and focus, cover-fitted to the front rectangle, so the front
    // shows the middle of the image.
    pm.fill_path(
        &front,
        &artwork_paint(&art_pm, &g::FRONT, art.focus, s),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    let front_mask = mask_of(&front);
    rim(
        &mut pm,
        &g::front_top_sides_path(s),
        &front_mask,
        [255, 255, 255, RIM_LIGHT],
        RIM_BAND,
        s,
    );
    rim(
        &mut pm,
        &g::front_bottom_path(s),
        &front_mask,
        [0, 0, 0, RIM_SHADE],
        RIM_BAND,
        s,
    );

    raster::Premul {
        width: RENDER_SIZE,
        height: RENDER_SIZE,
        data: pm.data().to_vec(),
    }
}

/// Renders the icon at every requested size, downsampling the one master render.
pub fn render_icon_set(art: &Artwork, sizes: &[u32]) -> IconSet {
    let master = render_master(art);
    let sizes = sizes
        .iter()
        .map(|&size| {
            let img = if size == RENDER_SIZE {
                raster::to_straight_rgba(&master)
            } else {
                raster::to_straight_rgba(&raster::downsample(&master, size))
            };
            (size, img)
        })
        .collect();
    IconSet { sizes }
}

/// Builds an icon set from a picture that is already a finished folder image.
///
/// Used for whole-folder renders from an image model: the artwork is not composited onto our
/// template, it *is* the icon. The picture is fitted into the square icon canvas, centred, with
/// its aspect preserved, then downsampled through the same Lanczos path as a composited icon so
/// the small sizes look identical in kind.
pub fn icon_set_from_image(img: &image::RgbaImage, sizes: &[u32]) -> IconSet {
    let master = fit_into_canvas(img, RENDER_SIZE);
    let premul = raster::straight_to_premul(&master);
    let sizes = sizes
        .iter()
        .map(|&size| {
            let out = if size == RENDER_SIZE {
                master.clone()
            } else {
                raster::to_straight_rgba(&raster::downsample(&premul, size))
            };
            (size, out)
        })
        .collect();
    IconSet { sizes }
}

/// Centres `img` in a transparent `size` x `size` canvas, scaled to fit without cropping.
fn fit_into_canvas(img: &image::RgbaImage, size: u32) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return image::RgbaImage::new(size, size);
    }
    let scale = (size as f32 / w as f32).min(size as f32 / h as f32);
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    let scaled = image::imageops::resize(img, nw, nh, image::imageops::FilterType::Lanczos3);
    let mut canvas = image::RgbaImage::new(size, size);
    image::imageops::overlay(
        &mut canvas,
        &scaled,
        ((size - nw) / 2) as i64,
        ((size - nh) / 2) as i64,
    );
    canvas
}

/// Renders one size straight to PNG bytes, for previews.
pub fn render_preview_png(art: &Artwork, size: u32) -> Vec<u8> {
    render_icon_set(art, &[size])
        .png(size)
        .expect("the size that was just rendered")
}

/// The stand-in skin for the "no skin yet" state: a flat macOS-blue vertical gradient, so the
/// plain folder goes through exactly the same render path as every real skin.
pub fn default_folder_artwork() -> Artwork {
    const TOP: [u8; 3] = [0x7C, 0xC8, 0xF5];
    const BOTTOM: [u8; 3] = [0x4E, 0xA9, 0xE4];
    let rgba = image::RgbaImage::from_fn(SKIN_WIDTH, SKIN_HEIGHT, |_, y| {
        let t = y as f32 / (SKIN_HEIGHT - 1) as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        image::Rgba([
            mix(TOP[0], BOTTOM[0]),
            mix(TOP[1], BOTTOM[1]),
            mix(TOP[2], BOTTOM[2]),
            255,
        ])
    });
    Artwork {
        rgba,
        focus: (0.5, 0.5),
    }
}

/// PNG preview of a finished folder image, at `size` px.
pub fn preview_png_from_image(img: &image::RgbaImage, size: u32) -> Vec<u8> {
    icon_set_from_image(img, &[size])
        .png(size)
        .expect("the size that was just rendered")
}

/// The neutral light grey the blank template is painted with.
const TEMPLATE_GREY: [u8; 4] = [0xCC, 0xCC, 0xCC, 0xFF];
/// Share of the frame left empty on each side of the blank template.
const TEMPLATE_MARGIN: f32 = 0.03;

/// The folder template itself, painted a flat neutral light grey and centred on an opaque
/// `backdrop`, `width` x `height` px.
///
/// Image models that can work from a picture are shown this as the exact folder to repaint, so a
/// whole-folder generation keeps FolderSkin's silhouette, tab and paper strip rather than a folder
/// the model makes up. It is the normal render with grey artwork, scaled so the folder fills the
/// frame less a 3% margin, and centred.
pub fn blank_template(width: u32, height: u32, backdrop: [u8; 3]) -> image::RgbaImage {
    let art = Artwork {
        rgba: image::RgbaImage::from_pixel(8, 8, image::Rgba(TEMPLATE_GREY)),
        focus: (0.5, 0.5),
    };
    // The folder's extent in canvas units: the front panel is its widest part, the tab its top.
    let (x0, x1, y0, y1) = (g::FRONT.x0, g::FRONT.x1, g::TAB_TOP, g::FRONT.y1);
    let usable = 1.0 - 2.0 * TEMPLATE_MARGIN;
    let px_per_unit = (width as f32 * usable / (x1 - x0)).min(height as f32 * usable / (y1 - y0));
    let size = ((g::CANVAS * px_per_unit).round() as u32).max(1);
    let icon = render_icon_set(&art, &[size]).sizes.remove(0).1;

    let s = size as f32 / g::CANVAS;
    let left = (width as f32 / 2.0 - (x0 + x1) / 2.0 * s).round() as i64;
    let top = (height as f32 / 2.0 - (y0 + y1) / 2.0 * s).round() as i64;
    let mut frame = image::RgbaImage::new(width, height);
    image::imageops::replace(&mut frame, &icon, left, top);
    crate::matte::flatten(&frame, backdrop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prerendered_image_is_centred_and_scaled_into_the_canvas() {
        // A wide, fully opaque picture must end up letterboxed with transparent bands.
        let src = image::RgbaImage::from_pixel(400, 200, image::Rgba([10, 120, 200, 255]));
        let set = icon_set_from_image(&src, &[512]);
        let img = &set.sizes[0].1;
        assert_eq!(img.dimensions(), (512, 512));
        assert_eq!(
            img.get_pixel(256, 256).0[3],
            255,
            "the picture sits in the middle"
        );
        assert_eq!(
            img.get_pixel(256, 4).0[3],
            0,
            "the band above it stays transparent"
        );
        assert_eq!(img.get_pixel(4, 256).0[3], 255, "it reaches the left edge");
    }

    fn solid(w: u32, h: u32, c: [u8; 4]) -> Artwork {
        Artwork {
            rgba: image::RgbaImage::from_pixel(w, h, image::Rgba(c)),
            focus: (0.5, 0.5),
        }
    }
    fn alpha_at(img: &image::RgbaImage, x: u32, y: u32) -> u8 {
        img.get_pixel(x, y).0[3]
    }

    #[test]
    fn render_is_deterministic() {
        let a = render_preview_png(&solid(1024, 958, [200, 30, 30, 255]), 256);
        let b = render_preview_png(&solid(1024, 958, [200, 30, 30, 255]), 256);
        assert_eq!(a, b);
    }

    #[test]
    fn silhouette_matches_template_at_1024() {
        let set = render_icon_set(&solid(1024, 958, [10, 200, 90, 255]), &[1024]);
        let img = &set.sizes[0].1;
        // inside the front panel, tab, paper strip: opaque
        for (x, y) in [(512, 600), (200, 60), (512, 145), (990, 700), (20, 700)] {
            assert_eq!(alpha_at(img, x, y), 255, "({x},{y})");
        }
        // outside: transparent (above the body right of the tab, left of the tab, below the
        // bottom, canvas corners)
        for (x, y) in [
            (700, 80),
            (40, 60),
            (512, 990),
            (5, 5),
            (1018, 1018),
            (1000, 120),
        ] {
            assert_eq!(alpha_at(img, x, y), 0, "({x},{y})");
        }
        // edges within 1.5 px of the constants: front left edge x=15, bottom y=973.5, tab top
        // y=36.5, body top y=97 at x=700
        let row = |y: u32| (0..1024).find(|&x| alpha_at(img, x, y) >= 128).unwrap();
        assert!((row(600) as f32 - 15.0).abs() <= 1.5);
        let col = |x: u32| (0..1024).find(|&y| alpha_at(img, x, y) >= 128).unwrap();
        assert!((col(200) as f32 - 36.5).abs() <= 1.5);
        assert!((col(700) as f32 - 97.0).abs() <= 1.5);
        let bottom = (0..1024)
            .rev()
            .find(|&y| alpha_at(img, 512, y) >= 128)
            .unwrap();
        assert!((bottom as f32 - 973.5).abs() <= 1.5);
    }

    #[test]
    fn paper_strip_is_off_white_and_artwork_fills_front() {
        let set = render_icon_set(&solid(1024, 958, [10, 200, 90, 255]), &[1024]);
        let img = &set.sizes[0].1;
        let paper = img.get_pixel(512, 146).0;
        assert!(paper[0] > 225 && paper[1] > 220 && paper[2] > 210);
        let front = img.get_pixel(512, 600).0;
        assert!(front[1] > 180 && front[0] < 60);
    }

    #[test]
    fn icon_set_contains_requested_sizes() {
        let set = render_icon_set(&solid(1024, 958, [1, 2, 3, 255]), &ICON_SIZES);
        assert_eq!(
            set.sizes.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            ICON_SIZES.to_vec()
        );
        assert!(set.png(16).unwrap().starts_with(&[0x89, b'P', b'N', b'G']));
    }

    /// The rim treatment at the master resolution, where a 4 px band is 8 real pixels: light
    /// ~+14 at the front's and back body's top edges, dark ~-35 at the front's bottom, each
    /// fading to nothing over 4 canvas px.
    #[test]
    fn rim_magnitudes_match_the_reference() {
        let master = crate::raster::to_straight_rgba(&render_master(&solid(
            1024,
            958,
            [128, 128, 128, 255],
        )));
        let lum = |x: u32, y: u32| master.get_pixel(x, y).0[1] as i32 - 128;

        // Front panel top edge: canvas y 160.5 → master y 321, band 8 master px.
        assert!(
            (lum(1600, 321) - 14).abs() <= 6,
            "front top {}",
            lum(1600, 321)
        );
        assert!(
            lum(1600, 329).abs() <= 2,
            "front top fade {}",
            lum(1600, 329)
        );
        // Front panel left and right edges get the same rim (canvas x 15 and 1009).
        assert!(
            (lum(30, 1200) - 14).abs() <= 6,
            "front left {}",
            lum(30, 1200)
        );
        assert!(
            (lum(2017, 1200) - 14).abs() <= 6,
            "front right {}",
            lum(2017, 1200)
        );
        // Front panel bottom edge: canvas y 973.5 → the last inside row is master 1946.
        assert!(
            (lum(1024, 1946) + 35).abs() <= 6,
            "front bottom {}",
            lum(1024, 1946)
        );
        assert!(
            lum(1024, 1938).abs() <= 2,
            "front bottom fade {}",
            lum(1024, 1938)
        );
        // Back body top edge: canvas y 97 → master y 194.
        assert!(
            (lum(1400, 194) - 14).abs() <= 6,
            "back top {}",
            lum(1400, 194)
        );
        assert!(
            lum(1400, 202).abs() <= 2,
            "back top fade {}",
            lum(1400, 202)
        );
    }

    /// The same three columns the reference icon was sampled on, at 1024. The front's top edge
    /// reads lower than the others because the Lanczos kernel reaches into the much brighter
    /// paper strip right above it; the rim itself is the same one the master test measures.
    #[test]
    fn rim_survives_the_downsample_to_1024() {
        let set = render_icon_set(&solid(1024, 958, [128, 128, 128, 255]), &[1024]);
        let img = &set.sizes[0].1;
        let lum = |x: u32, y: u32| img.get_pixel(x, y).0[1] as i32 - 128;

        // x = 800, rows 160..167. Row 160 straddles the edge and mixes in the paper above it,
        // so the rim's own peak is the brightest of the rows fully inside the panel.
        let front_top = (161..167).map(|y| lum(800, y)).max().unwrap();
        assert!((front_top - 14).abs() <= 6, "front top {front_top}");
        assert!(lum(800, 168).abs() <= 2, "front top fade {}", lum(800, 168));
        // x = 512, rows 969..973.
        let bottom = (969..974).map(|y| lum(512, y)).min().unwrap();
        assert!((bottom + 35).abs() <= 6, "front bottom {bottom}");
        assert!(
            lum(512, 967).abs() <= 2,
            "front bottom fade {}",
            lum(512, 967)
        );
        // x = 700, rows 97..101.
        let back_top = (97..102).map(|y| lum(700, y)).max().unwrap();
        assert!((back_top - 14).abs() <= 6, "back top {back_top}");
        assert!(lum(700, 103).abs() <= 2, "back top fade {}", lum(700, 103));
    }

    #[test]
    fn paper_has_a_bright_line_on_its_top_edge() {
        // The line is 1 canvas px, so measure it where it is a real pixel: the master.
        let master = crate::raster::to_straight_rgba(&render_master(&solid(
            1024,
            958,
            [128, 128, 128, 255],
        )));
        // Canvas y 131.3 → master y 262.6, so 263 is the first row fully inside the sheet.
        let line = master.get_pixel(1024, 263).0;
        let flat = master.get_pixel(1024, 270).0;
        assert_eq!(flat, PAPER_FILL);
        for c in 0..3 {
            let want = PAPER_HIGHLIGHT[c] as i32;
            assert!((line[c] as i32 - want).abs() <= 6, "{line:?}");
            assert!(line[c] as i32 > flat[c] as i32 + 4, "{line:?}");
        }
        // Gone again 2 px in.
        assert_eq!(master.get_pixel(1024, 265).0, PAPER_FILL);
    }

    #[test]
    fn no_pixels_outside_the_template_bbox() {
        // The reference has no drop shadow: the alpha bbox is the shape bbox ± anti-aliasing,
        // plus the few tenths of a percent of Lanczos ringing just outside it.
        let set = render_icon_set(&solid(1024, 958, [200, 30, 30, 255]), &[1024]);
        let img = &set.sizes[0].1;
        let bbox = |min_alpha: u8| {
            let (mut x0, mut y0, mut x1, mut y1) = (1024i32, 1024i32, -1i32, -1i32);
            for (x, y, px) in img.enumerate_pixels() {
                if px.0[3] >= min_alpha {
                    x0 = x0.min(x as i32);
                    y0 = y0.min(y as i32);
                    x1 = x1.max(x as i32);
                    y1 = y1.max(y as i32);
                }
            }
            (x0, y0, x1, y1)
        };
        // Everything the eye can see is within 2 px of the template.
        let (x0, y0, x1, y1) = bbox(9);
        assert!((x0 - 15).abs() <= 2 && (x1 - 1008).abs() <= 2, "{x0}..{x1}");
        assert!((y0 - 36).abs() <= 2 && (y1 - 973).abs() <= 2, "{y0}..{y1}");
        // And nothing at all is more than 3 px out — no shadow, no glow.
        let (x0, y0, x1, y1) = bbox(1);
        assert!((x0 - 15).abs() <= 3 && (x1 - 1008).abs() <= 3, "{x0}..{x1}");
        assert!((y0 - 36).abs() <= 3 && (y1 - 973).abs() <= 3, "{y0}..{y1}");
    }

    #[test]
    fn default_artwork_is_the_macos_blue_gradient() {
        let art = default_folder_artwork();
        assert_eq!(art.rgba.dimensions(), (1024, 958));
        assert_eq!(art.focus, (0.5, 0.5));
        assert_eq!(art.rgba.get_pixel(0, 0).0, [0x7C, 0xC8, 0xF5, 255]);
        assert_eq!(art.rgba.get_pixel(1023, 957).0, [0x4E, 0xA9, 0xE4, 255]);
        // Monotonic, vertical only.
        let mid = art.rgba.get_pixel(0, 479).0;
        assert_eq!(mid, art.rgba.get_pixel(1023, 479).0);
        assert!(mid[0] < 0x7C && mid[0] > 0x4E);
    }

    #[test]
    fn the_blank_template_is_our_folder_centred_on_the_backdrop() {
        let (w, h) = (1166, 1091);
        let img = blank_template(w, h, [255, 0, 255]);
        assert_eq!(img.dimensions(), (w, h));
        let magenta = [255, 0, 255, 255];
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert_eq!(img.get_pixel(x, y).0, magenta, "corner ({x},{y})");
        }
        assert!(img.pixels().all(|p| p.0[3] == 255), "the frame is opaque");

        // The folder's own extent. Magenta has no green and every part of the folder has plenty,
        // so this skips the faint Lanczos ringing just outside the outline.
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for (x, y, p) in img.enumerate_pixels() {
            if p.0[1] > 24 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
        // Height-bound: 94% of the frame, centred, at the template's own aspect (994 x 937).
        let (fw, fh) = (x1 - x0 + 1, y1 - y0 + 1);
        assert!((fh as f32 - 0.94 * h as f32).abs() <= 3.0, "height {fh}");
        assert!(
            (fw as f32 / fh as f32 - 994.0 / 937.0).abs() < 0.01,
            "{fw}x{fh}"
        );
        assert!(
            (x0 as i32 - (w - x1 - 1) as i32).abs() <= 2,
            "centred: {x0}..{x1}"
        );
        assert!(
            (y0 as i32 - (h - y1 - 1) as i32).abs() <= 2,
            "centred: {y0}..{y1}"
        );

        // Grey front panel, cream paper strip, backdrop beside the tab. Canvas points map to the
        // frame through the folder's box.
        let at = |cx: f32, cy: f32| {
            let x = x0 as f32 + (cx - 15.0) / 994.0 * fw as f32;
            let y = y0 as f32 + (cy - 36.5) / 937.0 * fh as f32;
            img.get_pixel(x.round() as u32, y.round() as u32).0
        };
        let front = at(512.0, 600.0);
        assert!(
            front[..3].iter().all(|&c| (c as i32 - 0xCC).abs() <= 4),
            "{front:?}"
        );
        let paper = at(512.0, 146.0);
        assert!(
            paper[0] > 225 && paper[1] > 220 && paper[2] > 210,
            "{paper:?}"
        );
        assert_eq!(at(700.0, 60.0), magenta, "beside the tab");
    }

    #[test]
    fn default_artwork_renders_a_blue_folder() {
        let png = render_preview_png(&default_folder_artwork(), 128);
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (128, 128));
        let px = img.get_pixel(64, 75).0;
        assert!(px[2] > px[1] && px[1] > px[0] && px[3] == 255, "{px:?}");
    }
}
