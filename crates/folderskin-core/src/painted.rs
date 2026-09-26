//! Cleaning up a picture an image model painted, before FolderSkin uses it.
//!
//! * [`trim_border`] cuts away the paper margin a model sometimes paints its art on. Asked for a
//!   poster or a print, a model often paints the print lying on paper: a flat margin, sometimes
//!   with a thin dark rule inside it, which on a folder becomes a blank tab and blank edges.
//!   [`trim_paper`] also cuts bands of paper down just two opposite sides, which klein paints too.
//! * [`cut_along_silhouette`] cuts a whole-folder picture out along FolderSkin's own silhouette.
//!   The model repaints the app's blank folder and keeps its shape, but not always its exact scale
//!   (it came back ~2% smaller in testing), and its backdrop drifts from magenta to a dusty pink,
//!   a purple or a dark grey, often brighter in one corner. So the backdrop is measured as the
//!   colour it has at every pixel ([`Backdrop`]), the painted folder is found against it, the
//!   silhouette is fitted to it, and the silhouette becomes the edge: no colour keying, so no pink
//!   fringe, and the edge is ours.
//! * [`is_blank`] spots the flat white or black picture a backend writes when it fails quietly
//!   (stable-diffusion.cpp's Metal backend does on some Macs).
//!
//! All three are pure, pixels in and pixels out. The paper and blank checks follow the reference
//! implementation the local-generation skill used (`fsgen.py`) step for step, and the tests hold
//! them to its results; the whole-folder cut outgrew it, and is held to klein's own paintings.

use crate::backdrop::Backdrop;
use crate::matte::distance;
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

/// Below this overlap between a cut-out folder's outline and FolderSkin's (intersection over
/// union), it is a folder of another shape: what `image check` holds a finished cut-out to.
pub const MIN_SILHOUETTE_FIT: f64 = 0.95;

/// Below this [`SilhouetteCut::fit`], the model changed the folder's shape, and cutting along
/// ours would cut into its painting or keep some of its backdrop. On klein's paintings a folder
/// that kept its shape disagrees with ours on well under a tenth of a percent of its area; one
/// that lost its tab, moved it, or had a corner cut off, on 0.8% or more.
pub const MIN_PAINTED_FIT: f64 = 0.995;

/// What [`cut_along_silhouette`] made of a picture.
#[derive(Clone, Debug)]
pub struct SilhouetteCut {
    /// How well the painted folder keeps FolderSkin's outline: 1 less the share of the
    /// silhouette's area painted outside it, less twice the share left bare inside it (backdrop
    /// where the folder should be is the worse mistake: the cut would keep it). 1 fits exactly,
    /// and 0 is nothing painted at all.
    pub fit: f64,
    /// The folder cut out, the same size as the picture; `None` when `fit` was below
    /// [`MIN_PAINTED_FIT`] and the picture is better left on its backdrop.
    pub image: Option<RgbaImage>,
}

/// The least a pixel differs from the backdrop, on [`matte`](crate::matte)'s 0 to 1 scale, to
/// count as paint (in the folder's box, or spilled past our outline), and the most it differs to
/// count as bare backdrop; twice the backdrop's own noise when that is more. klein leaves a
/// folder's back panel a pale grey this close to the pink around it.
const FAINT: f32 = 0.035;
/// The most a pixel just inside our outline differs from the backdrop to be painted over where
/// the backdrop reaches in past it; three times the backdrop's noise when that is more.
const CLEAR: f32 = 0.06;
/// The darkest a shadow the model casts on its backdrop is, as a share of the backdrop's colour.
const SHADOW_DEPTH: f32 = 0.3;
/// The lightest the backdrop's own colour is taken to be, as a share of the colour measured
/// there: a glow the model painted where [`Backdrop`] didn't see one.
const SHADOW_LIGHT: f32 = 1.1;
/// How far from the backdrop's colour scaled a pixel of shadow strays, per channel.
const SHADOW_NOISE: f32 = 10.0;
/// A row or column is part of the folder when more than this share of it is painted: enough
/// that a sparkle or the edge of a shadow in the margin doesn't stretch the folder's box, while
/// even the tab's rows are a third painted.
const PAINTED_SHARE: f64 = 0.1;
/// How far the model's edge may wander from ours without disagreeing, as a share of the folder's
/// shorter side (at least 3 px): klein's stays within about 8 px of it at 1024 px.
const OUTLINE_SLACK: f64 = 0.008;
/// How deep into the silhouette bare backdrop is followed from outside it, as a share of the
/// folder's shorter side: deep enough to cover a missing tab, not so deep that a pink sky which
/// touches the edge is taken for backdrop all the way in.
const BARE_REACH: f64 = 0.04;
/// How far in from the silhouette's edge backdrop that reaches in past it is painted over:
/// klein's edge stays within about 8 px of ours at 1024 px.
const EDGE_BAND: u32 = 10;
/// How many pixels of the model's own edge, beside backdrop that reaches in past ours, are split
/// into paint and backdrop: klein's edge fades from paint to backdrop over about four.
const EDGE_RINGS: u8 = 4;
/// How far around a pixel on the model's edge the paint just inside it is looked for.
const EDGE_REACH: u32 = 5;

/// Cuts a whole-folder picture out along FolderSkin's own `silhouette` (white inside the folder,
/// black around it, lined up with the frame the model was shown).
///
/// The backdrop is measured as the colour it has at every pixel ([`Backdrop`]), so a pink that
/// brightens towards a corner or a dark grey behind a night scene is told from the folder as
/// well as flat magenta is. The painted folder's box is where anything differs from it, and our
/// silhouette is stretched over that box. Whether the model kept the shape is read only where
/// it can be: paint outside our outline differs from the backdrop by definition, and backdrop
/// inside it has to be as plain as the backdrop and reach in from outside. A painting that
/// shares the backdrop's colours (a sunset's pink clouds, a dark street at night) is not taken
/// for backdrop, since it is neither.
pub fn cut_along_silhouette(img: &RgbaImage, silhouette: &GrayImage) -> SilhouetteCut {
    let nothing = SilhouetteCut {
        fit: 0.0,
        image: None,
    };
    let (w, h) = img.dimensions();
    let Some(backdrop) = Backdrop::measure(img) else {
        return nothing;
    };
    let at = |x: u32, y: u32| (y * w + x) as usize;
    let px = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        [p[0], p[1], p[2]]
    };
    let faint = (backdrop.noise * 2.0).max(FAINT);
    let clear = (backdrop.noise * 3.0).max(CLEAR);
    // How far each pixel is from the backdrop there, and whether it is the backdrop in shade: a
    // shadow the model casts must not count as paint around the folder, or its box grows by the
    // shadow's width.
    let mut off = vec![0f32; (w * h) as usize];
    let mut shade = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (p, k) = (px(x, y), backdrop.at(x, y));
            off[at(x, y)] = distance(&p, &k);
            shade[at(x, y)] = in_shadow(p, k);
        }
    }

    let mut rows = vec![0u32; h as usize];
    let mut cols = vec![0u32; w as usize];
    for y in 0..h {
        for x in 0..w {
            if off[at(x, y)] > faint && !shade[at(x, y)] {
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
        return nothing;
    };

    // Our silhouette, moved and stretched so its body spans the painted folder's box, then a
    // pixel in from its own edge, where the model's painting blends into the backdrop.
    let fitted = erode(&place_silhouette(silhouette, (w, h), (x0, y0, x1, y1)), 1);
    let inside: Vec<bool> = fitted.pixels().map(|p| p.0[0] > 127).collect();
    let area = inside.iter().filter(|&&i| i).count();
    if area == 0 {
        return nothing;
    }
    // Every pixel's distance to the other side of our outline, in thirds of a pixel.
    let depth_in = chamfer(&inside, w, h);
    let depth_out = chamfer(&inside.iter().map(|i| !i).collect::<Vec<_>>(), w, h);
    let side = f64::from((x1 - x0).min(y1 - y0));
    let slack = 3 * (side * OUTLINE_SLACK).round().max(3.0) as u32;
    let reach = 3 * (side * BARE_REACH).round() as u32;

    // Paint that spilled outside our outline: as little of it as found the box.
    let spilled = (0..inside.len())
        .filter(|&i| !inside[i] && depth_out[i] > slack && off[i] > faint && !shade[i])
        .count();
    // Bare backdrop inside it: as close to the backdrop as the backdrop itself is and as smooth,
    // grown in from outside our outline no deeper than `reach`. Outside our outline the backdrop
    // in shade counts too, so backdrop behind a drop shadow is still reached.
    let smooth = (backdrop.noise * 255.0 * 1.5).max(4.0);
    let bare_px = |x: u32, y: u32| off[at(x, y)] <= faint && roughness(img, x, y) <= smooth;
    let passes = |x: u32, y: u32| (!inside[at(x, y)] && shade[at(x, y)]) || bare_px(x, y);
    let mut reached = vec![false; inside.len()];
    let mut queue = std::collections::VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if !inside[at(x, y)] && passes(x, y) {
                reached[at(x, y)] = true;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < w && ny < h {
                let i = at(nx, ny);
                if !reached[i] && depth_in[i] <= reach && passes(nx, ny) {
                    reached[i] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
    let bare = (0..inside.len())
        .filter(|&i| reached[i] && inside[i] && depth_in[i] > slack)
        .count();
    let fit = (1.0 - (spilled as f64 + 2.0 * bare as f64) / area as f64).max(0.0);
    if fit < MIN_PAINTED_FIT {
        return SilhouetteCut { fit, image: None };
    }

    // The cut is our silhouette's own edge, and the painting inside it stays as painted: a pale
    // rim the model gave its folder, or a pink cloud that reaches the edge, is not the backdrop.
    // Where the model's edge sits a few pixels inside ours, the backdrop between the two edges
    // (and the model's own edge pixels, half backdrop) is painted over with the paint beside it,
    // as a designer bleeds art to a die line: the icon keeps FolderSkin's outline and shows no
    // backdrop. A pixel of paint taken for backdrop there costs nothing, since paint replaces it.
    let band = 3 * EDGE_BAND;
    let alpha_of = |i: usize| fitted.as_raw()[i];
    let backdropish = |x: u32, y: u32| off[at(x, y)] <= clear;
    let mut bled = vec![false; inside.len()];
    let mut queue = std::collections::VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            let i = at(x, y);
            if !inside[i] && (shade[i] || backdropish(x, y)) {
                bled[i] = true;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < w && ny < h {
                let i = at(nx, ny);
                if !bled[i] && depth_in[i] <= band && backdropish(nx, ny) {
                    bled[i] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
    // Only what shows is painted over.
    let over: Vec<bool> = (0..inside.len())
        .map(|i| bled[i] && alpha_of(i) > 0)
        .collect();
    // How many pixels (8-connected) each other pixel is from one painted over, up to
    // EDGE_RINGS + 1: the model's own edge, a blend of paint and backdrop a few pixels wide.
    let mut ring: Vec<u8> = over.iter().map(|&o| if o { 0 } else { u8::MAX }).collect();
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
    let mut out = RgbaImage::new(w, h);
    for (x, y, o) in out.enumerate_pixels_mut() {
        let i = at(x, y);
        let p = px(x, y);
        let mut colour = p.map(f32::from);
        if (1..=EDGE_RINGS).contains(&ring[i]) && alpha_of(i) > 0 {
            // Split the model's edge into paint and backdrop: how far the pixel is from the
            // backdrop there towards the paint just inside it.
            let (mut fore, mut n) = ([0f32; 3], 0f32);
            for ny in y.saturating_sub(EDGE_REACH)..=(y + EDGE_REACH).min(h - 1) {
                for nx in x.saturating_sub(EDGE_REACH)..=(x + EDGE_REACH).min(w - 1) {
                    let j = at(nx, ny);
                    if ring[j] > EDGE_RINGS && alpha_of(j) == 255 {
                        let q = px(nx, ny);
                        (0..3).for_each(|c| fore[c] += f32::from(q[c]));
                        n += 1.0;
                    }
                }
            }
            let k = backdrop.at(x, y).map(f32::from);
            let fore = fore.map(|v| v / n.max(1.0));
            let v = [fore[0] - k[0], fore[1] - k[1], fore[2] - k[2]];
            let vv = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            // Paint too close to the backdrop's colour to tell apart stays as it is.
            if n > 0.0 && vv >= 25.0 * 25.0 {
                let c = colour;
                let a = (((c[0] - k[0]) * v[0] + (c[1] - k[1]) * v[1] + (c[2] - k[2]) * v[2]) / vv)
                    .clamp(0.0, 1.0);
                // Mostly paint: take the backdrop's share out, which keeps the pixel's detail, but
                // no further from the paint beside it than the pixel was (dividing by `a` also
                // magnifies noise, and a magenta cast would come out green). Mostly backdrop:
                // take the paint's colour.
                colour = if a >= 0.75 {
                    [0, 1, 2].map(|j| {
                        let (lo, hi) = (c[j].min(fore[j]), c[j].max(fore[j]));
                        (k[j] + (c[j] - k[j]) / a).clamp(lo, hi)
                    })
                } else {
                    fore
                };
            }
        }
        let byte = |v: f32| v.round_ties_even().clamp(0.0, 255.0) as u8;
        o.0 = [
            byte(colour[0]),
            byte(colour[1]),
            byte(colour[2]),
            alpha_of(i),
        ];
    }
    bleed(&mut out, &over);
    // The model shades the folder's rim with the magenta around it; take that cast back out of
    // what is left of it.
    for (x, y, o) in out.enumerate_pixels_mut() {
        let k = backdrop.at(x, y).map(f32::from);
        if o.0[3] > 0 && o.0[3] < 255 && k[0].min(k[2]) - k[1] > 100.0 {
            let c = [o.0[0], o.0[1], o.0[2]].map(f32::from);
            let spill = (c[0].min(c[2]) - c[1]).max(0.0);
            o.0[0] = (c[0] - spill).round() as u8;
            o.0[2] = (c[2] - spill).round() as u8;
        }
    }
    SilhouetteCut {
        fit,
        image: Some(out),
    }
}

/// Paints every pixel of `img` marked in `over` with the colours of the pixels around it that
/// aren't, ring by ring inwards from them: each takes the mean of its neighbours already painted.
/// Alpha stays as it is.
fn bleed(img: &mut RgbaImage, over: &[bool]) {
    let (w, h) = img.dimensions();
    let at = |x: u32, y: u32| (y * w + x) as usize;
    let mut done: Vec<bool> = over.iter().map(|o| !o).collect();
    let mut front: Vec<(u32, u32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| over[at(x, y)])
        .collect();
    while !front.is_empty() {
        let mut painted = Vec::new();
        let mut waiting = Vec::new();
        for &(x, y) in &front {
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    if done[at(nx, ny)] && img.get_pixel(nx, ny).0[3] > 0 {
                        let q = img.get_pixel(nx, ny).0;
                        (0..3).for_each(|c| sum[c] += u32::from(q[c]));
                        n += 1;
                    }
                }
            }
            if n > 0 {
                painted.push((x, y, sum.map(|v| ((v + n / 2) / n) as u8)));
            } else {
                waiting.push((x, y));
            }
        }
        if painted.is_empty() {
            break;
        }
        for (x, y, c) in painted {
            let p = img.get_pixel_mut(x, y);
            p.0 = [c[0], c[1], c[2], p.0[3]];
            done[at(x, y)] = true;
        }
        front = waiting;
    }
}

/// True when `p` is the backdrop `k` in shade, or lit a little brighter than it was measured:
/// `k` scaled, give or take JPEG noise. Only on that line from black: a dusty pink backdrop points
/// nearly the same way in RGB as grey paint does, and a shadow is darker pink, not grey.
fn in_shadow(p: [u8; 3], k: [u8; 3]) -> bool {
    let (p, k) = (p.map(f32::from), k.map(f32::from));
    let kk = k[0] * k[0] + k[1] * k[1] + k[2] * k[2];
    if kk < 1.0 {
        return false;
    }
    let s = (p[0] * k[0] + p[1] * k[1] + p[2] * k[2]) / kk;
    let off = [p[0] - s * k[0], p[1] - s * k[1], p[2] - s * k[2]];
    let rms = ((off[0] * off[0] + off[1] * off[1] + off[2] * off[2]) / 3.0).sqrt();
    (SHADOW_DEPTH..=SHADOW_LIGHT).contains(&s) && rms <= SHADOW_NOISE
}

/// How busy the picture is around `(x, y)`: the largest channel's standard deviation over the
/// 5 x 5 pixels around it. The backdrop is smooth; paint rarely is.
fn roughness(img: &RgbaImage, x: u32, y: u32) -> f32 {
    let (w, h) = img.dimensions();
    let (xs, ys) = (
        x.saturating_sub(2)..(x + 3).min(w),
        y.saturating_sub(2)..(y + 3).min(h),
    );
    let (mut sum, mut squares, mut n) = ([0f32; 3], [0f32; 3], 0f32);
    for yy in ys {
        for xx in xs.clone() {
            let p = img.get_pixel(xx, yy).0;
            for c in 0..3 {
                let v = f32::from(p[c]);
                sum[c] += v;
                squares[c] += v * v;
            }
            n += 1.0;
        }
    }
    (0..3)
        .map(|c| {
            (squares[c] / n - (sum[c] / n) * (sum[c] / n))
                .max(0.0)
                .sqrt()
        })
        .fold(0.0, f32::max)
}

/// Each pixel's distance to the nearest pixel outside `set`, in thirds of a pixel (the 3-4
/// chamfer: 3 along a row or column, 4 diagonally); 0 outside it. Pixels beyond the frame count
/// as inside, so the frame's edge doesn't stop a set that reaches it.
fn chamfer(set: &[bool], w: u32, h: u32) -> Vec<u32> {
    let (w, h) = (w as usize, h as usize);
    let far = u32::MAX / 2;
    let mut d: Vec<u32> = set.iter().map(|&s| if s { far } else { 0 }).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if d[i] == 0 {
                continue;
            }
            let mut best = d[i];
            if x > 0 {
                best = best.min(d[i - 1] + 3);
            }
            if y > 0 {
                best = best.min(d[i - w] + 3);
                if x > 0 {
                    best = best.min(d[i - w - 1] + 4);
                }
                if x + 1 < w {
                    best = best.min(d[i - w + 1] + 4);
                }
            }
            d[i] = best;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            if d[i] == 0 {
                continue;
            }
            let mut best = d[i];
            if x + 1 < w {
                best = best.min(d[i + 1] + 3);
            }
            if y + 1 < h {
                best = best.min(d[i + w] + 3);
                if x + 1 < w {
                    best = best.min(d[i + w + 1] + 4);
                }
                if x > 0 {
                    best = best.min(d[i + w - 1] + 4);
                }
            }
            d[i] = best;
        }
    }
    d
}

/// `silhouette` moved and stretched so that its body, where it is at least half opaque, spans the
/// box `(x0, y0, x1, y1)`, on a black frame of `size`. The box is where the painted folder's
/// edge pixels are half paint, so the body lines up with it, and the silhouette's soft rim goes
/// with it, just outside the box.
fn place_silhouette(
    silhouette: &GrayImage,
    size: (u32, u32),
    (x0, y0, x1, y1): (u32, u32, u32, u32),
) -> GrayImage {
    let mut frame = GrayImage::new(size.0, size.1);
    let (mut sx0, mut sy0, mut sx1, mut sy1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in silhouette.enumerate_pixels() {
        if p.0[0] > 127 {
            (sx0, sy0) = (sx0.min(x), sy0.min(y));
            (sx1, sy1) = (sx1.max(x + 1), sy1.max(y + 1));
        }
    }
    if sx0 >= sx1 || x1 <= x0 || y1 <= y0 {
        return frame;
    }
    let scale = (
        f64::from(x1 - x0) / f64::from(sx1 - sx0),
        f64::from(y1 - y0) / f64::from(sy1 - sy0),
    );
    let (sw, sh) = silhouette.dimensions();
    let resized = imageops::resize(
        silhouette,
        (f64::from(sw) * scale.0).round().max(1.0) as u32,
        (f64::from(sh) * scale.1).round().max(1.0) as u32,
        FilterType::Lanczos3,
    );
    let offset = (
        (f64::from(x0) - f64::from(sx0) * scale.0).round() as i64,
        (f64::from(y0) - f64::from(sy0) * scale.1).round() as i64,
    );
    imageops::replace(&mut frame, &resized, offset.0, offset.1);
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
        assert!(cut.fit >= MIN_PAINTED_FIT, "fit {}", cut.fit);
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
        assert!(cut.fit < MIN_PAINTED_FIT, "fit {}", cut.fit);
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
