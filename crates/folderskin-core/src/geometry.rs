//! Folder template geometry in a 1024×1024 canvas (see spec §4).
//!
//! Everything is built in canvas units and scaled on the way out, so the compositor can ask for
//! the same template at any render size. The template is deliberately left–right symmetric even
//! though the measured reference icon is not.

use tiny_skia::{Path, PathBuilder, Transform};

/// Edge length of the design canvas every constant below is expressed in.
pub const CANVAS: f32 = 1024.0;

/// An axis-aligned rectangle in canvas units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
    pub fn scaled(&self, s: f32) -> Rect {
        Rect::new(self.x0 * s, self.y0 * s, self.x1 * s, self.y1 * s)
    }
}

/// Front panel: the flap the artwork mostly shows through.
pub const FRONT: Rect = Rect::new(15.0, 160.5, 1009.0, 973.5);
/// Corner radius of the front panel.
pub const FRONT_RADIUS: f32 = 55.0;
/// Back panel body, i.e. the back panel without the tab.
pub const BACK_BODY: Rect = Rect::new(29.0, 97.0, 995.0, 973.5);
/// Bounding box of the whole back panel (tab included); the back artwork is cover-fitted to it.
pub const BACK_BBOX: Rect = Rect::new(29.0, 36.5, 995.0, 973.5);
/// Corner radius of the back panel body.
pub const BACK_CORNER: f32 = 46.0;
/// Paper sheet peeking out between the two panels.
pub const PAPER: Rect = Rect::new(74.5, 131.3, 949.5, 973.5);
/// Corner radius of the paper sheet's top corners.
pub const PAPER_RADIUS: f32 = 12.0;
/// Top edge of the tab.
pub const TAB_TOP: f32 = 36.5;
/// Left edge of the tab.
pub const TAB_LEFT: f32 = 61.0;
/// Where the tab's top edge would meet its slanted right edge without the corner rounding.
pub const TAB_TOP_RIGHT_X: f32 = 422.5;
/// Slant of the tab's right edge: dx per dy.
// Measured from the reference icon; its closeness to `FRAC_1_PI` is a coincidence.
#[allow(clippy::approx_constant)]
pub const TAB_SLANT: f32 = 0.318;
/// Radius of the tab's top-left corner.
pub const TAB_TL_RADIUS: f32 = 22.0;
/// Radius of the tab's top-right corner.
pub const TAB_TR_RADIUS: f32 = 16.0;
/// Radius of the concave fillet from the slanted edge into the body's top edge.
pub const TAB_FILLET: f32 = 44.0;
/// Radius of both arcs of the S-curve that steps the tab's left edge out to the body's.
pub const SHOULDER_RADIUS: f32 = 23.0;
/// Where the S-curve leaves the tab's left edge.
pub const SHOULDER_TOP_Y: f32 = 84.0;

/// Appends a circular arc (angles in degrees, screen coordinates, y down).
///
/// The arc is approximated with cubics of at most 90°. If `pb` is empty the arc's start point is
/// a `move_to`, otherwise it is a `line_to`, so consecutive calls chain into one outline.
pub fn arc(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32, start_deg: f32, end_deg: f32) {
    let total = (end_deg - start_deg).to_radians();
    let segments = ((total.abs() / std::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = total / segments as f32;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut a = start_deg.to_radians();
    let start = (cx + r * a.cos(), cy + r * a.sin());
    if pb.is_empty() {
        pb.move_to(start.0, start.1)
    } else {
        pb.line_to(start.0, start.1)
    }
    for _ in 0..segments {
        let b = a + step;
        let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
        pb.cubic_to(
            cx + r * (ca - k * sa),
            cy + r * (sa + k * ca),
            cx + r * (cb + k * sb),
            cy + r * (sb - k * cb),
            cx + r * cb,
            cy + r * sb,
        );
        a = b;
    }
}

/// Angle in degrees of `p` as seen from `centre` (screen coordinates, y down).
fn angle_deg(centre: (f32, f32), p: (f32, f32)) -> f32 {
    (p.1 - centre.1).atan2(p.0 - centre.0).to_degrees()
}

/// Tangent points and arc centres of the tab, derived from the tab constants.
///
/// The back panel outline and the back panel's top rim both need these, and they must agree
/// exactly or the rim would not sit on the outline.
struct Tab {
    /// Where the tab's top edge meets the top-right corner arc.
    tr_arc_start: (f32, f32),
    /// Centre of the tab's top-right corner arc.
    tr_centre: (f32, f32),
    /// Where the top-right corner arc meets the slanted edge.
    tr_arc_end: (f32, f32),
    /// Where the slanted edge meets the concave fillet.
    fillet_start: (f32, f32),
    /// Centre of the concave fillet arc.
    fillet_centre: (f32, f32),
    /// Where the fillet meets the body's top edge.
    fillet_end: (f32, f32),
}

impl Tab {
    fn new() -> Self {
        // Unit direction of the slanted right edge, pointing down and to the right.
        let len = (TAB_SLANT * TAB_SLANT + 1.0).sqrt();
        let dir = (TAB_SLANT / len, 1.0 / len);
        // Turn angle between a horizontal edge and the slant; `half_tan` is the tangent
        // distance from the vertex for a given radius.
        let half_tan = (dir.0.acos() / 2.0).tan();

        // Top-right corner: the vertex is (TAB_TOP_RIGHT_X, TAB_TOP), turning from (1, 0) to dir.
        let t = TAB_TR_RADIUS * half_tan;
        let tr_arc_start = (TAB_TOP_RIGHT_X - t, TAB_TOP);
        let tr_centre = (tr_arc_start.0, TAB_TOP + TAB_TR_RADIUS);
        let tr_arc_end = (TAB_TOP_RIGHT_X + dir.0 * t, TAB_TOP + dir.1 * t);

        // Concave fillet: the slant meets the body's top edge at this (reflex) vertex, so the
        // fillet's centre sits above the top edge, outside the body.
        let vertex = (
            TAB_TOP_RIGHT_X + TAB_SLANT * (BACK_BODY.y0 - TAB_TOP),
            BACK_BODY.y0,
        );
        let t2 = TAB_FILLET * half_tan;
        let fillet_start = (vertex.0 - dir.0 * t2, vertex.1 - dir.1 * t2);
        let fillet_end = (vertex.0 + t2, vertex.1);
        let fillet_centre = (fillet_end.0, fillet_end.1 - TAB_FILLET);

        Self {
            tr_arc_start,
            tr_centre,
            tr_arc_end,
            fillet_start,
            fillet_centre,
            fillet_end,
        }
    }
}

fn rounded_rect(r: Rect, radius: f32) -> PathBuilder {
    let mut pb = PathBuilder::new();
    arc(&mut pb, r.x0 + radius, r.y0 + radius, radius, 180.0, 270.0);
    arc(&mut pb, r.x1 - radius, r.y0 + radius, radius, 270.0, 360.0);
    arc(&mut pb, r.x1 - radius, r.y1 - radius, radius, 0.0, 90.0);
    arc(&mut pb, r.x0 + radius, r.y1 - radius, radius, 90.0, 180.0);
    pb.close();
    pb
}

fn scaled(pb: PathBuilder, scale: f32) -> Path {
    let path = pb.finish().expect("non-empty path");
    path.transform(Transform::from_scale(scale, scale))
        .expect("valid transform")
}

/// Front panel outline (a rounded rectangle), clockwise from its top-left corner.
pub fn front_panel_path(scale: f32) -> Path {
    scaled(rounded_rect(FRONT, FRONT_RADIUS), scale)
}

/// Paper sheet outline: rounded at the top, square at the bottom (hidden behind the front).
pub fn paper_path(scale: f32) -> Path {
    let mut pb = PathBuilder::new();
    arc(
        &mut pb,
        PAPER.x0 + PAPER_RADIUS,
        PAPER.y0 + PAPER_RADIUS,
        PAPER_RADIUS,
        180.0,
        270.0,
    );
    arc(
        &mut pb,
        PAPER.x1 - PAPER_RADIUS,
        PAPER.y0 + PAPER_RADIUS,
        PAPER_RADIUS,
        270.0,
        360.0,
    );
    pb.line_to(PAPER.x1, PAPER.y1);
    pb.line_to(PAPER.x0, PAPER.y1);
    pb.close();
    scaled(pb, scale)
}

/// Back panel incl. tab, clockwise from the tab's top-left corner.
pub fn back_panel_path(scale: f32) -> Path {
    let tab = Tab::new();
    let mut pb = PathBuilder::new();
    // Tab top-left corner, then the top edge into the top-right corner.
    arc(
        &mut pb,
        TAB_LEFT + TAB_TL_RADIUS,
        TAB_TOP + TAB_TL_RADIUS,
        TAB_TL_RADIUS,
        180.0,
        270.0,
    );
    arc(
        &mut pb,
        tab.tr_centre.0,
        tab.tr_centre.1,
        TAB_TR_RADIUS,
        angle_deg(tab.tr_centre, tab.tr_arc_start),
        angle_deg(tab.tr_centre, tab.tr_arc_end),
    );
    // Slant down to the fillet (the arc's own `line_to`), then the concave fillet. Sweeping
    // from the slant's tangent point back to the top edge's runs the short way round, which
    // keeps the outline continuous.
    arc(
        &mut pb,
        tab.fillet_centre.0,
        tab.fillet_centre.1,
        TAB_FILLET,
        angle_deg(tab.fillet_centre, tab.fillet_start),
        angle_deg(tab.fillet_centre, tab.fillet_end),
    );
    // Body top edge and top-right corner.
    arc(
        &mut pb,
        BACK_BODY.x1 - BACK_CORNER,
        BACK_BODY.y0 + BACK_CORNER,
        BACK_CORNER,
        270.0,
        360.0,
    );
    // Right side and the two bottom corners (all hidden behind the front panel).
    arc(
        &mut pb,
        BACK_BODY.x1 - BACK_CORNER,
        BACK_BODY.y1 - BACK_CORNER,
        BACK_CORNER,
        0.0,
        90.0,
    );
    arc(
        &mut pb,
        BACK_BODY.x0 + BACK_CORNER,
        BACK_BODY.y1 - BACK_CORNER,
        BACK_CORNER,
        90.0,
        180.0,
    );
    // Left side up to the S-curve that steps in to the tab's left edge.
    let gap = TAB_LEFT - BACK_BODY.x0;
    let phi = (1.0 - gap / (2.0 * SHOULDER_RADIUS)).acos();
    let phi_deg = phi.to_degrees();
    let s_bottom = SHOULDER_TOP_Y + 2.0 * SHOULDER_RADIUS * phi.sin();
    pb.line_to(BACK_BODY.x0, s_bottom);
    arc(
        &mut pb,
        BACK_BODY.x0 + SHOULDER_RADIUS,
        s_bottom,
        SHOULDER_RADIUS,
        180.0,
        180.0 + phi_deg,
    );
    arc(
        &mut pb,
        TAB_LEFT - SHOULDER_RADIUS,
        SHOULDER_TOP_Y,
        SHOULDER_RADIUS,
        phi_deg,
        0.0,
    );
    pb.close();
    scaled(pb, scale)
}

/// Open path along the front panel's left side, top edge and right side (both top corners
/// included), for the rim light. Runs from the top of the bottom-left corner arc to the top of
/// the bottom-right one.
pub fn front_top_sides_path(scale: f32) -> Path {
    let r = FRONT_RADIUS;
    let mut pb = PathBuilder::new();
    pb.move_to(FRONT.x0, FRONT.y1 - r);
    arc(&mut pb, FRONT.x0 + r, FRONT.y0 + r, r, 180.0, 270.0);
    arc(&mut pb, FRONT.x1 - r, FRONT.y0 + r, r, 270.0, 360.0);
    pb.line_to(FRONT.x1, FRONT.y1 - r);
    scaled(pb, scale)
}

/// Open path along the front panel's bottom edge and its two bottom corners, for the shade.
pub fn front_bottom_path(scale: f32) -> Path {
    let r = FRONT_RADIUS;
    let mut pb = PathBuilder::new();
    arc(&mut pb, FRONT.x1 - r, FRONT.y1 - r, r, 0.0, 90.0);
    arc(&mut pb, FRONT.x0 + r, FRONT.y1 - r, r, 90.0, 180.0);
    scaled(pb, scale)
}

/// Open path along the back panel body's top edge, from the fillet's end through the body's
/// top-right corner, for the rim light.
pub fn back_top_edge_path(scale: f32) -> Path {
    let tab = Tab::new();
    let mut pb = PathBuilder::new();
    pb.move_to(tab.fillet_end.0, tab.fillet_end.1);
    arc(
        &mut pb,
        BACK_BODY.x1 - BACK_CORNER,
        BACK_BODY.y0 + BACK_CORNER,
        BACK_CORNER,
        270.0,
        360.0,
    );
    scaled(pb, scale)
}

/// Open path along the paper sheet's top edge, both top corners included, for its highlight.
pub fn paper_top_edge_path(scale: f32) -> Path {
    let r = PAPER_RADIUS;
    let mut pb = PathBuilder::new();
    pb.move_to(PAPER.x0, PAPER.y0 + r);
    arc(&mut pb, PAPER.x0 + r, PAPER.y0 + r, r, 180.0, 270.0);
    arc(&mut pb, PAPER.x1 - r, PAPER.y0 + r, r, 270.0, 360.0);
    scaled(pb, scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds(p: &tiny_skia::Path) -> tiny_skia::Rect {
        p.bounds()
    }
    fn first(p: &Path) -> (f32, f32) {
        let pt = p.points()[0];
        (pt.x, pt.y)
    }
    fn last(p: &Path) -> (f32, f32) {
        let pt = *p.points().last().unwrap();
        (pt.x, pt.y)
    }
    fn is_open(p: &Path) -> bool {
        !p.segments()
            .any(|s| matches!(s, tiny_skia::PathSegment::Close))
    }
    fn near(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
    }

    #[test]
    fn front_panel_bounds_match_spec() {
        let b = bounds(&front_panel_path(1.0));
        assert!((b.left() - 15.0).abs() < 0.01 && (b.right() - 1009.0).abs() < 0.01);
        assert!((b.top() - 160.5).abs() < 0.01 && (b.bottom() - 973.5).abs() < 0.01);
    }

    #[test]
    fn back_panel_bounds_match_spec() {
        let b = bounds(&back_panel_path(1.0));
        assert!((b.left() - 29.0).abs() < 0.01 && (b.right() - 995.0).abs() < 0.01);
        assert!((b.top() - 36.5).abs() < 0.01 && (b.bottom() - 973.5).abs() < 0.01);
    }

    #[test]
    fn template_is_left_right_symmetric() {
        assert!((FRONT.x0 - (CANVAS - FRONT.x1)).abs() < 0.01);
        assert!((BACK_BODY.x0 - (CANVAS - BACK_BODY.x1)).abs() < 0.01);
        assert!((PAPER.x0 - (CANVAS - PAPER.x1)).abs() < 0.01);
    }

    #[test]
    fn scale_scales_bounds() {
        let b = bounds(&front_panel_path(2.0));
        assert!((b.right() - 2018.0).abs() < 0.01);
    }

    #[test]
    fn arc_quarter_circle_endpoints() {
        let mut pb = tiny_skia::PathBuilder::new();
        arc(&mut pb, 0.0, 0.0, 10.0, -90.0, 0.0);
        let p = pb.finish().unwrap();
        let b = p.bounds();
        assert!((b.top() + 10.0).abs() < 0.05 && (b.right() - 10.0).abs() < 0.05);
    }

    #[test]
    fn arc_sweeps_backwards_too_and_chains_with_line_to() {
        let mut pb = tiny_skia::PathBuilder::new();
        arc(&mut pb, 0.0, 0.0, 10.0, 90.0, 0.0);
        arc(&mut pb, 100.0, 0.0, 10.0, 180.0, 270.0);
        let p = pb.finish().unwrap();
        // First arc: (0, 10) → (10, 0); second starts at (90, 0) via a line_to.
        assert!(near(first(&p), (0.0, 10.0)));
        assert!(near(last(&p), (100.0, -10.0)));
        assert!(p.segments().any(
            |s| matches!(s, tiny_skia::PathSegment::LineTo(pt) if (pt.x - 90.0).abs() < 0.01)
        ));
    }

    #[test]
    fn paper_bounds_match_spec() {
        let b = bounds(&paper_path(1.0));
        assert!((b.left() - PAPER.x0).abs() < 0.01 && (b.right() - PAPER.x1).abs() < 0.01);
        assert!((b.top() - PAPER.y0).abs() < 0.01 && (b.bottom() - PAPER.y1).abs() < 0.01);
    }

    #[test]
    fn tight_bounds_stay_inside_the_declared_rectangles() {
        // Catches an arc that sweeps the wrong way: it would bulge outside its own corner.
        for (path, r) in [
            (front_panel_path(1.0), FRONT),
            (back_panel_path(1.0), BACK_BBOX),
            (paper_path(1.0), PAPER),
        ] {
            let t = path.compute_tight_bounds().unwrap();
            assert!(t.left() >= r.x0 - 0.01 && t.right() <= r.x1 + 0.01);
            assert!(t.top() >= r.y0 - 0.01 && t.bottom() <= r.y1 + 0.01);
        }
    }

    #[test]
    fn rim_paths_are_open() {
        for p in [
            front_top_sides_path(1.0),
            front_bottom_path(1.0),
            back_top_edge_path(1.0),
            paper_top_edge_path(1.0),
        ] {
            assert!(is_open(&p));
        }
    }

    #[test]
    fn front_rim_paths_join_into_the_front_outline() {
        let top = front_top_sides_path(1.0);
        let bottom = front_bottom_path(1.0);
        assert!(near(first(&top), (FRONT.x0, FRONT.y1 - FRONT_RADIUS)));
        assert!(near(last(&top), (FRONT.x1, FRONT.y1 - FRONT_RADIUS)));
        // The bottom run picks up where the top/sides run ends and closes the loop.
        assert!(near(first(&bottom), last(&top)));
        assert!(near(last(&bottom), first(&top)));
    }

    #[test]
    fn back_top_edge_runs_from_the_fillet_to_the_right_side() {
        let tab = Tab::new();
        let p = back_top_edge_path(1.0);
        assert!(near(first(&p), tab.fillet_end));
        assert!(near(last(&p), (BACK_BODY.x1, BACK_BODY.y0 + BACK_CORNER)));
        // The fillet lands on the body's top edge, to the right of the tab's slanted edge.
        assert!((tab.fillet_end.1 - BACK_BODY.y0).abs() < 0.01);
        assert!(tab.fillet_end.0 > TAB_TOP_RIGHT_X);
    }

    #[test]
    fn paper_top_edge_spans_the_sheet_between_its_corners() {
        let p = paper_top_edge_path(1.0);
        assert!(near(first(&p), (PAPER.x0, PAPER.y0 + PAPER_RADIUS)));
        assert!(near(last(&p), (PAPER.x1, PAPER.y0 + PAPER_RADIUS)));
        assert!((bounds(&p).top() - PAPER.y0).abs() < 0.01);
    }

    #[test]
    fn fillet_is_concave_and_tangent_to_both_edges() {
        let tab = Tab::new();
        // Centre above the body's top edge means the arc bulges up into the notch.
        assert!(tab.fillet_centre.1 < BACK_BODY.y0);
        assert!((tab.fillet_centre.1 - (BACK_BODY.y0 - TAB_FILLET)).abs() < 0.01);
        // Both tangent points are exactly TAB_FILLET from the centre.
        for p in [tab.fillet_start, tab.fillet_end] {
            let d =
                ((p.0 - tab.fillet_centre.0).powi(2) + (p.1 - tab.fillet_centre.1).powi(2)).sqrt();
            assert!((d - TAB_FILLET).abs() < 0.01, "{d}");
        }
        // The slant's tangent points lie on the line through the tab's top-right vertex.
        for p in [tab.tr_arc_end, tab.fillet_start] {
            let x_on_slant = TAB_TOP_RIGHT_X + TAB_SLANT * (p.1 - TAB_TOP);
            assert!((p.0 - x_on_slant).abs() < 0.01, "{p:?}");
        }
    }

    #[test]
    fn shoulder_s_curve_reaches_the_tab_edge_near_the_spec_y() {
        // Spec §4: the S-curve spans y 84 → 128.
        let gap = TAB_LEFT - BACK_BODY.x0;
        let phi = (1.0 - gap / (2.0 * SHOULDER_RADIUS)).acos();
        let s_bottom = SHOULDER_TOP_Y + 2.0 * SHOULDER_RADIUS * phi.sin();
        assert!((s_bottom - 128.0).abs() < 0.5, "{s_bottom}");
    }
}
