//! Drives: the shapes FolderSkin draws for a volume, and a skin drawn on one.
//!
//! A folder has one template, in two looks. A drive has a shape for every kind of drive each
//! system draws: the Mac's upright disks, Windows' flat drives, Linux's disks, sticks and cards
//! (docs/DRIVES.md lists them). Each is FolderSkin's own drawing in the spirit of its system, made
//! in code on the same 1024-unit canvas as the folder ([`crate::geometry::CANVAS`]).
//!
//! Every shape has a face: the part a picture covers and where a design's layers sit. A shape is
//! drawn in two layers around it ([`draw::Drawing`]): the plain drive underneath, face and all, and
//! what goes over the face (its rims, the shading that makes it a solid thing, a connector or a
//! badge that has to stay in sight). So a drive with nothing on it is the two layers together, a
//! skin fills the face between them, and a see-through part of a design shows the drive's own face.
//!
//! Rendering follows the folder's single path: one master at [`RENDER_SIZE`], every smaller size a
//! Lanczos3 downsample of it ([`crate::compositor`]), and [`layers`] splits the same drawing into
//! the layers the composer stacks a design between, so its canvas shows the icon that is saved.

pub mod detect;
pub(crate) mod draw;
mod linux;
mod mac;
mod windows;

use crate::compositor::{Artwork, Piece, Role, TemplateLayers, RENDER_SIZE};
use crate::geometry::Rect;
use crate::{fit, raster};
use draw::{Drawing, Layer};
use tiny_skia::{FillRule, FilterQuality, Paint, Pattern, Pixmap, SpreadMode, Transform};

/// Which system's drives a shape is drawn after.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriveStyle {
    Mac,
    Windows,
    Linux,
}

impl DriveStyle {
    /// Every style, in the order the composer lists them.
    pub const ALL: [DriveStyle; 3] = [DriveStyle::Mac, DriveStyle::Windows, DriveStyle::Linux];

    pub fn id(self) -> &'static str {
        match self {
            DriveStyle::Mac => "mac",
            DriveStyle::Windows => "windows",
            DriveStyle::Linux => "linux",
        }
    }

    pub fn from_id(id: &str) -> Option<DriveStyle> {
        DriveStyle::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The style of the system FolderSkin is running on: a drive picked here is drawn the way this
    /// system draws drives.
    pub fn current() -> DriveStyle {
        if cfg!(target_os = "windows") {
            DriveStyle::Windows
        } else if cfg!(target_os = "macos") {
            DriveStyle::Mac
        } else {
            DriveStyle::Linux
        }
    }

    /// The kinds this style has a shape of its own for, in the order the composer lists them.
    pub fn kinds(self) -> &'static [DriveKind] {
        use DriveKind::*;
        match self {
            DriveStyle::Mac => &[
                Startup,
                Internal,
                External,
                Removable,
                Card,
                Optical,
                DiskImage,
                Network,
                TimeMachine,
            ],
            DriveStyle::Windows => &[Startup, Internal, Removable, Card, Optical, Network],
            DriveStyle::Linux => &[
                Internal,
                SolidState,
                External,
                Removable,
                Card,
                OpticalDrive,
                Optical,
                Server,
                Network,
                MultiDisk,
            ],
        }
    }
}

/// What kind of drive a volume is, and so which of a style's shapes it is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriveKind {
    /// The disk the system runs from.
    Startup,
    /// A disk inside the computer.
    Internal,
    /// An internal disk that doesn't spin. Only Linux draws it apart.
    SolidState,
    /// A disk that is plugged in and isn't removable media: a USB or Thunderbolt disk.
    External,
    /// Removable media, most often a USB stick.
    Removable,
    /// A memory card.
    Card,
    /// A disc: CD, DVD or Blu-ray.
    Optical,
    /// The drive a disc goes in. Linux draws it apart from the disc; a mounted disc is `Optical`.
    OpticalDrive,
    /// A mounted disk image.
    DiskImage,
    /// A network share.
    Network,
    /// A server on the network. Only Linux has a shape for it, for designing.
    Server,
    /// A disk Time Machine backs up to.
    TimeMachine,
    /// A RAID set of several disks.
    MultiDisk,
}

impl DriveKind {
    /// Every kind.
    pub const ALL: [DriveKind; 13] = [
        DriveKind::Startup,
        DriveKind::Internal,
        DriveKind::SolidState,
        DriveKind::External,
        DriveKind::Removable,
        DriveKind::Card,
        DriveKind::Optical,
        DriveKind::OpticalDrive,
        DriveKind::DiskImage,
        DriveKind::Network,
        DriveKind::Server,
        DriveKind::TimeMachine,
        DriveKind::MultiDisk,
    ];

    pub fn id(self) -> &'static str {
        match self {
            DriveKind::Startup => "startup",
            DriveKind::Internal => "internal",
            DriveKind::SolidState => "solid-state",
            DriveKind::External => "external",
            DriveKind::Removable => "removable",
            DriveKind::Card => "card",
            DriveKind::Optical => "optical",
            DriveKind::OpticalDrive => "optical-drive",
            DriveKind::DiskImage => "disk-image",
            DriveKind::Network => "network",
            DriveKind::Server => "server",
            DriveKind::TimeMachine => "time-machine",
            DriveKind::MultiDisk => "multi-disk",
        }
    }

    pub fn from_id(id: &str) -> Option<DriveKind> {
        DriveKind::ALL.into_iter().find(|k| k.id() == id)
    }
}

/// One drive shape FolderSkin draws: a kind, in a style that has a shape for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DriveShape {
    style: DriveStyle,
    kind: DriveKind,
}

impl DriveShape {
    /// The shape of `kind` in `style`, when `style` draws one.
    pub fn new(style: DriveStyle, kind: DriveKind) -> Option<DriveShape> {
        style
            .kinds()
            .contains(&kind)
            .then_some(DriveShape { style, kind })
    }

    /// The shape of `kind` in `style`, for the registry of shapes ([`crate::base`]), which lists
    /// only shapes a style draws. Its tests hold every entry to [`DriveShape::new`].
    pub(crate) const fn listed(style: DriveStyle, kind: DriveKind) -> DriveShape {
        DriveShape { style, kind }
    }

    pub fn style(self) -> DriveStyle {
        self.style
    }

    pub fn kind(self) -> DriveKind {
        self.kind
    }

    /// `mac-external`, `linux-solid-state`: the style's id and the kind's.
    pub fn id(self) -> String {
        format!("{}-{}", self.style.id(), self.kind.id())
    }

    /// The shape an [`DriveShape::id`] names, or `None` for anything else.
    pub fn from_id(id: &str) -> Option<DriveShape> {
        let (style, kind) = id.split_once('-')?;
        DriveShape::new(DriveStyle::from_id(style)?, DriveKind::from_id(kind)?)
    }

    /// Every shape FolderSkin draws, style by style, each style's in the order the composer
    /// lists them.
    pub fn all() -> Vec<DriveShape> {
        DriveStyle::ALL
            .into_iter()
            .flat_map(|style| {
                style
                    .kinds()
                    .iter()
                    .map(move |&kind| DriveShape { style, kind })
            })
            .collect()
    }

    /// The shape a drive of `kind` is drawn as in `style`: its own, or the nearest one the style
    /// has. Windows draws every fixed disk as a local disk, whatever it is and however it is
    /// connected, and Linux has no startup disk or Time Machine of its own, so each falls back to
    /// what its system shows for such a drive.
    pub fn for_kind(style: DriveStyle, kind: DriveKind) -> DriveShape {
        use DriveKind::*;
        let nearest = match (style, kind) {
            (_, kind) if style.kinds().contains(&kind) => kind,
            (DriveStyle::Mac, SolidState) => Internal,
            (DriveStyle::Mac, OpticalDrive) => Optical,
            (DriveStyle::Mac, Server) => Network,
            (DriveStyle::Mac, MultiDisk) => External,
            (DriveStyle::Windows, OpticalDrive) => Optical,
            (DriveStyle::Windows, Server) => Network,
            (DriveStyle::Windows, _) => Internal,
            (DriveStyle::Linux, Startup) => Internal,
            (DriveStyle::Linux, DiskImage) => Removable,
            (DriveStyle::Linux, TimeMachine) => External,
            (_, _) => Internal,
        };
        DriveShape {
            style,
            kind: nearest,
        }
    }

    /// The drive a drive skin is shown on when no drive is picked: `style`'s external drive,
    /// the one most people plug in.
    pub fn default_for(style: DriveStyle) -> DriveShape {
        DriveShape::for_kind(style, DriveKind::External)
    }

    /// The shape drawn, at `size` px square.
    fn draw(self, size: u32) -> Drawing {
        self.draw_into(Drawing::new(size))
    }

    /// Only the part `part` of the shape drawn, at `size` px square.
    fn draw_part(self, size: u32, part: &'static str) -> Drawing {
        self.draw_into(Drawing::only(size, part))
    }

    /// The shape gone through without painting anything: which parts it draws where, and its face.
    fn plan(self) -> Drawing {
        self.draw_into(Drawing::plan())
    }

    fn draw_into(self, mut d: Drawing) -> Drawing {
        match self.style {
            DriveStyle::Mac => mac::draw(self.kind, &mut d),
            DriveStyle::Windows => windows::draw(self.kind, &mut d),
            DriveStyle::Linux => linux::draw(self.kind, &mut d),
        }
        d
    }

    /// Where a picture is cover-fitted on this shape, in canvas units: the box around its face.
    pub fn face_box(self) -> Rect {
        self.draw(16).face_box()
    }

    /// A point in the face, in canvas units, that nothing over the face hides: where a design's
    /// new layer goes. The middle of the face's box, except on a disc, whose middle is its hole.
    pub fn face_point(self) -> (f32, f32) {
        self.draw(16).face_point()
    }

    /// The drive's whole extent in canvas units, as `[left, top, right, bottom]`, from its
    /// outlines.
    pub fn extent(self) -> [f32; 4] {
        self.draw(1024).extent()
    }

    /// The whole drive's width over its height: the shape of a frame it fills, as a folder's
    /// [`crate::compositor::Style::aspect`] is.
    pub fn aspect(self) -> f32 {
        let [x0, y0, x1, y1] = self.extent();
        (x1 - x0) / (y1 - y0)
    }

    /// The shape artwork is best made in for this drive, in pixels, so covering its face crops as
    /// little of it as it can: the face's own shape, its longer side
    /// [`crate::compositor::SKIN_WIDTH`]. A USB stick's label is tall, a Windows drive's top wide.
    pub fn artwork_size(self) -> (u32, u32) {
        let face = self.face_box();
        let long = crate::compositor::SKIN_WIDTH as f32;
        let (w, h) = (face.width(), face.height());
        if w >= h {
            (long as u32, (long * h / w).round() as u32)
        } else {
            ((long * w / h).round() as u32, long as u32)
        }
    }
}

/// The plain drive, the drive with `art` cover-fitted to its face, or the drive with a design
/// drawn on the whole canvas showing through its face, at `size` px, premultiplied.
fn render(shape: DriveShape, size: u32, fill: Fill<'_>) -> raster::Premul {
    let d = shape.draw(size);
    let mut pm = d.body.clone();
    let face = d.face();
    let art;
    let paint = match fill {
        Fill::Plain => None,
        Fill::Art(a) => {
            art = pattern_pixmap(&a.rgba);
            let box_ = d.face_box();
            let pl = fit::cover_fit(art.width(), art.height(), &box_, a.focus);
            let s = d.scale();
            Some(Paint {
                shader: Pattern::new(
                    art.as_ref(),
                    SpreadMode::Pad,
                    FilterQuality::Bicubic,
                    1.0,
                    Transform::from_row(pl.scale * s, 0.0, 0.0, pl.scale * s, pl.x * s, pl.y * s),
                ),
                anti_alias: true,
                ..Paint::default()
            })
        }
        Fill::Placed(design) => {
            let (w, h) = design.dimensions();
            if w == 0 || h == 0 {
                None
            } else {
                art = pattern_pixmap(design);
                Some(Paint {
                    shader: Pattern::new(
                        art.as_ref(),
                        SpreadMode::Pad,
                        FilterQuality::Bicubic,
                        1.0,
                        Transform::from_scale(size as f32 / w as f32, size as f32 / h as f32),
                    ),
                    anti_alias: true,
                    ..Paint::default()
                })
            }
        }
    };
    if let Some(paint) = paint {
        pm.fill_path(face, &paint, FillRule::Winding, Transform::identity(), None);
    }
    pm.draw_pixmap(
        0,
        0,
        d.over.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    raster::Premul {
        width: pm.width(),
        height: pm.height(),
        data: pm.take(),
    }
}

/// What goes on the drive's face.
enum Fill<'a> {
    /// Nothing: the drive as it is.
    Plain,
    /// Artwork, cover-fitted to the face.
    Art(&'a Artwork),
    /// A design on the whole icon canvas, showing through the face where it was drawn.
    Placed(&'a image::RgbaImage),
}

/// A picture as a premultiplied pixmap tiny-skia can use as a pattern. It must have pixels.
fn pattern_pixmap(rgba: &image::RgbaImage) -> Pixmap {
    let p = raster::straight_to_premul(rgba);
    let mut pm = Pixmap::new(p.width, p.height).expect("artwork size");
    pm.data_mut().copy_from_slice(&p.data);
    pm
}

/// The plain drive of `shape` at [`RENDER_SIZE`], premultiplied.
pub fn render_master_plain(shape: DriveShape) -> raster::Premul {
    render(shape, RENDER_SIZE, Fill::Plain)
}

/// The plain drive of `shape` as a picture `size` px square for a picker, drawn from a master of
/// [`crate::compositor::picture_master`] px rather than [`RENDER_SIZE`].
pub fn render_picture_plain(shape: DriveShape, size: u32) -> image::RgbaImage {
    let master = crate::compositor::picture_master(size);
    crate::compositor::to_picture(&render(shape, master, Fill::Plain), size)
}

/// `art` wrapped onto the drive of `shape` at [`RENDER_SIZE`]: cover-fitted to its face and cut to
/// it, premultiplied.
pub fn render_master(shape: DriveShape, art: &Artwork) -> raster::Premul {
    render(shape, RENDER_SIZE, Fill::Art(art))
}

/// A design drawn on the icon canvas, on the drive of `shape` at [`RENDER_SIZE`]: its pixels map
/// straight onto the canvas, so what was drawn at a point is at that point on the drive, and the
/// face shows it. A design that isn't square is stretched to fill the canvas; one with no pixels
/// leaves the drive plain.
pub fn render_master_placed(shape: DriveShape, design: &image::RgbaImage) -> raster::Premul {
    render(shape, RENDER_SIZE, Fill::Placed(design))
}

/// The drive of `shape` in the layers the composer stacks a design between, each straight-alpha
/// RGBA and `size` px square, in the shape the folder's take ([`TemplateLayers`]): `back` is
/// empty (a drive has one face), `middle` is the plain drive, `front` the face's coverage and
/// `top` what goes over the face. Stacked around a design they are [`render_master_placed`].
pub fn layers(shape: DriveShape, size: u32) -> TemplateLayers {
    let d = shape.draw(size);
    let mut face = tiny_skia::Mask::new(size, size).expect("mask size");
    face.fill_path(d.face(), FillRule::Winding, true, Transform::identity());
    let white = |alpha: &[u8]| {
        image::RgbaImage::from_raw(
            size,
            size,
            alpha.iter().flat_map(|&a| [255, 255, 255, a]).collect(),
        )
        .expect("one alpha value per pixel")
    };
    let straight = |pm: &Pixmap| {
        raster::to_straight_rgba(&raster::Premul {
            width: size,
            height: size,
            data: pm.data().to_vec(),
        })
    };
    let outline = d.outline();
    TemplateLayers {
        back: image::RgbaImage::from_pixel(size, size, image::Rgba([255, 255, 255, 0])),
        front: white(face.data()),
        middle: straight(&d.body),
        top: straight(&d.over),
        outline: white(
            &outline
                .pixels()
                .iter()
                .map(|p| p.alpha())
                .collect::<Vec<_>>(),
        ),
    }
}

/// The drive of `shape` taken apart for the composer at `size` px, in the order the composer
/// stacks the pieces: each part's own drawing under the face ([`Role::Paint`]), in the order the
/// drive draws them, then the face, where the design shows ([`Role::Surface`]), then what each
/// part draws over the face. Over the face, a part that's under it too has its light and shade
/// ([`Role::Light`]), and a part that's only over it (a badge, a disc drive's slab) its own
/// colours. Stacked back around a design they're [`render_master_placed`] (a test checks).
pub fn pieces(shape: DriveShape, size: u32) -> Vec<Piece> {
    let plan = shape.plan();
    let once = |parts: &[&'static str]| {
        let mut seen: Vec<&'static str> = Vec::new();
        for &p in parts {
            if !seen.contains(&p) {
                seen.push(p);
            }
        }
        seen
    };
    let (under, over) = (
        once(plan.parts_in(Layer::Body)),
        once(plan.parts_in(Layer::Over)),
    );
    let names = once(&[under.clone(), over.clone()].concat());
    // Each part drawn on its own, and cut down to its pixels at once.
    let mut cut = a_few_at_a_time(&names, |&part| {
        let d = shape.draw_part(size, part);
        let role = if under.contains(&part) {
            Role::Light
        } else {
            Role::Paint
        };
        (
            Piece::cut(part, Role::Paint, &d.body),
            Piece::cut(part, role, &d.over),
        )
    });
    let at = |part: &str| names.iter().position(|p| *p == part).expect("drawn");
    // The face at this size: the plan is drawn at one pixel.
    let mut face = tiny_skia::Mask::new(size, size).expect("mask size");
    let k = size as f32;
    let at_size = plan.face().clone().transform(Transform::from_scale(k, k));
    face.fill_path(
        &at_size.expect("a scale"),
        FillRule::Winding,
        true,
        Transform::identity(),
    );
    let mut out: Vec<Piece> = under.iter().filter_map(|p| cut[at(p)].0.take()).collect();
    out.extend(Piece::surface("face", &face));
    out.extend(over.iter().filter_map(|p| cut[at(p)].1.take()));
    out
}

/// `f` for every item, a few at a time on threads of their own, in the items' order. A part's
/// drawing holds two pictures the size of the icon, so no more than four are drawn at once.
fn a_few_at_a_time<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(4)
        .clamp(1, items.len().max(1));
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        mine.push((i, f(item)));
                    }
                    mine
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| {
                w.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor;

    #[test]
    fn every_shape_has_an_id_that_leads_back_to_it() {
        let all = DriveShape::all();
        assert_eq!(all.len(), 9 + 6 + 10);
        for shape in &all {
            assert_eq!(
                DriveShape::from_id(&shape.id()),
                Some(*shape),
                "{}",
                shape.id()
            );
        }
        assert_eq!(
            DriveShape::from_id("mac-solid-state"),
            None,
            "the Mac draws no SSD"
        );
        assert_eq!(DriveShape::from_id("linux-startup"), None);
        assert_eq!(DriveShape::from_id("mac"), None);
        assert_eq!(DriveShape::from_id("amiga-internal"), None);
        assert_eq!(
            DriveShape::from_id("linux-solid-state").map(|s| s.kind()),
            Some(DriveKind::SolidState),
            "a kind's id can have a dash in it"
        );
        for kind in DriveKind::ALL {
            assert_eq!(DriveKind::from_id(kind.id()), Some(kind));
        }
        for style in DriveStyle::ALL {
            assert_eq!(DriveStyle::from_id(style.id()), Some(style));
        }
    }

    #[test]
    fn every_kind_has_a_shape_on_every_system() {
        for style in DriveStyle::ALL {
            for kind in DriveKind::ALL {
                let shape = DriveShape::for_kind(style, kind);
                assert_eq!(shape.style(), style);
                assert!(style.kinds().contains(&shape.kind()), "{style:?} {kind:?}");
                if style.kinds().contains(&kind) {
                    assert_eq!(shape.kind(), kind, "its own shape when it has one");
                }
            }
        }
        // What each system draws for drives it has no shape of its own for.
        let on = |style, kind| DriveShape::for_kind(style, kind).kind();
        assert_eq!(
            on(DriveStyle::Windows, DriveKind::External),
            DriveKind::Internal
        );
        assert_eq!(
            on(DriveStyle::Windows, DriveKind::TimeMachine),
            DriveKind::Internal
        );
        assert_eq!(
            on(DriveStyle::Linux, DriveKind::Startup),
            DriveKind::Internal
        );
        assert_eq!(
            on(DriveStyle::Mac, DriveKind::SolidState),
            DriveKind::Internal
        );
        assert_eq!(
            on(DriveStyle::Mac, DriveKind::OpticalDrive),
            DriveKind::Optical
        );
        assert_eq!(
            DriveShape::default_for(DriveStyle::Linux).kind(),
            DriveKind::External
        );
    }

    fn solid(c: [u8; 4]) -> Artwork {
        Artwork {
            rgba: image::RgbaImage::from_pixel(64, 48, image::Rgba(c)),
            focus: (0.5, 0.5),
        }
    }

    /// Every shape sits inside the canvas with room to spare, fills as much of it as a drive of
    /// its kind can, and has a face of a useful size inside itself.
    #[test]
    fn every_shape_fills_the_canvas_and_has_a_face_inside_it() {
        for shape in DriveShape::all() {
            let [x0, y0, x1, y1] = shape.extent();
            let id = shape.id();
            assert!(
                x0 >= 16.0 && y0 >= 16.0 && x1 <= 1008.0 && y1 <= 1008.0,
                "{id}: {x0} {y0} {x1} {y1}"
            );
            // As big as its shape lets it be: its longer side most of the canvas, and even a stick
            // a third of it across.
            let (long, short) = ((x1 - x0).max(y1 - y0), (x1 - x0).min(y1 - y0));
            assert!(
                long >= 740.0 && short >= 300.0,
                "{id} is too small: {x0} {y0} {x1} {y1}"
            );
            let face = shape.face_box();
            assert!(
                face.x0 >= x0 - 0.5
                    && face.x1 <= x1 + 0.5
                    && face.y0 >= y0 - 0.5
                    && face.y1 <= y1 + 0.5,
                "{id}: its face {face:?} is outside the drive"
            );
            assert!(
                face.width() * face.height() >= 0.2 * (x1 - x0) * (y1 - y0),
                "{id}: its face is small: {face:?}"
            );
        }
    }

    /// A face covered in one flat colour shows that colour in its middle, and the drive around
    /// the face is the drive's own.
    #[test]
    fn artwork_lands_on_the_face() {
        let red = [220, 20, 40, 255];
        for shape in DriveShape::all() {
            let img = raster::to_straight_rgba(&render_master(shape, &solid(red)));
            let plain = raster::to_straight_rgba(&render_master_plain(shape));
            let face = shape.face_box();
            let s = RENDER_SIZE as f32 / crate::geometry::CANVAS;
            let (cx, cy) = shape.face_point();
            let px = img.get_pixel((cx * s) as u32, (cy * s) as u32).0;
            assert!(
                px[0] > 150 && px[1] < 90 && px[2] < 110,
                "{}: {px:?} in its face {face:?}",
                shape.id()
            );
            let changed = img
                .pixels()
                .zip(plain.pixels())
                .filter(|(a, b)| a != b)
                .count();
            assert!(changed > 0, "{}", shape.id());
        }
    }

    #[test]
    fn the_same_drive_makes_the_same_bytes() {
        let shape = DriveShape::from_id("windows-network").unwrap();
        let a = compositor::render_drive_preview_png(Some(&solid([10, 120, 200, 255])), 128, shape);
        let b = compositor::render_drive_preview_png(Some(&solid([10, 120, 200, 255])), 128, shape);
        assert_eq!(a, b);
    }

    /// Gradient design, as the folder's test uses: the composer's promise that the layers
    /// stacked around a design are the icon that's saved.
    fn gradient_design(size: u32) -> image::RgbaImage {
        let last = (size - 1) as f32;
        image::RgbaImage::from_fn(size, size, |x, y| {
            let (u, v) = (x as f32 / last, y as f32 / last);
            let mix = |a: f32, b: f32| (a + (b - a) * u).round() as u8;
            let alpha = if v < 0.3 {
                255
            } else if (0.45..0.55).contains(&v) {
                128
            } else if v > 0.8 && u < 0.5 {
                0
            } else {
                200
            };
            image::Rgba([mix(240.0, 20.0), mix(120.0, 180.0), mix(30.0, 170.0), alpha])
        })
    }

    #[test]
    fn the_layers_stacked_around_a_design_are_the_saved_drive() {
        // A few shapes of every style: a render at the master size of every shape would be slow
        // in a debug build, and they all go through the same layers.
        let size = 512;
        for id in [
            "mac-external",
            "mac-optical",
            "windows-network",
            "windows-card",
            "linux-removable",
            "linux-network",
        ] {
            let shape = DriveShape::from_id(id).unwrap();
            let design = gradient_design(size);
            let saved = render(shape, size, Fill::Placed(&design));
            let layers = layers(shape, size);
            let unit = |v: u8| v as f32 / 255.0;
            let premul = |img: &image::RgbaImage| raster::straight_to_premul(img).data;
            let (design_p, middle, top) =
                (premul(&design), premul(&layers.middle), premul(&layers.top));
            let over = |dst: [f32; 4], src: [f32; 4]| -> [f32; 4] {
                std::array::from_fn(|c| src[c] + dst[c] * (1.0 - src[3]))
            };
            let mut worst = 0;
            for (i, want) in saved.data.as_chunks::<4>().0.iter().enumerate() {
                let px = |buf: &[u8]| -> [f32; 4] { std::array::from_fn(|c| unit(buf[i * 4 + c])) };
                let face = unit(layers.front.as_raw()[i * 4 + 3]);
                let mut stack = px(&middle);
                stack = over(stack, px(&design_p).map(|c| c * face));
                stack = over(stack, px(&top));
                for (c, &want) in want.iter().enumerate() {
                    worst = worst.max(((stack[c] * 255.0).round() as i32 - want as i32).abs());
                }
            }
            assert!(
                worst <= 3,
                "{id}: the stacked layers are {worst} off the saved drive"
            );
        }
    }

    #[test]
    fn every_drive_comes_apart_into_parts_that_stack_back_into_it() {
        let size = 256;
        for shape in DriveShape::all() {
            let id = shape.id();
            let d = shape.draw(size);
            for layer in [Layer::Body, Layer::Over] {
                let parts = d.parts_in(layer);
                for (i, part) in parts.iter().enumerate() {
                    assert!(
                        !parts[..i].contains(part),
                        "{id}: another part is drawn in the middle of {part}'s drawing in \
                         {layer:?} ({parts:?}), so {part} can't be drawn on its own"
                    );
                }
            }
            let design = gradient_design(size);
            let saved = render(shape, size, Fill::Placed(&design));
            let pieces = pieces(shape, size);
            let design = raster::straight_to_premul(&design).data;
            let worst = compositor::worst_off(
                &compositor::stack_pieces(&pieces, size, &design),
                &saved.data,
            );
            assert!(
                worst <= 3,
                "{id}: the stacked pieces are {worst} off the saved drive"
            );
            let is =
                |part: &str, role: Role| pieces.iter().any(|p| p.part == part && p.role == role);
            assert!(is("face", Role::Surface), "{id} has a face for the design");
            assert!(
                is("face", Role::Paint),
                "{id}'s face has colours of its own"
            );
        }
        // The USB stick: its port, the two holes in it, the blue case and the label.
        let stick = pieces(DriveShape::from_id("linux-removable").unwrap(), size);
        let under: Vec<&str> = stick
            .iter()
            .take_while(|p| p.role != Role::Surface)
            .map(|p| p.part)
            .collect();
        assert_eq!(under, ["port", "holes", "case", "face"]);
    }

    #[test]
    fn a_drive_has_no_pixels_outside_its_extent_and_no_shadow() {
        for shape in DriveShape::all() {
            let img = raster::to_straight_rgba(&render(shape, 1024, Fill::Plain));
            let [x0, y0, x1, y1] = shape.extent();
            for (x, y, p) in img.enumerate_pixels() {
                let (x, y) = (x as f32, y as f32);
                let outside = x < x0 - 3.0 || x > x1 + 3.0 || y < y0 - 3.0 || y > y1 + 3.0;
                assert!(
                    !outside || p.0[3] == 0,
                    "{} has a pixel at ({x},{y}), outside {x0} {y0} {x1} {y1}",
                    shape.id()
                );
            }
        }
    }
}
