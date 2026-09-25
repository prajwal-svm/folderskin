//! One shape for the finished folders in a pack.
//!
//! A finished folder is a picture of a whole folder on transparency, which the app uses as it is
//! drawn ([`crate::matte::finished_cutout`]). Pictures made one at a time come out cropped tight to
//! their own folder, and no two folders an image model paints have quite the same proportions: one
//! pack's ran from 1.03 to 1.30 times as wide as they were tall. The app fits each picture whole
//! into the square icon ([`crate::compositor::icon_set_from_image`]), so in Finder a squat folder
//! came out smaller than a tall one beside it.
//!
//! So the folders in a pack share one shape. The pack's shape is the median of its folders' own
//! ([`pack_shape`]), and each folder is redrawn at exactly that shape, as wide as FolderSkin's own
//! folder and standing on its baseline ([`frame`], [`target`]), in a [`CANVAS`] px square
//! ([`redraw`]). A folder that would need more than [`TOLERANCE`] reshaping to match is an
//! outlier ([`is_outlier`]): stretched that far it would look wrong, so the tools report it and
//! leave it for a person to decide about.
//!
//! A folder's box is the bounding box of its pixels more than half opaque ([`folder_box`]), which
//! a soft edge or a faint shadow doesn't move.

use crate::{compositor, matte, raster};
use image::RgbaImage;

/// The side of the square a folder is redrawn in, in pixels.
pub const CANVAS: u32 = 1024;

/// How much a folder may be reshaped to take its pack's shape: 8%, as width ÷ height goes.
/// Nobody sees that much; much more, and lettering and faces look squashed.
pub const TOLERANCE: f32 = 0.08;

/// How far apart the folders of a pack of one shape may measure, as width ÷ height goes: 1%.
/// Folders redrawn at one shape measure well within it, since only the pixel their edges round
/// to differs.
pub const ONE_SHAPE: f32 = 0.01;

/// Alpha above which a pixel is part of a folder's box: more than half opaque.
pub const SOLID: u8 = 127;

/// How far a folder already redrawn at a shape may measure from it, as width ÷ height goes: half
/// of [`ONE_SHAPE`], so two folders left as they are because of it are still one shape together.
const REDRAWN: f32 = ONE_SHAPE / 2.0;

/// A box of pixels: columns `x0..x1` and rows `y0..y1`, the far edges left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelBox {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl PixelBox {
    pub fn width(&self) -> u32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> u32 {
        self.y1 - self.y0
    }

    /// Width ÷ height.
    pub fn aspect(&self) -> f32 {
        self.width() as f32 / self.height().max(1) as f32
    }
}

/// FolderSkin's own folder in the [`CANVAS`] square: the box its silhouette fills in the blank
/// template, the mask `folderskin-tools template --mask` writes, taken from the template itself
/// ([`compositor::blank_template_folder_box`]). Columns 31 to 993 and rows 58 to 966. A redrawn
/// folder is as wide as this one, starts at its left edge and stands on its bottom edge, the
/// baseline.
pub fn frame() -> PixelBox {
    let [x0, y0, x1, y1] = compositor::blank_template_folder_box(CANVAS, CANVAS);
    PixelBox { x0, y0, x1, y1 }
}

/// The box of `img`'s pixels whose alpha is above `above`, or `None` when it has none.
fn bounds_above(img: &RgbaImage, above: u8) -> Option<PixelBox> {
    let (x0, y0, x1, y1) = matte::alpha_bounds(img, above.checked_add(1)?)?;
    Some(PixelBox {
        x0,
        y0,
        x1: x1 + 1,
        y1: y1 + 1,
    })
}

/// A folder picture's box: its pixels more than half opaque. `None` when it has none.
pub fn folder_box(img: &RgbaImage) -> Option<PixelBox> {
    bounds_above(img, SOLID)
}

/// A folder picture's shape, its box's width ÷ height. `None` when it has no folder.
pub fn aspect(img: &RgbaImage) -> Option<f32> {
    folder_box(img).map(|b| b.aspect())
}

/// A pack's shape: the median of its folders' shapes, the mean of the middle two when there are
/// as many above as below. `None` for no folders.
pub fn pack_shape(aspects: &[f32]) -> Option<f32> {
    let mut sorted: Vec<f32> = aspects.to_vec();
    sorted.sort_by(f32::total_cmp);
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
    }
}

/// How much a folder of `aspect` is reshaped to take `shape`: `aspect / shape − 1`, above 0 for a
/// folder wider for its height than the pack's, below for a narrower one.
pub fn reshaping(aspect: f32, shape: f32) -> f32 {
    aspect / shape - 1.0
}

/// Whether a folder of `aspect` needs more than `tolerance` reshaping to take `shape`.
pub fn is_outlier(aspect: f32, shape: f32, tolerance: f32) -> bool {
    reshaping(aspect, shape).abs() > tolerance
}

/// How far apart folders of these shapes are: the widest for its height over the narrowest, less
/// one. 0 for fewer than two.
pub fn spread(aspects: &[f32]) -> f32 {
    let widest = aspects.iter().copied().fold(f32::MIN, f32::max);
    let narrowest = aspects.iter().copied().fold(f32::MAX, f32::min);
    if aspects.len() < 2 || narrowest <= 0.0 {
        return 0.0;
    }
    widest / narrowest - 1.0
}

/// Whether folders of these shapes are one: no two more than [`ONE_SHAPE`] apart.
pub fn is_one_shape(aspects: &[f32]) -> bool {
    spread(aspects) <= ONE_SHAPE
}

/// What taking one shape does to a pack's folders.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The pack's shape ([`pack_shape`]).
    pub shape: f32,
    /// Each folder's [`reshaping`], in the order the shapes were given.
    pub reshaping: Vec<f32>,
    /// The folders more than the tolerance off the shape, by their place in that order.
    pub outliers: Vec<usize>,
}

impl Plan {
    /// Whether folder `i` is an outlier.
    pub fn is_outlier(&self, i: usize) -> bool {
        self.outliers.contains(&i)
    }
}

/// The plan for a pack whose finished folders have `aspects`: its shape, and the folders more
/// than `tolerance` off it. `None` for fewer than two folders, which are left as they are.
pub fn plan(aspects: &[f32], tolerance: f32) -> Option<Plan> {
    if aspects.len() < 2 {
        return None;
    }
    let shape = pack_shape(aspects)?;
    let reshaping: Vec<f32> = aspects.iter().map(|&a| reshaping(a, shape)).collect();
    let outliers = (0..aspects.len())
        .filter(|&i| is_outlier(aspects[i], shape, tolerance))
        .collect();
    Some(Plan {
        shape,
        reshaping,
        outliers,
    })
}

/// Where a folder of `shape` goes in the [`CANVAS`] square: as wide as the [`frame`], from its
/// left edge, `round(width / shape)` tall, standing on its bottom edge. A folder too tall for its
/// width to fit above that edge is as tall as the space there instead, and centred over the frame.
pub fn target(shape: f32) -> PixelBox {
    let frame = frame();
    let (width, baseline) = (frame.width(), frame.y1);
    let height = (width as f32 / shape).round();
    if height.is_finite() && height <= baseline as f32 {
        let height = height.max(1.0) as u32;
        return PixelBox {
            x0: frame.x0,
            y0: baseline - height,
            x1: frame.x1,
            y1: baseline,
        };
    }
    // Taller than wide: the whole height above the baseline, and the width that shape gives it.
    let narrow = ((baseline as f32 * shape).round() as u32).clamp(1, width);
    let x0 = frame.x0 + (width - narrow) / 2;
    PixelBox {
        x0,
        y0: 0,
        x1: x0 + narrow,
        y1: baseline,
    }
}

/// `img`'s folder redrawn at `shape`: cropped to its box ([`folder_box`]), resampled with
/// Lanczos3 to exactly the size of [`target`], on premultiplied channels so the transparency
/// around it doesn't darken its edge, and put there in a transparent [`CANVAS`] px square. `None`
/// when it has no folder.
pub fn redraw(img: &RgbaImage, shape: f32) -> Option<RgbaImage> {
    let from = folder_box(img)?;
    let to = target(shape);
    let folder = image::imageops::crop_imm(img, from.x0, from.y0, from.width(), from.height());
    let drawn = raster::resize(&folder.to_image(), to.width(), to.height());
    let mut canvas = RgbaImage::new(CANVAS, CANVAS);
    image::imageops::replace(&mut canvas, &drawn, i64::from(to.x0), i64::from(to.y0));
    Some(canvas)
}

/// Where a picture's folder is, measured once, so whether it has been redrawn can be told without
/// its pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    /// The picture's own width and height.
    pub size: (u32, u32),
    /// Its folder: the pixels more than half opaque ([`folder_box`]).
    pub folder: PixelBox,
    /// Every pixel that shows at all.
    pub visible: PixelBox,
}

impl Placed {
    /// Measures `img`. `None` when no pixel of it is more than half opaque.
    pub fn of(img: &RgbaImage) -> Option<Placed> {
        Some(Placed {
            size: img.dimensions(),
            folder: folder_box(img)?,
            visible: bounds_above(img, 0)?,
        })
    }

    /// Whether this is a folder [`redraw`] drew at `shape` already, so drawing it again would
    /// only resample it. It is when the picture is square, everything in it that shows lies where
    /// [`target`] puts a folder of that shape, give or take the pixel an edge rounds to, and its
    /// own box is that shape within half of [`ONE_SHAPE`].
    ///
    /// A square smaller than [`CANVAS`], which is how a pack's picture too detailed for its size
    /// limit is stored ([`crate::pack::encode_picture`]), is measured against the target scaled
    /// to it, give or take the few pixels the resampling that made it smaller spreads its edge by.
    pub fn is_redrawn(&self, shape: f32) -> bool {
        let (w, h) = self.size;
        if w != h || w == 0 || w > CANVAS {
            return false;
        }
        let scale = w as f32 / CANVAS as f32;
        // Lanczos3 reaches three pixels past an edge when it makes a picture smaller.
        let slack = if w == CANVAS { 1.0 } else { 4.0 };
        let want = target(shape);
        let near = |got: u32, want: u32| (got as f32 - want as f32 * scale).abs() <= slack;
        let seen = self.visible;
        near(seen.x0, want.x0)
            && near(seen.y0, want.y0)
            && near(seen.x1, want.x1)
            && near(seen.y1, want.y1)
            && reshaping(self.folder.aspect(), shape).abs() <= REDRAWN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A folder-ish subject `w`×`h` px on transparency, `margin` px in from every side: a
    /// rounded-looking body with a soft one-pixel rim at alpha 100, red on its left half and blue
    /// on its right, so where it lands shows.
    fn folder(w: u32, h: u32, margin: u32) -> RgbaImage {
        let (fw, fh) = (w + 2 * margin, h + 2 * margin);
        RgbaImage::from_fn(fw, fh, |x, y| {
            let inside = (margin..margin + w).contains(&x) && (margin..margin + h).contains(&y);
            let rim = x + 1 == margin || x == margin + w || y + 1 == margin || y == margin + h;
            let colour = if x < margin + w / 2 {
                [200, 30, 40]
            } else {
                [30, 60, 200]
            };
            match (inside, rim) {
                (true, _) => Rgba([colour[0], colour[1], colour[2], 255]),
                (false, true) => Rgba([colour[0], colour[1], colour[2], 100]),
                _ => Rgba([0, 0, 0, 0]),
            }
        })
    }

    #[test]
    fn the_frame_is_folderskins_own_folder_in_the_template() {
        // The numbers `folderskin-tools template --mask` measures at 1024: 962 px wide, on 966.
        assert_eq!(
            frame(),
            PixelBox {
                x0: 31,
                y0: 58,
                x1: 993,
                y1: 966
            }
        );
        assert_eq!(frame().width(), 962);
    }

    #[test]
    fn a_folders_box_is_its_part_more_than_half_opaque() {
        let img = folder(300, 200, 20);
        // The rim at alpha 100 is left out; the body is 300 × 200 from (20, 20).
        assert_eq!(
            folder_box(&img),
            Some(PixelBox {
                x0: 20,
                y0: 20,
                x1: 320,
                y1: 220
            })
        );
        assert_eq!(aspect(&img), Some(1.5));
        // Alpha 127 is half, not more; 128 is.
        let edge = |a: u8| RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, a]));
        assert_eq!(folder_box(&edge(127)), None);
        assert_eq!(folder_box(&edge(128)).map(|b| b.width()), Some(4));
        assert_eq!(aspect(&RgbaImage::new(8, 8)), None);
    }

    #[test]
    fn a_packs_shape_is_the_median_of_its_folders() {
        assert_eq!(pack_shape(&[]), None);
        assert_eq!(pack_shape(&[1.2]), Some(1.2));
        assert_eq!(pack_shape(&[1.3, 1.03, 1.1]), Some(1.1), "in any order");
        let even = pack_shape(&[1.0, 1.4, 1.1, 1.2]).unwrap();
        assert!((even - 1.15).abs() < 1e-6, "the middle two: {even}");
        // One odd folder doesn't move it far, as a mean would.
        assert_eq!(pack_shape(&[1.15, 1.16, 1.17, 3.0, 1.18]), Some(1.17));
    }

    #[test]
    fn an_outlier_needs_more_than_the_tolerance() {
        assert!((reshaping(1.2, 1.0) - 0.2).abs() < 1e-6);
        assert!(reshaping(0.9, 1.0) < 0.0, "narrower is below 0");
        assert!(!is_outlier(1.079, 1.0, TOLERANCE));
        assert!(!is_outlier(0.921, 1.0, TOLERANCE));
        assert!(is_outlier(1.081, 1.0, TOLERANCE));
        assert!(is_outlier(0.919, 1.0, TOLERANCE));
        assert!(!is_outlier(1.3, 1.0, 0.5), "a wider tolerance takes it");

        // The median is 1.16: 1.40 is 21% wider for its height and 1.02 12% narrower.
        let plan = plan(&[1.10, 1.16, 1.40, 1.17, 1.02], TOLERANCE).unwrap();
        assert_eq!(plan.shape, 1.16);
        assert_eq!(plan.outliers, [2, 4]);
        assert!(plan.is_outlier(4) && !plan.is_outlier(0));
        assert_eq!(plan.reshaping.len(), 5);
        assert!((plan.reshaping[2] - 0.2069).abs() < 1e-3);
        assert!((plan.reshaping[4] + 0.1207).abs() < 1e-3);
        // 1.07 is 7.8% narrower, within 8%.
        let plan = super::plan(&[1.10, 1.16, 1.40, 1.17, 1.07], TOLERANCE).unwrap();
        assert_eq!(plan.outliers, [2]);
        assert_eq!(
            super::plan(&[1.2], TOLERANCE),
            None,
            "one folder is left alone"
        );
        assert_eq!(super::plan(&[], TOLERANCE), None);
    }

    #[test]
    fn folders_within_one_percent_are_one_shape() {
        assert_eq!(spread(&[]), 0.0);
        assert_eq!(spread(&[1.3]), 0.0);
        assert!(is_one_shape(&[1.170, 1.172, 1.168]));
        assert!(!is_one_shape(&[1.17, 1.19]));
        assert!((spread(&[1.3, 1.0, 1.1]) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_folder_goes_as_wide_as_folderskins_own_on_its_baseline() {
        let t = target(1.2);
        // 962 / 1.2 = 801.67, so 802 tall, standing on row 966.
        assert_eq!(
            t,
            PixelBox {
                x0: 31,
                y0: 164,
                x1: 993,
                y1: 966
            }
        );
        assert_eq!(target(1.0).height(), 962);
        // A folder taller than it is wide fits the height above the baseline, centred.
        let tall = target(0.8);
        assert_eq!((tall.y0, tall.y1, tall.height()), (0, 966, 966));
        assert_eq!(tall.width(), 773, "966 × 0.8");
        assert!(
            (tall.x0 - 31).abs_diff(993 - tall.x1) <= 1,
            "centred: {tall:?}"
        );
        // Nothing absurd makes an empty or outsized box.
        for shape in [0.0, -1.0, f32::NAN, f32::INFINITY, 1e9] {
            let t = target(shape);
            assert!(t.width() >= 1 && t.height() >= 1, "{shape}: {t:?}");
            assert!(t.x1 <= CANVAS && t.y1 <= CANVAS, "{shape}: {t:?}");
        }
    }

    #[test]
    fn a_redrawn_folder_takes_the_packs_shape_in_folderskins_place() {
        // 1.5 wide for its height, redrawn at 1.2: narrower for its height, as wide as the frame.
        let img = folder(600, 400, 30);
        let canvas = redraw(&img, 1.2).unwrap();
        assert_eq!(canvas.dimensions(), (CANVAS, CANVAS));
        let want = target(1.2);
        let placed = Placed::of(&canvas).unwrap();
        assert_eq!(placed.visible, want, "nothing outside the target");
        assert_eq!(placed.folder, want, "and all of it solid");
        assert!((placed.folder.aspect() - 1.2).abs() < 0.002);
        // Red on the left, blue on the right, as it was, and transparent around it.
        let px = |x: u32, y: u32| canvas.get_pixel(x, y).0;
        assert_eq!(px(want.x0 + 5, 500), [200, 30, 40, 255]);
        assert_eq!(px(want.x1 - 5, 500), [30, 60, 200, 255]);
        for (x, y) in [(10, 500), (1010, 500), (512, want.y0 - 2), (512, 1000)] {
            assert_eq!(px(x, y)[3], 0, "({x},{y})");
        }
        assert_eq!(redraw(&RgbaImage::new(64, 64), 1.2), None, "no folder");
    }

    #[test]
    fn a_redrawn_folder_is_known_as_one_and_nothing_else_is() {
        let canvas = redraw(&folder(600, 400, 30), 1.2).unwrap();
        let placed = Placed::of(&canvas).unwrap();
        assert!(placed.is_redrawn(1.2));
        // The median of folders measured a pixel apart still counts as its shape.
        assert!(placed.is_redrawn(962.0 / 801.0));
        assert!(!placed.is_redrawn(1.25), "another shape");
        let original = Placed::of(&folder(600, 400, 30)).unwrap();
        assert!(!original.is_redrawn(1.5), "not in the canvas");
        // Moved three pixels to the right.
        let mut moved = RgbaImage::new(CANVAS, CANVAS);
        image::imageops::replace(&mut moved, &canvas, 3, 0);
        assert!(!Placed::of(&moved).unwrap().is_redrawn(1.2));
        // A faint shadow below it counts too: the app trims to what shows.
        let mut shadowed = canvas.clone();
        shadowed.put_pixel(512, 1000, Rgba([0, 0, 0, 20]));
        assert!(!Placed::of(&shadowed).unwrap().is_redrawn(1.2));
        // Made 896 px to fit a pack's size limit, it is still in its place.
        let smaller = raster::shrink_to(canvas.clone(), 896);
        assert!(Placed::of(&smaller).unwrap().is_redrawn(1.2));
        assert!(!Placed::of(&smaller).unwrap().is_redrawn(1.3));
    }

    #[test]
    fn redrawing_a_redrawn_folder_at_its_shape_gives_it_back() {
        let canvas = redraw(&folder(600, 400, 30), 1.2).unwrap();
        // Its box is the target already, so nothing is resampled.
        assert_eq!(redraw(&canvas, 1.2).unwrap(), canvas);
    }
}
