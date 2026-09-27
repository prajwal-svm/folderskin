//! Small icon sizes: the 16, 24, 32, 48 and 64 px icons a file manager shows in its lists, such as
//! Finder's list and column views and Explorer's details view.
//!
//! Shrunk from a 2048 px master by Lanczos3 alone, a small icon comes out soft in two ways a
//! hand-drawn one doesn't. An edge of its outline that doesn't fall on a whole pixel leaves a row
//! or a column of pixels partly covered, a pale band against whatever is behind the icon. And a
//! pale line runs just inside the outline: Lanczos3 overshoots at a hard edge, and the overshoot is
//! clipped in alpha but not in the colour that goes with it, so those pixels come out lighter than
//! the icon itself.
//!
//! So each small size is finished on its own ([`shrink`]), from the same master as every other
//! size, whatever made that master: a folder, a drive, a design or a finished picture.
//!
//! 1. Lanczos3, with the weights the `image` crate uses, is kept in `f32` and unclipped, so where
//!    alpha overshoots, its colour is taken back with it and keeps the icon's own shade.
//! 2. An edge of the outline that runs straight along a row or a column is moved onto a whole
//!    pixel: a pixel it covers at least half of fills in, one it covers less than 30% of clears,
//!    and one in between moves part of the way. Curves and slants keep their smoothing, and so does
//!    alpha that is see-through on purpose.
//! 3. Inside the icon, where every pixel is opaque, lightness is sharpened a little against its
//!    neighbours, never past the lightest or the darkest of them, so a photo or lettering gets
//!    crisper without a halo.
//!
//! Past the downsample every size costs anyway, it is a few sums over at most 64 × 64 pixels, and
//! it comes out the same every time. Sizes above [`LARGEST`] are the plain downsample, as ever.

use crate::raster::Premul;

/// The largest size finished as a small one. Every size above it is the plain Lanczos3 downsample.
pub const LARGEST: u32 = 64;

/// Below this share of a pixel, a straight edge of the outline clears the pixel.
const CLEARS_BELOW: f32 = 0.3;
/// From this share of a pixel on, a straight edge of the outline fills the pixel in. Between the
/// two it moves part of the way, and the middle of that is under a half, so an edge that covers
/// about half a pixel keeps it: a thin tab or strip along the top of a folder isn't lost at 16 px.
const FILLS_FROM: f32 = 0.5;
/// How far apart, in alpha, the pixels either side of an edge start to make it one worth moving
/// onto a whole pixel, and how far apart they are when it is one entirely.
const APART: (f32, f32) = (0.5, 0.9);
/// How much the pixels along an edge may differ from the one on it, the two differences added,
/// for the edge still to run straight, and how much makes it a curve or a slant, left alone. The
/// first leaves room for the ripple Lanczos3 spreads from an edge a couple of pixels away.
const SLANT: (f32, f32) = (0.1, 0.5);
/// Alpha this close to clear or to opaque is taken as clear or opaque for the sides of an edge:
/// Lanczos3 rings about that much either side of a hard one.
const RINGING: f32 = 0.03;
/// How much lightness inside the icon is sharpened: its difference from its neighbours' mean is
/// added once more.
const SHARPEN: f32 = 1.0;
/// The alpha from which a pixel is opaque enough to be sharpened.
const OPAQUE: f32 = 0.999;
/// How much each of three pixels in a row counts in the mean lightness is sharpened against.
const MEAN: [f32; 3] = [0.25, 0.5, 0.25];

/// `src` shrunk to `size` × `size` px and finished as a small icon, premultiplied: see the
/// module's documentation. It works at any size, though only sizes up to [`LARGEST`] are made
/// this way.
pub fn shrink(src: &Premul, size: u32) -> Premul {
    let side = size as usize;
    if src.width == 0 || src.height == 0 || size == 0 {
        return Premul {
            width: size,
            height: size,
            data: vec![0; side * side * 4],
        };
    }
    let mut px = resample(src, size);
    for p in px.iter_mut() {
        *p = p.map(|c| c / 255.0);
        settle(p);
    }
    let px = sharpen(&straighten(&px, side), side);
    Premul {
        width: size,
        height: size,
        data: px
            .iter()
            .flat_map(|p| p.map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8))
            .collect(),
    }
}

/// `src` resampled to `size` × `size` by Lanczos3, as `image::imageops::resize` does it (the
/// columns, then the rows, summed in the same order), but neither clipped nor rounded: each pixel
/// premultiplied, 0 to 255, where an edge can take it a little past either end. `src` has pixels.
fn resample(src: &Premul, size: u32) -> Vec<[f32; 4]> {
    let (width, side) = (src.width as usize, size as usize);
    // Down the columns, into `side` rows as wide as the source.
    let mut columns = vec![[0.0f32; 4]; side * width];
    for (row, (top, weights)) in columns.chunks_exact_mut(width).zip(taps(src.height, size)) {
        for (k, &w) in weights.iter().enumerate() {
            let line = &src.data[(top + k) * width * 4..(top + k + 1) * width * 4];
            for (sum, p) in row.iter_mut().zip(line.as_chunks::<4>().0) {
                for (s, &v) in sum.iter_mut().zip(p) {
                    *s += v as f32 * w;
                }
            }
        }
    }
    // Then along the rows.
    let across = taps(src.width, size);
    let mut out = Vec::with_capacity(side * side);
    for row in columns.chunks_exact(width) {
        for (left, weights) in &across {
            let mut sum = [0.0f32; 4];
            for (p, &w) in row[*left..].iter().zip(weights) {
                for (s, &v) in sum.iter_mut().zip(p) {
                    *s += v * w;
                }
            }
            out.push(sum);
        }
    }
    out
}

/// Lanczos3 taps from `from` samples to `to`, laid out as the `image` crate lays them out: for
/// each output sample, its first input sample and the weights from there on, which add up to one.
/// `from` and `to` are at least 1.
fn taps(from: u32, to: u32) -> Vec<(usize, Vec<f32>)> {
    let ratio = from as f32 / to as f32;
    let stretch = ratio.max(1.0);
    let support = 3.0 * stretch;
    (0..to)
        .map(|o| {
            let centre = (o as f32 + 0.5) * ratio;
            let left = ((centre - support).floor() as i64).clamp(0, i64::from(from) - 1);
            let right = ((centre + support).ceil() as i64).clamp(left + 1, i64::from(from));
            // The kernel puts a pixel's centre at 0.
            let centre = centre - 0.5;
            let mut weights: Vec<f32> = (left..right)
                .map(|i| lanczos3((i as f32 - centre) / stretch))
                .collect();
            let sum: f32 = weights.iter().sum();
            weights.iter_mut().for_each(|w| *w /= sum);
            (left as usize, weights)
        })
        .collect()
}

fn lanczos3(x: f32) -> f32 {
    if x.abs() < 3.0 {
        sinc(x) * sinc(x / 3.0)
    } else {
        0.0
    }
}

fn sinc(x: f32) -> f32 {
    let a = x * std::f32::consts::PI;
    if x == 0.0 {
        1.0
    } else {
        a.sin() / a
    }
}

/// A pixel the resample took past either end, made a premultiplied pixel again. Alpha past opaque
/// comes back to opaque with its colour scaled down by as much, so the colour keeps its own shade
/// rather than coming out lighter, as clipping alpha alone does. Alpha at or below nothing is
/// nothing, and no colour is below nothing or above its alpha.
fn settle(p: &mut [f32; 4]) {
    if p[3] <= 0.0 {
        *p = [0.0; 4];
        return;
    }
    if p[3] > 1.0 {
        let back = p[3];
        p.iter_mut().for_each(|c| *c /= back);
        p[3] = 1.0;
    }
    let alpha = p[3];
    for c in &mut p[..3] {
        *c = c.clamp(0.0, alpha);
    }
}

/// Every edge of the outline that runs straight along a column or a row, moved onto a whole pixel
/// ([`along_an_edge`]); the rest as it was. A pixel keeps its own colour however much of it shows.
/// `px` is `side` pixels square, and beyond it is transparency.
fn straighten(px: &[[f32; 4]], side: usize) -> Vec<[f32; 4]> {
    let alpha = |x: usize, y: usize, dx: isize, dy: isize| -> f32 {
        let (x, y) = (x as isize + dx, y as isize + dy);
        let inside = (0..side as isize).contains(&x) && (0..side as isize).contains(&y);
        if inside {
            px[y as usize * side + x as usize][3]
        } else {
            0.0
        }
    };
    let mut out = px.to_vec();
    for y in 0..side {
        for x in 0..side {
            let a = alpha(x, y, 0, 0);
            if a <= 0.0 || a >= 1.0 {
                continue;
            }
            let (left, right) = (alpha(x, y, -1, 0), alpha(x, y, 1, 0));
            let (up, down) = (alpha(x, y, 0, -1), alpha(x, y, 0, 1));
            // An edge running down a column has its two sides left and right; one along a row, up
            // and down.
            let down_a_column = along_an_edge(a, (left, right), (up, down));
            let along_a_row = along_an_edge(a, (up, down), (left, right));
            let (straight, whole) = if down_a_column.0 >= along_a_row.0 {
                down_a_column
            } else {
                along_a_row
            };
            if straight > 0.0 {
                let moved = a + straight * (whole - a);
                out[y * side + x] = px[y * side + x].map(|c| c * moved / a);
            }
        }
    }
    out
}

/// How much a pixel of alpha `a` is part of an edge with `sides` beside it across the edge and
/// `ends` beside it along it, from 0 to 1, and the alpha that puts the edge on a whole pixel.
///
/// It is on a straight edge when its sides are far apart, one in and one out, while its ends are
/// as covered as it is: the edge carries on through them. A curve or a slant changes its cover
/// from one pixel to the next, and a gentle ramp of alpha has sides that are close, so neither is
/// moved. The whole-pixel alpha is between the two sides, so an edge between see-through and
/// clear stays as see-through as it was.
fn along_an_edge(a: f32, sides: (f32, f32), ends: (f32, f32)) -> (f32, f32) {
    let level = |v: f32| {
        if v < RINGING {
            0.0
        } else if v > 1.0 - RINGING {
            1.0
        } else {
            v
        }
    };
    let (low, high) = (level(sides.0.min(sides.1)), level(sides.0.max(sides.1)));
    let across = high - low;
    let apart = ramp(APART, across);
    let carries_on = 1.0 - ramp(SLANT, (ends.0 - a).abs() + (ends.1 - a).abs());
    let straight = apart * carries_on;
    if straight <= 0.0 {
        return (0.0, a);
    }
    let covered = ((a - low) / across).clamp(0.0, 1.0);
    (
        straight,
        low + across * smoothstep(CLEARS_BELOW, FILLS_FROM, covered),
    )
}

/// 0 up to `from`, 1 from `to` on, and a straight line between.
fn ramp((from, to): (f32, f32), x: f32) -> f32 {
    ((x - from) / (to - from)).clamp(0.0, 1.0)
}

/// 0 up to `from`, 1 from `to` on, and an S-curve between.
fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Lightness sharpened wherever a pixel and its eight neighbours are all opaque: its difference
/// from their weighted mean is added again ([`SHARPEN`]), no further than the lightest or the
/// darkest of the nine, and to red, green and blue alike, so no colour shifts and no halo forms.
/// `px` is `side` pixels square.
fn sharpen(px: &[[f32; 4]], side: usize) -> Vec<[f32; 4]> {
    let lightness: Vec<f32> = px
        .iter()
        .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
        .collect();
    let mut out = px.to_vec();
    // The edge of the picture has transparency beyond it, so it is never all opaque.
    for y in 1..side.saturating_sub(1) {
        for x in 1..side.saturating_sub(1) {
            let (mut mean, mut low, mut high) = (0.0f32, f32::MAX, f32::MIN);
            let mut opaque = true;
            for (dy, wy) in MEAN.iter().enumerate() {
                for (dx, wx) in MEAN.iter().enumerate() {
                    let i = (y + dy - 1) * side + (x + dx - 1);
                    opaque &= px[i][3] >= OPAQUE;
                    mean += lightness[i] * wy * wx;
                    low = low.min(lightness[i]);
                    high = high.max(lightness[i]);
                }
            }
            if !opaque {
                continue;
            }
            let own = lightness[y * side + x];
            let shift = (own + SHARPEN * (own - mean)).clamp(low, high) - own;
            let p = &mut out[y * side + x];
            let alpha = p[3];
            for c in &mut p[..3] {
                *c = (*c + shift).clamp(0.0, alpha);
            }
        }
    }
    out
}

/// How sharp a picture's outline is: the squares of the alpha steps between neighbouring pixels,
/// across and down, summed. An edge that goes from clear to opaque in one step counts one; the
/// same edge spread over two half steps counts a half.
#[cfg(test)]
pub(crate) fn edge_energy(img: &image::RgbaImage) -> f64 {
    let a = |x: u32, y: u32| img.get_pixel(x, y).0[3] as f64 / 255.0;
    let (w, h) = img.dimensions();
    let mut sum = 0.0;
    for y in 0..h {
        for x in 0..w {
            if x + 1 < w {
                sum += (a(x + 1, y) - a(x, y)).powi(2);
            }
            if y + 1 < h {
                sum += (a(x, y + 1) - a(x, y)).powi(2);
            }
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster;

    /// A master `side` px square, premultiplied, whose pixel at `(x, y)` is `at(x, y)` (straight).
    fn master(side: u32, at: impl Fn(u32, u32) -> [u8; 4]) -> Premul {
        raster::straight_to_premul(&image::RgbaImage::from_fn(side, side, |x, y| {
            image::Rgba(at(x, y))
        }))
    }

    /// `p` as straight-alpha RGBA.
    fn straight(p: &Premul) -> image::RgbaImage {
        raster::to_straight_rgba(p)
    }

    fn alpha(img: &image::RgbaImage, x: u32, y: u32) -> u8 {
        img.get_pixel(x, y).0[3]
    }

    /// Lightness of each pixel over white, 0 to 255.
    fn over_white(img: &image::RgbaImage) -> Vec<f64> {
        img.pixels()
            .map(|p| {
                let a = p.0[3] as f64 / 255.0;
                let c = |i: usize| p.0[i] as f64 * a + 255.0 * (1.0 - a);
                0.2126 * c(0) + 0.7152 * c(1) + 0.0722 * c(2)
            })
            .collect()
    }

    const BLUE: [u8; 4] = [60, 130, 200, 255];

    /// A flat blue box on transparency, `side` px square, its edges `[x0, x1) × [y0, y1)` in the
    /// master's pixels.
    fn blue_box(side: u32, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> Premul {
        master(side, |x, y| {
            if (x0..x1).contains(&x) && (y0..y1).contains(&y) {
                BLUE
            } else {
                [0, 0, 0, 0]
            }
        })
    }

    #[test]
    fn a_straight_edge_lands_on_a_whole_pixel() {
        // At 16 px from 512, a master pixel is a thirty-second of one. The box covers 69% of
        // column 2 and 25% of column 13, 19% of row 3 and 59% of row 12.
        let src = blue_box(512, (74, 424), (122, 403));
        let plain = straight(&raster::downsample(&src, 16));
        let crisp = straight(&shrink(&src, 16));
        for y in 5..11 {
            assert!(
                (150..=200).contains(&alpha(&plain, 2, y)),
                "{}",
                alpha(&plain, 2, y)
            );
            assert_eq!(alpha(&crisp, 2, y), 255, "more than half covered fills in");
            assert!((40..=90).contains(&alpha(&plain, 13, y)));
            assert_eq!(alpha(&crisp, 13, y), 0, "a quarter covered clears");
        }
        for x in 5..11 {
            assert_eq!(alpha(&crisp, x, 3), 0, "row 3, a fifth covered");
            assert_eq!(alpha(&crisp, x, 12), 255, "row 12, more than half");
        }
        // What fills in is the box's own blue.
        for y in 5..11 {
            let px = crisp.get_pixel(2, y).0;
            assert!(
                px.iter().zip(BLUE).all(|(&a, b)| a.abs_diff(b) <= 1),
                "{px:?}"
            );
        }
    }

    #[test]
    fn the_outline_is_crisper_than_a_plain_downsample_at_every_small_size() {
        // Each of its edges falls part of the way into a pixel at every small size.
        let src = blue_box(2048, (301, 1747), (433, 1861));
        for size in [16, 24, 32, 48, 64] {
            let plain = straight(&raster::downsample(&src, size));
            let crisp = straight(&shrink(&src, size));
            let (before, after) = (edge_energy(&plain), edge_energy(&crisp));
            assert!(
                after > before * 1.15,
                "{size} px: edge energy {before:.2} became {after:.2}"
            );
            // Against white, the step from the box to the window is steeper too.
            let steepest = |img: &image::RgbaImage| {
                let l = over_white(img);
                let s = size as usize;
                (0..s * s)
                    .filter(|i| i % s + 1 < s)
                    .map(|i| (l[i + 1] - l[i]).abs())
                    .fold(0.0, f64::max)
            };
            assert!(steepest(&crisp) >= steepest(&plain), "{size} px");
        }
    }

    #[test]
    fn no_pale_line_runs_inside_the_outline() {
        let src = blue_box(2048, (301, 1747), (433, 1861));
        for size in [16, 24, 32, 48, 64] {
            // A plain downsample overshoots at the edge and clips alpha alone, so the opaque pixels
            // just inside come out a paler blue than the box.
            let plain = straight(&raster::downsample(&src, size));
            let paler = plain
                .pixels()
                .filter(|p| p.0[3] == 255 && p.0[2] >= BLUE[2] + 4)
                .count();
            assert!(
                paler > 0,
                "{size} px: the plain downsample had no pale line to fix"
            );
            // Finished, every pixel that shows is the box's own blue, to within what storing a
            // faint pixel's colour premultiplied in eight bits rounds it by.
            let crisp = straight(&shrink(&src, size));
            for p in crisp.pixels().filter(|p| p.0[3] >= 64) {
                assert!(
                    p.0[..3].iter().zip(BLUE).all(|(&a, b)| a.abs_diff(b) <= 2),
                    "{size} px: {:?}",
                    p.0
                );
            }
        }
    }

    #[test]
    fn curves_keep_their_smoothing() {
        // A disc: its outline runs straight for a pixel or two at the top, bottom and sides, and
        // curves everywhere else, where the pixels it partly covers stay partly covered.
        let src = master(2048, |x, y| {
            let (dx, dy) = (x as f32 - 1011.3, y as f32 - 1030.6);
            if dx * dx + dy * dy < 870.0 * 870.0 {
                BLUE
            } else {
                [0, 0, 0, 0]
            }
        });
        for size in [16, 32, 64] {
            let soft = |img: &image::RgbaImage| {
                img.pixels()
                    .filter(|p| (24..=231).contains(&p.0[3]))
                    .count()
            };
            let plain = soft(&straight(&raster::downsample(&src, size)));
            let crisp = soft(&straight(&shrink(&src, size)));
            assert!(
                crisp * 3 >= plain * 2,
                "{size} px: {plain} smoothed pixels became {crisp}"
            );
        }
    }

    #[test]
    fn see_through_on_purpose_stays_see_through() {
        // A half see-through box: inside it, alpha stays what it was, and so does its colour.
        let src = master(1024, |x, y| {
            if (150..870).contains(&x) && (180..900).contains(&y) {
                [200, 60, 40, 128]
            } else {
                [0, 0, 0, 0]
            }
        });
        for size in [16, 32, 64] {
            let crisp = straight(&shrink(&src, size));
            let plain = straight(&raster::downsample(&src, size));
            let mid = size / 2;
            for (x, y) in [(mid, mid), (mid - 2, mid + 1), (mid + 2, mid - 1)] {
                assert_eq!(crisp.get_pixel(x, y), plain.get_pixel(x, y), "{size} px");
                assert!(alpha(&crisp, x, y).abs_diff(128) <= 1);
            }
            // Its straight edges move onto whole pixels only as far as it is see-through: nothing
            // comes out more opaque than the plain downsample's most opaque pixel.
            let most = |img: &image::RgbaImage| img.pixels().map(|p| p.0[3]).max().unwrap();
            assert!(most(&crisp) <= most(&plain), "{size} px");
        }
    }

    /// White with black bars two to three pixels wide at 32 px, like the strokes of big lettering,
    /// all opaque.
    fn lettering(side: u32) -> Premul {
        master(side, |x, y| {
            let bar = |at: f32, width: f32| {
                let u = x as f32 / side as f32 * 32.0;
                (at..at + width).contains(&u)
            };
            let row = (y as f32 / side as f32 * 32.0) as u32;
            let ink = (6..26).contains(&row)
                && (bar(3.3, 2.4) || bar(9.7, 2.9) || bar(15.1, 2.2) || bar(21.6, 3.1));
            if ink {
                [20, 20, 30, 255]
            } else {
                [250, 248, 240, 255]
            }
        })
    }

    #[test]
    fn lettering_gets_crisper_without_a_halo() {
        let src = lettering(2048);
        for size in [16, 24, 32, 48, 64] {
            let plain = over_white(&straight(&raster::downsample(&src, size)));
            let crisp = over_white(&straight(&shrink(&src, size)));
            let s = size as i64;
            let at =
                |l: &[f64], x: i64, y: i64| l[(y.clamp(0, s - 1) * s + x.clamp(0, s - 1)) as usize];
            let (mut steps_plain, mut steps_crisp) = (0.0, 0.0);
            for y in 0..s {
                for x in 0..s {
                    // No pixel goes lighter or darker than the plain downsample's pixel and its
                    // eight neighbours: sharper strokes, and no ring round them.
                    let near = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)));
                    let (low, high) = near.fold((f64::MAX, f64::MIN), |(lo, hi), (dx, dy)| {
                        let v = at(&plain, x + dx, y + dy);
                        (lo.min(v), hi.max(v))
                    });
                    let v = at(&crisp, x, y);
                    assert!(
                        v >= low - 1.0 && v <= high + 1.0,
                        "{size} px ({x},{y}): {v:.1} outside {low:.1}..{high:.1}"
                    );
                    if x + 1 < s {
                        steps_plain += (at(&plain, x + 1, y) - at(&plain, x, y)).powi(2);
                        steps_crisp += (at(&crisp, x + 1, y) - at(&crisp, x, y)).powi(2);
                    }
                }
            }
            assert!(
                steps_crisp > steps_plain * 1.05,
                "{size} px: {steps_plain:.0} became {steps_crisp:.0}"
            );
        }
    }

    #[test]
    fn every_pixel_stays_premultiplied_and_the_finish_is_the_same_every_time() {
        let photo_like = master(1024, |x, y| {
            let v = (x * 7 + y * 13) ^ (x * y / 5);
            let inside = (100..930).contains(&x) && (60..980).contains(&y);
            [
                v as u8,
                (v >> 3) as u8,
                (v >> 5) as u8,
                if inside { 255 } else { 0 },
            ]
        });
        for src in [
            photo_like,
            lettering(1024),
            blue_box(2048, (301, 1747), (433, 1861)),
        ] {
            for size in [16, 24, 32, 48, 64] {
                let once = shrink(&src, size);
                assert_eq!((once.width, once.height), (size, size));
                for p in once.data.as_chunks::<4>().0 {
                    assert!(p[..3].iter().all(|&c| c <= p[3]), "{p:?}");
                }
                assert_eq!(once.data, shrink(&src, size).data, "{size} px");
            }
        }
    }

    #[test]
    fn nothing_shrinks_to_nothing() {
        let empty = Premul {
            width: 0,
            height: 0,
            data: Vec::new(),
        };
        let out = shrink(&empty, 16);
        assert_eq!((out.width, out.height), (16, 16));
        assert!(out.data.iter().all(|&v| v == 0));
        let none = shrink(&blue_box(64, (4, 60), (4, 60)), 0);
        assert!(none.data.is_empty());
    }

    #[test]
    fn the_resample_is_lanczos3_as_the_image_crate_does_it() {
        // Where nothing overshoots and nothing is finished, it is the plain downsample, exactly:
        // a smooth, opaque gradient.
        let src = master(512, |x, y| [(x / 2) as u8, (y / 2) as u8, 128, 255]);
        let plain = raster::downsample(&src, 48);
        let ours: Vec<u8> = resample(&src, 48)
            .iter()
            .flat_map(|p| p.map(|c| c.round().clamp(0.0, 255.0) as u8))
            .collect();
        let off = plain.data.iter().zip(&ours).filter(|(a, b)| a != b).count();
        assert_eq!(off, 0, "{off} values differ from image's own Lanczos3");
    }
}
