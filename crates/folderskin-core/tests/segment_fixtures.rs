//! `segment::subject_mask`, the lift for a system without Vision, against klein's own free icons
//! in tests/fixtures/lift, each shrunk to 320 x 300 (the fox to 256 x 240, as the Vision test has
//! it), and beside each the mask Vision lifted from the picture at full size (`*.vision.png`).
//!
//! Each is one of the ways a backdrop fools a colour key: a white robot lit lavender by its sweep,
//! with dark grey rings at its joints; an owl whose purple book is the purple around it; a grey
//! camera in a clay fox's paws on a rose sweep; a cup with steam above it and a contact shadow
//! nearly black under it; a watercolour cactus on a pale wash of the paper; a pixel-art rocket
//! with a shadow drawn under it; a flamingo whose beak is black; a bottle whose shadow falls on a
//! spotlit floor. `robot-640` is the robot again at 640 x 600, big enough to be cut at the working
//! size and cut again at its own.

use folderskin_core::segment::{subject_mask, WORK_SIDE};
use image::GrayImage;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lift")
}

fn open(name: &str) -> image::DynamicImage {
    image::open(fixtures().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The share of the pixels either mask gives the subject that both do.
fn overlap(a: &GrayImage, b: &GrayImage) -> f64 {
    let (mut both, mut either) = (0u64, 0u64);
    for (p, q) in a.pixels().zip(b.pixels()) {
        let (p, q) = (p.0[0] >= 128, q.0[0] >= 128);
        both += u64::from(p && q);
        either += u64::from(p || q);
    }
    both as f64 / either as f64
}

/// Each icon and how much of the subject its cut must share with Vision's, a little under what
/// it shares on an Arm Mac, for the rounding another processor does.
const ICONS: [(&str, f64); 9] = [
    ("robot", 0.955),
    ("robot-640", 0.97),
    ("owl", 0.96),
    ("fox", 0.96),
    ("coffee", 0.96),
    ("cactus", 0.95),
    ("rocket", 0.975),
    ("flamingo", 0.97),
    ("bottle", 0.945),
];

#[test]
fn klein_s_free_icons_come_off_as_vision_lifts_them() {
    for (name, floor) in ICONS {
        let img = open(&format!("{name}.jpg")).to_rgba8();
        let vision = open(&format!("{name}.vision.png")).to_luma8();
        let mask = subject_mask(&img).unwrap_or_else(|| panic!("{name}: no subject found"));
        assert_eq!(mask.dimensions(), img.dimensions(), "{name}");
        let shared = overlap(&mask, &vision);
        assert!(
            shared >= floor,
            "{name}: {shared:.4} of the subject shared with Vision's cut, under {floor}"
        );
    }
}

/// Over the working size, the edge is cut again at the picture's own size and softened: the
/// mask has pixels between subject and backdrop, and only along the edge.
#[test]
fn a_bigger_picture_is_cut_again_at_its_own_size() {
    let img = open("robot-640.jpg").to_rgba8();
    assert!(img.width().max(img.height()) > WORK_SIDE);
    let mask = subject_mask(&img).expect("the robot");
    let (w, h) = mask.dimensions();
    let soft: Vec<(u32, u32)> = mask
        .enumerate_pixels()
        .filter(|(_, _, p)| (8..248).contains(&p.0[0]))
        .map(|(x, y, _)| (x, y))
        .collect();
    assert!(soft.len() > 200, "only {} soft pixels", soft.len());
    // Each soft pixel has the subject and the backdrop within the three pixels it's mixed from.
    let near = |x: u32, y: u32, want: fn(u8) -> bool| {
        (y.saturating_sub(3)..=(y + 3).min(h - 1)).any(|ny| {
            (x.saturating_sub(3)..=(x + 3).min(w - 1)).any(|nx| want(mask.get_pixel(nx, ny).0[0]))
        })
    };
    for &(x, y) in &soft {
        assert!(
            near(x, y, |v| v >= 248) && near(x, y, |v| v < 8),
            "a soft pixel at {x},{y} away from the edge"
        );
    }
}

/// The same picture gives the same mask, byte for byte.
#[test]
fn the_cut_is_the_same_every_time() {
    let img = open("coffee.jpg").to_rgba8();
    assert_eq!(subject_mask(&img), subject_mask(&img));
}
