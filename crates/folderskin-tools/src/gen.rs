//! The ten built-in skins, generated procedurally and deterministically.
//!
//! Every pixel is a pure function of its position and a fixed per-skin seed, so
//! `skin gen` reproduces the shipped files byte for byte on any machine.

use folderskin_core::manifest::{SkinEntry, SKIN_HEIGHT, SKIN_WIDTH};
use image::{Rgba, RgbaImage};

pub const W: u32 = SKIN_WIDTH;
pub const H: u32 = SKIN_HEIGHT;

type Rgb = [f32; 3];

// ---------- small maths ----------

fn hex(s: &str) -> Rgb {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).expect("hex colour");
    [
        ((v >> 16) & 255) as f32 / 255.0,
        ((v >> 8) & 255) as f32 / 255.0,
        (v & 255) as f32 / 255.0,
    ]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn to_pixel(c: Rgb) -> Rgba<u8> {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    Rgba([q(c[0]), q(c[1]), q(c[2]), 255])
}

/// Integer hash → 0..1, stable across platforms.
fn hash(x: i64, y: i64, seed: u64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 32;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Smoothly interpolated lattice noise in 0..1.
fn value_noise(x: f32, y: f32, seed: u64) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i64, yi as i64);
    let n00 = hash(xi, yi, seed);
    let n10 = hash(xi + 1, yi, seed);
    let n01 = hash(xi, yi + 1, seed);
    let n11 = hash(xi + 1, yi + 1, seed);
    let a = n00 + (n10 - n00) * sx;
    let b = n01 + (n11 - n01) * sx;
    a + (b - a) * sy
}

/// Fractal noise in 0..1 (3 octaves).
fn fbm(x: f32, y: f32, seed: u64) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    for o in 0..3u64 {
        sum += amp * value_noise(x * f, y * f, seed + o * 7919);
        amp *= 0.5;
        f *= 2.0;
    }
    sum / 0.875
}

/// Film grain: ±strength around zero, different per pixel.
fn grain(x: u32, y: u32, seed: u64, strength: f32) -> f32 {
    (hash(x as i64, y as i64, seed) - 0.5) * 2.0 * strength
}

/// Small deterministic generator for placing features (stars, bubbles).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

fn render(f: impl Fn(u32, u32, f32, f32) -> Rgb + Sync) -> RgbaImage {
    let (fw, fh) = ((W - 1) as f32, (H - 1) as f32);
    RgbaImage::from_fn(W, H, |x, y| to_pixel(f(x, y, x as f32 / fw, y as f32 / fh)))
}

// ---------- the ten skins ----------

fn aurora() -> RgbaImage {
    let (top, bottom) = (hex("0B1730"), hex("14213D"));
    let bands = [
        (hex("5EE7D9"), 0.30, 0.00),
        (hex("7CF29C"), 0.43, 0.37),
        (hex("B388FF"), 0.56, 0.71),
    ];
    let mut rng = Rng::new(11);
    let stars: Vec<(f32, f32, f32, f32)> = (0..140)
        .map(|_| {
            (
                rng.range(0.0, 1.0),
                rng.range(0.0, 0.7),
                rng.range(0.6, 1.7),
                rng.range(0.35, 1.0),
            )
        })
        .collect();
    render(|x, y, u, v| {
        let mut c = mix(top, bottom, v);
        for (col, base_y, phase) in bands {
            let yc = base_y
                + 0.11 * (std::f32::consts::TAU * (u * 1.3 + phase)).sin()
                + 0.045 * (std::f32::consts::TAU * (u * 3.1 + phase * 2.0)).sin();
            let d = (v - yc) / 0.075;
            let ribbon = (-d * d).exp() * (0.45 + 0.55 * fbm(u * 3.0, v * 3.0, 41));
            let veil = (-(d * 0.35) * (d * 0.35)).exp() * 0.18;
            let k = (ribbon * 0.62 + veil).min(0.8);
            // screen blend keeps the ribbon colour instead of burning to white
            c = [
                c[0] + (col[0] - c[0]) * k,
                c[1] + (col[1] - c[1]) * k,
                c[2] + (col[2] - c[2]) * k,
            ];
        }
        for &(sx, sy, r, b) in &stars {
            let dx = (u - sx) * W as f32;
            let dy = (v - sy) * H as f32;
            let d2 = dx * dx + dy * dy;
            if d2 < 36.0 {
                let k = (-d2 / (r * r)).exp() * b;
                c = [c[0] + k, c[1] + k, c[2] + k * 0.95];
            }
        }
        let horizon = smoothstep(0.7, 1.0, v) * 0.10;
        c = [
            c[0] + horizon * 0.2,
            c[1] + horizon * 0.45,
            c[2] + horizon * 0.5,
        ];
        let g = grain(x, y, 3, 0.03);
        [c[0] + g, c[1] + g, c[2] + g]
    })
}

fn sunset() -> RgbaImage {
    let stops = [
        (0.0, hex("FFB35C")),
        (0.35, hex("FF6B6B")),
        (0.70, hex("8A4FFF")),
        (1.0, hex("2B1B5A")),
    ];
    let sun = hex("FFE29A");
    render(|x, y, u, v| {
        let mut c = stops[0].1;
        for w in stops.windows(2) {
            let ((y0, c0), (y1, c1)) = (w[0], w[1]);
            if v >= y0 {
                c = mix(c0, c1, (v - y0) / (y1 - y0));
            }
        }
        let dx = (u - 0.5) * (W as f32 / H as f32);
        let dy = v - 0.55;
        let d = (dx * dx + dy * dy).sqrt();
        let disc = 1.0 - smoothstep(0.19, 0.25, d);
        let halo = (-(d - 0.22).max(0.0) * 9.0).exp() * 0.35;
        c = mix(c, sun, disc);
        c = [c[0] + halo * 0.9, c[1] + halo * 0.6, c[2] + halo * 0.35];
        let g = grain(x, y, 5, 0.04);
        [c[0] + g, c[1] + g, c[2] + g]
    })
}

fn mesh() -> RgbaImage {
    let base = hex("F6EFE4");
    let blobs = [
        (0.20, 0.25, hex("FF8A65"), 0.28),
        (0.80, 0.18, hex("FFC48C"), 0.30),
        (0.86, 0.76, hex("C7B3FF"), 0.32),
        (0.22, 0.82, hex("9BE7C4"), 0.30),
        (0.55, 0.50, hex("8EC5FF"), 0.26),
    ];
    render(|_x, _y, u, v| {
        let wu = u + 0.05 * (fbm(u * 2.0, v * 2.0, 77) - 0.5);
        let wv = v + 0.05 * (fbm(u * 2.0 + 9.0, v * 2.0, 78) - 0.5);
        let mut c = base;
        for (bx, by, col, sigma) in blobs {
            let dx = (wu - bx) * 1.0;
            let dy = (wv - by) * (H as f32 / W as f32);
            let w = (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp() * 0.92;
            c = mix(c, col, w);
        }
        c
    })
}

fn ember() -> RgbaImage {
    let (core, edge) = (hex("FF7A18"), hex("5A0F0F"));
    let mut rng = Rng::new(23);
    let sparks: Vec<(f32, f32, f32)> = (0..60)
        .map(|_| {
            (
                rng.range(0.05, 0.95),
                rng.range(0.1, 0.9),
                rng.range(0.8, 2.2),
            )
        })
        .collect();
    render(|x, y, u, v| {
        let dx = (u - 0.5) * 1.15;
        let dy = (v - 0.62) * (H as f32 / W as f32) * 1.15;
        let d = (dx * dx + dy * dy).sqrt();
        let mut c = mix(core, edge, smoothstep(0.0, 0.78, d));
        let n = 0.88 + 0.24 * fbm(u * 4.0, v * 4.0, 91);
        c = scale(c, n);
        for &(sx, sy, r) in &sparks {
            let ddx = (u - sx) * W as f32;
            let ddy = (v - sy) * H as f32;
            let d2 = ddx * ddx + ddy * ddy;
            if d2 < 40.0 {
                let k = (-d2 / (r * r)).exp() * 0.9;
                c = [c[0] + k, c[1] + k * 0.75, c[2] + k * 0.35];
            }
        }
        let g = grain(x, y, 7, 0.05);
        [c[0] + g, c[1] + g, c[2] + g]
    })
}

fn paper() -> RgbaImage {
    let (warm, cool, rule) = (hex("F3EAD7"), hex("EFE6D8"), hex("E3D6BD"));
    render(|x, y, u, v| {
        let mut c = mix(warm, cool, (u + v) * 0.5);
        let fibre = 0.5 + 0.5 * fbm(u * 40.0, v * 40.0, 13);
        c = scale(c, 0.97 + 0.05 * fibre);
        let ruled = if y >= 60 && (y - 60) % 44 == 0 {
            1.0
        } else {
            0.0
        };
        c = mix(c, scale(rule, 0.93), ruled);
        let margin = 1.0 - smoothstep(0.6, 1.6, (x as f32 - 140.0).abs());
        c = mix(c, hex("E8A0A0"), margin * 0.9);
        let vignette = 1.0
            - 0.06
                * smoothstep(
                    0.55,
                    1.0,
                    ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 1.4,
                );
        c = scale(c, vignette);
        let g = grain(x, y, 17, 0.07);
        [c[0] + g, c[1] + g, c[2] + g]
    })
}

fn denim() -> RgbaImage {
    let base = hex("2E4A7A");
    render(|x, y, u, v| {
        let a = (std::f32::consts::TAU * (u + v) * 240.0).sin();
        let b = (std::f32::consts::TAU * (u - v) * 240.0).sin();
        let weave = 1.0 + 0.05 * a + 0.05 * b;
        let cloth = 0.94 + 0.12 * fbm(u * 6.0, v * 6.0, 29);
        let fade = 1.0 + 0.10 * (1.0 - smoothstep(0.2, 0.9, v));
        let mut c = scale(base, weave * cloth * fade);
        // two rows of orange top-stitching, like a pocket seam
        let thread = hex("E8A24A");
        for (row, phase) in [(846.0f32, 0.0f32), (872.0, 17.0)] {
            let along = (x as f32 + phase) % 34.0;
            let dash = smoothstep(0.0, 1.2, along) * (1.0 - smoothstep(19.0, 20.2, along));
            let across = 1.0 - smoothstep(1.6, 3.0, (y as f32 - row).abs());
            let shadow = (1.0 - smoothstep(2.2, 5.0, (y as f32 - row - 1.5).abs())) * dash * 0.35;
            c = scale(c, 1.0 - shadow);
            c = mix(c, thread, dash * across);
        }
        let g = grain(x, y, 31, 0.05);
        c = [c[0] + g, c[1] + g, c[2] + g];
        c
    })
}

fn slate() -> RgbaImage {
    let (top, bottom) = (hex("2B2F36"), hex("3B4048"));
    render(|x, y, u, v| {
        let mut c = mix(top, bottom, v);
        let t = (u + v) * 0.5;
        let sweep = smoothstep(0.28, 0.5, t) * (1.0 - smoothstep(0.5, 0.72, t));
        c = scale(c, 1.0 + 0.22 * sweep);
        let t2 = (u * 0.7 + v) * 0.6;
        let sweep2 = smoothstep(0.62, 0.72, t2) * (1.0 - smoothstep(0.72, 0.84, t2));
        c = scale(c, 1.0 + 0.09 * sweep2);
        let mottle = 0.96 + 0.08 * fbm(u * 5.0, v * 5.0, 37);
        c = scale(c, mottle);
        let g = grain(x, y, 43, 0.025);
        [c[0] + g, c[1] + g, c[2] + g]
    })
}

fn halftone() -> RgbaImage {
    let (base, ink) = (hex("FFD23F"), hex("1B1B1B"));
    let (cos_a, sin_a) = (22f32.to_radians().cos(), 22f32.to_radians().sin());
    let cell = 40.0;
    render(|x, y, u, v| {
        let (fx, fy) = (x as f32, y as f32);
        let rx = fx * cos_a + fy * sin_a;
        let ry = -fx * sin_a + fy * cos_a;
        let (cx, cy) = ((rx / cell).round() * cell, (ry / cell).round() * cell);
        let d = ((rx - cx).powi(2) + (ry - cy).powi(2)).sqrt();
        let r = 16.0 * smoothstep(0.05, 0.95, (u + v) * 0.5);
        let dot = 1.0 - smoothstep(r - 0.8, r + 0.8, d);
        mix(base, ink, dot)
    })
}

fn stripes() -> RgbaImage {
    let (coral, cream) = (hex("FF6F61"), hex("FFF1E6"));
    let period = 56.0;
    render(|x, y, _u, _v| {
        let s = (x as f32 + y as f32) / period;
        let idx = s.floor() as i64;
        let frac = s - s.floor();
        let dist_px = frac.min(1.0 - frac) * period;
        let edge_dark = 1.0 - 0.12 * (1.0 - smoothstep(2.4, 3.6, dist_px));
        let anti = smoothstep(-0.6, 0.6, (frac - 0.0) * period);
        let colour = if idx.rem_euclid(2) == 0 { coral } else { cream };
        let other = if idx.rem_euclid(2) == 0 { cream } else { coral };
        let c = mix(other, colour, anti);
        let c = scale(c, edge_dark);
        let g = grain(x, y, 53, 0.015);
        [c[0] + g, c[1] + g, c[2] + g]
    })
}

fn bubbles() -> RgbaImage {
    let (top, bottom) = (hex("1B998B"), hex("157F73"));
    let palette = [hex("F4F1DE"), hex("F2CC8F"), hex("E07A5F")];
    let mut rng = Rng::new(61);
    let circles: Vec<(f32, f32, f32, Rgb)> = (0..14)
        .map(|i| {
            (
                rng.range(0.0, W as f32),
                rng.range(0.0, H as f32),
                rng.range(60.0, 190.0),
                palette[i % 3],
            )
        })
        .collect();
    render(|x, y, _u, v| {
        let mut c = mix(top, bottom, v);
        for &(cx, cy, r, col) in &circles {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            if d < r + 2.0 {
                let inside = 1.0 - smoothstep(r - 0.8, r + 0.8, d);
                let rim = (1.0 - smoothstep(r - 2.8, r - 0.8, d)) * smoothstep(r - 5.0, r - 2.8, d);
                c = mix(c, col, inside * 0.55);
                c = mix(c, [1.0, 1.0, 1.0], rim * 0.25);
            }
        }
        c
    })
}

/// The shipped set, in gallery order.
pub fn builtin_skins() -> Vec<(SkinEntry, RgbaImage)> {
    let entry = |id: &str, name: &str, collection: &str| SkinEntry {
        id: id.into(),
        name: name.into(),
        collection: collection.into(),
        file: String::new(),
        focus: [0.5, 0.5],
        author: "FolderSkin".into(),
        license: "CC0-1.0".into(),
    };
    vec![
        (entry("aurora", "Aurora", "glow"), aurora()),
        (entry("sunset", "Sunset", "glow"), sunset()),
        (entry("mesh", "Mesh", "glow"), mesh()),
        (entry("ember", "Ember", "glow"), ember()),
        (entry("paper", "Paper", "grain"), paper()),
        (entry("denim", "Denim", "grain"), denim()),
        (entry("slate", "Slate", "grain"), slate()),
        (entry("halftone", "Halftone", "pop"), halftone()),
        (entry("stripes", "Stripes", "pop"), stripes()),
        (entry("bubbles", "Bubbles", "pop"), bubbles()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_core::manifest::Manifest;

    #[test]
    fn ten_valid_unique_skins_of_the_right_size() {
        let skins = builtin_skins();
        assert_eq!(skins.len(), 10);
        let mut ids: Vec<&str> = skins.iter().map(|(e, _)| e.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
        for (e, img) in &skins {
            assert!(Manifest::is_valid_id(&e.id), "{}", e.id);
            assert!(
                ["glow", "grain", "pop"].contains(&e.collection.as_str()),
                "{}",
                e.collection
            );
            assert_eq!(img.dimensions(), (W, H), "{}", e.id);
            assert_eq!(e.license, "CC0-1.0");
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a = builtin_skins();
        let b = builtin_skins();
        for ((ea, ia), (eb, ib)) in a.iter().zip(b.iter()) {
            assert_eq!(ea.id, eb.id);
            assert_eq!(ia.as_raw(), ib.as_raw(), "{} differs between runs", ea.id);
        }
    }

    #[test]
    fn skins_are_not_flat() {
        for (e, img) in builtin_skins() {
            let px = img.as_raw();
            let (mut lo, mut hi) = (255u8, 0u8);
            for p in px.chunks(4) {
                lo = lo.min(p[0].min(p[1]).min(p[2]));
                hi = hi.max(p[0].max(p[1]).max(p[2]));
            }
            assert!(hi - lo > 40, "{} looks flat ({lo}..{hi})", e.id);
        }
    }
}

#[cfg(test)]
mod dump {
    /// `FOLDERSKIN_DUMP_DIR=/tmp/x cargo test -p folderskin-tools --release -- --ignored dump_skins`
    #[test]
    #[ignore]
    fn dump_skins() {
        let dir = std::path::PathBuf::from(
            std::env::var("FOLDERSKIN_DUMP_DIR").expect("FOLDERSKIN_DUMP_DIR"),
        );
        std::fs::create_dir_all(&dir).unwrap();
        for (e, img) in super::builtin_skins() {
            img.save(dir.join(format!("{}.png", e.id))).unwrap();
        }
    }
}
