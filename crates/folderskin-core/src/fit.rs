//! Cover-fit maths: how a skin image is scaled and offset to fill a template rectangle.
//!
//! "Cover" means the image is scaled up until it covers the whole rectangle, then cropped. The
//! `focus` point (0..1 in image space) decides which part survives the crop: (0.5, 0.5) centres
//! it, (0.0, 0.0) keeps the top-left corner.

use crate::geometry::Rect;

/// Where to draw an image so that it covers a target rectangle.
///
/// The image is drawn scaled by `scale` with its top-left corner at (`x`, `y`), in the same
/// units as the target rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
}

/// Scale and offset that make `img_w`×`img_h` cover `target`, cropping around `focus`.
///
/// Degenerate (zero-sized) image dimensions are treated as 1 px so the result stays finite.
pub fn cover_fit(img_w: u32, img_h: u32, target: &Rect, focus: (f32, f32)) -> Placement {
    let (iw, ih) = (img_w.max(1) as f32, img_h.max(1) as f32);
    let (w, h) = (target.width(), target.height());
    let scale = (w / iw).max(h / ih);
    Placement {
        scale,
        x: target.x0 - (iw * scale - w) * focus.0,
        y: target.y0 - (ih * scale - h) * focus.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry;

    #[test]
    fn cover_fit_centres_a_wide_image_horizontally() {
        let r = geometry::Rect::new(0.0, 0.0, 100.0, 100.0);
        let p = cover_fit(200, 100, &r, (0.5, 0.5));
        assert!((p.scale - 1.0).abs() < 1e-6 && (p.x + 50.0).abs() < 1e-4 && p.y.abs() < 1e-4);
    }

    #[test]
    fn cover_fit_focus_zero_keeps_top_left() {
        let r = geometry::Rect::new(10.0, 20.0, 110.0, 70.0); // 100×50
        let p = cover_fit(100, 100, &r, (0.0, 0.0));
        assert!(
            (p.scale - 1.0).abs() < 1e-6 && (p.x - 10.0).abs() < 1e-4 && (p.y - 20.0).abs() < 1e-4
        );
    }

    #[test]
    fn cover_fit_focus_one_keeps_bottom_right() {
        let r = geometry::Rect::new(0.0, 0.0, 100.0, 50.0);
        let p = cover_fit(100, 100, &r, (1.0, 1.0));
        assert!((p.scale - 1.0).abs() < 1e-6);
        assert!((p.x - 0.0).abs() < 1e-4 && (p.y + 50.0).abs() < 1e-4);
    }

    #[test]
    fn cover_fit_always_covers_the_target() {
        let r = geometry::Rect::new(15.0, 160.5, 1009.0, 973.5);
        for (w, h) in [(1024, 958), (100, 4000), (4000, 100), (1, 1)] {
            let p = cover_fit(w, h, &r, (0.5, 0.5));
            assert!(p.x <= r.x0 + 1e-3 && p.y <= r.y0 + 1e-3, "{w}×{h}");
            assert!(p.x + w as f32 * p.scale >= r.x1 - 1e-3, "{w}×{h}");
            assert!(p.y + h as f32 * p.scale >= r.y1 - 1e-3, "{w}×{h}");
        }
    }

    /// A 1024×958 skin (the authoring size) shows its top in the tab and its middle in the
    /// front panel — the two panels share one focus point but different rectangles.
    #[test]
    fn skin_sized_artwork_shows_its_top_in_the_tab_and_its_middle_in_the_front() {
        let back = cover_fit(1024, 958, &geometry::BACK_BBOX, (0.5, 0.5));
        assert!((back.scale - 0.9781).abs() < 1e-3, "{}", back.scale);
        // The back bbox has the artwork's aspect, so no vertical crop: row 0 is the tab's top.
        assert!((back.y - geometry::BACK_BBOX.y0).abs() < 0.2);
        assert!(back.x < geometry::BACK_BBOX.x0);

        let front = cover_fit(1024, 958, &geometry::FRONT, (0.5, 0.5));
        assert!((front.scale - 0.9707).abs() < 1e-3, "{}", front.scale);
        // The front is shorter, so the crop eats top and bottom: it shows the middle.
        assert!((front.x - geometry::FRONT.x0).abs() < 0.2);
        assert!(front.y < geometry::FRONT.y0 - 40.0);
    }
}
