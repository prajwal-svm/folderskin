//! The backdrop around a painted subject, as the colour it has at every pixel.
//!
//! An image model asked to paint on a flat colour rarely keeps it flat. klein turns #FF00FF into
//! a dusty pink that brightens towards one corner, a dark grey behind a night scene, or a studio
//! sweep with a glow where its light falls. One colour for the whole backdrop (the median of the
//! frame's edge) is then off by as much as a sunset's clouds differ from it, and a painting that
//! shares the backdrop's colours can't be told from it.
//!
//! [`Backdrop::measure`] estimates the backdrop from the one place it is always visible: the
//! frame's outer band. A quadratic in x and y is fitted to the band (twice more without the
//! pixels that stray, so a subject touching the edge doesn't bend it), which carries a smooth
//! gradient across the frame. What the band still differs from it by (a glow in a corner, a
//! vignette) is read along each side, smoothed, and carried inward with a Coons patch, which
//! matches all four sides at once. Near the frame's edge, where a folder's outline and a free
//! icon's backdrop are, the estimate is the backdrop to within its own noise.

use crate::matte::{distance, FLAT_TOLERANCE};
use image::RgbaImage;

/// The backdrop's colour at every pixel of a picture, and how plain the frame's edge is.
#[derive(Clone, Debug)]
pub struct Backdrop {
    width: u32,
    colours: Vec<[u8; 3]>,
    /// How far the frame's outer band strays from the estimate: the 90th percentile of its
    /// pixels' distances ([`crate::matte`]'s 0 to 1 scale). A painted backdrop measures about
    /// 0.01; a photo reaching the edge far more.
    pub noise: f32,
    /// The share of the outer band within [`FLAT_TOLERANCE`] of the estimate.
    pub plain: f32,
}

/// The outer band is this share of the shorter side deep, and at least a pixel.
const BAND_SHARE: u32 = 100;

impl Backdrop {
    /// The backdrop around `img`, estimated from its outer band. `None` for a picture too small
    /// to have one.
    pub fn measure(img: &RgbaImage) -> Option<Self> {
        let (w, h) = img.dimensions();
        if w < 16 || h < 16 {
            return None;
        }
        let band = (w.min(h) / BAND_SHARE).max(1);
        let ring: Vec<(u32, u32)> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| x < band || y < band || x >= w - band || y >= h - band)
            .collect();

        // A quadratic through the band, refitted without the pixels that stray from it.
        let mut quad = Quad::fit(img, &ring)?;
        for _ in 0..3 {
            let off: Vec<f32> = ring
                .iter()
                .map(|&(x, y)| distance(&rgb(img, x, y), &quad.colour(x, y)))
                .collect();
            let mut sorted = off.clone();
            sorted.sort_by(f32::total_cmp);
            let limit = (sorted[sorted.len() / 2] * 3.0).max(FLAT_TOLERANCE / 2.0);
            let kept: Vec<(u32, u32)> = ring
                .iter()
                .zip(&off)
                .filter(|(_, &d)| d <= limit)
                .map(|(p, _)| *p)
                .collect();
            if kept.len() < ring.len() / 2 {
                break;
            }
            quad = Quad::fit(img, &kept)?;
        }

        // What the band differs from the quadratic by, along each side: the median across the
        // band's depth, then a running median and a running mean along the side.
        let side = |len: u32, at: &dyn Fn(u32, u32) -> (u32, u32)| -> [Vec<f64>; 3] {
            let mut out: [Vec<f64>; 3] = std::array::from_fn(|_| vec![0.0; len as usize]);
            let mut values: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
            for i in 0..len {
                values.iter_mut().for_each(Vec::clear);
                for d in 0..band {
                    let (x, y) = at(i, d);
                    let (p, q) = (rgb(img, x, y), quad.at(x, y));
                    for c in 0..3 {
                        values[c].push(f64::from(p[c]) - q[c]);
                    }
                }
                for c in 0..3 {
                    out[c][i as usize] = middle(&mut values[c]);
                }
            }
            let (median_reach, mean_reach) =
                ((len as usize / 60).max(3), (len as usize / 120).max(2));
            out.map(|v| running_mean(&running_median(&v, median_reach), mean_reach))
        };
        let top = side(w, &|i, d| (i, d));
        let bottom = side(w, &|i, d| (i, h - 1 - d));
        let left = side(h, &|i, d| (d, i));
        let right = side(h, &|i, d| (w - 1 - d, i));
        let (last_x, last_y) = (w as usize - 1, h as usize - 1);

        let mut colours = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let v = f64::from(y) / f64::from(h - 1);
            for x in 0..w {
                let u = f64::from(x) / f64::from(w - 1);
                let q = quad.at(x, y);
                let (xi, yi) = (x as usize, y as usize);
                colours.push(std::array::from_fn(|c| {
                    let (t, b, l, r) = (&top[c], &bottom[c], &left[c], &right[c]);
                    // Coons: each pair of opposite sides blended across, less the corners, which
                    // both pairs count.
                    let corners = (1.0 - u) * (1.0 - v) * (t[0] + l[0]) / 2.0
                        + u * (1.0 - v) * (t[last_x] + r[0]) / 2.0
                        + (1.0 - u) * v * (b[0] + l[last_y]) / 2.0
                        + u * v * (b[last_x] + r[last_y]) / 2.0;
                    let edge = (1.0 - v) * t[xi] + v * b[xi] + (1.0 - u) * l[yi] + u * r[yi];
                    (q[c] + edge - corners).round().clamp(0.0, 255.0) as u8
                }));
            }
        }
        let mut backdrop = Self {
            width: w,
            colours,
            noise: 0.0,
            plain: 0.0,
        };
        let mut off: Vec<f32> = ring
            .iter()
            .map(|&(x, y)| distance(&rgb(img, x, y), &backdrop.at(x, y)))
            .collect();
        backdrop.plain =
            off.iter().filter(|&&d| d <= FLAT_TOLERANCE).count() as f32 / off.len() as f32;
        off.sort_by(f32::total_cmp);
        backdrop.noise = off[off.len() * 9 / 10];
        Some(backdrop)
    }

    /// The backdrop's colour at `(x, y)`.
    pub fn at(&self, x: u32, y: u32) -> [u8; 3] {
        self.colours[(y * self.width + x) as usize]
    }
}

fn rgb(img: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let p = img.get_pixel(x, y).0;
    [p[0], p[1], p[2]]
}

/// Each channel a quadratic in x and y, over a frame whose sides run from -1 to 1.
struct Quad {
    coefficients: [[f64; 6]; 3],
    width: f64,
    height: f64,
}

impl Quad {
    fn terms(&self, x: u32, y: u32) -> [f64; 6] {
        let u = (f64::from(x) + 0.5) / self.width * 2.0 - 1.0;
        let v = (f64::from(y) + 0.5) / self.height * 2.0 - 1.0;
        [1.0, u, v, u * u, v * v, u * v]
    }

    /// The least-squares fit to `img` at `points`, or `None` when they don't pin one down.
    fn fit(img: &RgbaImage, points: &[(u32, u32)]) -> Option<Self> {
        let mut quad = Self {
            coefficients: [[0.0; 6]; 3],
            width: f64::from(img.width()),
            height: f64::from(img.height()),
        };
        let mut normal = [[0.0; 6]; 6];
        let mut sums = [[0.0; 6]; 3];
        for &(x, y) in points {
            let t = quad.terms(x, y);
            let p = img.get_pixel(x, y).0;
            for i in 0..6 {
                for j in 0..6 {
                    normal[i][j] += t[i] * t[j];
                }
                for c in 0..3 {
                    sums[c][i] += t[i] * f64::from(p[c]);
                }
            }
        }
        for (coefficients, sums) in quad.coefficients.iter_mut().zip(sums) {
            *coefficients = solve(normal, sums)?;
        }
        Some(quad)
    }

    fn at(&self, x: u32, y: u32) -> [f64; 3] {
        let t = self.terms(x, y);
        self.coefficients
            .map(|k| k.iter().zip(&t).map(|(a, b)| a * b).sum())
    }

    fn colour(&self, x: u32, y: u32) -> [u8; 3] {
        self.at(x, y).map(|v| v.round().clamp(0.0, 255.0) as u8)
    }
}

/// `a` x = `b` by Gaussian elimination with partial pivoting; `None` when `a` is singular.
fn solve(mut a: [[f64; 6]; 6], mut b: [f64; 6]) -> Option<[f64; 6]> {
    for col in 0..6 {
        let pivot = (col..6).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-9 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let (pivot_row, pivot_b) = (a[col], b[col]);
        for row in (0..6).filter(|&row| row != col) {
            let f = a[row][col] / pivot_row[col];
            for (value, p) in a[row].iter_mut().zip(pivot_row).skip(col) {
                *value -= f * p;
            }
            b[row] -= f * pivot_b;
        }
    }
    Some(std::array::from_fn(|i| b[i] / a[i][i]))
}

/// The middle value of `values` (the upper one of an even count).
fn middle(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn running_median(v: &[f64], reach: usize) -> Vec<f64> {
    let mut window = Vec::with_capacity(2 * reach + 1);
    (0..v.len())
        .map(|i| {
            window.clear();
            window.extend_from_slice(&v[i.saturating_sub(reach)..(i + reach + 1).min(v.len())]);
            middle(&mut window)
        })
        .collect()
}

fn running_mean(v: &[f64], reach: usize) -> Vec<f64> {
    (0..v.len())
        .map(|i| {
            let window = &v[i.saturating_sub(reach)..(i + reach + 1).min(v.len())];
            window.iter().sum::<f64>() / window.len() as f64
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A backdrop brightening from one corner to the other, with a glow in the top left.
    fn sweep(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let glow = (1.0 - ((u * u + v * v).sqrt() / 0.5)).max(0.0) * 60.0;
            Rgba([
                (170.0 + 40.0 * u + glow).min(255.0) as u8,
                (100.0 + 30.0 * v + glow).min(255.0) as u8,
                (150.0 - 20.0 * u + glow).min(255.0) as u8,
                255,
            ])
        })
    }

    #[test]
    fn a_smooth_backdrop_is_found_to_within_a_few_levels_everywhere_near_the_edge() {
        let img = sweep(200, 180);
        let b = Backdrop::measure(&img).unwrap();
        assert!(b.plain > 0.99, "plain {}", b.plain);
        assert!(b.noise < 0.01, "noise {}", b.noise);
        for (x, y) in [(5, 5), (30, 20), (190, 10), (10, 170), (100, 12), (185, 90)] {
            let (p, e) = (img.get_pixel(x, y).0, b.at(x, y));
            for c in 0..3 {
                assert!(
                    (i32::from(p[c]) - i32::from(e[c])).abs() <= 6,
                    "at {x},{y}: {p:?} against {e:?}"
                );
            }
        }
    }

    #[test]
    fn a_subject_on_the_backdrop_doesnt_change_it() {
        let mut img = sweep(200, 180);
        let clean = Backdrop::measure(&img).unwrap();
        // A dark subject in the middle, touching the bottom edge.
        for y in 60..180 {
            for x in 80..120 {
                img.put_pixel(x, y, Rgba([20, 30, 40, 255]));
            }
        }
        let b = Backdrop::measure(&img).unwrap();
        for (x, y) in [(20, 20), (180, 30), (40, 150), (160, 150)] {
            let (a, e) = (clean.at(x, y), b.at(x, y));
            for c in 0..3 {
                assert!(
                    (i32::from(a[c]) - i32::from(e[c])).abs() <= 8,
                    "at {x},{y}: {a:?} against {e:?}"
                );
            }
        }
    }

    #[test]
    fn a_picture_that_reaches_the_edge_is_not_plain() {
        let busy = RgbaImage::from_fn(200, 180, |x, y| {
            let v = ((x * 37 + y * 91) % 97) as u8;
            Rgba([v * 2, 255 - v, (x % 50 * 5) as u8, 255])
        });
        let b = Backdrop::measure(&busy).unwrap();
        assert!(b.plain < 0.5, "plain {}", b.plain);
        assert!(Backdrop::measure(&RgbaImage::new(8, 8)).is_none());
    }
}
