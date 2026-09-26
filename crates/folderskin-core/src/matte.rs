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

use crate::backdrop::Backdrop;
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
pub const MIN_SUBJECT_SHARE: f32 = 0.02;

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
pub(crate) fn distance(px: &[u8; 3], key: &[u8; 3]) -> f32 {
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

/// How far a flat backdrop's pixels may stray from its colour: JPEG noise on a flat fill, and no
/// more.
pub const FLAT_TOLERANCE: f32 = 0.06;

/// How close to the measured backdrop colour a pixel must be to count as backdrop in
/// [`cutout_connected`]: tight, because the key comes from the picture itself.
pub const CONNECTED_TOLERANCE: f32 = 0.08;

/// How many pixels into the subject [`cutout_connected`] looks for colour mixed with the
/// backdrop. A model's anti-aliased edge is two or three pixels wide.
const EDGE_RINGS: u8 = 2;

/// A backdrop whose channels differ by no more than this (of 255) is neutral: black, grey or
/// white, with no hue of its own.
const NEUTRAL_CHROMA: u8 = 16;

/// The bounds of the tolerance [`cutout_connected`] measures for a neutral backdrop.
const NEUTRAL_TOLERANCE: std::ops::RangeInclusive<f32> = 0.02..=0.045;

/// Islands of the subject smaller than this, in pixels, left in a neutral backdrop are noise.
const SPECK: usize = 256;

/// Whether `key` is a neutral colour: black, grey or white.
fn is_neutral(key: [u8; 3]) -> bool {
    let (max, min) = (
        key.iter().max().unwrap_or(&0),
        key.iter().min().unwrap_or(&0),
    );
    max - min <= NEUTRAL_CHROMA
}

/// How far from a neutral `key` the backdrop's own pixels stray: two and a half times the 99th
/// percentile of the edge band's distance from it, within [`NEUTRAL_TOLERANCE`]. A flat JPEG
/// backdrop measures about 0.01, so this is near 0.025: far tighter than [`CONNECTED_TOLERANCE`],
/// which a navy suit on a dark grey backdrop would pass.
fn neutral_tolerance(img: &RgbaImage, key: [u8; 3]) -> f32 {
    let (w, h) = img.dimensions();
    let band = (w.min(h) / 100).max(1);
    let mut d: Vec<f32> = img
        .enumerate_pixels()
        .filter(|&(x, y, _)| x < band || y < band || x >= w - band || y >= h - band)
        .map(|(_, _, p)| distance(&[p.0[0], p.0[1], p.0[2]], &key))
        .collect();
    if d.is_empty() {
        return *NEUTRAL_TOLERANCE.start();
    }
    d.sort_by(f32::total_cmp);
    let p99 = d[(d.len() - 1) * 99 / 100];
    (p99 * 2.5).clamp(*NEUTRAL_TOLERANCE.start(), *NEUTRAL_TOLERANCE.end())
}

/// The colour of a flat backdrop around a picture, whatever colour it is, or `None`.
///
/// Image models asked for #FF00FF often paint a steady raspberry or hot pink instead, so a
/// maintainer tool can't count on the exact key. This takes the outer band (1% of the shorter
/// side deep, as [`surround`] does) and returns its median colour when at least 90% of the band
/// is opaque and within [`FLAT_TOLERANCE`] of it. A photo or painting reaching the edge varies
/// far more than that.
pub fn flat_backdrop(img: &RgbaImage) -> Option<[u8; 3]> {
    let (w, h) = img.dimensions();
    if w < 4 || h < 4 {
        return None;
    }
    let band = (w.min(h) / 100).max(1);
    let edge: Vec<[u8; 4]> = img
        .enumerate_pixels()
        .filter(|&(x, y, _)| x < band || y < band || x >= w - band || y >= h - band)
        .map(|(_, _, p)| p.0)
        .collect();
    let median = |c: usize| {
        let mut v: Vec<u8> = edge.iter().map(|p| p[c]).collect();
        v.sort_unstable();
        v[v.len() / 2]
    };
    let key = [median(0), median(1), median(2)];
    let flat = edge
        .iter()
        .filter(|p| p[3] > CLEAR_ALPHA && distance(&[p[0], p[1], p[2]], &key) <= FLAT_TOLERANCE)
        .count();
    (flat as f32 >= 0.9 * edge.len() as f32).then_some(key)
}

/// True when `px` is the key colour in shade, or lit a little brighter: the key scaled by a
/// brightness factor, give or take JPEG noise. Models draw a soft shadow under a folder even when
/// told not to, and on a raspberry backdrop that shadow is a darker raspberry, far from the key
/// by [`distance`] but on the same line from black.
fn in_key_shade(px: [u8; 3], key: [u8; 3]) -> bool {
    let p = px.map(f32::from);
    let k = key.map(f32::from);
    let kk = k[0] * k[0] + k[1] * k[1] + k[2] * k[2];
    if kk == 0.0 {
        return false;
    }
    let s = (p[0] * k[0] + p[1] * k[1] + p[2] * k[2]) / kk;
    let off = [p[0] - s * k[0], p[1] - s * k[1], p[2] - s * k[2]];
    let rms = ((off[0] * off[0] + off[1] * off[1] + off[2] * off[2]) / 3.0).sqrt();
    // Near black everything is close to the line, so very dark pixels don't count.
    (0.15..=1.3).contains(&s) && off[1].abs() <= 10.0 && rms <= 0.04 * 255.0
}

/// Cuts a subject out of a flat `key` backdrop, removing only the backdrop that reaches the
/// picture's edge.
///
/// [`cutout`] removes the key colour wherever it appears, which is safe for pure magenta but not
/// for a key a model drifted to: a raspberry backdrop sits close to the reds of a painting, and
/// those would come out as holes. Here the backdrop grows inward from the edge through pixels
/// within [`CONNECTED_TOLERANCE`] of the key or [`in_key_shade`], so a drop shadow goes with it,
/// and everything it can't reach stays. The pixels along the subject's edge are a blend of subject
/// and backdrop; each is split back into the two, using the backdrop and the subject colours
/// nearby, so the edge keeps the subject's colour with a soft alpha instead of a pink rim. Then
/// the result is trimmed like [`cutout`].
///
/// A neutral backdrop (black, grey, white) has no hue to follow into shade, and on a dark one a
/// subject's ink lines and dark clothes come within [`CONNECTED_TOLERANCE`]: grown the usual way,
/// the backdrop runs in wherever a dark coat meets the folder's bottom edge and eats the coat, the
/// hair and every outline joined to them. So there the backdrop is only what lies within its own
/// noise ([`neutral_tolerance`]), with no shade, and it grows sideways and down but never up. The
/// space under the subject is still reached, row by row from the sides, and a coat standing on
/// the bottom edge is not. Islands of the subject smaller than [`SPECK`] left in the backdrop,
/// JPEG noise the growth went around, go with it.
pub fn cutout_connected(img: &RgbaImage, key: [u8; 3]) -> RgbaImage {
    let (w, h) = img.dimensions();
    let at = |x: u32, y: u32| (y * w + x) as usize;
    let rgb = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        [p[0], p[1], p[2]]
    };
    let neutral = is_neutral(key);
    let tolerance = if neutral {
        neutral_tolerance(img, key)
    } else {
        CONNECTED_TOLERANCE
    };
    let is_backdrop = |x: u32, y: u32| {
        let p = rgb(x, y);
        distance(&p, &key) <= tolerance || (!neutral && in_key_shade(p, key))
    };

    // 0 for the backdrop; for the rest, how many pixels (8-connected) it is from the backdrop, up
    // to EDGE_RINGS + 1, and u8::MAX further in.
    let mut ring = vec![u8::MAX; (w * h) as usize];
    let mut queue = std::collections::VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            let on_edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            if on_edge && is_backdrop(x, y) {
                ring[at(x, y)] = 0;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        // On a neutral backdrop, never up: see above.
        let up = if neutral { h } else { y.wrapping_sub(1) };
        let neighbours = [(x.wrapping_sub(1), y), (x + 1, y), (x, up), (x, y + 1)];
        for (nx, ny) in neighbours {
            if nx < w && ny < h && ring[at(nx, ny)] == u8::MAX && is_backdrop(nx, ny) {
                ring[at(nx, ny)] = 0;
                queue.push_back((nx, ny));
            }
        }
    }
    if neutral {
        clear_specks(&mut ring, w, h);
    }
    for r in 1..=EDGE_RINGS + 1 {
        let previous = ring.clone();
        for y in 0..h {
            for x in 0..w {
                if previous[at(x, y)] != u8::MAX {
                    continue;
                }
                let touches = (y.saturating_sub(1)..=(y + 1).min(h - 1)).any(|ny| {
                    (x.saturating_sub(1)..=(x + 1).min(w - 1))
                        .any(|nx| previous[at(nx, ny)] == r - 1)
                });
                if touches {
                    ring[at(x, y)] = r;
                }
            }
        }
    }

    // How far around an edge pixel to look for the backdrop and the solid subject.
    const REACH: u32 = 4;
    let mut out = img.clone();
    for y in 0..h {
        for x in 0..w {
            let r = ring[at(x, y)];
            if r == 0 {
                out.get_pixel_mut(x, y).0[3] = 0;
                continue;
            }
            if r > EDGE_RINGS {
                continue;
            }
            let (mut back, mut backs) = ([0f32; 3], 0f32);
            let (mut fore, mut fores) = ([0f32; 3], 0f32);
            for ny in y.saturating_sub(REACH)..=(y + REACH).min(h - 1) {
                for nx in x.saturating_sub(REACH)..=(x + REACH).min(w - 1) {
                    let p = rgb(nx, ny).map(f32::from);
                    match ring[at(nx, ny)] {
                        0 => {
                            (0..3).for_each(|c| back[c] += p[c]);
                            backs += 1.0;
                        }
                        n if n > EDGE_RINGS => {
                            (0..3).for_each(|c| fore[c] += p[c]);
                            fores += 1.0;
                        }
                        _ => {}
                    }
                }
            }
            if backs == 0.0 || fores == 0.0 {
                continue;
            }
            let k = back.map(|v| v / backs);
            let f = fore.map(|v| v / fores);
            let v = [f[0] - k[0], f[1] - k[1], f[2] - k[2]];
            let vv = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            // A subject too close to the backdrop's colour to tell apart; leave it as it is.
            if vv < 25.0 * 25.0 {
                continue;
            }
            let c = rgb(x, y).map(f32::from);
            let a = (((c[0] - k[0]) * v[0] + (c[1] - k[1]) * v[1] + (c[2] - k[2]) * v[2]) / vv)
                .clamp(0.0, 1.0);
            // Mostly subject: remove the backdrop's share, which keeps the pixel's detail. Mostly
            // backdrop: that would magnify noise, so take the nearby subject's colour instead.
            let colour = if a >= 0.6 {
                [0, 1, 2].map(|i| (k[i] + (c[i] - k[i]) / a).clamp(0.0, 255.0))
            } else {
                f
            };
            let px = out.get_pixel_mut(x, y);
            for (channel, value) in px.0.iter_mut().zip(colour) {
                *channel = value.round() as u8;
            }
            px.0[3] = (f32::from(px.0[3]) * a).round() as u8;
        }
    }
    autocrop(&out, 0)
}

/// Marks as backdrop (0) every 4-connected island of the subject (`u8::MAX`) smaller than
/// [`SPECK`] pixels: noise in a neutral backdrop that its growth went around.
fn clear_specks(ring: &mut [u8], w: u32, h: u32) {
    let at = |x: u32, y: u32| (y * w + x) as usize;
    let mut seen = vec![false; ring.len()];
    let mut island = Vec::new();
    for start in 0..ring.len() {
        if seen[start] || ring[start] != u8::MAX {
            continue;
        }
        island.clear();
        seen[start] = true;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            island.push(i);
            let (x, y) = ((i % w as usize) as u32, (i / w as usize) as u32);
            let neighbours = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in neighbours {
                if nx < w && ny < h {
                    let n = at(nx, ny);
                    if !seen[n] && ring[n] == u8::MAX {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        if island.len() < SPECK {
            for &i in &island {
                ring[i] = 0;
            }
        }
    }
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

/// A pixel the mask gives less than this is left out altogether: haze around a lifted subject.
const MASK_CLEAR: u8 = 8;
/// A pixel the mask gives this much or more is the subject's own.
const MASK_SOLID: u8 = 250;
/// How far around a pixel on the mask's soft edge the subject's own colour is looked for.
const MASK_REACH: u32 = 4;

/// Cuts `img`'s subject out along `mask` (how much each pixel belongs to it, as
/// [`crate::lift::subject_mask`] gives it), trimmed to the subject.
///
/// Along the mask's soft edge a pixel's colour is part backdrop. That part is taken out, using the
/// backdrop there ([`Backdrop`] when the frame's edge is plain, else the backdrop pixels nearby)
/// and the subject's own colour just inside, so the edge keeps the subject's colours instead of a
/// lavender or pink rim. A pixel that is mostly backdrop takes the subject's colour: dividing by a
/// small share magnifies noise.
pub fn cut_by_mask(img: &RgbaImage, mask: &image::GrayImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let backdrop = Backdrop::measure(img).filter(|b| b.plain >= 0.9);
    let m = |x: u32, y: u32| mask.get_pixel(x, y).0[0];
    let rgb = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        [p[0], p[1], p[2]].map(f32::from)
    };
    // The mean colour of the pixels within `reach` of (x, y) that `keep` picks.
    let mean = |x: u32, y: u32, reach: u32, keep: &dyn Fn(u8) -> bool| {
        let (mut sum, mut n) = ([0f32; 3], 0f32);
        for ny in y.saturating_sub(reach)..=(y + reach).min(h - 1) {
            for nx in x.saturating_sub(reach)..=(x + reach).min(w - 1) {
                if keep(m(nx, ny)) {
                    let p = rgb(nx, ny);
                    (0..3).for_each(|c| sum[c] += p[c]);
                    n += 1.0;
                }
            }
        }
        (n > 0.0).then(|| sum.map(|v| v / n))
    };
    let mut out = img.clone();
    for (x, y, px) in out.enumerate_pixels_mut() {
        let a = m(x, y);
        if a < MASK_CLEAR {
            px.0[3] = 0;
            continue;
        }
        if a >= MASK_SOLID {
            continue;
        }
        let share = f32::from(a) / 255.0;
        let c = rgb(x, y);
        let fore = mean(x, y, MASK_REACH, &|v| v >= MASK_SOLID);
        let back = backdrop
            .as_ref()
            .map(|b| b.at(x, y).map(f32::from))
            .or_else(|| mean(x, y, 2 * MASK_REACH, &|v| v < MASK_CLEAR));
        let colour = match (fore, back) {
            (Some(f), Some(k))
                if (0..3).map(|i| (f[i] - k[i]).powi(2)).sum::<f32>() >= 25.0 * 25.0 =>
            {
                if share >= 0.5 {
                    // No further from the subject's colour than the pixel was.
                    [0, 1, 2].map(|i| {
                        (k[i] + (c[i] - k[i]) / share).clamp(c[i].min(f[i]), c[i].max(f[i]))
                    })
                } else {
                    f
                }
            }
            (Some(f), _) if share < 0.5 => f,
            _ => c,
        };
        px.0 = [
            colour[0].round() as u8,
            colour[1].round() as u8,
            colour[2].round() as u8,
            (f32::from(px.0[3]) * share).round() as u8,
        ];
    }
    autocrop(&out, 0)
}

/// True when `img` stands on a plain backdrop all round, drifted or not: then one subject in it
/// can be lifted off it, where a painting that fills the frame has none.
pub fn on_plain_backdrop(img: &RgbaImage) -> bool {
    Backdrop::measure(img).is_some_and(|b| b.plain >= 0.9)
}

/// The subject of `img` lifted off whatever it was painted on by the system ([`crate::lift`]),
/// cut out and trimmed; `None` where the system can't lift subjects, finds none, or finds one too
/// small to be the picture's ([`MIN_SUBJECT_SHARE`] of the frame).
pub fn lifted(img: &RgbaImage) -> Option<RgbaImage> {
    let mask = crate::lift::subject_mask(img)?;
    let solid = mask.pixels().filter(|p| p.0[0] >= 128).count();
    let frame = img.width() as f32 * img.height() as f32;
    (solid as f32 >= MIN_SUBJECT_SHARE * frame).then(|| cut_by_mask(img, &mask))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A green disc with a soft edge on a lavender sweep, and its coverage as a lifting model
    /// gives it.
    fn disc_on_lavender(size: u32) -> (RgbaImage, image::GrayImage) {
        let c = size as f32 / 2.0;
        let r = size as f32 * 0.3;
        let cover = |x: u32, y: u32| {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            ((r + 1.5 - d) / 3.0).clamp(0.0, 1.0)
        };
        let img = RgbaImage::from_fn(size, size, |x, y| {
            let a = cover(x, y);
            let back = [180.0 + 20.0 * x as f32 / size as f32, 150.0, 200.0];
            let fore = [30.0, 170.0, 60.0];
            let mix = |i: usize| (fore[i] * a + back[i] * (1.0 - a)).round() as u8;
            Rgba([mix(0), mix(1), mix(2), 255])
        });
        let mask = image::GrayImage::from_fn(size, size, |x, y| {
            image::Luma([(cover(x, y) * 255.0).round() as u8])
        });
        (img, mask)
    }

    #[test]
    fn a_masked_subject_keeps_its_own_colour_along_its_edge() {
        let (img, mask) = disc_on_lavender(120);
        let cut = cut_by_mask(&img, &mask);
        // Trimmed to the disc, 72 px across give or take its soft edge.
        assert!(
            (cut.width() as i32 - 75).abs() <= 3,
            "width {}",
            cut.width()
        );
        assert_eq!(cut.get_pixel(0, 0).0[3], 0, "the corner is clear");
        let (mut soft, mut tinted) = (0, 0);
        for p in cut.pixels().filter(|p| p.0[3] > 20 && p.0[3] < 235) {
            soft += 1;
            // Lavender left in a green edge shows as red and blue above green.
            if i32::from(p.0[0].max(p.0[2])) > i32::from(p.0[1]) {
                tinted += 1;
            }
        }
        assert!(soft > 100, "{soft} soft edge pixels");
        assert_eq!(tinted, 0, "{tinted} of {soft} edge pixels still lavender");
        let middle = cut.get_pixel(cut.width() / 2, cut.height() / 2).0;
        assert_eq!(middle, [30, 170, 60, 255]);
    }

    #[test]
    fn nothing_is_lifted_where_the_system_cant() {
        // Off macOS there is no lifting to ask; on a Mac a flat picture has no subject.
        let flat = RgbaImage::from_pixel(64, 64, Rgba([90, 90, 200, 255]));
        assert!(lifted(&flat).is_none());
    }

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

    /// What a chat model paints when asked for #FF00FF but drifting: a steady raspberry, with a
    /// crimson patch inside the subject that is nearly the backdrop's colour.
    fn raspberry_render() -> RgbaImage {
        RgbaImage::from_fn(200, 200, |x, y| {
            if !(40..160).contains(&x) || !(40..160).contains(&y) {
                jitter([189, 0, 103], x, y, 2)
            } else if (80..120).contains(&x) && (80..120).contains(&y) {
                Rgba([180, 12, 70, 255])
            } else {
                Rgba([40, 90, 180, 255])
            }
        })
    }

    #[test]
    fn a_drifted_flat_backdrop_is_found_and_a_scene_is_not() {
        let key = flat_backdrop(&raspberry_render()).expect("the raspberry backdrop");
        assert!(distance(&key, &[189, 0, 103]) < 0.02, "{key:?}");
        assert_eq!(
            surround(&raspberry_render(), MAGENTA),
            Surround::Opaque,
            "not magenta"
        );
        let scene = RgbaImage::from_fn(200, 150, |x, y| Rgba([(x + y) as u8, 90, 200, 255]));
        assert_eq!(flat_backdrop(&scene), None);
        let clear = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        assert_eq!(
            flat_backdrop(&clear),
            None,
            "transparency is not a backdrop colour"
        );
    }

    #[test]
    fn a_connected_cutout_keeps_the_key_colour_inside_the_subject() {
        let img = raspberry_render();
        let key = flat_backdrop(&img).unwrap();
        let cut = cutout_connected(&img, key);
        assert_eq!(cut.dimensions(), (120, 120), "trimmed to the subject");
        assert_eq!(cut.get_pixel(60, 60).0[3], 255, "the crimson patch stays");
        assert_eq!(cut.get_pixel(5, 5).0[3], 255, "the subject stays solid");
        // Keying the same colour everywhere would have punched the patch out.
        let global = key_out(&img, key, KeyOptions::default());
        assert!(global.get_pixel(100, 100).0[3] < 255);
        assert_eq!(global.get_pixel(0, 0).0[3], 0);
    }

    #[test]
    fn a_drop_shadow_on_the_backdrop_goes_with_it() {
        // A soft shadow under the subject: the backdrop's own colour, darker towards the subject.
        let key = [203u8, 2, 134];
        let img = RgbaImage::from_fn(200, 200, |x, y| {
            let shade = |s: f32| key.map(|c| (f32::from(c) * s).round() as u8);
            let [r, g, b] = if (40..160).contains(&x) && (40..150).contains(&y) {
                [150, 140, 120]
            } else if (40..160).contains(&x) && (150..168).contains(&y) {
                shade(0.2 + 0.8 * (y - 150) as f32 / 18.0)
            } else {
                key
            };
            Rgba([r, g, b, 255])
        });
        let cut = cutout_connected(&img, flat_backdrop(&img).unwrap());
        assert_eq!(
            cut.dimensions(),
            (120, 110),
            "the shadow isn't part of the subject"
        );
        assert!(
            cut.pixels().all(|p| p.0[1] > 100 || p.0[3] == 0),
            "nothing pink is left"
        );
    }

    #[test]
    fn an_edge_blended_with_the_backdrop_keeps_the_subjects_colour() {
        // A blue square whose outermost column is half blue, half raspberry, as anti-aliasing
        // leaves it.
        let key = [189u8, 0, 103];
        let blue = [40u8, 90, 180];
        let half = [0, 1, 2].map(|i| ((u16::from(key[i]) + u16::from(blue[i])) / 2) as u8);
        let img = RgbaImage::from_fn(200, 200, |x, y| {
            let inside = (40..160).contains(&y);
            let [r, g, b] = match x {
                39 if inside => half,
                40..=159 if inside => blue,
                _ => key,
            };
            Rgba([r, g, b, 255])
        });
        let cut = cutout_connected(&img, key);
        assert_eq!(cut.dimensions(), (121, 120));
        let rim = cut.get_pixel(0, 60).0;
        assert!((110..=145).contains(&rim[3]), "half transparent: {rim:?}");
        let off = [0, 1, 2].map(|i| (i16::from(rim[i]) - i16::from(blue[i])).abs());
        assert!(off.iter().all(|&d| d <= 6), "blue, not pink: {rim:?}");
    }

    /// A pop-art folder on a flat dark grey backdrop, as a batch of renders came: a blue panel,
    /// a dark coat standing on its bottom edge that is as good as the backdrop's colour, and a
    /// near-black ink line running to the panel's left edge. Plus a speck of JPEG noise.
    fn dark_backdrop_render() -> RgbaImage {
        RgbaImage::from_fn(200, 200, |x, y| {
            let folder = (30..170).contains(&x) && (30..170).contains(&y);
            if !folder {
                return if (10..12).contains(&x) && (185..187).contains(&y) {
                    Rgba([62, 62, 62, 255])
                } else {
                    jitter([31, 31, 31], x, y, 2)
                };
            }
            if (70..130).contains(&x) && (120..170).contains(&y) {
                Rgba([28, 28, 30, 255]) // the coat
            } else if y == 100 && x < 130 {
                Rgba([8, 8, 8, 255]) // the ink line
            } else {
                Rgba([40, 90, 180, 255])
            }
        })
    }

    #[test]
    fn a_neutral_backdrop_doesnt_run_into_a_dark_subject() {
        let img = dark_backdrop_render();
        let key = flat_backdrop(&img).expect("the grey backdrop");
        assert!(is_neutral(key), "{key:?}");
        let cut = cutout_connected(&img, key);
        assert_eq!(cut.dimensions(), (140, 140), "just the folder, speck gone");
        assert_eq!(cut.get_pixel(70, 120).0[3], 255, "the coat stays");
        assert_eq!(cut.get_pixel(70, 139).0[3], 255, "down to the bottom edge");
        assert_eq!(cut.get_pixel(20, 70).0[3], 255, "the ink line stays");
        assert_eq!(cut.get_pixel(5, 5).0[3], 255, "the panel is solid");
    }

    #[test]
    fn a_neutral_backdrop_is_measured_tightly_and_a_coloured_one_is_not() {
        let img = dark_backdrop_render();
        let tight = neutral_tolerance(&img, [31, 31, 31]);
        assert!(NEUTRAL_TOLERANCE.contains(&tight), "{tight}");
        assert!(tight < CONNECTED_TOLERANCE / 2.0);
        assert!(
            !is_neutral([189, 0, 103]),
            "raspberry keeps its shade and tolerance"
        );
        assert!(is_neutral([255, 255, 250]), "near white is neutral too");
    }

    #[test]
    fn tiny_pictures_do_not_trip_the_edge_scan() {
        for (w, h) in [(0, 0), (1, 1), (2, 1), (3, 3)] {
            let img = RgbaImage::from_pixel(w, h, Rgba([10, 20, 30, 255]));
            assert_eq!(surround(&img, MAGENTA), Surround::Opaque, "{w}x{h}");
        }
    }
}
