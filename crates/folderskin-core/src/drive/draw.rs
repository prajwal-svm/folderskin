//! What the drive shapes are drawn with: paths in canvas units, paints, and the two layers a
//! drive is drawn in around its face.
//!
//! Every shape is written in the 1024-unit canvas the folder uses ([`crate::geometry::CANVAS`]).
//! A [`Drawing`] scales it to the size it is drawn at, so a shape is the same drawing at 2048 px
//! for an icon and at 1024 px for the composer's layers.

use crate::geometry::{arc, fillet, Rect, CANVAS};
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, Paint, Path,
    PathBuilder, Pixmap, Point, RadialGradient, Shader, SpreadMode, Stroke, SweepGradient,
    Transform,
};

/// A colour as `0xRRGGBB`, opaque.
pub(crate) const fn rgb(hex: u32) -> [u8; 4] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

/// A colour as `0xRRGGBB` with `alpha` out of 255.
pub(crate) const fn rgba(hex: u32, alpha: u8) -> [u8; 4] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, alpha]
}

/// `c` with its alpha scaled by `by` (0 to 1).
pub(crate) fn fade(c: [u8; 4], by: f32) -> [u8; 4] {
    [
        c[0],
        c[1],
        c[2],
        (c[3] as f32 * by).round().clamp(0.0, 255.0) as u8,
    ]
}

/// `c` mixed towards `to` by `t` (0 to 1), alpha included.
pub(crate) fn mix(c: [u8; 4], to: [u8; 4], t: f32) -> [u8; 4] {
    std::array::from_fn(|i| (c[i] as f32 + (to[i] as f32 - c[i] as f32) * t).round() as u8)
}

/// How a path is painted, in canvas units.
#[derive(Clone, Debug)]
pub(crate) enum Ink {
    Solid([u8; 4]),
    /// From one point to another, the stops placed along the way (0 to 1).
    Linear((f32, f32), (f32, f32), Vec<(f32, [u8; 4])>),
    /// From a centre out to a radius.
    Radial((f32, f32), f32, Vec<(f32, [u8; 4])>),
    /// Round a centre, starting at an angle in degrees, clockwise from three o'clock.
    Sweep((f32, f32), f32, Vec<(f32, [u8; 4])>),
}

/// A solid colour.
pub(crate) fn solid(c: [u8; 4]) -> Ink {
    Ink::Solid(c)
}

/// Down from `y0` to `y1`.
pub(crate) fn down(y0: f32, y1: f32, stops: &[(f32, [u8; 4])]) -> Ink {
    Ink::Linear((0.0, y0), (0.0, y1), stops.to_vec())
}

/// Across from `x0` to `x1`.
pub(crate) fn across(x0: f32, x1: f32, stops: &[(f32, [u8; 4])]) -> Ink {
    Ink::Linear((x0, 0.0), (x1, 0.0), stops.to_vec())
}

/// Out from `(cx, cy)` to `r`.
pub(crate) fn radial(cx: f32, cy: f32, r: f32, stops: &[(f32, [u8; 4])]) -> Ink {
    Ink::Radial((cx, cy), r, stops.to_vec())
}

/// Round `(cx, cy)` from `start` degrees.
pub(crate) fn sweep(cx: f32, cy: f32, start: f32, stops: &[(f32, [u8; 4])]) -> Ink {
    Ink::Sweep((cx, cy), start, stops.to_vec())
}

/// Which of a drive's two layers something goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    /// Under the face's picture: the plain drive, its face included.
    Body,
    /// Over it: what stays in sight whatever is on the face.
    Over,
}

// ---------- paths, in canvas units ----------

/// A rectangle with every corner rounded by `r`.
pub(crate) fn rrect(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Path {
    rrect4(x0, y0, x1, y1, [r; 4])
}

/// A rectangle with its corners rounded by `[top left, top right, bottom right, bottom left]`.
pub(crate) fn rrect4(x0: f32, y0: f32, x1: f32, y1: f32, r: [f32; 4]) -> Path {
    let half = ((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let [tl, tr, br, bl] = r.map(|r| r.clamp(0.0, half));
    let mut pb = PathBuilder::new();
    corner(&mut pb, x0, y0, tl, 180.0);
    corner(&mut pb, x1, y0, tr, 270.0);
    corner(&mut pb, x1, y1, br, 0.0);
    corner(&mut pb, x0, y1, bl, 90.0);
    pb.close();
    pb.finish().expect("a rectangle")
}

/// One rounded corner at `(x, y)`, the arc starting at `from` degrees (its centre inside the
/// rectangle), or the bare corner when `r` is 0.
fn corner(pb: &mut PathBuilder, x: f32, y: f32, r: f32, from: f32) {
    if r <= 0.0 {
        if pb.is_empty() {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
        return;
    }
    let (cx, cy) = match from as i32 {
        180 => (x + r, y + r),
        270 => (x - r, y + r),
        0 => (x - r, y - r),
        _ => (x + r, y - r),
    };
    arc(pb, cx, cy, r, from, from + 90.0);
}

/// A circle, drawn clockwise, or anticlockwise to cut a hole in a shape it's added to.
pub(crate) fn circle_path(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32, clockwise: bool) {
    let mut sub = PathBuilder::new();
    if clockwise {
        arc(&mut sub, cx, cy, r, 0.0, 360.0);
    } else {
        arc(&mut sub, cx, cy, r, 360.0, 0.0);
    }
    sub.close();
    if let Some(p) = sub.finish() {
        pb.push_path(&p);
    }
}

/// A circle.
pub(crate) fn circle(cx: f32, cy: f32, r: f32) -> Path {
    let mut pb = PathBuilder::new();
    circle_path(&mut pb, cx, cy, r, true);
    pb.finish().expect("a circle")
}

/// A ring between two radii.
pub(crate) fn ring(cx: f32, cy: f32, outer: f32, inner: f32) -> Path {
    let mut pb = PathBuilder::new();
    circle_path(&mut pb, cx, cy, outer, true);
    circle_path(&mut pb, cx, cy, inner, false);
    pb.finish().expect("a ring")
}

/// A closed polygon through `points`, each corner rounded by a circular arc of radius `r` (0
/// for sharp) that meets both of its edges smoothly.
pub(crate) fn polygon(points: &[(f32, f32)], r: f32) -> Path {
    let mut pb = PathBuilder::new();
    let n = points.len();
    for i in 0..n {
        fillet(
            &mut pb,
            points[(i + n - 1) % n],
            points[i],
            points[(i + 1) % n],
            r,
        );
    }
    pb.close();
    pb.finish().expect("a polygon")
}

/// An open line through `points`, its corners rounded as [`polygon`] rounds them, so a line
/// along part of a polygon's outline lies exactly on it: how a rim light follows a shape's top.
pub(crate) fn open_polygon(points: &[(f32, f32)], r: f32) -> Path {
    let mut pb = PathBuilder::new();
    let n = points.len();
    pb.move_to(points[0].0, points[0].1);
    for i in 1..n.saturating_sub(1) {
        fillet(&mut pb, points[i - 1], points[i], points[i + 1], r);
    }
    if let Some(last) = points.last() {
        pb.line_to(last.0, last.1);
    }
    pb.finish().expect("a line")
}

/// An open line through `points`.
pub(crate) fn polyline(points: &[(f32, f32)]) -> Path {
    let mut pb = PathBuilder::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            pb.move_to(p.0, p.1);
        } else {
            pb.line_to(p.0, p.1);
        }
    }
    pb.finish().expect("a line")
}

/// The area a line `width` wide along `path` covers, with round ends and joins: how a glyph made
/// of lines becomes a shape that can be filled, pressed in or used as a mask.
pub(crate) fn outline_of(path: &Path, width: f32) -> Path {
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    path.stroke(&stroke, 4.0).expect("a line with some length")
}

/// Several paths as one.
pub(crate) fn join(paths: &[Path]) -> Path {
    let mut pb = PathBuilder::new();
    for p in paths {
        pb.push_path(p);
    }
    pb.finish().expect("some paths")
}

/// `path` moved by `(dx, dy)`.
pub(crate) fn moved(path: &Path, dx: f32, dy: f32) -> Path {
    path.clone()
        .transform(Transform::from_translate(dx, dy))
        .expect("a translation")
}

// ---------- the drawing ----------

/// A drive shape being drawn at one size: the two layers around its face, the face itself and
/// the edges the composer's outline shows.
pub(crate) struct Drawing {
    size: u32,
    scale: f32,
    /// Everything under the face's picture: the drive with nothing on it.
    pub body: Pixmap,
    /// Everything over the face's picture.
    pub over: Pixmap,
    face: Option<Path>,
    face_box: Rect,
    face_point: Option<(f32, f32)>,
    edges: Vec<Path>,
}

impl Drawing {
    pub fn new(size: u32) -> Drawing {
        Drawing {
            size,
            scale: size as f32 / CANVAS,
            body: Pixmap::new(size, size).expect("render size"),
            over: Pixmap::new(size, size).expect("render size"),
            face: None,
            face_box: Rect::new(0.0, 0.0, 0.0, 0.0),
            face_point: None,
            edges: Vec::new(),
        }
    }

    /// Pixels per canvas unit.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// `path`, in canvas units, at this drawing's size.
    fn dev(&self, path: &Path) -> Path {
        path.clone()
            .transform(Transform::from_scale(self.scale, self.scale))
            .expect("a scale")
    }

    fn layer(&mut self, layer: Layer) -> &mut Pixmap {
        match layer {
            Layer::Body => &mut self.body,
            Layer::Over => &mut self.over,
        }
    }

    /// The face: where a picture goes, cut to `path`, and cover-fitted to `path`'s box.
    pub fn set_face(&mut self, path: &Path) {
        let b = path.compute_tight_bounds().unwrap_or(path.bounds());
        self.face_box = Rect::new(b.left(), b.top(), b.right(), b.bottom());
        self.face = Some(self.dev(path));
    }

    /// A point in the face, in canvas units, that nothing over the face hides: its box's middle
    /// unless the shape says otherwise (a disc's middle is its hole).
    pub fn set_face_point(&mut self, x: f32, y: f32) {
        self.face_point = Some((x, y));
    }

    pub fn face(&self) -> &Path {
        self.face.as_ref().expect("every drive has a face")
    }

    pub fn face_box(&self) -> Rect {
        self.face_box
    }

    pub fn face_point(&self) -> (f32, f32) {
        let b = self.face_box;
        self.face_point
            .unwrap_or(((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0))
    }

    /// Records `path` as one of the drive's edges, for the composer's outline.
    pub fn edge(&mut self, path: &Path) {
        let p = self.dev(path);
        self.edges.push(p);
    }

    /// The drive's visible extent in canvas units, `[left, top, right, bottom]`: the box of every
    /// pixel either layer draws at all.
    pub fn extent(&self) -> [f32; 4] {
        let (mut x0, mut y0, mut x1, mut y1) = (self.size, self.size, 0, 0);
        for (i, (a, b)) in self
            .body
            .pixels()
            .iter()
            .zip(self.over.pixels())
            .enumerate()
        {
            if a.alpha() > 0 || b.alpha() > 0 {
                let (x, y) = (i as u32 % self.size, i as u32 / self.size);
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
        [x0, y0, x1, y1].map(|v| v as f32 / self.scale)
    }

    /// The edges as a white line 2.5 canvas units wide, for the composer to show where the drive
    /// is while a design is made flat.
    pub fn outline(&self) -> Pixmap {
        let mut pm = Pixmap::new(self.size, self.size).expect("render size");
        let mut white = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        white.set_color_rgba8(255, 255, 255, 255);
        let stroke = Stroke {
            width: 2.5 * self.scale,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        for edge in &self.edges {
            pm.stroke_path(edge, &white, &stroke, Transform::identity(), None);
        }
        let face = self.face().clone();
        pm.stroke_path(&face, &white, &stroke, Transform::identity(), None);
        pm
    }

    /// A tiny-skia shader for `ink`, at this drawing's size.
    fn shader(&self, ink: &Ink) -> Shader<'static> {
        let s = self.scale;
        let stops = |stops: &[(f32, [u8; 4])]| -> Vec<GradientStop> {
            stops
                .iter()
                .map(|&(at, c)| GradientStop::new(at, Color::from_rgba8(c[0], c[1], c[2], c[3])))
                .collect()
        };
        let fallback = |stops: &[(f32, [u8; 4])]| {
            let c = stops.first().map_or([0, 0, 0, 0], |s| s.1);
            Shader::SolidColor(Color::from_rgba8(c[0], c[1], c[2], c[3]))
        };
        match ink {
            Ink::Solid(c) => Shader::SolidColor(Color::from_rgba8(c[0], c[1], c[2], c[3])),
            Ink::Linear(a, b, st) => LinearGradient::new(
                Point::from_xy(a.0 * s, a.1 * s),
                Point::from_xy(b.0 * s, b.1 * s),
                stops(st),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or_else(|| fallback(st)),
            Ink::Radial(c, r, st) => RadialGradient::new(
                Point::from_xy(c.0 * s, c.1 * s),
                0.0,
                Point::from_xy(c.0 * s, c.1 * s),
                r * s,
                stops(st),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or_else(|| fallback(st)),
            Ink::Sweep(c, start, st) => SweepGradient::new(
                Point::from_xy(0.0, 0.0),
                0.0,
                360.0,
                stops(st),
                SpreadMode::Pad,
                Transform::from_rotate(*start).post_translate(c.0 * s, c.1 * s),
            )
            .unwrap_or_else(|| fallback(st)),
        }
    }

    fn paint(&self, ink: &Ink) -> Paint<'static> {
        Paint {
            shader: self.shader(ink),
            anti_alias: true,
            ..Paint::default()
        }
    }

    /// The anti-aliased coverage of `path` (canvas units).
    pub fn mask(&self, path: &Path) -> Mask {
        let mut m = Mask::new(self.size, self.size).expect("mask size");
        m.fill_path(
            &self.dev(path),
            FillRule::Winding,
            true,
            Transform::identity(),
        );
        m
    }

    /// Fills `path` with `ink`.
    pub fn fill(&mut self, layer: Layer, path: &Path, ink: &Ink) {
        let (p, paint) = (self.dev(path), self.paint(ink));
        self.layer(layer)
            .fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }

    /// Fills `path` with `ink`, only where `clip` is.
    pub fn fill_in(&mut self, layer: Layer, path: &Path, ink: &Ink, clip: &Path) {
        let mask = self.mask(clip);
        self.fill_masked(layer, path, ink, &mask);
    }

    /// Fills `path` with `ink`, only as far as `mask` lets it.
    pub fn fill_masked(&mut self, layer: Layer, path: &Path, ink: &Ink, mask: &Mask) {
        let (p, paint) = (self.dev(path), self.paint(ink));
        self.layer(layer).fill_path(
            &p,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            Some(mask),
        );
    }

    /// A line `width` canvas units wide along `path`.
    pub fn stroke(&mut self, layer: Layer, path: &Path, ink: &Ink, width: f32) {
        let (p, paint) = (self.dev(path), self.paint(ink));
        let stroke = Stroke {
            width: width * self.scale,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        self.layer(layer)
            .stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }

    /// Light (or shade) just inside `edge`, fading inward over `band` canvas units: the rim light
    /// the folder's panels have ([`crate::compositor`]), for a drive's edges. `edge` may be an open
    /// part of `inside`'s outline; the line sits on it and `inside` throws its outer half away.
    pub fn rim(&mut self, layer: Layer, edge: &Path, inside: &Path, c: [u8; 4], band: f32) {
        let mask = self.mask(inside);
        let e = self.dev(edge);
        for (w, a) in [(2.0, 0.45), (1.25, 0.55), (0.625, 0.65)] {
            let mut paint = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            paint.set_color_rgba8(c[0], c[1], c[2], (c[3] as f32 * a) as u8);
            let stroke = Stroke {
                width: w * band * self.scale,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            self.layer(layer)
                .stroke_path(&e, &paint, &stroke, Transform::identity(), Some(&mask));
        }
    }

    /// `glyph` pressed into the surface under it, the way a mark is pressed into a disk's front:
    /// a shade darker than `surface`, its top edge in shadow and a lit lip under its bottom edge.
    /// `depth` is how far the shadow and the lip reach, in canvas units.
    pub fn press(&mut self, layer: Layer, glyph: &Path, surface: [u8; 4], depth: f32) {
        let lip = moved(glyph, 0.0, depth);
        self.fill(layer, &lip, &solid(rgba(0xffffff, 120)));
        let dark = mix(surface, rgb(0x000000), 0.14);
        self.fill(layer, glyph, &solid(dark));
        // The glyph less its own copy moved down: the band along its top that the wall shades.
        let mut top = self.mask(glyph);
        let mut under = self.mask(&moved(glyph, 0.0, depth * 1.3));
        under.invert();
        for (a, b) in top.data_mut().iter_mut().zip(under.data()) {
            *a = ((*a as u16 * *b as u16 + 127) / 255) as u8;
        }
        let b = glyph.bounds();
        let cover = rrect(
            b.left() - 4.0,
            b.top() - 4.0,
            b.right() + 4.0,
            b.bottom() + 4.0,
            0.0,
        );
        self.fill_masked(layer, &cover, &solid(rgba(0x000000, 70)), &top);
    }
}
