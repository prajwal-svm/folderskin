//! The Windows folder template, in the same 1024×1024 canvas as [`crate::geometry`].
//!
//! Measured from the folder Windows 11 draws (its 256 px stock icon, ×4): a back panel with a
//! tab at its top left and a front panel that covers most of it. Unlike FolderSkin's own folder
//! there is no paper sheet between them, and the front's top edge steps down under the tab
//! instead of running straight across: the two panels meet in a gentle S on either side of it.

use crate::geometry::{arc, Rect};
use tiny_skia::{Path, PathBuilder, Transform};

/// Left and right edges of both panels.
pub const LEFT: f32 = 64.0;
pub const RIGHT: f32 = 960.0;
/// Bottom edge of both panels.
pub const BOTTOM: f32 = 840.0;

/// Top edge of the tab, and where its top edge runs from and to.
pub const TAB_TOP: f32 = 136.0;
/// Where the tab's top edge starts down its S into the body's top edge.
pub const TAB_STEP_X: f32 = 356.0;
/// Top edge of the back panel's body, right of the tab.
pub const BODY_TOP: f32 = 232.0;
/// Where the tab's S meets the body's top edge.
pub const BODY_STEP_X: f32 = 464.0;

/// The front panel's top edge: lower under the tab, higher to its right.
pub const FRONT_TOP_LEFT: f32 = 296.0;
pub const FRONT_TOP: f32 = 248.0;
/// Where the front's top edge starts up its S, and where it's at the top.
pub const FRONT_STEP_X0: f32 = 376.0;
pub const FRONT_STEP_X1: f32 = 464.0;

/// Corner radii: the tab's top left, the body's and front's top right, the front's top left and
/// the bottom corners they share.
pub const TAB_RADIUS: f32 = 40.0;
pub const CORNER: f32 = 36.0;

/// Bounding box of the back panel, tab included; artwork is cover-fitted to it.
pub const BACK_BBOX: Rect = Rect::new(LEFT, TAB_TOP, RIGHT, BOTTOM);
/// Bounding box of the front panel; artwork is cover-fitted to it too, so the two panels line up.
pub const FRONT: Rect = Rect::new(LEFT, FRONT_TOP, RIGHT, BOTTOM);

/// An S from `(x0, y0)` to `(x1, y1)` that leaves and arrives level: how both the tab and the
/// front's top edge step between their two heights.
fn step(pb: &mut PathBuilder, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mid = (x0 + x1) / 2.0;
    pb.line_to(x0, y0);
    pb.cubic_to(mid, y0, mid, y1, x1, y1);
}

fn scaled(pb: PathBuilder, scale: f32) -> Path {
    pb.finish()
        .expect("non-empty path")
        .transform(Transform::from_scale(scale, scale))
        .expect("valid transform")
}

/// The back panel, tab included, clockwise from the tab's top-left corner.
pub fn back_panel_path(scale: f32) -> Path {
    let mut pb = PathBuilder::new();
    arc(
        &mut pb,
        LEFT + TAB_RADIUS,
        TAB_TOP + TAB_RADIUS,
        TAB_RADIUS,
        180.0,
        270.0,
    );
    step(&mut pb, TAB_STEP_X, TAB_TOP, BODY_STEP_X, BODY_TOP);
    arc(
        &mut pb,
        RIGHT - CORNER,
        BODY_TOP + CORNER,
        CORNER,
        270.0,
        360.0,
    );
    arc(&mut pb, RIGHT - CORNER, BOTTOM - CORNER, CORNER, 0.0, 90.0);
    arc(&mut pb, LEFT + CORNER, BOTTOM - CORNER, CORNER, 90.0, 180.0);
    pb.close();
    scaled(pb, scale)
}

/// The front panel, clockwise from its top-left corner.
pub fn front_panel_path(scale: f32) -> Path {
    let mut pb = PathBuilder::new();
    arc(
        &mut pb,
        LEFT + CORNER,
        FRONT_TOP_LEFT + CORNER,
        CORNER,
        180.0,
        270.0,
    );
    step(
        &mut pb,
        FRONT_STEP_X0,
        FRONT_TOP_LEFT,
        FRONT_STEP_X1,
        FRONT_TOP,
    );
    arc(
        &mut pb,
        RIGHT - CORNER,
        FRONT_TOP + CORNER,
        CORNER,
        270.0,
        360.0,
    );
    arc(&mut pb, RIGHT - CORNER, BOTTOM - CORNER, CORNER, 0.0, 90.0);
    arc(&mut pb, LEFT + CORNER, BOTTOM - CORNER, CORNER, 90.0, 180.0);
    pb.close();
    scaled(pb, scale)
}

/// Open path along the front's top edge, S and both top corners included: its highlight, and the
/// shadow it casts on the back panel.
pub fn front_top_edge_path(scale: f32) -> Path {
    let mut pb = PathBuilder::new();
    pb.move_to(LEFT, FRONT_TOP_LEFT + CORNER);
    arc(
        &mut pb,
        LEFT + CORNER,
        FRONT_TOP_LEFT + CORNER,
        CORNER,
        180.0,
        270.0,
    );
    step(
        &mut pb,
        FRONT_STEP_X0,
        FRONT_TOP_LEFT,
        FRONT_STEP_X1,
        FRONT_TOP,
    );
    arc(
        &mut pb,
        RIGHT - CORNER,
        FRONT_TOP + CORNER,
        CORNER,
        270.0,
        360.0,
    );
    scaled(pb, scale)
}

/// Open path along the front's bottom edge and its two bottom corners, for its darker lip.
pub fn front_bottom_path(scale: f32) -> Path {
    let mut pb = PathBuilder::new();
    arc(&mut pb, RIGHT - CORNER, BOTTOM - CORNER, CORNER, 0.0, 90.0);
    arc(&mut pb, LEFT + CORNER, BOTTOM - CORNER, CORNER, 90.0, 180.0);
    scaled(pb, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn within(p: &Path, r: Rect) -> bool {
        let t = p.compute_tight_bounds().unwrap();
        t.left() >= r.x0 - 0.01
            && t.right() <= r.x1 + 0.01
            && t.top() >= r.y0 - 0.01
            && t.bottom() <= r.y1 + 0.01
    }

    #[test]
    fn panels_fill_their_boxes_and_no_further() {
        for (path, r) in [
            (back_panel_path(1.0), BACK_BBOX),
            (front_panel_path(1.0), FRONT),
        ] {
            let b = path.bounds();
            assert!(
                (b.left() - r.x0).abs() < 0.01 && (b.right() - r.x1).abs() < 0.01,
                "{b:?}"
            );
            assert!(
                (b.top() - r.y0).abs() < 0.01 && (b.bottom() - r.y1).abs() < 0.01,
                "{b:?}"
            );
            assert!(within(&path, r));
        }
    }

    #[test]
    fn match_the_measured_windows_folder() {
        // The stock 256 px icon's silhouette: x 16..239, y 34..209; its front's top 62, 74 under the tab.
        assert_eq!((LEFT / 4.0, RIGHT / 4.0 - 1.0), (16.0, 239.0));
        assert_eq!((TAB_TOP / 4.0, BOTTOM / 4.0 - 1.0), (34.0, 209.0));
        assert_eq!((FRONT_TOP / 4.0, FRONT_TOP_LEFT / 4.0), (62.0, 74.0));
    }

    #[test]
    fn the_front_steps_down_under_the_tab() {
        let front = front_panel_path(1.0);
        let mut left = tiny_skia::Mask::new(1024, 1024).unwrap();
        left.fill_path(
            &front,
            tiny_skia::FillRule::Winding,
            false,
            Transform::identity(),
        );
        let at = |x: u32, y: u32| left.data()[(y * 1024 + x) as usize];
        // Under the tab the front starts lower than it does to the right.
        assert_eq!(at(200, 270), 0);
        assert!(at(200, 310) > 0);
        assert!(at(700, 260) > 0);
    }

    #[test]
    fn edge_paths_are_open_and_meet_the_outline() {
        for p in [front_top_edge_path(1.0), front_bottom_path(1.0)] {
            assert!(!p
                .segments()
                .any(|s| matches!(s, tiny_skia::PathSegment::Close)));
        }
        let top = front_top_edge_path(1.0);
        let end = *top.points().last().unwrap();
        assert!((end.x - RIGHT).abs() < 0.01 && (end.y - (FRONT_TOP + CORNER)).abs() < 0.01);
    }
}
