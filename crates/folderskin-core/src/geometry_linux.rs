//! The Linux folder template, in the same 1024×1024 canvas as [`crate::geometry`].
//!
//! GNOME's and KDE's folders are flat: a back panel with a tab at its top left that slopes down
//! to the body, and a lighter front panel over most of it, with no paper sheet between the two.
//! This is FolderSkin's own drawing in that spirit, as wide for its height as Windows' folder and
//! with a gentler tab than either of the other two.

use crate::geometry::{fillet, Rect};
use tiny_skia::{Path, PathBuilder, Transform};

/// Left and right edges of both panels.
pub const LEFT: f32 = 72.0;
pub const RIGHT: f32 = 952.0;
/// Bottom edge of both panels.
pub const BOTTOM: f32 = 872.0;
/// Top edge of the tab, and where it ends before sloping down to the body.
pub const TAB_TOP: f32 = 140.0;
pub const TAB_RIGHT: f32 = 372.0;
/// How far right the tab's slope runs for every unit it drops.
pub const TAB_SLOPE: f32 = 1.15;
/// Top edge of the back panel's body, right of the tab.
pub const BODY_TOP: f32 = 206.0;
/// Top edge of the front panel.
pub const FRONT_TOP: f32 = 276.0;
/// Rounding of the panels' corners, of the tab's top left, and where the slope bends.
pub const CORNER: f32 = 40.0;
pub const TAB_CORNER: f32 = 34.0;
pub const SLOPE_CORNER: f32 = 30.0;

/// Bounding box of the back panel, tab included; artwork is cover-fitted to it.
pub const BACK_BBOX: Rect = Rect::new(LEFT, TAB_TOP, RIGHT, BOTTOM);
/// The front panel; artwork is cover-fitted to it too, so the two panels line up.
pub const FRONT: Rect = Rect::new(LEFT, FRONT_TOP, RIGHT, BOTTOM);

/// Where the tab's slope meets the body's top edge.
fn slope_foot() -> f32 {
    TAB_RIGHT + (BODY_TOP - TAB_TOP) * TAB_SLOPE
}

/// A closed outline through `points`, each `(x, y, radius)` corner rounded by its own radius.
fn rounded(points: &[(f32, f32, f32)]) -> PathBuilder {
    let mut pb = PathBuilder::new();
    let n = points.len();
    for i in 0..n {
        let at = |j: usize| (points[j].0, points[j].1);
        fillet(
            &mut pb,
            at((i + n - 1) % n),
            at(i),
            at((i + 1) % n),
            points[i].2,
        );
    }
    pb.close();
    pb
}

/// An open line through `points`, each corner rounded as [`rounded`] rounds it.
fn rounded_open(points: &[(f32, f32, f32)]) -> PathBuilder {
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for i in 1..points.len() - 1 {
        let at = |j: usize| (points[j].0, points[j].1);
        fillet(&mut pb, at(i - 1), at(i), at(i + 1), points[i].2);
    }
    let last = points[points.len() - 1];
    pb.line_to(last.0, last.1);
    pb
}

fn scaled(pb: PathBuilder, scale: f32) -> Path {
    pb.finish()
        .expect("non-empty path")
        .transform(Transform::from_scale(scale, scale))
        .expect("valid transform")
}

/// The corners of the back panel, clockwise from the tab's top left.
fn back_corners() -> [(f32, f32, f32); 6] {
    [
        (LEFT, TAB_TOP, TAB_CORNER),
        (TAB_RIGHT, TAB_TOP, SLOPE_CORNER),
        (slope_foot(), BODY_TOP, SLOPE_CORNER),
        (RIGHT, BODY_TOP, CORNER),
        (RIGHT, BOTTOM, CORNER),
        (LEFT, BOTTOM, CORNER),
    ]
}

/// The back panel, tab included, clockwise from the tab's top-left corner.
pub fn back_panel_path(scale: f32) -> Path {
    scaled(rounded(&back_corners()), scale)
}

/// The front panel, a rectangle rounded at every corner.
pub fn front_panel_path(scale: f32) -> Path {
    scaled(
        rounded(&[
            (LEFT, FRONT_TOP, CORNER),
            (RIGHT, FRONT_TOP, CORNER),
            (RIGHT, BOTTOM, CORNER),
            (LEFT, BOTTOM, CORNER),
        ]),
        scale,
    )
}

/// Open path along the front's top edge and a little way down each side: its highlight, and the
/// shadow it casts on the back panel.
pub fn front_top_edge_path(scale: f32) -> Path {
    let down = FRONT_TOP + CORNER * 2.0;
    scaled(
        rounded_open(&[
            (LEFT, down, 0.0),
            (LEFT, FRONT_TOP, CORNER),
            (RIGHT, FRONT_TOP, CORNER),
            (RIGHT, down, 0.0),
        ]),
        scale,
    )
}

/// Open path along the back panel's top: the tab, its slope and the body's top edge.
pub fn back_top_edge_path(scale: f32) -> Path {
    let [tab, bend, foot, corner, _, _] = back_corners();
    scaled(
        rounded_open(&[
            (LEFT, TAB_TOP + TAB_CORNER * 2.0, 0.0),
            tab,
            bend,
            foot,
            corner,
            (RIGHT, BODY_TOP + CORNER * 2.0, 0.0),
        ]),
        scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panels_fill_their_boxes_and_no_further() {
        for (path, r) in [
            (back_panel_path(1.0), BACK_BBOX),
            (front_panel_path(1.0), FRONT),
        ] {
            let t = path.compute_tight_bounds().unwrap();
            assert!(
                (t.left() - r.x0).abs() < 0.5 && (t.right() - r.x1).abs() < 0.5,
                "{t:?}"
            );
            assert!(
                (t.top() - r.y0).abs() < 0.5 && (t.bottom() - r.y1).abs() < 0.5,
                "{t:?}"
            );
        }
    }

    #[test]
    fn the_tab_is_at_the_top_left_and_slopes_down_to_the_body() {
        let mut m = tiny_skia::Mask::new(1024, 1024).unwrap();
        m.fill_path(
            &back_panel_path(1.0),
            tiny_skia::FillRule::Winding,
            false,
            Transform::identity(),
        );
        let at = |x: u32, y: u32| m.data()[(y * 1024 + x) as usize];
        assert!(at(200, 160) > 0, "the tab");
        assert_eq!(at(700, 170), 0, "nothing right of it above the body");
        assert!(at(700, 230) > 0, "the body");
        // Half way down the slope, it has moved right.
        let mid_y = ((TAB_TOP + BODY_TOP) / 2.0) as u32;
        let edge = (300..600).find(|&x| at(x, mid_y) == 0).unwrap() as f32;
        assert!(edge > TAB_RIGHT && edge < slope_foot(), "{edge}");
    }

    #[test]
    fn edge_paths_are_open_and_meet_the_outline() {
        for p in [front_top_edge_path(1.0), back_top_edge_path(1.0)] {
            assert!(!p
                .segments()
                .any(|s| matches!(s, tiny_skia::PathSegment::Close)));
        }
        let top = front_top_edge_path(1.0).compute_tight_bounds().unwrap();
        assert!((top.top() - FRONT_TOP).abs() < 0.5);
        let back = back_top_edge_path(1.0).compute_tight_bounds().unwrap();
        assert!((back.top() - TAB_TOP).abs() < 0.5 && (back.right() - RIGHT).abs() < 0.5);
    }
}
