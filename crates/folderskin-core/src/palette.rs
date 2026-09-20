//! What colour a skin is, read from its own pixels, for the gallery's colour filter: up to three
//! colour names that each cover a fair part of the folder, and whether it's light or dark overall.
//!
//! This is the same classification the gallery used to run in the webview on every thumbnail it
//! had never seen. Doing it here, once, where the thumbnail is already in memory, saves the web
//! view decoding every picture in the library at startup — the single largest thing it did.
//! `src/lib/palette.ts` keeps the colour list and the swatches the filter draws; the naming and
//! the counting live here, and the tests below are the ones that were written against it.

use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Bump when the naming or the counting changes, so skins classified by an older reading are read
/// again rather than left with a palette this version would not have given them.
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Colour {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    Brown,
    Black,
    Grey,
    White,
}

/// Every colour that can be named, in the order the filter lists them.
pub const COLOURS: [Colour; 12] = [
    Colour::Red,
    Colour::Orange,
    Colour::Yellow,
    Colour::Green,
    Colour::Teal,
    Colour::Blue,
    Colour::Purple,
    Colour::Pink,
    Colour::Brown,
    Colour::Black,
    Colour::Grey,
    Colour::White,
];

impl Colour {
    fn index(self) -> usize {
        COLOURS.iter().position(|c| *c == self).expect("a listed colour")
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Palette {
    pub colours: Vec<Colour>,
    pub tone: Tone,
}

/// Side of the square a picture is sampled down to before it's read: plenty for its main colours,
/// and the same size the webview used, so a skin keeps the colours it was filed under.
pub const SAMPLE: u32 = 48;

/// A colour counts when at least this share of the folder is that colour.
const MIN_SHARE: f32 = 0.12;
const MAX_COLOURS: usize = 3;
/// Below this average brightness (0 to 1) a skin is dark.
const DARK_BELOW: f64 = 0.45;

/// The colour name a person would give one pixel.
pub fn colour_of(r: u8, g: u8, b: u8) -> Colour {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let value = max as f32 / 255.0;
    let saturation = if max == 0 {
        0.0
    } else {
        (max - min) as f32 / max as f32
    };
    if value < 0.16 {
        return Colour::Black;
    }
    if saturation < 0.16 || max - min < 28 {
        if value > 0.86 {
            return Colour::White;
        }
        if value < 0.3 {
            return Colour::Black;
        }
        return Colour::Grey;
    }
    let hue = hue_of(r, g, b, max, min);
    let pastel = value > 0.88 && saturation < 0.5;
    // A pale red is what people call pink.
    if hue < 12.0 || hue >= 345.0 {
        return if pastel { Colour::Pink } else { Colour::Red };
    }
    // Browns are dark or muted oranges and yellows: wood, earth, old varnish. Pale ones (peach,
    // apricot) stay orange.
    if hue < 42.0 {
        return if value < 0.62 || (saturation < 0.45 && !pastel) {
            Colour::Brown
        } else {
            Colour::Orange
        };
    }
    if hue < 68.0 {
        return if value < 0.5 {
            Colour::Brown
        } else {
            Colour::Yellow
        };
    }
    if hue < 160.0 {
        return Colour::Green;
    }
    if hue < 195.0 {
        return Colour::Teal;
    }
    if hue < 255.0 {
        return Colour::Blue;
    }
    if hue < 290.0 {
        return Colour::Purple;
    }
    Colour::Pink
}

/// Tested red, then green, then blue, as the webview tests them, so a pixel with two channels
/// equally high is named the same way here.
fn hue_of(r: u8, g: u8, b: u8, max: u8, min: u8) -> f32 {
    let d = (max - min) as f32;
    let (rf, gf, bf) = (r as f32, g as f32, b as f32);
    let h = if max == r {
        ((gf - bf) / d) % 6.0
    } else if max == g {
        (bf - rf) / d + 2.0
    } else {
        (rf - gf) / d + 4.0
    };
    (h * 60.0 + 360.0) % 360.0
}

/// The palette of an image from its RGBA pixels. Pixels that are mostly see-through (around the
/// folder's shape) don't count.
pub fn of_pixels(rgba: &[u8]) -> Palette {
    let mut counts = [0usize; COLOURS.len()];
    // Which colour was met first, so colours that tie are ranked as the webview's stable sort
    // over an insertion-ordered map ranked them.
    let mut first_seen = [usize::MAX; COLOURS.len()];
    let mut seen = 0usize;
    let mut light = 0.0f64;
    for px in rgba.as_chunks::<4>().0 {
        if px[3] < 128 {
            continue;
        }
        let (r, g, b) = (px[0], px[1], px[2]);
        let i = colour_of(r, g, b).index();
        if counts[i] == 0 {
            first_seen[i] = seen;
        }
        counts[i] += 1;
        light += (0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64) / 255.0;
        seen += 1;
    }
    if seen == 0 {
        return Palette {
            colours: Vec::new(),
            tone: Tone::Light,
        };
    }
    let mut ranked: Vec<(Colour, usize)> = COLOURS
        .iter()
        .enumerate()
        .filter(|(i, _)| counts[*i] > 0)
        .map(|(i, c)| (*c, counts[i]))
        .collect();
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| first_seen[a.0.index()].cmp(&first_seen[b.0.index()]))
    });
    let colours = ranked
        .iter()
        .enumerate()
        .filter(|(rank, (_, n))| *rank == 0 || *n as f32 / seen as f32 >= MIN_SHARE)
        .map(|(_, (c, _))| *c)
        .take(MAX_COLOURS)
        .collect();
    Palette {
        colours,
        tone: if (light / seen as f64) < DARK_BELOW {
            Tone::Dark
        } else {
            Tone::Light
        },
    }
}

/// The palette of a picture, sampled down to [`SAMPLE`] square first — as the gallery did in a
/// canvas — so a large picture costs no more to read than a small one.
pub fn of_picture(img: &RgbaImage) -> Palette {
    if img.width() == SAMPLE && img.height() == SAMPLE {
        return of_pixels(img.as_raw());
    }
    let small = image::imageops::resize(
        img,
        SAMPLE,
        SAMPLE,
        image::imageops::FilterType::Triangle,
    );
    of_pixels(small.as_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RGBA pixels: `count` of each colour, in order.
    fn pixels(runs: &[(usize, [u8; 4])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (count, px) in runs {
            for _ in 0..*count {
                out.extend_from_slice(px);
            }
        }
        out
    }

    fn opaque(count: usize, rgb: [u8; 3]) -> (usize, [u8; 4]) {
        (count, [rgb[0], rgb[1], rgb[2], 255])
    }

    #[test]
    fn names_the_colours_people_would() {
        assert_eq!(colour_of(229, 72, 77), Colour::Red);
        assert_eq!(colour_of(247, 107, 21), Colour::Orange);
        assert_eq!(colour_of(245, 197, 24), Colour::Yellow);
        assert_eq!(colour_of(48, 164, 108), Colour::Green);
        assert_eq!(colour_of(18, 165, 148), Colour::Teal);
        assert_eq!(colour_of(58, 134, 255), Colour::Blue);
        assert_eq!(colour_of(142, 78, 198), Colour::Purple);
        assert_eq!(colour_of(233, 61, 130), Colour::Pink);
    }

    #[test]
    fn calls_dark_and_muted_oranges_brown_and_greys_by_how_light_they_are() {
        assert_eq!(colour_of(110, 70, 35), Colour::Brown);
        assert_eq!(colour_of(210, 180, 140), Colour::Brown);
        assert_eq!(colour_of(100, 90, 20), Colour::Brown);
        assert_eq!(colour_of(12, 12, 14), Colour::Black);
        assert_eq!(colour_of(128, 128, 132), Colour::Grey);
        assert_eq!(colour_of(245, 244, 240), Colour::White);
    }

    #[test]
    fn calls_a_pale_red_pink_and_keeps_pale_oranges_orange() {
        assert_eq!(colour_of(250, 200, 205), Colour::Pink);
        assert_eq!(colour_of(250, 218, 196), Colour::Orange);
    }

    #[test]
    fn has_a_name_for_every_corner_of_the_cube() {
        for rgb in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [120, 60, 20],
            [128, 128, 128],
        ] {
            assert!(COLOURS.contains(&colour_of(rgb[0], rgb[1], rgb[2])), "{rgb:?}");
        }
    }

    #[test]
    fn lists_the_colours_that_cover_a_fair_part_of_the_picture_biggest_first() {
        let p = of_pixels(&pixels(&[
            opaque(60, [58, 134, 255]),
            opaque(30, [245, 197, 24]),
            opaque(10, [229, 72, 77]),
        ]));
        assert_eq!(p.colours, vec![Colour::Blue, Colour::Yellow]);
    }

    #[test]
    fn keeps_at_most_three_and_always_the_biggest() {
        let p = of_pixels(&pixels(&[
            opaque(25, [58, 134, 255]),
            opaque(25, [245, 197, 24]),
            opaque(25, [229, 72, 77]),
            opaque(25, [48, 164, 108]),
        ]));
        assert_eq!(p.colours.len(), 3);
        let one = of_pixels(&pixels(&[opaque(100, [58, 134, 255])]));
        assert_eq!(one.colours, vec![Colour::Blue]);
    }

    #[test]
    fn ignores_the_see_through_pixels_around_the_folder() {
        let p = of_pixels(&pixels(&[(90, [0, 0, 0, 0]), opaque(10, [48, 164, 108])]));
        assert_eq!(p.colours, vec![Colour::Green]);
    }

    #[test]
    fn says_whether_the_picture_is_light_or_dark_overall() {
        assert_eq!(of_pixels(&pixels(&[opaque(100, [20, 24, 40])])).tone, Tone::Dark);
        assert_eq!(of_pixels(&pixels(&[opaque(100, [245, 197, 24])])).tone, Tone::Light);
        assert_eq!(
            of_pixels(&[]),
            Palette {
                colours: Vec::new(),
                tone: Tone::Light
            }
        );
    }

    #[test]
    fn a_picture_is_sampled_down_before_it_is_read() {
        let blue = RgbaImage::from_pixel(256, 256, image::Rgba([58, 134, 255, 255]));
        assert_eq!(of_picture(&blue).colours, vec![Colour::Blue]);
        let small = RgbaImage::from_pixel(SAMPLE, SAMPLE, image::Rgba([48, 164, 108, 255]));
        assert_eq!(of_picture(&small).colours, vec![Colour::Green]);
    }

    #[test]
    fn the_names_serialise_the_way_the_filter_reads_them() {
        let json = serde_json::to_string(&Palette {
            colours: vec![Colour::Blue, Colour::White],
            tone: Tone::Dark,
        })
        .unwrap();
        assert_eq!(json, r#"{"colours":["blue","white"],"tone":"dark"}"#);
    }
}
