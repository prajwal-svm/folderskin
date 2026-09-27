//! A black-and-white picture pressed into the folder: a symbol, a logo or a letter.
//!
//! It is drawn exactly as the composer draws an icon pressed in (`src/composer/render.ts`,
//! `pressIn`, and the colours in `src/composer/color.ts`): a deeper shade of the folder's own
//! colour, so the mark belongs to the folder and still reads, a lit lip just below it, and a
//! shaded band along its top edge, as if the mark were stamped into the front panel. It lands
//! where the composer puts a first icon: in the middle of the folder's front, [`ICON_SIZE`] canvas
//! units across at most.
//!
//! [`coverage`] reads the picture: its alpha where its background is see-through, and otherwise
//! how far each pixel is from the background's shade, so black on white and white on black both
//! work. [`design`] lays the mark and the folder's colour out on the icon canvas, to be placed on
//! a folder with [`crate::compositor::render_placed_icon_set_with`].

use crate::compositor::{Style, RENDER_SIZE};
use crate::{geometry as g, geometry_linux as gl, geometry_windows as gw};
use image::{imageops, GrayImage, Luma, RgbaImage};

/// How wide and tall a mark may be on the canvas, as the composer sizes a first icon.
pub const ICON_SIZE: f32 = 340.0;
/// How deep a mark is pressed in by default, 0 to 100, as the composer has it.
pub const DEPTH: f32 = 60.0;
/// Coverage at or under this (0..=255) is background when a mark's margins are trimmed.
const TRIM_BELOW: u8 = 8;

/// The colour macOS gives a plain folder at its foot, which a mark on the default folder takes
/// its shade from, as the composer's `FOLDER_BLUE_BOTTOM`.
const FOLDER_BLUE_BOTTOM: [u8; 3] = [0x4E, 0xA9, 0xE4];

/// How much of each pixel of `img` is the mark, 0 to 255, trimmed to the mark with `trim`. A
/// picture whose edge is mostly see-through gives its alpha. An opaque one gives how far each
/// pixel's shade is from its edge's, so a black mark on white and a white one on black are both
/// marks. `None` when there's no mark: a blank or empty picture.
pub fn coverage(img: &RgbaImage, trim: bool) -> Option<GrayImage> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let edge: Vec<[u8; 4]> = (0..w)
        .flat_map(|x| [(x, 0), (x, h - 1)])
        .chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]))
        .map(|(x, y)| img.get_pixel(x, y).0)
        .collect();
    let clear = edge.iter().filter(|p| p[3] < 128).count() * 2 > edge.len();
    let luma = |p: [u8; 4]| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
    let cover = if clear {
        GrayImage::from_fn(w, h, |x, y| Luma([img.get_pixel(x, y).0[3]]))
    } else {
        let ground = edge.iter().map(|&p| luma(p)).sum::<f32>() / edge.len() as f32;
        // As far as the picture goes from its ground's shade, towards black or white.
        let span = ground.max(255.0 - ground).max(1.0);
        GrayImage::from_fn(w, h, |x, y| {
            let p = img.get_pixel(x, y).0;
            let away = (luma(p) - ground).abs() / span * (p[3] as f32 / 255.0);
            Luma([(away * 255.0).round().clamp(0.0, 255.0) as u8])
        })
    };
    if !trim {
        return cover.pixels().any(|p| p.0[0] > TRIM_BELOW).then_some(cover);
    }
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in cover.enumerate_pixels() {
        if p.0[0] > TRIM_BELOW {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (x0 <= x1).then(|| imageops::crop_imm(&cover, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image())
}

/// What the folder is filled with behind the mark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    /// The folder's own colours, as the app draws a plain folder of that look.
    Plain,
    /// One colour.
    Solid([u8; 3]),
}

/// The design the composer would make of `mark` ([`coverage`]) pressed `depth` deep (0 to 100)
/// into a folder of `style` filled with `fill`: the fill everywhere, and the mark in the middle of
/// the folder's front, fitted into [`ICON_SIZE`] canvas units. [`RENDER_SIZE`] pixels square, for
/// [`crate::compositor::render_placed_icon_set_with`].
pub fn design(mark: &GrayImage, fill: Fill, style: Style, depth: f32) -> RgbaImage {
    let size = RENDER_SIZE;
    let k = size as f32 / g::CANVAS;
    let (top, bottom) = match fill {
        Fill::Solid(c) => (c, c),
        Fill::Plain => plain_colours(style),
    };
    // The folder's colour, down its height as the plain folder's runs.
    let (from, to) = (folder_top(style) * k, front_of(style).y1 * k);
    let mut out = RgbaImage::from_fn(size, size, |_, y| {
        let t = ((y as f32 - from) / (to - from)).clamp(0.0, 1.0);
        let c: [f32; 3] =
            std::array::from_fn(|i| top[i] as f32 + (bottom[i] as f32 - top[i] as f32) * t);
        image::Rgba([
            c[0].round() as u8,
            c[1].round() as u8,
            c[2].round() as u8,
            255,
        ])
    });

    let (mw, mh) = mark.dimensions();
    if mw == 0 || mh == 0 {
        return out;
    }
    // Fitted into the icon's box, in pixels, and centred on the front.
    let box_px = ICON_SIZE * k;
    let scale = box_px / mw.max(mh) as f32;
    let (w, h) = (
        ((mw as f32 * scale).round() as u32).max(1),
        ((mh as f32 * scale).round() as u32).max(1),
    );
    let sized = imageops::resize(mark, w, h, imageops::FilterType::CatmullRom);
    let front = front_of(style);
    let (cx, cy) = (
        (front.x0 + front.x1) / 2.0 * k,
        (front.y0 + front.y1) / 2.0 * k,
    );
    let (left, top_px) = (
        (cx - w as f32 / 2.0).round() as i64,
        (cy - h as f32 / 2.0).round() as i64,
    );

    // The mark's shade, and the lip and the band, from the folder's colour where the mark sits.
    let under = if fill == Fill::Plain { bottom } else { top };
    let shade_of_folder = if fill == Fill::Plain && style == Style::Mac {
        FOLDER_BLUE_BOTTOM
    } else {
        under
    };
    let tint = tint(shade_of_folder);
    let (lip, band) = light(shade_of_folder);
    // How far the lip and the band reach: the composer's `embossReach`, at least a pixel.
    let reach = (box_px * 0.011 * depth.clamp(0.0, 100.0) / 60.0).max(1.0);

    let cover = |x: i64, y: f32| -> f32 {
        // The mark's coverage at column `x` and a row that may fall between two, blended.
        let (y0, t) = (y.floor(), y - y.floor());
        let at = |row: i64| -> f32 {
            if x < 0 || row < 0 || x >= w as i64 || row >= h as i64 {
                0.0
            } else {
                sized.get_pixel(x as u32, row as u32).0[0] as f32 / 255.0
            }
        };
        at(y0 as i64) * (1.0 - t) + at(y0 as i64 + 1) * t
    };
    let reach_px = reach.ceil() as i64 + 1;
    for y in (top_px - 1).max(0)..(top_px + h as i64 + reach_px).min(size as i64) {
        for x in left.max(0)..(left + w as i64).min(size as i64) {
            let (mx, my) = (x - left, (y - top_px) as f32);
            let mark_here = cover(mx, my);
            // The lip: the mark moved down by the reach, showing below it.
            let lip_here = cover(mx, my - reach);
            // The band: the mark less itself moved down by three quarters of the reach, its top edge.
            let band_here = (mark_here * (1.0 - cover(mx, my - reach * 0.75))).max(0.0);
            if lip_here <= 0.0 && mark_here <= 0.0 {
                continue;
            }
            let p = out.get_pixel_mut(x as u32, y as u32);
            let mut c = [p.0[0] as f32, p.0[1] as f32, p.0[2] as f32];
            let over = |c: &mut [f32; 3], colour: [u8; 3], alpha: f32| {
                for i in 0..3 {
                    c[i] = c[i] * (1.0 - alpha) + colour[i] as f32 * alpha;
                }
            };
            over(&mut c, [255, 255, 255], lip_here * lip);
            over(&mut c, tint, mark_here);
            over(&mut c, [0, 0, 0], band_here * band);
            p.0 = [
                c[0].round() as u8,
                c[1].round() as u8,
                c[2].round() as u8,
                255,
            ];
        }
    }
    out
}

/// The top of the folder of `style`, in canvas units: where its tab starts.
fn folder_top(style: Style) -> f32 {
    match style {
        Style::Mac => g::TAB_TOP,
        Style::Windows => gw::TAB_TOP,
        Style::Linux => gl::TAB_TOP,
    }
}

/// The front panel of the folder of `style`, in canvas units.
fn front_of(style: Style) -> g::Rect {
    match style {
        Style::Mac => g::FRONT,
        Style::Windows => gw::FRONT,
        Style::Linux => gl::FRONT,
    }
}

/// The plain folder's colours of `style`, top and foot: the app's own blue on a Mac, the yellow
/// Explorer draws its folders in, and the blue GNOME's and KDE's share.
fn plain_colours(style: Style) -> ([u8; 3], [u8; 3]) {
    match style {
        Style::Mac => ([0x7C, 0xC8, 0xF5], [0x4E, 0xA9, 0xE4]),
        Style::Windows => ([0xFF, 0xE6, 0x9A], [0xFF, 0xCC, 0x48]),
        Style::Linux => ([0x6C, 0xAA, 0xF2], [0x2F, 0x7B, 0xE0]),
    }
}

/// How light a colour looks, 0 to 1, as the composer's `luminance` (WCAG's relative luminance).
fn luminance(c: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// `t` of the way from `a` to `b`, rounded as the composer's `mix` and `toHex` round.
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

/// The shade of a mark pressed into a folder of colour `folder`, as the composer's `embossTint`:
/// deeper than the folder, or on a very dark folder lighter, so it reads and still belongs.
pub fn tint(folder: [u8; 3]) -> [u8; 3] {
    let l = luminance(folder);
    if l < 0.06 {
        return mix(folder, [255, 255, 255], 0.26);
    }
    let deeper = if l > 0.6 {
        0.3
    } else if l > 0.25 {
        0.24
    } else {
        0.2
    };
    mix(folder, [0x0C, 0x1A, 0x2C], deeper)
}

/// How strong the lit lip (white) and the shaded band (black) are on a folder of colour `folder`,
/// 0 to 1, as the composer's `embossLight`.
pub fn light(folder: [u8; 3]) -> (f32, f32) {
    let l = luminance(folder);
    let lip = if l < 0.06 {
        0x24
    } else if l < 0.25 {
        0x4D
    } else {
        0x99
    };
    let band = if l < 0.06 { 0x59 } else { 0x3D };
    (lip as f32 / 255.0, band as f32 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A black disc on `ground`, `size` px square, the disc `r` px across its radius.
    fn disc(size: u32, r: f32, ground: Rgba<u8>, ink: Rgba<u8>) -> RgbaImage {
        let c = size as f32 / 2.0;
        RgbaImage::from_fn(size, size, |x, y| {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            if d <= r {
                ink
            } else {
                ground
            }
        })
    }

    #[test]
    fn a_mark_is_read_from_its_alpha_or_its_shade() {
        let clear = Rgba([0, 0, 0, 0]);
        let black = Rgba([0, 0, 0, 255]);
        let white = Rgba([255, 255, 255, 255]);
        // Black on see-through, black on white and white on black are the same mark, trimmed.
        for (ground, ink) in [(clear, black), (white, black), (black, white)] {
            let cover = coverage(&disc(100, 20.0, ground, ink), true).expect("a mark");
            assert_eq!(cover.dimensions(), (40, 40), "{ground:?} / {ink:?}");
            assert_eq!(cover.get_pixel(20, 20).0[0], 255);
            assert_eq!(cover.get_pixel(0, 0).0[0], 0);
        }
        // Untrimmed, it keeps its margins.
        let cover = coverage(&disc(100, 20.0, clear, black), false).unwrap();
        assert_eq!(cover.dimensions(), (100, 100));
        // Nothing drawn is no mark.
        assert!(coverage(&RgbaImage::from_pixel(50, 50, white), true).is_none());
        assert!(coverage(&RgbaImage::new(0, 0), true).is_none());
    }

    #[test]
    fn the_shade_and_the_light_follow_the_composer() {
        // The values src/composer/color.ts gives for the same folders.
        assert_eq!(tint(FOLDER_BLUE_BOTTOM), [0x3E, 0x87, 0xB8]);
        assert_eq!(tint([0x10, 0x10, 0x10]), [0x4E, 0x4E, 0x4E]);
        assert_eq!(
            light(FOLDER_BLUE_BOTTOM),
            (0x99 as f32 / 255.0, 0x3D as f32 / 255.0)
        );
        assert_eq!(
            light([0x10, 0x10, 0x10]),
            (0x24 as f32 / 255.0, 0x59 as f32 / 255.0)
        );
    }

    #[test]
    fn a_mark_is_pressed_into_the_middle_of_the_front() {
        let mark = coverage(
            &disc(200, 90.0, Rgba([0, 0, 0, 0]), Rgba([0, 0, 0, 255])),
            true,
        )
        .unwrap();
        let out = design(&mark, Fill::Solid([0x4E, 0xA9, 0xE4]), Style::Mac, DEPTH);
        assert_eq!(out.dimensions(), (RENDER_SIZE, RENDER_SIZE));
        let k = RENDER_SIZE as f32 / g::CANVAS;
        let (cx, cy) = (
            ((g::FRONT.x0 + g::FRONT.x1) / 2.0 * k) as u32,
            ((g::FRONT.y0 + g::FRONT.y1) / 2.0 * k) as u32,
        );
        // The middle is the mark's shade, the corner the folder's colour.
        assert_eq!(out.get_pixel(cx, cy).0, [0x3E, 0x87, 0xB8, 255]);
        assert_eq!(out.get_pixel(5, 5).0, [0x4E, 0xA9, 0xE4, 255]);
        // Just under the mark's foot, the lip is lighter than the folder; just inside its top
        // edge, the band is darker than the mark.
        let half = (ICON_SIZE * k / 2.0) as u32;
        let lip = out.get_pixel(cx, cy + half + 3).0;
        assert!(lip[0] > 0x4E && lip[1] > 0xA9, "lip {lip:?}");
        let band = out.get_pixel(cx, cy - half + 2).0;
        assert!(band[2] < 0xB8, "band {band:?}");
    }

    #[test]
    fn the_plain_folder_runs_from_its_top_colour_to_its_foot() {
        let mark = coverage(
            &disc(64, 20.0, Rgba([0, 0, 0, 0]), Rgba([0, 0, 0, 255])),
            true,
        )
        .unwrap();
        let out = design(&mark, Fill::Plain, Style::Windows, DEPTH);
        let k = RENDER_SIZE as f32 / g::CANVAS;
        assert_eq!(out.get_pixel(3, 0).0, [0xFF, 0xE6, 0x9A, 255]);
        assert_eq!(
            out.get_pixel(3, (gw::FRONT.y1 * k) as u32).0,
            [0xFF, 0xCC, 0x48, 255]
        );
    }
}
