//! Prints the folder template's shapes as SVG path data on the 1024 canvas.
//!
//! The app's empty drop target (`src/components/FolderGhost.tsx`) is drawn from this output, so
//! it keeps the exact silhouette the compositor renders, FolderSkin's own folder, Windows' and
//! Linux's. Rerun after changing `geometry.rs`, `geometry_windows.rs` or `geometry_linux.rs`:
//!
//! ```sh
//! cargo run -q -p folderskin-core --example outline_svg
//! ```

use folderskin_core::{geometry as g, geometry_linux as l, geometry_windows as w};
use tiny_skia::{Path, PathSegment};

fn svg(path: &Path) -> String {
    let mut d = String::new();
    for seg in path.segments() {
        let part = match seg {
            PathSegment::MoveTo(p) => format!("M{} {}", n(p.x), n(p.y)),
            PathSegment::LineTo(p) => format!("L{} {}", n(p.x), n(p.y)),
            PathSegment::QuadTo(a, p) => format!("Q{} {} {} {}", n(a.x), n(a.y), n(p.x), n(p.y)),
            PathSegment::CubicTo(a, b, p) => format!(
                "C{} {} {} {} {} {}",
                n(a.x),
                n(a.y),
                n(b.x),
                n(b.y),
                n(p.x),
                n(p.y)
            ),
            PathSegment::Close => "Z".to_string(),
        };
        d.push_str(&part);
    }
    d
}

/// One decimal is well below a pixel at any size the app draws the outline.
fn n(v: f32) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{}", r as i32)
    } else {
        format!("{r}")
    }
}

fn main() {
    println!("back:  {}", svg(&g::back_panel_path(1.0)));
    println!("paper: {}", svg(&g::paper_path(1.0)));
    println!("front: {}", svg(&g::front_panel_path(1.0)));
    println!("windows back:  {}", svg(&w::back_panel_path(1.0)));
    println!("windows front: {}", svg(&w::front_panel_path(1.0)));
    println!("linux back:  {}", svg(&l::back_panel_path(1.0)));
    println!("linux front: {}", svg(&l::front_panel_path(1.0)));
}
