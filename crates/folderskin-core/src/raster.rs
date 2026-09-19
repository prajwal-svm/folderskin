//! Pixel plumbing: premultiplied buffers, Lanczos3 downsampling and PNG encoding.
//!
//! tiny-skia renders into premultiplied RGBA8, and resampling has to happen on premultiplied
//! channels or transparent pixels bleed their (black) colour into the edges. So the master
//! render stays premultiplied through every downsample and is converted to straight alpha only
//! at the very end, on the way into an `image::RgbaImage`.

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

#[cfg(test)]
mod tests {
    use super::*;

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
