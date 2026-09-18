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
//!
//! [`finished_cutout`] makes the same decision for a picture the user brings in: a folder a chat
//! assistant painted on magenta (or on real transparency) is already the icon, while an ordinary
//! picture belongs on FolderSkin's template.

use image::RgbaImage;

/// The key colour FolderSkin asks models for: pure magenta.
pub const MAGENTA: [u8; 3] = [255, 0, 255];

/// Alpha at or below which a pixel counts as see-through when judging what surrounds a picture.
pub const CLEAR_ALPHA: u8 = 16;

/// Key tolerance for deciding whether a picture the user brought in sits on the key colour.
///
/// Stricter than [`KeyOptions::default`], which keys a render the app already knows was asked for
/// the key colour. This one has to tell such a render apart from a photo of something on a pink or
/// purple backdrop. #FF00FF from an image model lands within about 0.05, and a slightly lighter,
/// darker or bluer magenta within 0.11; magenta paper photographed in a studio sits at 0.14 and
/// beyond, and a vivid sunset sky around 0.14 too.
pub const DETECT_TOLERANCE: f32 = 0.12;

/// Share of the picture the keyed-out subject must cover, so an all-magenta picture is not "cut
/// out" to nothing.
const MIN_SUBJECT_SHARE: f32 = 0.02;

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

/// What surrounds the subject of a picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surround {
    /// Real transparency: the picture is already a cut-out.
    Transparent,
    /// A flat backdrop of the key colour, ready to be keyed out.
    Keyed,
    /// Ordinary picture content reaches the edges.
    Opaque,
}

/// How much of a picture's edge satisfies `pred`.
///
/// Returns the share of an outer band one percent of the shorter side deep (at least one pixel),
/// and whether each of the four corner squares of that size satisfies it almost everywhere (90%).
/// The corners matter for a cut-out trimmed to its own outline: its edges are mostly subject, but
/// a folder's rounded corners and the gap beside its tab never reach the corners of the frame.
fn edge_share(img: &RgbaImage, pred: impl Fn(&[u8; 4]) -> bool) -> (f32, bool) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return (0.0, false);
    }
    let band = (w.min(h) / 100).max(1);
    let (mut hits, mut total) = (0u64, 0u64);
    let mut count = |x: u32, y: u32| {
        total += 1;
        if pred(&img.get_pixel(x, y).0) {
            hits += 1;
        }
    };
    for y in 0..h {
        if y < band || y >= h - band || w <= 2 * band {
            (0..w).for_each(|x| count(x, y));
        } else {
            (0..band).chain(w - band..w).for_each(|x| count(x, y));
        }
    }
    let corner = |x0: u32, y0: u32| {
        let inside = (y0..y0 + band)
            .flat_map(|y| (x0..x0 + band).map(move |x| (x, y)))
            .filter(|&(x, y)| pred(&img.get_pixel(x, y).0))
            .count();
        inside as f32 >= 0.9 * (band * band) as f32
    };
    let corners =
        corner(0, 0) && corner(w - band, 0) && corner(0, h - band) && corner(w - band, h - band);
    (hits as f32 / total as f32, corners)
}

/// Reads what surrounds a picture: real transparency, the flat `key` colour, or neither.
///
/// * **Transparent** when at least half of the outer band has alpha at or below
///   [`CLEAR_ALPHA`], or when all four corners do and at least a fifth of the band does (a
///   cut-out trimmed tight to its outline).
/// * **Keyed** when [`has_key_background`] holds at the strict [`DETECT_TOLERANCE`] (60% of the
///   border on the key colour), or when all four corners and a fifth of the band are on it.
/// * **Opaque** otherwise: a photo, a painting, a screenshot.
pub fn surround(img: &RgbaImage, key: [u8; 3]) -> Surround {
    let (clear, clear_corners) = edge_share(img, |p| p[3] <= CLEAR_ALPHA);
    if clear >= 0.5 || (clear >= 0.2 && clear_corners) {
        return Surround::Transparent;
    }
    let strict = KeyOptions {
        tolerance: DETECT_TOLERANCE,
        ..KeyOptions::default()
    };
    let (keyed, key_corners) = edge_share(img, |p| {
        p[3] > CLEAR_ALPHA && distance(&[p[0], p[1], p[2]], &key) <= DETECT_TOLERANCE
    });
    if has_key_background(img, key, strict) || (keyed >= 0.2 && key_corners) {
        Surround::Keyed
    } else {
        Surround::Opaque
    }
}

/// The picture as a finished folder image, trimmed to its subject, or `None` when it is an
/// ordinary picture that belongs on the template.
///
/// A picture with real transparency around it is trimmed as it is. A picture on the flat `key`
/// colour is keyed out, despilled and trimmed exactly like a keyed render from the assistant, as
/// long as something substantial is left ([`MIN_SUBJECT_SHARE`] of the frame). A picture with no
/// visible pixel at all has no subject and returns `None`.
pub fn finished_cutout(img: &RgbaImage, key: [u8; 3]) -> Option<RgbaImage> {
    match surround(img, key) {
        Surround::Transparent => alpha_bounds(img, 8).map(|_| autocrop(img, 0)),
        Surround::Keyed => {
            let cut = cutout(img, key, KeyOptions::default());
            let solid = cut.pixels().filter(|p| p.0[3] >= 128).count() as f32;
            let frame = img.width() as f32 * img.height() as f32;
            (solid >= MIN_SUBJECT_SHARE * frame).then_some(cut)
        }
        Surround::Opaque => None,
    }
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

    // ---------- telling finished folder images from pictures ----------

    /// Deterministic noise in `-amplitude..=amplitude`, so the synthetic photos never flake.
    fn noise(x: u32, y: u32, amplitude: i32) -> i32 {
        let mut n = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        ((n >> 16) % (2 * amplitude as u32 + 1)) as i32 - amplitude
    }

    fn jitter(c: [u8; 3], x: u32, y: u32, amplitude: i32) -> Rgba<u8> {
        let j = |v: u8, salt: u32| {
            (v as i32 + noise(x + salt, y + 7 * salt, amplitude)).clamp(0, 255) as u8
        };
        Rgba([j(c[0], 0), j(c[1], 101), j(c[2], 211), 255])
    }

    fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
        let m = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * t).round() as u8;
        [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
    }

    /// Inside a rectangle with rounded corners of radius `r`.
    fn in_rounded(x: u32, y: u32, (x0, y0, x1, y1): (u32, u32, u32, u32), r: u32) -> bool {
        if x < x0 || x >= x1 || y < y0 || y >= y1 {
            return false;
        }
        let (fx, fy, r) = (x as f32 + 0.5, y as f32 + 0.5, r as f32);
        let cx = fx.clamp(x0 as f32 + r, x1 as f32 - r);
        let cy = fy.clamp(y0 as f32 + r, y1 as f32 - r);
        (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r
    }

    /// Inside a folder-like silhouette filling `rect`: a rounded body with one rounded tab on its
    /// top left, so the frame's top right beside the tab is empty, as on a real folder.
    fn in_folder(x: u32, y: u32, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> bool {
        let (w, h) = (x1 - x0, y1 - y0);
        let r = w / 12;
        let body = (x0, y0 + h / 8, x1, y1);
        let tab = (x0 + w / 20, y0, x0 + w * 2 / 5, y0 + h / 4);
        in_rounded(x, y, body, r) || in_rounded(x, y, tab, r / 2)
    }

    /// A folder painted `fill` on `background`, the folder filling `rect`.
    fn folder_on(size: (u32, u32), rect: (u32, u32, u32, u32), background: Rgba<u8>) -> RgbaImage {
        RgbaImage::from_fn(size.0, size.1, |x, y| {
            if in_folder(x, y, rect) {
                jitter([40, 120, 200], x, y, 6)
            } else {
                background
            }
        })
    }

    #[test]
    fn a_cutout_with_transparent_margins_reads_as_transparent() {
        let img = folder_on((240, 220), (20, 24, 220, 200), Rgba([0, 0, 0, 0]));
        assert_eq!(surround(&img, MAGENTA), Surround::Transparent);
        let cut = finished_cutout(&img, MAGENTA).expect("a folder with alpha is finished");
        assert_eq!(cut.dimensions(), (200, 176), "trimmed to the folder");
    }

    #[test]
    fn a_cutout_trimmed_to_its_outline_still_reads_as_transparent() {
        // The folder touches all four edges; only its corners and the gap beside the tab are clear.
        let img = folder_on((200, 180), (0, 0, 200, 180), Rgba([0, 0, 0, 0]));
        let (share, corners) = edge_share(&img, |p| p[3] <= CLEAR_ALPHA);
        assert!(share < 0.5 && corners, "share {share}, corners {corners}");
        assert_eq!(surround(&img, MAGENTA), Surround::Transparent);
        assert!(finished_cutout(&img, MAGENTA).is_some());
    }

    #[test]
    fn a_folder_on_magenta_reads_as_keyed_and_is_cut_out() {
        let backdrop = |x, y| jitter(MAGENTA, x, y, 3);
        let img = RgbaImage::from_fn(240, 220, |x, y| {
            if in_folder(x, y, (30, 30, 210, 196)) {
                jitter([40, 120, 200], x, y, 6)
            } else {
                backdrop(x, y)
            }
        });
        assert_eq!(surround(&img, MAGENTA), Surround::Keyed);
        let cut = finished_cutout(&img, MAGENTA).expect("a keyed folder is finished");
        let (w, h) = cut.dimensions();
        assert!(
            (w as i32 - 180).abs() <= 2 && (h as i32 - 166).abs() <= 2,
            "{w}x{h}"
        );
        assert_eq!(
            cut.get_pixel(w - 1, 0).0[3],
            0,
            "beside the tab is keyed out"
        );
    }

    #[test]
    fn a_keyed_folder_survives_a_jpeg_round_trip() {
        // Grok hands its images back as JPEG; compression noise must not break detection.
        let img = folder_on((240, 220), (30, 30, 210, 196), Rgba([255, 0, 255, 255]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
            .encode_image(&image::DynamicImage::ImageRgba8(img).to_rgb8())
            .unwrap();
        let back = image::load_from_memory(&jpeg).unwrap().to_rgba8();
        assert_eq!(surround(&back, MAGENTA), Surround::Keyed);
        let cut = finished_cutout(&back, MAGENTA).unwrap();
        let (w, h) = cut.dimensions();
        assert!(
            (w as i32 - 180).abs() <= 3 && (h as i32 - 166).abs() <= 3,
            "{w}x{h}"
        );
    }

    #[test]
    fn a_slightly_off_magenta_backdrop_still_counts_as_the_key() {
        let img = folder_on((240, 220), (30, 30, 210, 196), Rgba([244, 14, 236, 255]));
        assert_eq!(surround(&img, MAGENTA), Surround::Keyed);
    }

    #[test]
    fn an_ordinary_photo_is_a_picture() {
        let img = RgbaImage::from_fn(240, 180, |x, y| {
            if y < 100 {
                jitter(
                    mix([110, 170, 230], [190, 215, 240], y as f32 / 100.0),
                    x,
                    y,
                    5,
                )
            } else {
                jitter([70, 110, 50], x, y, 12)
            }
        });
        assert_eq!(surround(&img, MAGENTA), Surround::Opaque);
        assert!(finished_cutout(&img, MAGENTA).is_none());
    }

    #[test]
    fn a_photo_with_a_pink_sky_is_a_picture() {
        // A vivid sunset: magenta at the top of the sky, close enough for the keyer's own
        // tolerance, fading to pink at the horizon, over dark land.
        let img = RgbaImage::from_fn(240, 180, |x, y| {
            if y < 125 {
                jitter(
                    mix([240, 40, 215], [252, 160, 185], y as f32 / 125.0),
                    x,
                    y,
                    5,
                )
            } else {
                jitter([45, 32, 58], x, y, 8)
            }
        });
        assert_eq!(surround(&img, MAGENTA), Surround::Opaque);
        assert!(finished_cutout(&img, MAGENTA).is_none());
    }

    #[test]
    fn a_product_shot_on_a_magenta_backdrop_is_a_picture() {
        // A box photographed on magenta paper: the paper is near enough the key for the keyer's
        // own tolerance, which is why import detection uses a stricter one.
        let img = RgbaImage::from_fn(240, 200, |x, y| {
            let paper = mix([234, 50, 208], [222, 38, 192], y as f32 / 200.0);
            if (80..160).contains(&x) && (50..150).contains(&y) {
                jitter([128, 128, 134], x, y, 6)
            } else {
                jitter(paper, x, y, 3)
            }
        });
        assert!(has_key_background(&img, MAGENTA, KeyOptions::default()));
        assert_eq!(surround(&img, MAGENTA), Surround::Opaque);
        assert!(finished_cutout(&img, MAGENTA).is_none());
    }

    #[test]
    fn a_picture_with_nothing_on_the_backdrop_has_no_subject() {
        let magenta = RgbaImage::from_pixel(64, 64, Rgba([255, 0, 255, 255]));
        assert_eq!(surround(&magenta, MAGENTA), Surround::Keyed);
        assert!(finished_cutout(&magenta, MAGENTA).is_none());
        let clear = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        assert!(finished_cutout(&clear, MAGENTA).is_none());
    }

    #[test]
    fn tiny_pictures_do_not_trip_the_edge_scan() {
        for (w, h) in [(0, 0), (1, 1), (2, 1), (3, 3)] {
            let img = RgbaImage::from_pixel(w, h, Rgba([10, 20, 30, 255]));
            assert_eq!(surround(&img, MAGENTA), Surround::Opaque, "{w}x{h}");
        }
    }
}
