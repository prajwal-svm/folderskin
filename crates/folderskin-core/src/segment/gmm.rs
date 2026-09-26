//! The colours of a subject or a backdrop as a few Gaussians in RGB, fitted the same way every
//! time: k-means started from points spread along the colours' widest direction, then each
//! cluster's mean and covariance.

use std::f64::consts::PI;

/// The most Gaussians a mixture has.
pub const MAX_PARTS: usize = 8;
/// Colours are fitted from at most this many samples, taken evenly.
const MAX_SAMPLES: usize = 40_000;
/// Rounds of k-means.
const ROUNDS: usize = 12;
/// Added to each channel's variance, in 0..=255 units squared: a flat colour would otherwise be
/// infinitely sure of itself.
const VARIANCE_FLOOR: f64 = 9.0;

#[derive(Clone, Debug)]
struct Gaussian {
    mean: [f64; 3],
    inverse: [[f64; 3]; 3],
    /// ln weight - ln sqrt|2 pi covariance|.
    norm: f64,
}

/// A mixture of Gaussians over RGB colours in 0..=255.
#[derive(Clone, Debug)]
pub struct Gmm {
    parts: Vec<Gaussian>,
}

impl Gmm {
    /// `k` Gaussians (at most [`MAX_PARTS`]) fitted to `colours`; `None` for too few colours to
    /// fit them to.
    pub fn fit(colours: &[[f32; 3]], k: usize) -> Option<Gmm> {
        let k = k.clamp(1, MAX_PARTS);
        if colours.len() < k * 8 {
            return None;
        }
        let step = (colours.len() / MAX_SAMPLES).max(1);
        let xs: Vec<[f64; 3]> = colours
            .iter()
            .step_by(step)
            .map(|c| c.map(f64::from))
            .collect();
        let mean = mean_of(xs.iter());
        let axis = principal_axis(&xs, mean);
        // Start from k points spread evenly along the widest direction.
        let mut order: Vec<(f64, usize)> = xs
            .iter()
            .enumerate()
            .map(|(i, p)| ((0..3).map(|c| (p[c] - mean[c]) * axis[c]).sum(), i))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut centres: Vec<[f64; 3]> = (0..k)
            .map(|j| xs[order[order.len() * (2 * j + 1) / (2 * k)].1])
            .collect();
        let mut label = vec![0usize; xs.len()];
        for _ in 0..ROUNDS {
            for (l, p) in label.iter_mut().zip(&xs) {
                *l = (0..k)
                    .min_by(|&a, &b| {
                        squared(p, &centres[a])
                            .total_cmp(&squared(p, &centres[b]))
                            .then(a.cmp(&b))
                    })
                    .unwrap_or(0);
            }
            for (j, centre) in centres.iter_mut().enumerate() {
                let members = xs.iter().zip(&label).filter(|(_, &l)| l == j);
                if let Some(m) = mean_of_nonempty(members.map(|(p, _)| p)) {
                    *centre = m;
                }
            }
        }
        let parts = (0..k)
            .filter_map(|j| {
                let members: Vec<&[f64; 3]> = xs
                    .iter()
                    .zip(&label)
                    .filter(|(_, &l)| l == j)
                    .map(|(p, _)| p)
                    .collect();
                Gaussian::fit(&members, members.len() as f64 / xs.len() as f64)
            })
            .collect();
        Some(Gmm { parts })
    }

    /// The natural log of the colour's density under the mixture.
    pub fn log_density(&self, colour: [f32; 3]) -> f64 {
        let x = colour.map(f64::from);
        let mut terms = [f64::NEG_INFINITY; MAX_PARTS];
        for (t, g) in terms.iter_mut().zip(&self.parts) {
            *t = g.log_density(x);
        }
        let best = terms.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if best == f64::NEG_INFINITY {
            return best;
        }
        best + terms.iter().map(|t| (t - best).exp()).sum::<f64>().ln()
    }
}

impl Gaussian {
    fn fit(members: &[&[f64; 3]], weight: f64) -> Option<Gaussian> {
        if members.len() < 4 {
            return None;
        }
        let mean = mean_of(members.iter().copied());
        let mut cov = [[0.0; 3]; 3];
        for p in members {
            for a in 0..3 {
                for b in 0..3 {
                    cov[a][b] += (p[a] - mean[a]) * (p[b] - mean[b]);
                }
            }
        }
        let n = members.len() as f64;
        for (a, row) in cov.iter_mut().enumerate() {
            for v in row.iter_mut() {
                *v /= n;
            }
            row[a] += VARIANCE_FLOOR;
        }
        let det = determinant(&cov).max(1e-9);
        Some(Gaussian {
            mean,
            inverse: inverse(&cov, det),
            norm: weight.ln() - 0.5 * ((2.0 * PI).powi(3) * det).ln(),
        })
    }

    fn log_density(&self, x: [f64; 3]) -> f64 {
        let d = [
            x[0] - self.mean[0],
            x[1] - self.mean[1],
            x[2] - self.mean[2],
        ];
        let mut q = 0.0;
        for a in 0..3 {
            for b in 0..3 {
                q += d[a] * self.inverse[a][b] * d[b];
            }
        }
        self.norm - 0.5 * q
    }
}

fn squared(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (0..3).map(|c| (a[c] - b[c]).powi(2)).sum()
}

fn mean_of<'a>(xs: impl Iterator<Item = &'a [f64; 3]>) -> [f64; 3] {
    mean_of_nonempty(xs).unwrap_or([0.0; 3])
}

fn mean_of_nonempty<'a>(xs: impl Iterator<Item = &'a [f64; 3]>) -> Option<[f64; 3]> {
    let (mut sum, mut n) = ([0.0; 3], 0usize);
    for p in xs {
        (0..3).for_each(|c| sum[c] += p[c]);
        n += 1;
    }
    (n > 0).then(|| sum.map(|v| v / n as f64))
}

/// The direction the colours spread most along, by power iteration on their covariance.
fn principal_axis(xs: &[[f64; 3]], mean: [f64; 3]) -> [f64; 3] {
    let mut cov = [[0.0; 3]; 3];
    for p in xs {
        for a in 0..3 {
            for b in 0..3 {
                cov[a][b] += (p[a] - mean[a]) * (p[b] - mean[b]);
            }
        }
    }
    let mut axis = [1.0, 1.0, 1.0];
    for _ in 0..30 {
        let next: [f64; 3] =
            std::array::from_fn(|a| (0..3).map(|b| cov[a][b] * axis[b]).sum::<f64>());
        let length = next.iter().map(|v| v * v).sum::<f64>().sqrt();
        if length < 1e-12 {
            break;
        }
        axis = next.map(|v| v / length);
    }
    axis
}

fn determinant(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

fn inverse(m: &[[f64; 3]; 3], det: f64) -> [[f64; 3]; 3] {
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / det,
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two clusters of colour come back as two, and a colour between them is less likely than
    /// either.
    #[test]
    fn two_colours_are_told_apart() {
        let mut colours = Vec::new();
        for i in 0..400 {
            let wobble = (i % 7) as f32 - 3.0;
            colours.push([200.0 + wobble, 40.0 - wobble, 180.0]);
            colours.push([30.0, 160.0 + wobble, 60.0 - wobble]);
        }
        let gmm = Gmm::fit(&colours, 2).unwrap();
        let magenta = gmm.log_density([200.0, 40.0, 180.0]);
        let green = gmm.log_density([30.0, 160.0, 60.0]);
        let between = gmm.log_density([115.0, 100.0, 120.0]);
        assert!(
            magenta > between + 20.0 && green > between + 20.0,
            "{magenta} {green} {between}"
        );
        // The same colours fit the same way every time.
        assert_eq!(
            gmm.log_density([90.0, 90.0, 90.0]),
            Gmm::fit(&colours, 2)
                .unwrap()
                .log_density([90.0, 90.0, 90.0])
        );
    }
}
