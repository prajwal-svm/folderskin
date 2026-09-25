//! Pixel plumbing: premultiplied buffers, Lanczos3 downsampling, and PNG and WebP encoding.
//!
//! tiny-skia renders into premultiplied RGBA8, and resampling has to happen on premultiplied
//! channels or transparent pixels bleed their (black) colour into the edges. So the master
//! render stays premultiplied through every downsample and is converted to straight alpha only
//! at the very end, on the way into an `image::RgbaImage`.
//!
//! WebP comes from libwebp (the `webp` crate): lossless for pictures that are kept, such as a
//! pack's pictures and the library's, and lossy only for previews nobody keeps.

use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba, RgbaImage};

/// An RGBA8 buffer with premultiplied alpha — the same layout as `tiny_skia::Pixmap`.
#[derive(Clone, Debug)]
pub struct Premul {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Premul {
    /// Wraps the buffer as an `image` view for resampling. Panics if `data` is the wrong length.
    fn view(&self) -> ImageBuffer<Rgba<u8>, &[u8]> {
        ImageBuffer::from_raw(self.width, self.height, self.data.as_slice())
            .expect("premultiplied buffer length matches its dimensions")
    }
}

/// Resamples to `size`×`size` with Lanczos3, on the premultiplied channels.
pub fn downsample(src: &Premul, size: u32) -> Premul {
    let out = image::imageops::resize(
        &src.view(),
        size,
        size,
        image::imageops::FilterType::Lanczos3,
    );
    Premul {
        width: size,
        height: size,
        data: out.into_raw(),
    }
}

/// Converts premultiplied RGBA8 to the straight-alpha RGBA8 that PNG (and `image`) expect.
pub fn to_straight_rgba(p: &Premul) -> RgbaImage {
    let mut out = RgbaImage::new(p.width, p.height);
    let (pixels, _) = p.data.as_chunks::<4>();
    for (px, chunk) in out.pixels_mut().zip(pixels) {
        let a = chunk[3];
        if a == 0 {
            continue; // already (0, 0, 0, 0)
        }
        let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        *px = Rgba([un(chunk[0]), un(chunk[1]), un(chunk[2]), a]);
    }
    out
}

/// Converts straight-alpha RGBA8 into a premultiplied buffer tiny-skia can use as a pattern.
pub fn straight_to_premul(img: &RgbaImage) -> Premul {
    let mut data = Vec::with_capacity(img.as_raw().len());
    for px in img.pixels() {
        let a = px.0[3];
        let pm = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
        data.extend_from_slice(&[pm(px.0[0]), pm(px.0[1]), pm(px.0[2]), a]);
    }
    Premul {
        width: img.width(),
        height: img.height(),
        data,
    }
}

/// `img` with its longer side at most `max_side`, resampled with Lanczos3 and keeping its shape.
/// A picture that is small enough already comes back as it is.
pub fn shrink_to(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (nw, nh) = shrunk_size(w, h, max_side);
    if (nw, nh) == (w, h) {
        return img;
    }
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3)
}

/// The size [`shrink_to`] makes a `w`×`h` picture.
pub fn shrunk_size(w: u32, h: u32, max_side: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_side {
        return (w, h);
    }
    let s = max_side as f32 / longest as f32;
    (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    )
}

/// Encodes an image as a PNG, compressed as hard as the encoder can (icons are written once
/// and read forever).
pub fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    PngEncoder::new_with_quality(&mut buf, CompressionType::Best, PngFilterType::Adaptive)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("encoding a PNG into memory cannot fail");
    buf
}

/// Encodes `img` as a lossless WebP at libwebp's method 5, quality 75. What a pack's pictures are
/// shared as. On 1024 px renders that is within half a percent of the smallest file libwebp makes
/// (`cwebp -lossless -z 9`: method 6, quality 100), in about a twentieth of the time: well under
/// a second a picture rather than eleven.
///
/// Every pixel's alpha comes back exactly, and the colour of every pixel that shows; only the
/// colour hidden under fully transparent pixels may change (libwebp's `exact` off), which lets a
/// cut-out folder's clear surround cost almost nothing. A picture with no transparency is encoded
/// without an alpha channel.
pub fn encode_webp_lossless(img: &RgbaImage) -> Vec<u8> {
    encode_lossless(img, 5, 75.0)
}

/// The same pixels as [`encode_webp_lossless`], at libwebp's fastest lossless setting (method 0,
/// quality 0): about a tenth bigger, in about the time a PNG takes, and still about a third
/// smaller than one. For pictures kept as they arrive, where waiting seconds for a smaller file
/// isn't worth it.
pub fn encode_webp_lossless_quick(img: &RgbaImage) -> Vec<u8> {
    encode_lossless(img, 0, 0.0)
}

/// `img` as a lossless WebP at libwebp's `method` (0 to 6) and `quality`, which in lossless mode
/// is how hard it looks for a smaller file rather than how much it keeps.
fn encode_lossless(img: &RgbaImage, method: i32, quality: f32) -> Vec<u8> {
    let mut config = webp::WebPConfig::new().expect("libwebp's default settings are valid");
    config.lossless = 1;
    config.method = method;
    config.quality = quality;
    config.exact = 0;
    encode_webp(img, &config)
}

/// Encodes `img` as a lossy WebP at `quality` (0 to 100), with lossless alpha: for previews such
/// as a contact sheet, which are looked at once and never kept.
pub fn encode_webp_lossy(img: &RgbaImage, quality: f32) -> Vec<u8> {
    let mut config = webp::WebPConfig::new().expect("libwebp's default settings are valid");
    config.quality = quality.clamp(0.0, 100.0);
    config.method = 6;
    config.alpha_quality = 100;
    encode_webp(img, &config)
}

/// `img` encoded by libwebp with `config`: as RGB when every pixel is opaque, so the file has no
/// alpha channel, and as RGBA otherwise. libwebp takes any picture from 1 to 16,383 px a side,
/// which every picture FolderSkin makes is.
fn encode_webp(img: &RgbaImage, config: &webp::WebPConfig) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let rgb;
    let encoder = if img.pixels().all(|p| p.0[3] == 255) {
        rgb = image::DynamicImage::ImageRgba8(img.clone()).into_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), w, h)
    } else {
        webp::Encoder::from_rgba(img.as_raw(), w, h)
    };
    encoder
        .encode_advanced(config)
        .map(|webp| webp.to_vec())
        .unwrap_or_else(|e| panic!("libwebp couldn't encode a {w}×{h} picture: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose every pixel differs from its neighbours, as a photo's do, with alpha
    /// from `alpha`.
    fn busy(w: u32, h: u32, alpha: impl Fn(u32, u32) -> u8) -> RgbaImage {
        let mut state = 0x2545_f491_u32;
        RgbaImage::from_fn(w, h, |x, y| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            };
            Rgba([next(), next(), next(), alpha(x, y)])
        })
    }

    #[test]
    fn lossless_webp_gives_back_every_pixel_of_an_opaque_picture() {
        let img = busy(300, 200, |_, _| 255);
        for bytes in [encode_webp_lossless(&img), encode_webp_lossless_quick(&img)] {
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..16], b"WEBPVP8L", "the lossless kind");
            let back = image::load_from_memory(&bytes).unwrap();
            // Encoded without an alpha channel, since it had nothing to say.
            assert_eq!(back.color(), image::ColorType::Rgb8);
            assert_eq!(back.to_rgba8(), img, "every pixel exact");
        }
    }

    #[test]
    fn lossless_webp_keeps_a_cutouts_alpha_and_every_colour_that_shows() {
        // A cut-out: clear around the edge, a soft rim, solid inside.
        let img = busy(257, 256, |x, y| match x.min(y).min(256 - x).min(255 - y) {
            0..=9 => 0,
            10..=19 => (x * 7 + y * 3) as u8 | 1,
            _ => 255,
        });
        for bytes in [encode_webp_lossless(&img), encode_webp_lossless_quick(&img)] {
            let back = image::load_from_memory(&bytes).unwrap();
            assert_eq!(back.color(), image::ColorType::Rgba8);
            let back = back.to_rgba8();
            for (got, was) in back.pixels().zip(img.pixels()) {
                assert_eq!(got.0[3], was.0[3], "alpha exact everywhere");
                if was.0[3] > 0 {
                    assert_eq!(got.0, was.0, "colour exact wherever it shows");
                }
            }
        }
    }

    #[test]
    fn the_thorough_setting_is_no_bigger_than_the_quick_one() {
        // Smooth, as a painting is, so there's something to find.
        let img = RgbaImage::from_fn(256, 256, |x, y| {
            Rgba([x as u8, y as u8, (x ^ y) as u8, 255])
        });
        let small = encode_webp_lossless(&img);
        let quick = encode_webp_lossless_quick(&img);
        assert!(
            small.len() <= quick.len(),
            "{} > {}",
            small.len(),
            quick.len()
        );
        assert!(small.len() < encode_png(&img).len());
    }

    #[test]
    fn lossy_webp_is_for_previews_and_keeps_its_alpha() {
        let img = busy(256, 256, |x, _| if x < 128 { 0 } else { 255 });
        let bytes = encode_webp_lossy(&img, 80.0);
        assert_eq!(&bytes[..4], b"RIFF");
        let back = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (256, 256));
        assert_eq!(back.get_pixel(10, 10).0[3], 0);
        assert_eq!(back.get_pixel(200, 10).0[3], 255);
    }

    #[test]
    fn downsample_halves_dimensions_and_keeps_opaque_pixels_opaque() {
        let src = Premul {
            width: 4,
            height: 4,
            data: vec![255; 64],
        };
        let d = downsample(&src, 2);
        assert_eq!((d.width, d.height), (2, 2));
        assert!(d.data.iter().all(|&v| v == 255));
    }

    #[test]
    fn demultiply_restores_colour() {
        let p = Premul {
            width: 1,
            height: 1,
            data: vec![128, 64, 0, 128],
        };
        let img = to_straight_rgba(&p);
        let px = img.get_pixel(0, 0).0;
        assert_eq!(px[3], 128);
        assert!((px[0] as i32 - 255).abs() <= 1);
        assert!((px[1] as i32 - 128).abs() <= 1);
    }

    #[test]
    fn png_roundtrip() {
        let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 200]));
        let bytes = encode_png(&img);
        let back = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(back, img);
    }

    #[test]
    fn premultiplying_never_exceeds_alpha_and_round_trips_opaque_pixels() {
        let img = image::RgbaImage::from_fn(16, 16, |x, y| {
            image::Rgba([(x * 16) as u8, (y * 16) as u8, 255, (x * 17) as u8])
        });
        let pm = straight_to_premul(&img);
        let (pixels, _) = pm.data.as_chunks::<4>();
        for chunk in pixels {
            assert!(chunk[0] <= chunk[3] && chunk[1] <= chunk[3] && chunk[2] <= chunk[3]);
        }
        // Fully opaque pixels survive the round trip exactly.
        let back = to_straight_rgba(&pm);
        for y in 0..16 {
            assert_eq!(back.get_pixel(15, y), img.get_pixel(15, y));
        }
    }

    #[test]
    fn downsampling_premultiplied_data_does_not_darken_edges() {
        // Left half transparent, right half opaque red. Resampling straight-alpha data would
        // average the transparent black into the red and leave a dark fringe.
        let mut src = Premul {
            width: 16,
            height: 16,
            data: vec![0; 16 * 16 * 4],
        };
        for y in 0..16 {
            for x in 8..16 {
                let i = (y * 16 + x) * 4;
                src.data[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let small = to_straight_rgba(&downsample(&src, 8));
        for y in 0..8 {
            for x in 0..8 {
                let px = small.get_pixel(x, y).0;
                if px[3] > 32 {
                    assert!(px[0] >= 240, "({x},{y}) = {px:?}");
                }
            }
        }
        assert_eq!(small.get_pixel(7, 4).0, [255, 0, 0, 255]);
    }
}
