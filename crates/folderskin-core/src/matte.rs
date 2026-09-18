//! Turning a generated image into a clean cutout.
//!
//! Image models that cannot return a real alpha channel are asked to render the subject on a
//! flat key colour (magenta by default, because it almost never appears in folder art). This
//! module removes that colour, cleans the fringe it leaves behind, and trims the result to the
//! subject.
//!
//! The three steps matter in this order:
//!
//! 1. [`key_out`] turns key-coloured pixels transparent and gives the boundary a soft edge.
//! 2. [`despill`] removes the key colour that bled into the subject's own edge pixels, which is
//!    what makes a naive chroma key look like it has a coloured halo.
//! 3. [`autocrop`] trims the transparent margin so the subject fills the frame.

use image::RgbaImage;

/// The key colour FolderSkin asks models for: pure magenta.
pub const MAGENTA: [u8; 3] = [255, 0, 255];

/// How close a pixel must be to the key colour to count as background, and how wide the soft
/// edge between background and subject is. Both are distances in 0..=1 chroma space.
#[derive(Clone, Copy, Debug)]
pub struct KeyOptions {
    /// Pixels at or below this distance from the key colour become fully transparent.
    pub tolerance: f32,
    /// Pixels between `tolerance` and `tolerance + feather` fade in.
    pub feather: f32,
}

impl Default for KeyOptions {
    fn default() -> Self {
        Self {
            tolerance: 0.18,
            feather: 0.14,
        }
    }
}

/// Distance from `px` to `key` in 0..=1, weighted so hue matters more than brightness.
///
/// A flat magenta backdrop that the model rendered slightly unevenly still keys out, while
/// genuinely magenta *subject* pixels (a pink balloon, a neon sign) survive because their
/// green channel differs.
fn distance(px: &[u8; 3], key: &[u8; 3]) -> f32 {
    let d = |a: u8, b: u8| (a as f32 - b as f32) / 255.0;
    let (dr, dg, db) = (d(px[0], key[0]), d(px[1], key[1]), d(px[2], key[2]));
    // Green carries the most information for a magenta key (and red for a green key), so weight
    // the channel the key is missing most heavily.
    let weights = if key[1] < key[0] && key[1] < key[2] {
        (0.5, 1.0, 0.5) // magenta key: green is the discriminator
    } else if key[0] < key[1] && key[0] < key[2] {
        (1.0, 0.5, 0.5) // green/cyan key
    } else {
        (0.7, 0.7, 0.7)
    };
    ((dr * dr * weights.0 + dg * dg * weights.1 + db * db * weights.2)
        / (weights.0 + weights.1 + weights.2))
        .sqrt()
}

/// Fraction of the image's border pixels that sit on the key colour.
///
/// The app uses this to tell "the model ignored the background instruction" (a useless render,
/// worth retrying) apart from "the key worked".
pub fn key_coverage(img: &RgbaImage, key: [u8; 3], opts: KeyOptions) -> f32 {
    let (w, h) = img.dimensions();
    if w < 2 || h < 2 {
        return 0.0;
    }
    let mut on_key = 0u32;
    let mut total = 0u32;
    let mut sample = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        if distance(&[p[0], p[1], p[2]], &key) <= opts.tolerance {
            on_key += 1;
        }
        total += 1;
    };
    for x in 0..w {
        sample(x, 0);
        sample(x, h - 1);
    }
    for y in 1..h - 1 {
        sample(0, y);
        sample(w - 1, y);
    }
    on_key as f32 / total.max(1) as f32
}

/// True when enough of the border is the key colour for keying to be worth doing.
pub fn has_key_background(img: &RgbaImage, key: [u8; 3], opts: KeyOptions) -> bool {
    key_coverage(img, key, opts) >= 0.6
}

/// Replaces the key colour with transparency, feathering the boundary.
pub fn key_out(img: &RgbaImage, key: [u8; 3], opts: KeyOptions) -> RgbaImage {
    let mut out = img.clone();
    for px in out.pixels_mut() {
        let d = distance(&[px.0[0], px.0[1], px.0[2]], &key);
        let alpha = if d <= opts.tolerance {
            0.0
        } else if d >= opts.tolerance + opts.feather {
            1.0
        } else {
            (d - opts.tolerance) / opts.feather
        };
        px.0[3] = (px.0[3] as f32 * alpha).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Removes key colour that bled into semi-transparent edge pixels.
///
/// For a magenta key this pulls red and blue down towards green wherever both exceed it, which
/// is exactly the fringe a magenta backdrop leaves. Fully opaque interior pixels are untouched,
/// so magenta *inside* the artwork survives.
pub fn despill(img: &mut RgbaImage, key: [u8; 3]) {
    let magenta_like = key[1] < key[0] && key[1] < key[2];
    let green_like = key[0] < key[1] && key[0] < key[2];
    for px in img.pixels_mut() {
        if px.0[3] == 0 || px.0[3] == 255 {
            continue; // only the soft rim spills
        }
        let [r, g, b, _] = px.0;
        if magenta_like {
            let cap = g.saturating_add(((255 - g) as f32 * 0.35) as u8);
            px.0[0] = r.min(cap.max(g));
            px.0[2] = b.min(cap.max(g));
        } else if green_like {
            let cap = r.max(b);
            px.0[1] = g.min(cap);
        }
    }
}

/// The bounding box of everything at least `threshold` opaque, or `None` when the image is empty.
pub fn alpha_bounds(img: &RgbaImage, threshold: u8) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, px) in img.enumerate_pixels() {
        if px.0[3] >= threshold {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    (x0 <= x1 && y0 <= y1).then_some((x0, y0, x1, y1))
}

/// Trims transparent margins, keeping `pad` pixels of breathing room.
pub fn autocrop(img: &RgbaImage, pad: u32) -> RgbaImage {
    let Some((x0, y0, x1, y1)) = alpha_bounds(img, 8) else {
        return img.clone();
    };
    let (w, h) = img.dimensions();
    let x0 = x0.saturating_sub(pad);
    let y0 = y0.saturating_sub(pad);
    let x1 = (x1 + pad).min(w - 1);
    let y1 = (y1 + pad).min(h - 1);
    image::imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

/// Key out, despill and trim in one call.
pub fn cutout(img: &RgbaImage, key: [u8; 3], opts: KeyOptions) -> RgbaImage {
    let mut keyed = key_out(img, key, opts);
    despill(&mut keyed, key);
    autocrop(&keyed, 0)
}

/// Fits an opaque generated image to the skin format by cropping to the target aspect around a
/// focus point. Used when the model returned artwork rather than a cut-out folder.
pub fn crop_to_aspect(
    img: &RgbaImage,
    target_w: u32,
    target_h: u32,
    focus: (f32, f32),
) -> RgbaImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let target = target_w as f32 / target_h as f32;
    let (cw, ch) = if w / h > target {
        (h * target, h)
    } else {
        (w, w / target)
    };
    let x0 = ((w - cw) * focus.0.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, w - cw) as u32;
    let y0 = ((h - ch) * focus.1.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, h - ch) as u32;
    let cropped =
        image::imageops::crop_imm(img, x0, y0, cw.round() as u32, ch.round() as u32).to_image();
    image::imageops::resize(
        &cropped,
        target_w,
        target_h,
        image::imageops::FilterType::Lanczos3,
    )
}

/// Flattens any transparency onto `bg`, for models that need an opaque reference image.
pub fn flatten(img: &RgbaImage, bg: [u8; 3]) -> RgbaImage {
    let mut out = img.clone();
    for px in out.pixels_mut() {
        let a = px.0[3] as f32 / 255.0;
        for (channel, &under) in px.0.iter_mut().zip(bg.iter()) {
            *channel = (*channel as f32 * a + under as f32 * (1.0 - a)).round() as u8;
        }
        px.0[3] = 255;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A magenta field with an opaque green square in the middle.
    fn plate(size: u32, subject: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(size, size, Rgba([255, 0, 255, 255]));
        for y in size / 4..size * 3 / 4 {
            for x in size / 4..size * 3 / 4 {
                img.put_pixel(x, y, subject);
            }
        }
        img
    }

    #[test]
    fn key_out_removes_the_backdrop_and_keeps_the_subject() {
        let img = plate(40, Rgba([20, 160, 70, 255]));
        let out = key_out(&img, MAGENTA, KeyOptions::default());
        assert_eq!(out.get_pixel(0, 0).0[3], 0, "corner should be transparent");
        assert_eq!(
            out.get_pixel(20, 20).0[3],
            255,
            "subject should stay opaque"
        );
    }

    #[test]
    fn a_pink_subject_survives_a_magenta_key() {
        // Pink has a much higher green channel than the key, so it must not key out.
        let img = plate(40, Rgba([255, 150, 220, 255]));
        let out = key_out(&img, MAGENTA, KeyOptions::default());
        assert_eq!(out.get_pixel(20, 20).0[3], 255);
        assert_eq!(out.get_pixel(0, 0).0[3], 0);
    }

    #[test]
    fn coverage_distinguishes_a_keyed_render_from_a_scene() {
        let keyed = plate(40, Rgba([20, 160, 70, 255]));
        assert!(has_key_background(&keyed, MAGENTA, KeyOptions::default()));
        let scene = RgbaImage::from_pixel(40, 40, Rgba([30, 90, 140, 255]));
        assert!(!has_key_background(&scene, MAGENTA, KeyOptions::default()));
        assert!(key_coverage(&scene, MAGENTA, KeyOptions::default()) < 0.01);
    }

    #[test]
    fn despill_pulls_magenta_out_of_the_soft_rim() {
        let mut img = RgbaImage::from_pixel(1, 1, Rgba([230, 90, 210, 128]));
        despill(&mut img, MAGENTA);
        let px = img.get_pixel(0, 0).0;
        assert!(
            px[0] < 230 && px[2] < 210,
            "red and blue should come down: {px:?}"
        );
        assert_eq!(px[1], 90, "green is the reference and must not move");
    }

    #[test]
    fn despill_leaves_opaque_interior_pixels_alone() {
        let mut img = RgbaImage::from_pixel(1, 1, Rgba([230, 90, 210, 255]));
        despill(&mut img, MAGENTA);
        assert_eq!(img.get_pixel(0, 0).0, [230, 90, 210, 255]);
    }

    #[test]
    fn autocrop_trims_to_the_subject() {
        let img = plate(40, Rgba([20, 160, 70, 255]));
        let cut = cutout(&img, MAGENTA, KeyOptions::default());
        // The subject is the middle half, 20x20, give or take the feathered edge.
        assert!(
            (cut.width() as i32 - 20).abs() <= 2,
            "width {}",
            cut.width()
        );
        assert!(
            (cut.height() as i32 - 20).abs() <= 2,
            "height {}",
            cut.height()
        );
    }

    #[test]
    fn autocrop_on_a_fully_transparent_image_is_a_no_op() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        assert_eq!(autocrop(&img, 0).dimensions(), (8, 8));
        assert!(alpha_bounds(&img, 8).is_none());
    }

    #[test]
    fn crop_to_aspect_honours_the_focus_point() {
        let mut src = RgbaImage::from_pixel(200, 100, Rgba([0, 0, 0, 255]));
        for x in 150..200 {
            for y in 0..100 {
                src.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let red_share = |img: &RgbaImage| {
            let n = img.pixels().filter(|p| p.0[0] > 200).count();
            n as f32 / (img.width() * img.height()) as f32
        };
        let left = crop_to_aspect(&src, 64, 60, (0.0, 0.5));
        let right = crop_to_aspect(&src, 64, 60, (1.0, 0.5));
        assert_eq!(left.dimensions(), (64, 60));
        // Focus 0 keeps the black left end of the source; focus 1 keeps the red right end.
        assert!(
            red_share(&left) < 0.01,
            "left crop should hold no red: {}",
            red_share(&left)
        );
        assert!(
            red_share(&right) > 0.4,
            "right crop should be mostly red: {}",
            red_share(&right)
        );
    }

    #[test]
    fn flatten_composites_onto_the_given_colour() {
        let img = RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 0]));
        let out = flatten(&img, [10, 20, 30]);
        assert_eq!(out.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }
}
