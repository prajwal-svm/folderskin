//! A subject lifted off the backdrop it was painted on, where the system can't lift it
//! ([`crate::lift`]): on Windows, on Linux and on a Mac before macOS 14.
//!
//! A free icon is asked for on a flat key colour, and a model keeps it about half the time. The
//! rest of the time the backdrop drifts: klein turns magenta into a lavender or rose studio sweep
//! with a glow and a soft shadow, a watercolour into pale paper with a wash, a photo into a
//! spotlit floor. A colour key then can't tell the backdrop from a white robot lit lavender by it,
//! or a grey camera from the shadow under it.
//!
//! This cuts the subject out with a graph cut, the way GrabCut does: every pixel is labelled
//! subject or backdrop at the least total cost, where a label costs what the pixel's colour makes
//! unlikely and a change of label between neighbours costs what their likeness makes unlikely, so
//! the cut runs along the subject's edges. What sets the costs:
//!
//! - **The backdrop as painted**: [`Backdrop`] estimates it at every pixel from the frame's edge,
//!   and the colours of the backdrop pixels found so far are learned beside it.
//! - **The subject's colours**, learned from the pixels no backdrop explains, then from each cut.
//! - **Shadows**: a pixel that is the backdrop darkened, compared with the backdrop as lit around
//!   it (a spotlight included), is the backdrop's shadow, as long as the frame's edge reaches it
//!   without crossing an edge. A black beak or a grey camera behind one is the subject's, however
//!   dark.
//! - **How far each pixel is from the frame along the smoothest path** ([`geodesic`]), measured
//!   on a coarse copy: a glow, a wash or a soft shadow is reached for nothing, the subject only
//!   by crossing its edge. It leans a pixel one way or the other, and decides where to start.
//!
//! The cut is made at [`WORK_SIDE`] px, a few times over while the colours are learned, then
//! tidied: a speck the backdrop's colour goes, and a shadow drawn apart under the subject; a small
//! hole in the subject closes unless it shows the backdrop. The edge is then cut again at the
//! picture's own size in a narrow band, and softened where a pixel is part subject and part
//! backdrop. [`crate::matte::cut_by_mask`] takes it from there, as it takes Vision's mask.
//!
//! Tested against Vision on 26 of klein's pictures: 19 free icons and 7 whole folders. It agrees
//! with Vision to within a few pixels of edge on most of them (0.96 to 0.996 of their union
//! shared), and is the better of the two on a bicycle, whose wheels Vision fills, and a flamingo
//! lagoon on a folder, where Vision takes only the flamingo. A neon sign on a dark wall keeps the
//! wall its glow lights inside its outline.

mod gmm;
mod maxflow;

use crate::backdrop::Backdrop;
use gmm::Gmm;
use image::{imageops, imageops::FilterType, GrayImage, Luma, RgbaImage};
use maxflow::Graph;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The longer side the cut is made at, in px, before its edge is cut again at full size.
pub const WORK_SIDE: u32 = 384;
/// The longer side the geodesic distance is measured at: coarse enough that a lit rim a few
/// pixels wide is one step, while a wash across the paper stays smooth.
const GEODESIC_SIDE: u32 = 128;
/// Rounds of learning the colours and cutting again.
const ROUNDS: usize = 4;
/// Gaussians in each colour model.
const PARTS: usize = 5;
/// What a change of label costs between two neighbours of the same colour, in nats.
const SMOOTHNESS: f64 = 50.0;
/// Costs in nats become whole capacities at this many per nat.
const SCALE: f64 = 64.0;
/// No pixel is surer than this, in nats, whatever its colour.
const MAX_COST: f64 = 60.0;
/// The capacity that keeps the frame's band on the backdrop.
const HARD: i64 = 1 << 30;
/// The frame's outer band is this share of the shorter side deep.
const BAND_SHARE: u32 = 100;
/// The nats the geodesic distance leans a pixel by, at most, and the distance (0..=255 colour
/// units) at which it leans it all the way to the subject.
const REACH_NATS: f64 = 1.0;
const REACH_FULL: f32 = 40.0;
/// A seed of the subject is at least this far from the frame, and the frame reaches a pixel
/// "for nothing" within [`OPEN_REACH`].
const SEED_REACH: f32 = 1.0;
const OPEN_REACH: f32 = 0.5;
/// A shadow is the backdrop scaled by between these, and within [`SHADOW_SPREAD`] of that line
/// (colour units).
const SHADOW_DARKEST: f32 = 0.05;
const SHADOW_LIGHTEST: f32 = 0.98;
const SHADOW_SPREAD: f64 = 5.0;
/// How likely a pixel on the backdrop is to be in shadow, before anything is known of it.
const SHADOW_PRIOR: f64 = 0.15;
/// The geodesic distance at which a shadow is half as likely: one the frame reaches only across
/// an edge is the subject's.
const SHADOW_REACH: f32 = 20.0;
/// Beyond this geodesic distance a pixel the colour of the backdrop's shadow can seed the
/// subject: a white mug's lavender stripe on a lavender sweep, which no other part explains.
const ENCLOSED_REACH: f32 = 2.0 * SHADOW_REACH;
/// The share of the frame's edge that must be within the backdrop's own noise: a painting that
/// fills the frame has no subject to lift.
const PLAIN: f32 = 0.9;

/// How much each pixel of `img` belongs to its subject, the way [`crate::lift::subject_mask`]
/// gives it: 255 on it, 0 around it, soft along its edge, the size of `img`. `None` when `img`
/// isn't a subject on a plain backdrop, or no subject stands out from it.
pub fn subject_mask(img: &RgbaImage) -> Option<GrayImage> {
    let (w, h) = img.dimensions();
    let small = if w.max(h) > WORK_SIDE {
        let scale = WORK_SIDE as f32 / w.max(h) as f32;
        let (sw, sh) = (
            ((w as f32 * scale).round() as u32).max(16),
            ((h as f32 * scale).round() as u32).max(16),
        );
        imageops::resize(img, sw, sh, FilterType::Triangle)
    } else {
        img.clone()
    };
    let cut = Cut::make(&small)?;
    Some(if small.dimensions() == (w, h) {
        cut.soft(img)
    } else {
        cut.at_full_size(img)
    })
}

/// The cut made at the working size, and what it learned.
struct Cut {
    w: u32,
    h: u32,
    /// Subject (true) or backdrop.
    fg: Vec<bool>,
    /// The backdrop as painted, from the frame's edge in.
    backdrop: Vec<[f32; 3]>,
    /// The backdrop as lit, filled in under the subject and its shadows.
    lit: Vec<[f32; 3]>,
    /// Each pixel's geodesic distance from the frame.
    reach: Vec<f32>,
    costs: Costs,
}

impl Cut {
    fn make(img: &RgbaImage) -> Option<Cut> {
        let (w, h) = img.dimensions();
        let n = (w * h) as usize;
        let backdrop_model = Backdrop::measure(img).filter(|b| b.plain >= PLAIN)?;
        let colour: Vec<[f32; 3]> = img.pixels().map(|p| rgb(p.0)).collect();
        let backdrop: Vec<[f32; 3]> = (0..n)
            .map(|i| backdrop_model.at(i as u32 % w, i as u32 / w).map(f32::from))
            .collect();
        // The backdrop's own noise, in colour units.
        let sigma = f64::from(backdrop_model.noise * 255.0).max(4.0);
        let band = (w.min(h) / BAND_SHARE).max(2);
        let in_band = |i: usize| {
            let (x, y) = (i as u32 % w, i as u32 / w);
            x < band || y < band || x >= w - band || y >= h - band
        };
        let reach = reach_from_frame(img, (2.0 * sigma).max(6.0) as f32);
        let off = |i: usize| distance(colour[i], backdrop[i]);
        let shadowy = |i: usize| {
            let (s, r) = shadow_fit(colour[i], backdrop[i]);
            (0.2..=SHADOW_LIGHTEST).contains(&s) && r < 10.0
        };

        // Seeds of the subject: well away from the backdrop and behind an edge, and not its
        // shadow, unless the frame reaches it only far across an edge, where no shadow falls.
        let far = (6.0 * sigma).max(30.0) as f32;
        let mut fg: Vec<bool> = (0..n)
            .map(|i| {
                !in_band(i)
                    && reach[i] > SEED_REACH
                    && off(i) > far
                    && (!shadowy(i) || reach[i] > ENCLOSED_REACH)
            })
            .collect();
        // The backdrop for certain: the band, what the backdrop model explains, a shadow and
        // anything else the frame reaches for nothing. The rest is unknown until the first cut.
        let sure_backdrop: Vec<bool> = (0..n)
            .map(|i| {
                in_band(i)
                    || off(i) < (3.0 * sigma) as f32
                    || (shadowy(i) && reach[i] <= SEED_REACH)
                    || reach[i] <= OPEN_REACH
            })
            .collect();
        // And a seed is a colour the backdrop doesn't have: less likely under its colours than
        // nearly all of the backdrop is. The glow a subject throws around itself is the
        // backdrop's.
        let known: Vec<[f32; 3]> = (0..n)
            .filter(|&i| sure_backdrop[i])
            .map(|i| colour[i])
            .collect();
        if let Some(model) = Gmm::fit(&known, PARTS) {
            let mut own: Vec<f64> = known
                .iter()
                .step_by((known.len() / 20_000).max(1))
                .map(|&c| model.log_density(c))
                .collect();
            own.sort_by(f64::total_cmp);
            let floor = own[own.len() / 20];
            for (f, &c) in fg.iter_mut().zip(&colour) {
                if *f && model.log_density(c) >= floor {
                    *f = false;
                }
            }
        }
        if fg.iter().filter(|&&f| f).count() < n / 200 {
            return None;
        }

        let links = links(&colour, w, h);
        let mut lit = backdrop.clone();
        let mut costs = None;
        for round in 0..ROUNDS {
            if round > 0 {
                // The backdrop as lit, from the backdrop pixels the last cut found that aren't
                // in shadow.
                let weight: Vec<f32> = (0..n)
                    .map(|i| {
                        let (s, r) = shadow_fit(colour[i], lit[i]);
                        let shade = (SHADOW_DARKEST..=0.9).contains(&s) && r < 15.0;
                        if !fg[i] && !shade {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect();
                lit = push_pull(&colour, &weight, w, h);
            }
            // The first colours are learned from what's certain, the rest from the last cut.
            let not_fg: Vec<bool> = fg.iter().map(|f| !f).collect();
            let backdrop_set = if round == 0 { &sure_backdrop } else { &not_fg };
            let c = Costs::learn(&colour, &lit, &fg, backdrop_set, sigma)?;
            let mut graph = Graph::new(n, links.len());
            for i in 0..n {
                let (to_source, to_sink) = if in_band(i) {
                    (0, HARD)
                } else {
                    capacities(c.of(colour[i], backdrop[i], lit[i], reach[i]))
                };
                graph.add_terminals(i, to_source, to_sink);
            }
            for &(i, j, capacity) in &links {
                graph.add_edge(i as usize, j as usize, capacity);
            }
            graph.maxflow();
            for (i, f) in fg.iter_mut().enumerate() {
                *f = graph.on_source_side(i);
            }
            costs = Some(c);
        }
        let not_fg: Vec<bool> = fg.iter().map(|f| !f).collect();
        let costs = Costs::learn(&colour, &lit, &fg, &not_fg, sigma).or(costs)?;

        // A speck the backdrop's colour goes, and a shadow drawn apart under the subject; a small
        // hole closes unless it shows the backdrop.
        let like_backdrop = |region: &[usize]| {
            let total: f32 = region.iter().map(|&i| distance(colour[i], lit[i])).sum();
            total / region.len() as f32 <= far
        };
        flip_small(&mut fg, w, h, true, n / 2000, &like_backdrop);
        drop_ground_shadow(&mut fg, &colour, &lit, w, h);
        flip_small(&mut fg, w, h, false, n / 500, &|region: &[usize]| {
            !like_backdrop(region)
        });
        (fg.iter().filter(|&&f| f).count() >= n / 200).then_some(Cut {
            w,
            h,
            fg,
            backdrop,
            lit,
            reach,
            costs,
        })
    }

    /// The cut as a mask of `img`, which is the size it was made at, its edge softened.
    fn soft(&self, img: &RgbaImage) -> GrayImage {
        let n = self.fg.len();
        let candidates: Vec<usize> = (0..n).collect();
        soften(img, &self.fg, &candidates, &|i| self.backdrop[i])
    }

    /// The cut's edge cut again at `img`'s own size, in a band around it, then softened.
    fn at_full_size(&self, img: &RgbaImage) -> GrayImage {
        let (w, h) = img.dimensions();
        let n = (w * h) as usize;
        let (sw, sh) = (self.w, self.h);
        let coarse = GrayImage::from_fn(sw, sh, |x, y| {
            Luma([if self.fg[(y * sw + x) as usize] {
                255
            } else {
                0
            }])
        });
        let mut fg: Vec<bool> = imageops::resize(&coarse, w, h, FilterType::Triangle)
            .pixels()
            .map(|p| p.0[0] >= 128)
            .collect();
        // Where the coarse cut's labels change, and the band around it: a coarse pixel and one
        // more either side.
        let reach = ((w as f32 / sw as f32).ceil() as i64 + 1).max(2);
        let mut in_band = vec![false; n];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let changes =
                    (x + 1 < w && fg[i + 1] != fg[i]) || (y + 1 < h && fg[i + w as usize] != fg[i]);
                if !changes {
                    continue;
                }
                for ny in (y as i64 - reach).max(0)..=(y as i64 + reach).min(h as i64 - 1) {
                    for nx in (x as i64 - reach).max(0)..=(x as i64 + reach).min(w as i64 - 1) {
                        in_band[(ny as u32 * w + nx as u32) as usize] = true;
                    }
                }
            }
        }
        let ids: Vec<usize> = (0..n).filter(|&i| in_band[i]).collect();
        let mut node = vec![u32::MAX; n];
        for (k, &i) in ids.iter().enumerate() {
            node[i] = k as u32;
        }
        let colour = |i: usize| rgb(img.get_pixel(i as u32 % w, i as u32 / w).0);
        // The working size's estimates, at this size.
        let at = |i: usize| {
            (
                (i as u32 % w) as f32 * sw as f32 / w as f32,
                (i as u32 / w) as f32 * sh as f32 / h as f32,
            )
        };
        let backdrop = |i: usize| {
            let (x, y) = at(i);
            sample3(&self.backdrop, sw, sh, x, y)
        };
        let lit = |i: usize| {
            let (x, y) = at(i);
            sample3(&self.lit, sw, sh, x, y)
        };
        let reach_of = |i: usize| {
            let (x, y) = at(i);
            sample1(&self.reach, sw, sh, x, y)
        };

        // The same smoothness per length of edge as the working size's.
        let beta = contrast(ids.iter().copied(), w, h, &colour);
        let smoothness = SMOOTHNESS * sw as f64 / w as f64;
        let mut graph = Graph::new(ids.len(), ids.len() * 4);
        let mut fixed = vec![(0i64, 0i64); ids.len()];
        for (k, &i) in ids.iter().enumerate() {
            let (x, y) = ((i as u32 % w) as i64, (i as u32 / w) as i64);
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = (ny as u32 * w + nx as u32) as usize;
                let length = if dx != 0 && dy != 0 {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                };
                let capacity = link(smoothness / length, beta, colour(i), colour(j));
                if in_band[j] {
                    // Each pair once.
                    if (dy, dx) > (0, 0) {
                        graph.add_edge(k, node[j] as usize, capacity);
                    }
                } else if fg[j] {
                    fixed[k].0 += capacity as i64;
                } else {
                    fixed[k].1 += capacity as i64;
                }
            }
        }
        for (k, &i) in ids.iter().enumerate() {
            let (to_source, to_sink) =
                capacities(self.costs.of(colour(i), backdrop(i), lit(i), reach_of(i)));
            graph.add_terminals(k, to_source + fixed[k].0, to_sink + fixed[k].1);
        }
        graph.maxflow();
        for (k, &i) in ids.iter().enumerate() {
            fg[i] = graph.on_source_side(k);
        }
        // No speck or pinhole the band's cut left behind.
        let tiny = (n / 20_000).max(4);
        flip_small(&mut fg, w, h, true, tiny, &|_: &[usize]| true);
        flip_small(&mut fg, w, h, false, tiny, &|_: &[usize]| true);
        soften(img, &fg, &ids, &backdrop)
    }
}

/// What labelling a pixel subject or backdrop costs.
struct Costs {
    subject: Gmm,
    backdrop: Gmm,
    sigma: f64,
}

impl Costs {
    /// The colours of the subject from `fg`, and of the backdrop from `backdrop_set` less its
    /// shadows, which have their own explanation: a dark shadow would teach the backdrop's
    /// colours the dark side of a cup standing in it.
    fn learn(
        colour: &[[f32; 3]],
        lit: &[[f32; 3]],
        fg: &[bool],
        backdrop_set: &[bool],
        sigma: f64,
    ) -> Option<Costs> {
        let subject: Vec<[f32; 3]> = (0..colour.len())
            .filter(|&i| fg[i])
            .map(|i| colour[i])
            .collect();
        let backdrop: Vec<[f32; 3]> = (0..colour.len())
            .filter(|&i| {
                let (s, r) = shadow_fit(colour[i], lit[i]);
                backdrop_set[i] && !((0.2..=0.9).contains(&s) && r < 15.0)
            })
            .map(|i| colour[i])
            .collect();
        Some(Costs {
            subject: Gmm::fit(&subject, PARTS)?,
            backdrop: Gmm::fit(&backdrop, PARTS)?,
            sigma,
        })
    }

    /// What labelling a pixel of colour `p` the subject costs, and the backdrop, in nats, where
    /// the backdrop is painted `backdrop` and lit `lit` and the frame reaches it at `reach`.
    fn of(&self, p: [f32; 3], backdrop: [f32; 3], lit: [f32; 3], reach: f32) -> (f64, f64) {
        let subject = self.subject.log_density(p);
        // The backdrop: as the model has it, or as its colours are, or in shadow.
        let d2 = f64::from(distance(p, backdrop)).powi(2);
        let model = -1.5 * (2.0 * std::f64::consts::PI * self.sigma * self.sigma).ln()
            - d2 / (2.0 * self.sigma * self.sigma);
        let mut back = log_sum(
            model + 0.5f64.ln(),
            self.backdrop.log_density(p) + 0.5f64.ln(),
        );
        let open = 1.0 / (1.0 + f64::from(reach / SHADOW_REACH).powi(2));
        let (s, r) = shadow_fit(p, lit);
        if (SHADOW_DARKEST..=SHADOW_LIGHTEST).contains(&s) {
            // Anywhere along the line from black to the lit backdrop, and a Gaussian across it.
            let length = f64::from(lit.iter().map(|v| v * v).sum::<f32>().sqrt()).max(1.0);
            let along = -(f64::from(SHADOW_LIGHTEST - SHADOW_DARKEST) * length).ln();
            let across = -(2.0 * std::f64::consts::PI * SHADOW_SPREAD * SHADOW_SPREAD).ln()
                - f64::from(r).powi(2) / (2.0 * SHADOW_SPREAD * SHADOW_SPREAD);
            back = log_sum(back, along + across + (SHADOW_PRIOR * open).ln());
        }
        let lean = f64::from((reach / REACH_FULL).min(1.0));
        (
            (-subject).clamp(0.0, MAX_COST) + REACH_NATS * (1.0 - lean),
            (-back).clamp(0.0, MAX_COST) + REACH_NATS * lean,
        )
    }
}

/// A pixel's two costs as capacities from the source and to the sink: only their difference
/// matters.
fn capacities((subject, backdrop): (f64, f64)) -> (i64, i64) {
    let least = subject.min(backdrop);
    (
        ((backdrop - least) * SCALE).round() as i64,
        ((subject - least) * SCALE).round() as i64,
    )
}

/// The eight neighbours, as offsets.
const NEIGHBOURS: [(i64, i64); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
];

/// How much a change of label costs between two neighbours: `weight` for the same colour, much
/// less across an edge, by `beta`.
fn link(weight: f64, beta: f64, a: [f32; 3], b: [f32; 3]) -> i32 {
    let d2 = f64::from(distance(a, b)).powi(2);
    (weight * (-beta * d2).exp() * SCALE).round() as i32
}

/// GrabCut's contrast: one over twice the mean squared difference between neighbours, over
/// `pixels`, so an edge is judged against the picture's own.
fn contrast(
    pixels: impl Iterator<Item = usize>,
    w: u32,
    h: u32,
    colour: &dyn Fn(usize) -> [f32; 3],
) -> f64 {
    let (mut total, mut count) = (0.0, 0.0);
    for i in pixels {
        let (x, y) = ((i as u32 % w) as i64, (i as u32 / w) as i64);
        for (dx, dy) in [(1i64, 0i64), (0, 1), (1, 1), (-1, 1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                let j = (ny as u32 * w + nx as u32) as usize;
                total += f64::from(distance(colour(i), colour(j))).powi(2);
                count += 1.0;
            }
        }
    }
    if total > 0.0 {
        count / (2.0 * total)
    } else {
        0.0
    }
}

/// Every pair of neighbours at the working size, once, with what cutting between them costs.
fn links(colour: &[[f32; 3]], w: u32, h: u32) -> Vec<(u32, u32, i32)> {
    let n = colour.len();
    let beta = contrast(0..n, w, h, &|i| colour[i]);
    let mut links = Vec::with_capacity(n * 4);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let i = (y as u32 * w + x as u32) as usize;
            for (dx, dy) in [(1i64, 0i64), (0, 1), (1, 1), (-1, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = (ny as u32 * w + nx as u32) as usize;
                let length = if dx != 0 && dy != 0 {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                };
                links.push((
                    i as u32,
                    j as u32,
                    link(SMOOTHNESS / length, beta, colour[i], colour[j]),
                ));
            }
        }
    }
    links
}

/// Each pixel's distance from the frame along the path that climbs least ([`geodesic`]),
/// measured on a copy [`GEODESIC_SIDE`] px across and brought back to `img`'s size.
fn reach_from_frame(img: &RgbaImage, slack: f32) -> Vec<f32> {
    let (w, h) = img.dimensions();
    let scale = (GEODESIC_SIDE as f32 / w.max(h) as f32).min(1.0);
    let (gw, gh) = (
        ((w as f32 * scale).round() as u32).max(8),
        ((h as f32 * scale).round() as u32).max(8),
    );
    let coarse = imageops::resize(img, gw, gh, FilterType::Triangle);
    let colour: Vec<[f32; 3]> = coarse.pixels().map(|p| rgb(p.0)).collect();
    let distances = geodesic(&colour, gw, gh, (gw.min(gh) / BAND_SHARE).max(1), slack);
    (0..w * h)
        .map(|i| {
            let x = ((i % w) as f32 + 0.5) * gw as f32 / w as f32 - 0.5;
            let y = ((i / w) as f32 + 0.5) * gh as f32 / h as f32 - 0.5;
            sample1(&distances, gw, gh, x, y)
        })
        .collect()
}

/// Each pixel's distance from the frame's outer band (`band` px deep) along the path that climbs
/// least: a step between neighbours costs how far their colours differ beyond `slack`, so a
/// gradient, a glow or a soft shadow is crossed for nothing and an edge costs its height.
fn geodesic(colour: &[[f32; 3]], w: u32, h: u32, band: u32, slack: f32) -> Vec<f32> {
    let n = colour.len();
    let mut best = vec![f32::INFINITY; n];
    let mut queue = BinaryHeap::new();
    for y in 0..h {
        for x in 0..w {
            if x < band || y < band || x >= w - band || y >= h - band {
                let i = (y * w + x) as usize;
                best[i] = 0.0;
                queue.push(Reverse((0u32, i as u32)));
            }
        }
    }
    // Distances are queued in sixteenths, so equal ones leave in the same order every time.
    let key = |d: f32| (d * 16.0).round() as u32;
    while let Some(Reverse((k, i))) = queue.pop() {
        let i = i as usize;
        if k > key(best[i]) {
            continue;
        }
        let (x, y) = ((i as u32 % w) as i64, (i as u32 / w) as i64);
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let j = (ny as u32 * w + nx as u32) as usize;
            let d = best[i] + (distance(colour[i], colour[j]) - slack).max(0.0);
            if d < best[j] {
                best[j] = d;
                queue.push(Reverse((key(d), j as u32)));
            }
        }
    }
    best
}

/// `colour` where `weight` is 1, and filled in smoothly from around where it's 0: the push-pull
/// of Gortler and others, averaging down a pyramid and blending back up.
fn push_pull(colour: &[[f32; 3]], weight: &[f32], w: u32, h: u32) -> Vec<[f32; 3]> {
    // Each level: colour times weight, and the weight, capped at 1.
    let mut levels: Vec<(u32, u32, Vec<[f32; 4]>)> = vec![(
        w,
        h,
        colour
            .iter()
            .zip(weight)
            .map(|(c, &a)| [c[0] * a, c[1] * a, c[2] * a, a])
            .collect(),
    )];
    loop {
        let (lw, lh, px) = levels.last().expect("a level");
        if *lw <= 1 && *lh <= 1 {
            break;
        }
        let (nw, nh) = (lw.div_ceil(2), lh.div_ceil(2));
        let mut next = vec![[0f32; 4]; (nw * nh) as usize];
        for y in 0..*lh {
            for x in 0..*lw {
                let p = px[(y * lw + x) as usize];
                let q = &mut next[((y / 2) * nw + x / 2) as usize];
                (0..4).for_each(|t| q[t] += p[t]);
            }
        }
        for q in &mut next {
            if q[3] > 1.0 {
                let k = 1.0 / q[3];
                q.iter_mut().for_each(|v| *v *= k);
            }
        }
        levels.push((nw, nh, next));
    }
    // Back up: a pixel short of weight takes the rest from the level above.
    for l in (0..levels.len() - 1).rev() {
        let (upper, lower) = levels.split_at_mut(l + 1);
        let (lw, lh, px) = &mut upper[l];
        let (cw, _, coarse) = &lower[0];
        for y in 0..*lh {
            for x in 0..*lw {
                let up = coarse[((y / 2) * cw + x / 2) as usize];
                let p = &mut px[(y * *lw + x) as usize];
                let rest = 1.0 - p[3].min(1.0);
                let up_weight = up[3].max(1e-6);
                for t in 0..3 {
                    p[t] += rest * up[t] / up_weight;
                }
                p[3] = 1.0;
            }
        }
    }
    levels[0].2.iter().map(|p| [p[0], p[1], p[2]]).collect()
}

/// A mask of `fg` whose edge is softened: each of `candidates` on the edge is the mix of the
/// subject's colour just inside and the backdrop's just outside that it looks like.
fn soften(
    img: &RgbaImage,
    fg: &[bool],
    candidates: &[usize],
    backdrop: &dyn Fn(usize) -> [f32; 3],
) -> GrayImage {
    let (w, h) = img.dimensions();
    let on_edge = |i: usize| {
        let (x, y) = ((i as u32 % w) as i64, (i as u32 / w) as i64);
        [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
            .iter()
            .any(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0
                    && ny >= 0
                    && nx < w as i64
                    && ny < h as i64
                    && fg[(ny as u32 * w + nx as u32) as usize] != fg[i]
            })
    };
    let colour = |i: usize| rgb(img.get_pixel(i as u32 % w, i as u32 / w).0);
    let mut alpha: Vec<u8> = fg.iter().map(|&f| if f { 255 } else { 0 }).collect();
    for &i in candidates {
        if !on_edge(i) {
            continue;
        }
        let (x, y) = ((i as u32 % w) as i64, (i as u32 / w) as i64);
        let (mut fore, mut fore_n, mut back, mut back_n) = ([0f32; 3], 0f32, [0f32; 3], 0f32);
        for ny in (y - 3).max(0)..=(y + 3).min(h as i64 - 1) {
            for nx in (x - 3).max(0)..=(x + 3).min(w as i64 - 1) {
                let j = (ny as u32 * w + nx as u32) as usize;
                if on_edge(j) {
                    continue;
                }
                let c = colour(j);
                if fg[j] {
                    (0..3).for_each(|t| fore[t] += c[t]);
                    fore_n += 1.0;
                } else {
                    (0..3).for_each(|t| back[t] += c[t]);
                    back_n += 1.0;
                }
            }
        }
        if fore_n < 3.0 {
            continue;
        }
        let f = fore.map(|v| v / fore_n);
        let b = if back_n >= 3.0 {
            back.map(|v| v / back_n)
        } else {
            backdrop(i)
        };
        let fb = [f[0] - b[0], f[1] - b[1], f[2] - b[2]];
        let length2 = fb.iter().map(|v| v * v).sum::<f32>();
        // Colours too alike to tell a mix by: the cut's own label stands.
        if length2 < 24.0 * 24.0 {
            continue;
        }
        let p = colour(i);
        let share =
            ((p[0] - b[0]) * fb[0] + (p[1] - b[1]) * fb[1] + (p[2] - b[2]) * fb[2]) / length2;
        alpha[i] = (share.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    GrayImage::from_raw(w, h, alpha).expect("one alpha per pixel")
}

/// Drops a part apart from the subject that is a shadow drawn under it: below the largest part,
/// flat, and darker and duller than the backdrop around it.
fn drop_ground_shadow(fg: &mut [bool], colour: &[[f32; 3]], lit: &[[f32; 3]], w: u32, h: u32) {
    let parts = regions(fg, w, h, true);
    let Some(main) = parts.iter().max_by_key(|p| p.len()) else {
        return;
    };
    let (_, main_top, _, main_bottom) = bounds(main, w);
    let luma = |c: [f32; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let chroma = |c: [f32; 3]| {
        c.iter().cloned().fold(f32::MIN, f32::max) - c.iter().cloned().fold(f32::MAX, f32::min)
    };
    let mut shadows = Vec::new();
    for part in &parts {
        if part.len() * 3 > main.len() {
            continue;
        }
        let (x0, y0, x1, y1) = bounds(part, w);
        let below = (y0 + y1) / 2 > main_bottom.saturating_sub((main_bottom - main_top) / 20);
        let flat = x1 - x0 + 1 >= 2 * (y1 - y0 + 1);
        let mean = |of: &[[f32; 3]]| {
            let mut sum = [0f32; 3];
            for &i in part.iter() {
                (0..3).for_each(|t| sum[t] += of[i][t]);
            }
            sum.map(|v| v / part.len() as f32)
        };
        let (c, b) = (mean(colour), mean(lit));
        let shade = luma(c) < luma(b) - 8.0 && chroma(c) <= chroma(b) + 8.0;
        if below && flat && shade {
            shadows.push(part);
        }
    }
    for part in shadows {
        for &i in part {
            fg[i] = false;
        }
    }
}

/// The box around a region: left, top, right, bottom.
fn bounds(region: &[usize], w: u32) -> (u32, u32, u32, u32) {
    region.iter().fold((u32::MAX, u32::MAX, 0, 0), |b, &i| {
        let (x, y) = (i as u32 % w, i as u32 / w);
        (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y))
    })
}

/// Flips the regions of `value` smaller than `min` pixels that `flip` agrees to; for the backdrop
/// (`value` false), only those that don't reach the frame.
fn flip_small(
    fg: &mut [bool],
    w: u32,
    h: u32,
    value: bool,
    min: usize,
    flip: &dyn Fn(&[usize]) -> bool,
) {
    for region in regions(fg, w, h, value) {
        if region.len() >= min {
            continue;
        }
        let reaches_frame = region.iter().any(|&i| {
            let (x, y) = (i as u32 % w, i as u32 / w);
            x == 0 || y == 0 || x == w - 1 || y == h - 1
        });
        if (value || !reaches_frame) && flip(&region) {
            for i in region {
                fg[i] = !value;
            }
        }
    }
}

/// The 8-connected regions of `value`.
fn regions(fg: &[bool], w: u32, h: u32, value: bool) -> Vec<Vec<usize>> {
    let mut seen = vec![false; fg.len()];
    let mut out = Vec::new();
    for start in 0..fg.len() {
        if seen[start] || fg[start] != value {
            continue;
        }
        let mut region = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            region.push(i);
            let (x, y) = ((i as u32 % w) as i64, (i as u32 / w) as i64);
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let j = (ny as u32 * w + nx as u32) as usize;
                if !seen[j] && fg[j] == value {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        out.push(region);
    }
    out
}

/// How well `p` is `backdrop` darkened: the scale, and how far `p` is from the backdrop at that
/// scale (colour units).
fn shadow_fit(p: [f32; 3], backdrop: [f32; 3]) -> (f32, f32) {
    let bb: f32 = backdrop.iter().map(|v| v * v).sum();
    if bb < 1.0 {
        return (1.0, f32::MAX);
    }
    let s = (p[0] * backdrop[0] + p[1] * backdrop[1] + p[2] * backdrop[2]) / bb;
    let r = [0, 1, 2]
        .iter()
        .map(|&c| (p[c] - s * backdrop[c]).powi(2))
        .sum::<f32>()
        .sqrt();
    (s, r)
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn rgb(p: [u8; 4]) -> [f32; 3] {
    [p[0], p[1], p[2]].map(f32::from)
}

fn log_sum(a: f64, b: f64) -> f64 {
    let most = a.max(b);
    if most == f64::NEG_INFINITY {
        return most;
    }
    most + ((a - most).exp() + (b - most).exp()).ln()
}

/// `grid` (w x h) at pixel coordinates (x, y), bilinearly, clamped to the edge.
fn sample1(grid: &[f32], w: u32, h: u32, x: f32, y: f32) -> f32 {
    let (x, y) = (x.clamp(0.0, (w - 1) as f32), y.clamp(0.0, (h - 1) as f32));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: u32, y: u32| grid[(y * w + x) as usize];
    (at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx) * (1.0 - fy)
        + (at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx) * fy
}

/// [`sample1`] for colours.
fn sample3(grid: &[[f32; 3]], w: u32, h: u32, x: f32, y: f32) -> [f32; 3] {
    let (x, y) = (x.clamp(0.0, (w - 1) as f32), y.clamp(0.0, (h - 1) as f32));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: u32, y: u32| grid[(y * w + x) as usize];
    std::array::from_fn(|c| {
        (at(x0, y0)[c] * (1.0 - fx) + at(x1, y0)[c] * fx) * (1.0 - fy)
            + (at(x0, y1)[c] * (1.0 - fx) + at(x1, y1)[c] * fx) * fy
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A lavender studio sweep, lighter towards the floor, as klein paints magenta.
    fn sweep(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
        let t = y as f32 / h as f32;
        let s = x as f32 / w as f32;
        [
            170.0 + 35.0 * t + 6.0 * s,
            150.0 + 35.0 * t,
            205.0 + 20.0 * t - 4.0 * s,
        ]
    }

    /// How much of a pixel a shape covers, given its signed distance from the shape's edge
    /// (positive inside), with a one-pixel soft edge.
    fn cover(inside: f32) -> f32 {
        (inside + 0.5).clamp(0.0, 1.0)
    }

    /// A scene on the sweep: `shade` darkens the backdrop by a factor at each point (a shadow),
    /// then `subject` paints a colour with its coverage over it.
    fn scene(
        w: u32,
        h: u32,
        shade: impl Fn(f32, f32) -> f32,
        subject: impl Fn(f32, f32) -> Option<([f32; 3], f32)>,
    ) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let k = shade(fx, fy);
            let mut c = sweep(x, y, w, h).map(|v| v * k);
            if let Some((colour, a)) = subject(fx, fy) {
                c = [0, 1, 2].map(|i| c[i] * (1.0 - a) + colour[i] * a);
            }
            Rgba([
                c[0].round() as u8,
                c[1].round() as u8,
                c[2].round() as u8,
                255,
            ])
        })
    }

    /// A green disc at the middle with a soft shadow on the floor under it.
    fn disc_with_shadow(w: u32, h: u32) -> (RgbaImage, impl Fn(f32, f32) -> f32) {
        let (cx, cy, r) = (w as f32 * 0.5, h as f32 * 0.45, w as f32 * 0.22);
        let inside = move |x: f32, y: f32| r - ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        let shadow = move |x: f32, y: f32| {
            let (dx, dy) = ((x - cx) / (r * 1.1), (y - (cy + r * 1.05)) / (r * 0.22));
            1.0 - 0.5 * (-(dx * dx + dy * dy) * 1.5).exp()
        };
        let img = scene(w, h, shadow, move |x, y| {
            let a = cover(inside(x, y));
            // Shaded, lighter at the top left.
            let light = 0.8 + 0.2 * ((cy - y) + (cx - x)) / (2.0 * r);
            (a > 0.0).then_some(([40.0 * light, 150.0 * light, 80.0 * light], a))
        });
        (img, inside)
    }

    /// The share of the pixels the mask or `truth` gives the subject that both do.
    fn overlap(mask: &GrayImage, truth: impl Fn(f32, f32) -> bool) -> f64 {
        let (mut both, mut either) = (0u64, 0u64);
        for (x, y, p) in mask.enumerate_pixels() {
            let (a, b) = (p.0[0] >= 128, truth(x as f32 + 0.5, y as f32 + 0.5));
            both += u64::from(a && b);
            either += u64::from(a || b);
        }
        both as f64 / either as f64
    }

    #[test]
    fn a_subject_comes_off_its_sweep_and_its_shadow_stays_behind() {
        let (w, h) = (360, 340);
        let (img, inside) = disc_with_shadow(w, h);
        let mask = subject_mask(&img).expect("the disc");
        let shared = overlap(&mask, |x, y| inside(x, y) > 0.0);
        assert!(shared > 0.97, "{shared}");
        // The middle of the shadow, just under the disc, is the backdrop's.
        let (sx, sy) = (w / 2, (h as f32 * 0.45 + w as f32 * 0.22 * 1.1) as u32);
        assert!(mask.get_pixel(sx, sy).0[0] < 16, "shadow at {sx},{sy}");
    }

    /// Bigger than the working size: cut again at full size, the edge in the right place to
    /// within a pixel and soft.
    #[test]
    fn a_big_picture_is_cut_at_its_own_size() {
        let (w, h) = (900, 850);
        let (img, inside) = disc_with_shadow(w, h);
        let mask = subject_mask(&img).expect("the disc");
        assert_eq!(mask.dimensions(), (w, h));
        for (x, y, p) in mask.enumerate_pixels() {
            let d = inside(x as f32 + 0.5, y as f32 + 0.5);
            if d > 1.5 {
                assert!(p.0[0] >= 240, "inside at {x},{y}: {}", p.0[0]);
            } else if d < -1.5 {
                assert!(p.0[0] <= 16, "outside at {x},{y}: {}", p.0[0]);
            }
        }
        let soft = mask
            .pixels()
            .filter(|p| (16..240).contains(&p.0[0]))
            .count();
        assert!(soft > 500, "only {soft} soft pixels");
    }

    /// A band of the subject that is the backdrop's colour darkened, as a robot's joints are, is
    /// still the subject's: the frame reaches it only across the subject's edge.
    #[test]
    fn a_dark_part_behind_an_edge_stays() {
        let (w, h) = (360, 340);
        let (x0, x1, y0, y1) = (110.0, 250.0, 70.0, 270.0);
        let img = scene(
            w,
            h,
            |_, _| 1.0,
            move |x, y| {
                let a = cover((x - x0).min(x1 - x).min(y - y0).min(y1 - y));
                // White, with a band across it the colour of the sweep at 40%.
                let band = (150.0..180.0).contains(&y);
                let colour = if band {
                    sweep(x as u32, y as u32, w, h).map(|v| v * 0.4)
                } else {
                    [236.0, 234.0, 240.0]
                };
                (a > 0.0).then_some((colour, a))
            },
        );
        let mask = subject_mask(&img).expect("the block");
        assert!(mask.get_pixel(180, 165).0[0] >= 240, "the band went");
        let shared = overlap(&mask, |x, y| x > x0 && x < x1 && y > y0 && y < y1);
        assert!(shared > 0.97, "{shared}");
    }

    /// A flat, dull shadow drawn apart under the subject, as pixel art has it, goes.
    #[test]
    fn a_shadow_drawn_apart_under_the_subject_goes() {
        let (w, h) = (360, 340);
        let (cx, cy) = (180.0, 150.0);
        let img = scene(
            w,
            h,
            |_, _| 1.0,
            move |x, y| {
                let body = cover(70.0 - ((x - cx).powi(2) + (y - cy).powi(2)).sqrt());
                let ground =
                    cover(1.0 - ((x - cx) / 60.0).powi(2) - ((y - 265.0) / 10.0).powi(2)) * 0.9;
                if body > 0.0 {
                    Some(([220.0, 60.0, 40.0], body))
                } else if ground > 0.0 {
                    Some(([120.0, 116.0, 124.0], ground))
                } else {
                    None
                }
            },
        );
        let mask = subject_mask(&img).expect("the ball");
        assert!(
            mask.get_pixel(180, 265).0[0] < 16,
            "the drawn shadow stayed"
        );
        assert!(mask.get_pixel(180, 150).0[0] >= 240, "the ball went");
    }

    /// The backdrop seen through a ring is the backdrop's, however it's enclosed.
    #[test]
    fn the_backdrop_through_a_ring_stays_out() {
        let (w, h) = (360, 340);
        let (cx, cy) = (180.0, 170.0);
        let img = scene(
            w,
            h,
            |_, _| 1.0,
            move |x, y| {
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                let a = cover((110.0 - d).min(d - 55.0));
                (a > 0.0).then_some(([230.0, 170.0, 30.0], a))
            },
        );
        let mask = subject_mask(&img).expect("the ring");
        assert!(mask.get_pixel(180, 170).0[0] < 16, "the hole filled");
        assert!(mask.get_pixel(180, 90).0[0] >= 240, "the ring went");
    }

    #[test]
    fn a_plain_sweep_has_no_subject() {
        let img = scene(360, 340, |_, _| 1.0, |_, _| None);
        assert!(subject_mask(&img).is_none());
    }

    /// A picture that fills the frame to its edge has no backdrop to lift a subject off.
    #[test]
    fn a_painting_to_the_edge_has_no_subject() {
        let img = RgbaImage::from_fn(360, 340, |x, y| {
            let v = ((x * 7 + y * 13) % 97) as u8;
            Rgba([v * 2, 255 - v, (x % 64) as u8 * 4, 255])
        });
        assert!(subject_mask(&img).is_none());
    }

    #[test]
    fn push_pull_keeps_what_it_has_and_fills_what_it_lacks() {
        let (w, h) = (9, 7);
        let colour: Vec<[f32; 3]> = (0..w * h)
            .map(|i| [(i % w) as f32 * 10.0, 50.0, 90.0])
            .collect();
        let weight: Vec<f32> = (0..w * h)
            .map(|i| if (i % w) == 4 { 0.0 } else { 1.0 })
            .collect();
        let filled = push_pull(&colour, &weight, w, h);
        for i in 0..(w * h) as usize {
            if weight[i] == 1.0 {
                assert_eq!(filled[i], colour[i]);
            } else {
                // Between its neighbours' reds, and the same blue.
                assert!((20.0..=60.0).contains(&filled[i][0]), "{:?}", filled[i]);
                assert!((filled[i][2] - 90.0).abs() < 0.01);
            }
        }
    }

    #[test]
    fn the_distance_from_the_frame_climbs_only_at_edges() {
        let (w, h) = (40u32, 40u32);
        // A gradient, with a square of another colour in the middle.
        let colour: Vec<[f32; 3]> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                if (15..25).contains(&x) && (15..25).contains(&y) {
                    [200.0, 30.0, 30.0]
                } else {
                    [x as f32 * 2.0, 100.0, 100.0]
                }
            })
            .collect();
        let d = geodesic(&colour, w, h, 1, 6.0);
        assert_eq!(d[(5 * w + 30) as usize], 0.0, "the gradient costs nothing");
        assert!(
            d[(20 * w + 20) as usize] > 100.0,
            "the square is behind an edge"
        );
    }
}
