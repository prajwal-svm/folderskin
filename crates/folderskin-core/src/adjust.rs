//! Colour adjustments: the composer's own (`src/composer/imagefx.ts`) in Rust, so a picture
//! adjusted from the command line comes out the way the composer draws it.
//!
//! Every adjustment is an affine map of a pixel's red, green and blue, so they fold into one
//! 3 × 4 matrix and a picture takes a single pass whatever is set. Alpha is left alone, and the
//! arithmetic is the web view's: double precision, then clamped and rounded half to even, which is
//! what writing into a `Uint8ClampedArray` does.

use image::RgbaImage;

/// Rows of `[r, g, b, 1]` → channel, as 3 × 4 numbers.
pub type Matrix = [f64; 12];

/// The matrix that changes nothing.
pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// The composer's picture adjustments, in its own units. Zero means "not set" for every field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fx {
    /// -100 to 100
    pub brightness: f64,
    /// -100 to 100
    pub contrast: f64,
    /// -100 to 100
    pub saturation: f64,
    /// -180 to 180 degrees
    pub hue: f64,
    /// 0 to 100
    pub grayscale: f64,
    /// 0 to 100
    pub sepia: f64,
    /// 0 to 100
    pub invert: f64,
}

impl Fx {
    /// True when nothing is set, so a picture would come out unchanged.
    pub fn is_none(&self) -> bool {
        *self == Fx::default()
    }
}

/// `a` after `b`: first `b`, then `a`.
pub fn compose(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [0.0; 12];
    for r in 0..3 {
        for c in 0..4 {
            let mut v = 0.0;
            for k in 0..3 {
                v += a[r * 4 + k] * b[k * 4 + c];
            }
            if c == 3 {
                v += a[r * 4 + 3];
            }
            out[r * 4 + c] = v;
        }
    }
    out
}

/// Rec. 709 luma weights, as the composer uses them.
const LR: f64 = 0.2126;
const LG: f64 = 0.7152;
const LB: f64 = 0.0722;

fn scale(s: f64) -> Matrix {
    [s, 0.0, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 0.0, s, 0.0]
}

fn saturate(s: f64) -> Matrix {
    [
        LR + (1.0 - LR) * s,
        LG - LG * s,
        LB - LB * s,
        0.0,
        LR - LR * s,
        LG + (1.0 - LG) * s,
        LB - LB * s,
        0.0,
        LR - LR * s,
        LG - LG * s,
        LB + (1.0 - LB) * s,
        0.0,
    ]
}

/// The CSS `hue-rotate` matrix.
fn hue_rotate(deg: f64) -> Matrix {
    let a = deg.to_radians();
    let (s, c) = a.sin_cos();
    [
        0.213 + c * 0.787 - s * 0.213,
        0.715 - c * 0.715 - s * 0.715,
        0.072 - c * 0.072 + s * 0.928,
        0.0,
        0.213 - c * 0.213 + s * 0.143,
        0.715 + c * 0.285 + s * 0.14,
        0.072 - c * 0.072 - s * 0.283,
        0.0,
        0.213 - c * 0.213 - s * 0.787,
        0.715 - c * 0.715 + s * 0.715,
        0.072 + c * 0.928 + s * 0.072,
        0.0,
    ]
}

/// `m` applied `t` of the way (0 to 1) from the identity.
fn mix_with(m: &Matrix, t: f64) -> Matrix {
    let mut out = IDENTITY;
    for (o, &v) in out.iter_mut().zip(m) {
        *o += (v - *o) * t;
    }
    out
}

const SEPIA: Matrix = [
    0.393, 0.769, 0.189, 0.0, 0.349, 0.686, 0.168, 0.0, 0.272, 0.534, 0.131, 0.0,
];

/// The colour adjustments of `fx` as one matrix, in the order a photo editor applies them.
pub fn fx_matrix(fx: &Fx) -> Matrix {
    let mut m = IDENTITY;
    if fx.brightness != 0.0 {
        m = compose(&scale(1.0 + fx.brightness / 100.0), &m);
    }
    if fx.contrast != 0.0 {
        let k = (1.0 + fx.contrast / 100.0).max(0.0);
        let o = 128.0 * (1.0 - k);
        m = compose(&[k, 0.0, 0.0, o, 0.0, k, 0.0, o, 0.0, 0.0, k, o], &m);
    }
    if fx.saturation != 0.0 {
        m = compose(&saturate((1.0 + fx.saturation / 100.0).max(0.0)), &m);
    }
    if fx.hue != 0.0 {
        m = compose(&hue_rotate(fx.hue), &m);
    }
    if fx.grayscale != 0.0 {
        m = compose(&mix_with(&saturate(0.0), fx.grayscale / 100.0), &m);
    }
    if fx.sepia != 0.0 {
        m = compose(&mix_with(&SEPIA, fx.sepia / 100.0), &m);
    }
    if fx.invert != 0.0 {
        let t = fx.invert / 100.0;
        let k = 1.0 - 2.0 * t;
        let o = 255.0 * t;
        m = compose(&[k, 0.0, 0.0, o, 0.0, k, 0.0, o, 0.0, 0.0, k, o], &m);
    }
    m
}

/// A channel value as a `Uint8ClampedArray` stores it: clamped, then rounded half to even.
fn clamp_u8(v: f64) -> u8 {
    if v.is_nan() {
        0
    } else {
        v.clamp(0.0, 255.0).round_ties_even() as u8
    }
}

/// Applies a colour matrix to a straight-alpha picture, in place.
pub fn apply_matrix(img: &mut RgbaImage, m: &Matrix) {
    for px in img.pixels_mut() {
        let [r, g, b, _] = px.0.map(f64::from);
        px.0[0] = clamp_u8(m[0] * r + m[1] * g + m[2] * b + m[3]);
        px.0[1] = clamp_u8(m[4] * r + m[5] * g + m[6] * b + m[7]);
        px.0[2] = clamp_u8(m[8] * r + m[9] * g + m[10] * b + m[11]);
    }
}

/// The picture with `fx` applied.
pub fn adjust(img: &RgbaImage, fx: &Fx) -> RgbaImage {
    let mut out = img.clone();
    if !fx.is_none() {
        apply_matrix(&mut out, &fx_matrix(fx));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn px(rgba: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(1, 1, Rgba(rgba))
    }

    fn after(rgba: [u8; 4], fx: Fx) -> [u8; 4] {
        adjust(&px(rgba), &fx).get_pixel(0, 0).0
    }

    // The same cases as src/composer/imagefx.test.ts, so the two stay in step.

    #[test]
    fn leaves_a_picture_alone_with_nothing_set() {
        assert!(Fx::default().is_none());
        let mut img = px([10, 200, 30, 255]);
        apply_matrix(&mut img, &fx_matrix(&Fx::default()));
        assert_eq!(img.get_pixel(0, 0).0, [10, 200, 30, 255]);
    }

    #[test]
    fn brightens_darkens_and_inverts() {
        let brighter = Fx {
            brightness: 50.0,
            ..Fx::default()
        };
        assert_eq!(after([100, 100, 100, 255], brighter), [150, 150, 150, 255]);
        let inverted = Fx {
            invert: 100.0,
            ..Fx::default()
        };
        assert_eq!(after([0, 128, 255, 200], inverted), [255, 127, 0, 200]);
        let darker = Fx {
            brightness: -50.0,
            ..Fx::default()
        };
        assert_eq!(after([100, 100, 100, 255], darker), [50, 50, 50, 255]);
    }

    #[test]
    fn takes_the_colour_out_and_puts_contrast_in() {
        let grey = after(
            [255, 0, 0, 255],
            Fx {
                grayscale: 100.0,
                ..Fx::default()
            },
        );
        assert_eq!((grey[0], grey[1]), (grey[1], grey[2]));
        let flat = after(
            [40, 90, 200, 255],
            Fx {
                contrast: -100.0,
                ..Fx::default()
            },
        );
        assert_eq!(flat, [128, 128, 128, 255]);
        let desaturated = after(
            [255, 0, 0, 255],
            Fx {
                saturation: -100.0,
                ..Fx::default()
            },
        );
        assert_eq!(
            (desaturated[0], desaturated[1]),
            (desaturated[1], desaturated[2])
        );
    }

    #[test]
    fn composes_matrices_in_order() {
        // Brighten then invert is not invert then brighten.
        let both = Fx {
            brightness: 50.0,
            invert: 100.0,
            ..Fx::default()
        };
        assert_eq!(after([100, 100, 100, 255], both)[0], 105);
        let id = fx_matrix(&Fx::default());
        assert_eq!(compose(&id, &id), id);
    }

    #[test]
    fn clamps_and_rounds_like_the_web_view() {
        assert_eq!(clamp_u8(-3.0), 0);
        assert_eq!(clamp_u8(300.0), 255);
        assert_eq!(clamp_u8(f64::NAN), 0);
        // Uint8ClampedArray rounds half to even.
        assert_eq!((clamp_u8(2.5), clamp_u8(3.5)), (2, 4));
    }

    #[test]
    fn a_full_turn_of_hue_changes_almost_nothing() {
        let turned = after(
            [30, 140, 220, 255],
            Fx {
                hue: 360.0,
                ..Fx::default()
            },
        );
        for (a, b) in turned.iter().zip([30, 140, 220, 255]) {
            assert!(a.abs_diff(b) <= 1, "{turned:?}");
        }
        let sepia = after(
            [30, 140, 220, 255],
            Fx {
                sepia: 100.0,
                ..Fx::default()
            },
        );
        assert!(sepia[0] > sepia[2], "sepia is warm: {sepia:?}");
    }
}
