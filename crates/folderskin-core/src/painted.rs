//! Cleaning up a picture an image model painted, before FolderSkin uses it.
//!
//! * [`trim_border`] cuts away the paper margin a model sometimes paints its art on. Asked for a
//!   poster or a print, a model often paints the print lying on paper: a flat margin, sometimes
//!   with a thin dark rule inside it, which on a folder becomes a blank tab and blank edges.
//!   [`trim_paper`] also cuts bands of paper down just two opposite sides, which klein paints too.
//! * [`cut_along_silhouette`] cuts a whole-folder picture out along FolderSkin's own silhouette.
//!   The model repaints the app's blank folder and keeps its shape, but not always its exact scale
//!   (it came back ~2% smaller in testing), and its backdrop drifts from magenta to purple. So the
//!   painted folder is found against whatever backdrop it has, the silhouette is fitted to it, and
//!   the silhouette becomes the edge: no colour keying, so no pink fringe, and the edge is ours.
//! * [`is_blank`] spots the flat white or black picture a backend writes when it fails quietly
//!   (stable-diffusion.cpp's Metal backend does on some Macs).
//!
//! All three are pure, pixels in and pixels out, and follow the reference implementation the
//! local-generation skill used (`fsgen.py`) step for step; the tests hold them to its results.

use image::imageops::{self, FilterType};
use image::{GrayImage, Luma, RgbaImage};

/// How many pixels a paper margin took off each side of a picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Border {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl Border {
    /// The sides with paper on them: "left 52, right 50 px".
    pub fn describe(&self) -> String {
        let sides: Vec<String> = [
            ("top", self.top),
            ("bottom", self.bottom),
            ("left", self.left),
            ("right", self.right),
        ]
        .iter()
        .filter(|(_, px)| *px > 0)
        .map(|(side, px)| format!("{side} {px}"))
        .collect();
        format!("{} px", sides.join(", "))
    }
}

/// Standard deviation of luminance below which a picture counts as blank.
const BLANK_STD: f64 = 2.0;

/// True when the picture is one flat colour, give or take a little noise: what a backend writes
/// when it fails without saying so.
pub fn is_blank(img: &RgbaImage) -> bool {
    let n = img.width() as f64 * img.height() as f64;
    if n == 0.0 {
        return true;
    }
    // Luminance as Pillow's "L" conversion computes it (ITU-R 601-2, fixed point).
    let luma = |p: &image::Rgba<u8>| {
        let [r, g, b, _] = p.0.map(u32::from);
        ((r * 19595 + g * 38470 + b * 7471 + 0x8000) >> 16) as f64
    };
    let mean = img.pixels().map(luma).sum::<f64>() / n;
    let var = img.pixels().map(|p| (luma(p) - mean).powi(2)).sum::<f64>() / n;
    var.sqrt() < BLANK_STD
}

// ---------- paper margins ----------

/// One side of a picture, read as lines from that edge inwards: rows from the top or the bottom,
/// columns from the left or the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

const SIDES: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

struct Lines<'a> {
    img: &'a RgbaImage,
    side: Side,
}

impl Lines<'_> {
    /// How many lines there are from this edge to the opposite one.
    fn count(&self) -> u32 {
        match self.side {
            Side::Top | Side::Bottom => self.img.height(),
            Side::Left | Side::Right => self.img.width(),
        }
    }

    /// How many pixels each line has.
    fn span(&self) -> u32 {
        match self.side {
            Side::Top | Side::Bottom => self.img.width(),
            Side::Left | Side::Right => self.img.height(),
        }
    }

    /// Pixel `at` of line `line`, counted from the edge.
    fn px(&self, line: u32, at: u32) -> [f64; 3] {
        let (w, h) = self.img.dimensions();
        let (x, y) = match self.side {
            Side::Top => (at, line),
            Side::Bottom => (at, h - 1 - line),
            Side::Left => (line, at),
            Side::Right => (w - 1 - line, at),
        };
        let p = self.img.get_pixel(x, y).0;
        [p[0] as f64, p[1] as f64, p[2] as f64]
    }

    /// The middle of every line: a margin on the next side doesn't spoil the line.
    fn middle(&self) -> std::ops::Range<u32> {
        let span = self.span();
        span / 20..span - span / 20
    }

    /// Each channel's median over pixels `range` of line `line`.
    fn median(&self, line: u32, range: std::ops::Range<u32>) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (c, slot) in out.iter_mut().enumerate() {
            let mut values: Vec<f64> = range.clone().map(|at| self.px(line, at)[c]).collect();
            *slot = median(&mut values);
        }
        out
    }

    /// Share of the middle of line `line` within `tolerance` of `paper` in every channel.
    fn share(&self, line: u32, paper: &[f64; 3]) -> f64 {
        let range = self.middle();
        let len = range.len().max(1) as f64;
        let close = range
            .filter(|&at| {
                let p = self.px(line, at);
                (0..3).all(|c| (p[c] - paper[c]).abs() <= PAPER_TOLERANCE)
            })
            .count();
        close as f64 / len
    }

    /// Share of the middle of line `line` darker than a printed rule is.
    fn dark_share(&self, line: u32) -> f64 {
        let range = self.middle();
        let len = range.len().max(1) as f64;
        let dark = range
            .filter(|&at| self.px(line, at).iter().all(|&v| v < RULE_DARKNESS))
            .count();
        dark as f64 / len
    }
}

/// The median of `values`, the mean of the middle two when there is an even number of them.
fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    }
}

/// How far a paper pixel may stray from the paper's colour, per channel.
const PAPER_TOLERANCE: f64 = 28.0;
/// Every channel of a printed frame line is below this.
const RULE_DARKNESS: f64 = 110.0;

/// How far a paper margin reaches in from one side, 0 where there is none.
///
/// A side counts only when a flat band of one colour ends at an edge the art starts on, most of
/// whose pixels are something else; a studio backdrop that simply surrounds the subject carries
/// on past where the subject begins, so it isn't cut.
fn margin(lines: &Lines) -> u32 {
    let n = lines.count();
    let paper = lines.median(0, lines.middle());
    let limit = n / 5;
    let dark = |i: u32| i < limit && lines.dark_share(i) >= 0.5;

    let mut margin = 0;
    while margin < limit && lines.share(margin, &paper) >= 0.96 {
        margin += 1;
    }
    if margin < (n / 200).max(4) {
        return 0;
    }
    // The art has to start within a few anti-aliased lines of the margin's end.
    let window = (margin + (n / 100).max(4)).min(limit);
    let Some(start) = (margin..window).find(|&i| lines.share(i, &paper) < 0.5) else {
        return 0;
    };
    // A printed frame line inside the margin: mostly dark lines, soft at both edges.
    let mut edge = start;
    if let Some(first) = (start..start + 3).find(|&i| dark(i)) {
        edge = first;
        while edge - first < (n / 60).max(4) && dark(edge) {
            edge += 1;
        }
    }
    edge + 3
}

/// How many lines in from a side are still mostly `paper`: a watercolour's torn or bleeding
/// edge, which never ends as cleanly as [`margin`] wants. Only asked once the other sides have
/// shown the picture sits on paper.
fn ragged_width(lines: &Lines, paper: &[f64; 3]) -> u32 {
    let n = lines.count();
    let mut width = 0;
    while width < n / 10 && lines.share(width, paper) >= 0.6 {
        width += 1;
    }
    if width >= 4 {
        width + 3
    } else {
        0
    }
}

/// The paper margin around a picture, if it sits on one: at least three sides must show a
/// margin, and the fourth is then read as a ragged edge.
pub fn find_border(img: &RgbaImage) -> Option<Border> {
    let (w, h) = img.dimensions();
    if w < 20 || h < 20 {
        return None;
    }
    let lines = |side| Lines { img, side };
    let mut widths = SIDES.map(|side| margin(&lines(side)));
    if widths.iter().filter(|&&x| x > 0).count() < 3 {
        return None;
    }
    if let Some(missing) = widths.iter().position(|&x| x == 0) {
        let most = *widths.iter().max().unwrap_or(&0);
        let widest = widths.iter().position(|&x| x == most).unwrap_or(0);
        // The paper's colour from the widest margin's outermost line, all of it.
        let outer = lines(SIDES[widest]);
        let paper = outer.median(0, 0..outer.span());
        widths[missing] = ragged_width(&lines(SIDES[missing]), &paper);
    }
    let [top, bottom, left, right] = widths;
    (left + right < w && top + bottom < h).then_some(Border {
        top,
        bottom,
        left,
        right,
    })
}

/// Bands of the same paper down two opposite sides and nowhere else: a print the model painted
/// full width but not full height, or the other way round. [`find_border`] wants three sides, so
/// it leaves these; each band still has to end sharply where the art starts, which a studio
/// backdrop around a subject never does, and both have to be the same paper.
pub fn find_bands(img: &RgbaImage) -> Option<Border> {
    let (w, h) = img.dimensions();
    if w < 20 || h < 20 {
        return None;
    }
    let [top, bottom, left, right] = SIDES.map(|side| margin(&Lines { img, side }));
    let (a, b) = match (top > 0, bottom > 0, left > 0, right > 0) {
        (true, true, false, false) => (Side::Top, Side::Bottom),
        (false, false, true, true) => (Side::Left, Side::Right),
        _ => return None,
    };
    let paper = |side| {
        let lines = Lines { img, side };
        lines.median(0, lines.middle())
    };
    let (a, b) = (paper(a), paper(b));
    if (0..3).any(|c| (a[c] - b[c]).abs() > PAPER_TOLERANCE) {
        return None;
    }
    (left + right < w && top + bottom < h).then_some(Border {
        top,
        bottom,
        left,
        right,
    })
}

/// Cuts a paper margin off a picture and fills the frame again, cover-fitted and centred so
/// nothing is stretched. `None` when the picture doesn't sit on paper.
pub fn trim_border(img: &RgbaImage) -> Option<(Border, RgbaImage)> {
    let border = find_border(img)?;
    Some((border, cut_and_fill(img, border)))
}

/// [`trim_border`], or failing that the bands [`find_bands`] finds, cut the same way.
pub fn trim_paper(img: &RgbaImage) -> Option<(Border, RgbaImage)> {
    trim_border(img).or_else(|| {
        let bands = find_bands(img)?;
        Some((bands, cut_and_fill(img, bands)))
    })
}

/// `img` less `border`, cover-fitted back to its own size and centred.
fn cut_and_fill(img: &RgbaImage, border: Border) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (iw, ih) = (
        w - border.left - border.right,
        h - border.top - border.bottom,
    );
    let inner = imageops::crop_imm(img, border.left, border.top, iw, ih).to_image();
    let scale = (w as f64 / iw as f64).max(h as f64 / ih as f64);
    let (rw, rh) = (
        ((iw as f64 * scale).round_ties_even() as u32).max(w),
        ((ih as f64 * scale).round_ties_even() as u32).max(h),
    );
    let resized = imageops::resize(&inner, rw, rh, FilterType::Lanczos3);
    let (x0, y0) = ((rw - w) / 2, (rh - h) / 2);
    imageops::crop_imm(&resized, x0, y0, w, h).to_image()
}

// ---------- whole folders ----------

/// Below this overlap between the painted folder and FolderSkin's silhouette, the model changed
/// the folder's shape, and cutting along ours would cut into its painting.
pub const MIN_SILHOUETTE_FIT: f64 = 0.95;

/// What [`cut_along_silhouette`] made of a picture.
#[derive(Clone, Debug)]
pub struct SilhouetteCut {
    /// How well FolderSkin's silhouette covers the painted folder: intersection over union, 0 to 1.
    pub fit: f64,
    /// The folder cut out, the same size as the picture; `None` when `fit` was below
    /// [`MIN_SILHOUETTE_FIT`] and the picture is better left on its backdrop.
    pub image: Option<RgbaImage>,
}

/// How far from the backdrop, in its most different channel, a pixel is painted.
const PAINTED: f32 = 60.0;
/// A pixel pointing this closely the backdrop's way in RGB is the backdrop in shadow.
const SHADOW_COSINE: f32 = 0.985;
/// A row or column is part of the folder when more than this share of it is painted.
const PAINTED_SHARE: f64 = 0.02;
/// How far in from the silhouette's edge the backdrop is keyed and unmixed.
const EDGE_BAND: u32 = 10;

/// Cuts a whole-folder picture out along FolderSkin's own `silhouette` (white inside the folder,
/// black around it, lined up with the frame the model was shown).
pub fn cut_along_silhouette(img: &RgbaImage, silhouette: &GrayImage) -> SilhouetteCut {
    let (w, h) = img.dimensions();
    let backdrop = ring_median(img);
    let px = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    let distance = |p: &[f32; 3]| {
        (0..3)
            .map(|c| (p[c] - backdrop[c]).abs())
            .fold(0.0f32, f32::max)
    };
    let backdrop_len = backdrop.iter().map(|v| v * v).sum::<f32>().sqrt();
    // A shadow the model casts on the backdrop is the backdrop's own colour, darker: the same
    // direction in RGB. It must not count when finding the folder's box, or the box grows by the
    // shadow's width. Only there: inside the folder a red lantern can point the same way.
    let shadow = |p: &[f32; 3]| {
        let len = p.iter().map(|v| v * v).sum::<f32>().sqrt() * backdrop_len + 1e-6;
        (p[0] * backdrop[0] + p[1] * backdrop[1] + p[2] * backdrop[2]) / len > SHADOW_COSINE
    };

    let mut rows = vec![0u32; h as usize];
    let mut cols = vec![0u32; w as usize];
    for y in 0..h {
        for x in 0..w {
            let p = px(x, y);
            if distance(&p) > PAINTED && !shadow(&p) {
                rows[y as usize] += 1;
                cols[x as usize] += 1;
            }
        }
    }
    let busy = |counts: &[u32], len: u32| -> Option<(u32, u32)> {
        let limit = len as f64 * PAINTED_SHARE;
        let first = counts.iter().position(|&n| n as f64 > limit)?;
        let last = counts.iter().rposition(|&n| n as f64 > limit)?;
        Some((first as u32, last as u32 + 1))
    };
    let (Some((y0, y1)), Some((x0, x1))) = (busy(&rows, w), busy(&cols, h)) else {
        return SilhouetteCut {
            fit: 0.0,
            image: None,
        };
    };

    // Our silhouette, trimmed to itself and stretched over the painted folder's box, then a
    // pixel in from its own edge, where the model's painting blends into the backdrop.
    let fitted = erode(&place_silhouette(silhouette, (w, h), (x0, y0, x1, y1)), 1);

    let (mut both, mut either) = (0u64, 0u64);
    for (x, y, f) in fitted.enumerate_pixels() {
        let p = px(x, y);
        let inside = f.0[0] > 127;
        let solid = distance(&p) > PAINTED && !(shadow(&p) && !inside);
        both += u64::from(inside && solid);
        either += u64::from(inside || solid);
    }
    let fit = both as f64 / either.max(1) as f64;
    if fit < MIN_SILHOUETTE_FIT {
        return SilhouetteCut { fit, image: None };
    }

    // The model's edge wanders a few pixels either side of ours. In a band just inside our edge,
    // backdrop-coloured pixels go too, and a pixel that is part backdrop gets the backdrop taken
    // out of its colour, so the edge carries the painting's colours instead of a pink rim. Deeper
    // in, a pink lantern stays a pink lantern.
    let deep = erode(&fitted, EDGE_BAND);
    let magenta_backdrop = backdrop[0].min(backdrop[2]) - backdrop[1] > 100.0;
    let mut out = RgbaImage::new(w, h);
    for (x, y, o) in out.enumerate_pixels_mut() {
        let p = px(x, y);
        let f = fitted.get_pixel(x, y).0[0] as f32 / 255.0;
        let band = f > 0.0 && deep.get_pixel(x, y).0[0] == 0;
        let (mut colour, mut alpha) = (p, f);
        if band {
            let keep = ((distance(&p) - 40.0) / 80.0).clamp(0.0, 1.0);
            alpha = f * keep;
            for c in 0..3 {
                let unmixed = (p[c] - (1.0 - keep) * backdrop[c]) / keep.max(0.05);
                colour[c] = unmixed.clamp(0.0, 255.0);
            }
            if magenta_backdrop {
                // The model shades the folder's rim with the magenta around it; take that cast
                // back out.
                let spill = (colour[0].min(colour[2]) - colour[1]).max(0.0);
                colour[0] -= spill;
                colour[2] -= spill;
            }
        }
        let byte = |v: f32| v.round_ties_even().clamp(0.0, 255.0) as u8;
        o.0 = [
            byte(colour[0]),
            byte(colour[1]),
            byte(colour[2]),
            byte(alpha * 255.0),
        ];
    }
    SilhouetteCut {
        fit,
        image: Some(out),
    }
}

/// The backdrop's colour: each channel's median over a ring four pixels deep around the frame.
fn ring_median(img: &RgbaImage) -> [f32; 3] {
    let (w, h) = img.dimensions();
    let depth = 4.min(w).min(h);
    let mut ring: Vec<[u8; 4]> = Vec::new();
    // Rows first, then columns, corners in both, as the reference reads them.
    for y in (0..depth).chain(h - depth..h) {
        ring.extend((0..w).map(|x| img.get_pixel(x, y).0));
    }
    for y in 0..h {
        for x in (0..depth).chain(w - depth..w) {
            ring.push(img.get_pixel(x, y).0);
        }
    }
    let mut out = [0.0; 3];
    for (c, slot) in out.iter_mut().enumerate() {
        let mut values: Vec<f64> = ring.iter().map(|p| p[c] as f64).collect();
        *slot = median(&mut values) as f32;
    }
    out
}

/// `silhouette` trimmed to its own non-black pixels, resized (Lanczos) to the box
/// `(x0, y0, x1, y1)` and placed there on a black frame of `size`.
fn place_silhouette(
    silhouette: &GrayImage,
    size: (u32, u32),
    (x0, y0, x1, y1): (u32, u32, u32, u32),
) -> GrayImage {
    let mut frame = GrayImage::new(size.0, size.1);
    let (mut sx0, mut sy0, mut sx1, mut sy1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in silhouette.enumerate_pixels() {
        if p.0[0] > 0 {
            (sx0, sy0) = (sx0.min(x), sy0.min(y));
            (sx1, sy1) = (sx1.max(x + 1), sy1.max(y + 1));
        }
    }
    if sx0 >= sx1 || x1 <= x0 || y1 <= y0 {
        return frame;
    }
    let trimmed = imageops::crop_imm(silhouette, sx0, sy0, sx1 - sx0, sy1 - sy0).to_image();
    let resized = imageops::resize(&trimmed, x1 - x0, y1 - y0, FilterType::Lanczos3);
    imageops::replace(&mut frame, &resized, i64::from(x0), i64::from(y0));
    frame
}

/// A square minimum filter `2 * radius + 1` pixels wide, edges repeated outwards. Square, so it
/// runs as a pass along the rows and one down the columns.
fn erode(img: &GrayImage, radius: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let r = radius as i64;
    let pass = |src: &GrayImage, along_rows: bool| {
        GrayImage::from_fn(w, h, |x, y| {
            let (pos, len) = if along_rows {
                (x as i64, w as i64)
            } else {
                (y as i64, h as i64)
            };
            let lowest = (pos - r..=pos + r)
                .map(|i| {
                    let i = i.clamp(0, len - 1) as u32;
                    let (sx, sy) = if along_rows { (i, y) } else { (x, i) };
                    src.get_pixel(sx, sy).0[0]
                })
                .min()
                .unwrap_or(0);
            Luma([lowest])
        })
    };
    pass(&pass(img, true), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// Colourful, busy "art" that is nowhere near flat paper.
    fn art(x: u32, y: u32) -> Rgba<u8> {
        let v = (x * 7 + y * 13) % 256;
        Rgba([
            (40 + v / 2) as u8,
            ((x * 3) % 200 + 30) as u8,
            ((y * 5) % 180 + 50) as u8,
            255,
        ])
    }

    /// `art` on cream paper `margin` px wide, with a dark rule `rule` px wide just inside it.
    fn framed(w: u32, h: u32, margin: u32, rule: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let edge = x.min(y).min(w - 1 - x).min(h - 1 - y);
            if edge < margin {
                Rgba([244, 238, 222, 255])
            } else if edge < margin + rule {
                Rgba([25, 22, 20, 255])
            } else {
                art(x, y)
            }
        })
    }

    #[test]
    fn a_flat_picture_is_blank_and_a_painting_is_not() {
        assert!(is_blank(&RgbaImage::from_pixel(64, 64, Rgba([255; 4]))));
        assert!(is_blank(&RgbaImage::from_pixel(
            64,
            64,
            Rgba([0, 0, 0, 255])
        )));
        // A little noise is still blank.
        let noisy = RgbaImage::from_fn(64, 64, |x, y| {
            let v = 250 + ((x + y) % 3) as u8;
            Rgba([v, v, v, 255])
        });
        assert!(is_blank(&noisy));
        assert!(!is_blank(&framed(64, 64, 0, 0)));
        assert!(is_blank(&RgbaImage::new(0, 0)));
    }

    #[test]
    fn a_print_on_paper_is_found_on_all_four_sides() {
        let img = framed(320, 300, 14, 3);
        let border = find_border(&img).expect("a margin");
        // Paper, then the rule, then three anti-aliasing lines of safety.
        for (side, width) in [
            ("top", border.top),
            ("bottom", border.bottom),
            ("left", border.left),
            ("right", border.right),
        ] {
            assert_eq!(width, 14 + 3 + 3, "{side}: {border:?}");
        }
        let (_, trimmed) = trim_border(&img).unwrap();
        assert_eq!(
            trimmed.dimensions(),
            (320, 300),
            "the frame is filled again"
        );
        for (x, y) in [(0, 0), (319, 0), (0, 299), (319, 299), (160, 0), (0, 150)] {
            let p = trimmed.get_pixel(x, y).0;
            assert!(
                !(p[0] > 230 && p[1] > 225 && p[2] > 205),
                "paper left at {x},{y}: {p:?}"
            );
        }
    }

    #[test]
    fn a_margin_without_a_rule_is_cut_too() {
        let border = find_border(&framed(320, 300, 12, 0)).unwrap();
        assert_eq!(border.top, 12 + 3);
        assert_eq!(border.left, 12 + 3);
    }

    #[test]
    fn a_torn_fourth_side_is_read_as_ragged_paper() {
        // Clean paper on three sides; on the right, paper that the art bleeds into unevenly.
        let img = RgbaImage::from_fn(320, 300, |x, y| {
            let from_right = 319 - x;
            let torn = 10 + (y * 7 % 9);
            let edge = x.min(y).min(299 - y);
            if edge < 14 || (from_right < torn && (x + y) % 4 != 0) {
                Rgba([244, 238, 222, 255])
            } else {
                art(x, y)
            }
        });
        let border = find_border(&img).expect("three clean sides");
        assert_eq!((border.top, border.bottom, border.left), (17, 17, 17));
        assert!(border.right >= 10, "the ragged side is cut too: {border:?}");
    }

    #[test]
    fn a_studio_backdrop_around_a_subject_is_not_paper() {
        // A flat backdrop that carries on past where the subject begins.
        let img = RgbaImage::from_fn(320, 300, |x, y| {
            let (dx, dy) = (x as i32 - 160, y as i32 - 150);
            if dx * dx + dy * dy < 90 * 90 {
                art(x, y)
            } else {
                Rgba([200, 210, 225, 255])
            }
        });
        assert_eq!(find_border(&img), None);
        assert!(trim_border(&img).is_none());
        // A painting that reaches every edge has no margin at all.
        assert_eq!(find_border(&framed(320, 300, 0, 0)), None);
    }

    #[test]
    fn two_margins_are_not_a_border_but_are_bands() {
        let img = RgbaImage::from_fn(320, 300, |x, y| {
            if (14..286).contains(&y) {
                art(x, y)
            } else {
                Rgba([244, 238, 222, 255])
            }
        });
        assert_eq!(find_border(&img), None, "a border wants three sides");
        assert_eq!(
            find_bands(&img),
            Some(Border {
                top: 17,
                bottom: 17,
                left: 0,
                right: 0
            })
        );
        let (_, trimmed) = trim_paper(&img).unwrap();
        assert_eq!(trimmed.dimensions(), (320, 300));
        assert!(
            trimmed.pixels().all(|p| p.0 != [244, 238, 222, 255]),
            "no paper left"
        );
    }

    #[test]
    fn bands_must_be_opposite_and_the_same_paper() {
        // White down the left, cream down the right: two papers is not one print.
        let mixed = RgbaImage::from_fn(320, 300, |x, y| match x {
            0..14 => Rgba([255, 255, 255, 255]),
            306.. => Rgba([120, 200, 120, 255]),
            _ => art(x, y),
        });
        assert_eq!(find_bands(&mixed), None);
        // One side only.
        let one = RgbaImage::from_fn(320, 300, |x, y| {
            if x < 14 {
                Rgba([255, 255, 255, 255])
            } else {
                art(x, y)
            }
        });
        assert_eq!(find_bands(&one), None);
        assert!(trim_paper(&one).is_none());
        // A margin all round is a border, and trim_paper still takes it.
        let framed = framed(320, 300, 14, 3);
        assert_eq!(find_bands(&framed), None);
        assert_eq!(
            trim_paper(&framed).unwrap().0,
            find_border(&framed).unwrap()
        );
    }

    /// Paint for a folder, green enough never to be mistaken for the purple around it.
    fn folder_paint(x: u32, y: u32) -> Rgba<u8> {
        Rgba([
            (20 + x * 3 % 120) as u8,
            (130 + y * 2 % 100) as u8,
            (30 + (x + y) % 90) as u8,
            255,
        ])
    }

    /// FolderSkin's silhouette at `w` x `h`, as `folderskin template --mask` writes it.
    fn silhouette(w: u32, h: u32) -> GrayImage {
        let cut = crate::compositor::blank_template_cutout(w, h);
        GrayImage::from_fn(w, h, |x, y| Luma([cut.get_pixel(x, y).0[3]]))
    }

    /// The blank folder "repainted" the way a model does it: a little smaller than asked, painted
    /// all over, on a purple that drifted from magenta, with a shadow to its lower right.
    fn painted_folder(w: u32, h: u32, shrink: f32) -> RgbaImage {
        let sil = silhouette(w, h);
        let (sw, sh) = (
            (w as f32 * shrink).round() as u32,
            (h as f32 * shrink).round() as u32,
        );
        let small = imageops::resize(&sil, sw, sh, FilterType::Triangle);
        let (ox, oy) = ((w - sw) / 2, (h - sh) / 2);
        let backdrop = [214.0f32, 30.0, 206.0];
        RgbaImage::from_fn(w, h, |x, y| {
            let inside = |x: u32, y: u32| {
                if x >= ox && y >= oy && x - ox < sw && y - oy < sh {
                    small.get_pixel(x - ox, y - oy).0[0] as f32 / 255.0
                } else {
                    0.0
                }
            };
            let a = inside(x, y);
            let shade = if x > 6 && y > 6 && inside(x - 6, y - 6) > 0.5 {
                0.7
            } else {
                1.0
            };
            let paint = folder_paint(x, y).0;
            let mix = |c: usize| paint[c] as f32 * a + backdrop[c] * shade * (1.0 - a);
            Rgba([mix(0) as u8, mix(1) as u8, mix(2) as u8, 255])
        })
    }

    #[test]
    fn a_repainted_folder_is_cut_out_along_our_silhouette() {
        let (w, h) = (320, 300);
        let cut = cut_along_silhouette(&painted_folder(w, h, 0.98), &silhouette(w, h));
        assert!(cut.fit >= MIN_SILHOUETTE_FIT, "fit {}", cut.fit);
        let img = cut.image.expect("cut out");
        assert_eq!(img.dimensions(), (w, h));
        assert_eq!(img.get_pixel(2, 2).0[3], 0, "the backdrop is gone");
        assert_eq!(img.get_pixel(w - 3, h - 3).0[3], 0, "so is the shadow");
        let middle = img.get_pixel(w / 2, h * 2 / 3).0;
        assert_eq!(
            middle,
            folder_paint(w / 2, h * 2 / 3).0,
            "the painting stays as painted"
        );
        // No magenta rim: every visible edge pixel has had the backdrop taken out.
        let pink = img
            .pixels()
            .filter(|p| p.0[3] > 0 && p.0[3] < 255)
            .filter(|p| p.0[0].min(p.0[2]) as i32 - p.0[1] as i32 > 100)
            .count();
        assert_eq!(pink, 0, "{pink} pink edge pixels");
    }

    #[test]
    fn a_folder_the_model_reshaped_is_left_alone() {
        let (w, h) = (320, 300);
        // A plain rectangle where the folder should be.
        let rect = RgbaImage::from_fn(w, h, |x, y| {
            if (60..260).contains(&x) && (40..260).contains(&y) {
                art(x, y)
            } else {
                Rgba([255, 0, 255, 255])
            }
        });
        let cut = cut_along_silhouette(&rect, &silhouette(w, h));
        assert!(cut.fit < MIN_SILHOUETTE_FIT, "fit {}", cut.fit);
        assert!(cut.image.is_none());
        // And nothing painted at all fits nothing.
        let empty = RgbaImage::from_pixel(w, h, Rgba([255, 0, 255, 255]));
        let cut = cut_along_silhouette(&empty, &silhouette(w, h));
        assert_eq!(cut.fit, 0.0);
        assert!(cut.image.is_none());
    }

    #[test]
    fn erosion_takes_a_square_off_every_edge() {
        let mut img = GrayImage::new(9, 9);
        for y in 2..7 {
            for x in 2..7 {
                img.put_pixel(x, y, Luma([255]));
            }
        }
        let eroded = erode(&img, 1);
        assert_eq!(eroded.get_pixel(4, 4).0[0], 255);
        assert_eq!(eroded.get_pixel(2, 4).0[0], 0);
        assert_eq!(eroded.get_pixel(3, 3).0[0], 255);
        // Edges repeat outwards, so a full frame stays full.
        let full = GrayImage::from_pixel(5, 5, Luma([200]));
        assert!(erode(&full, 3).pixels().all(|p| p.0[0] == 200));
    }

    #[test]
    fn medians_match_numpy() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), 2.5);
        assert_eq!(median(&mut []), 0.0);
    }
}
