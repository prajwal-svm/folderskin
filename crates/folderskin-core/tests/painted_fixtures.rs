//! `painted` against the pictures in tests/fixtures/painted and what the reference implementation
//! (the local-generation skill's `fsgen.py`, which these functions replaced) made of them.
//!
//! `print-on-paper.png` is a real FLUX.2 [klein] picture, shrunk: pop art printed on paper with
//! a frame line. The others are drawn to the same recipe a model follows: a watercolour on paper
//! with one torn edge, a subject on a studio backdrop (no margin), FolderSkin's blank folder
//! repainted 2% small on a drifting purple with a shadow, and one reshaped into a plain card.
//! `expected.json` holds the reference's results for the margins: each margin, the trimmed
//! picture's mean colour and a grid of its pixels, plus the reference's own cut-out of the
//! painted folder.
//!
//! The `klein-*.jpg` pictures are klein's own whole folders, shrunk to 320 x 300, each on a
//! backdrop that drifted from magenta: a lighthouse whose pink clouds and a canyon whose pale back
//! panel share the dusty pink around them, a night street dark on a dark grey, a flamingo lagoon
//! whose edge sits a few pixels inside ours on magenta, and a galaxy on Windows' folder. The
//! reference took the first three for reshaped folders; they aren't.

use folderskin_core::backdrop::Backdrop;
use folderskin_core::base::{Base, MAC_FOLDER, WINDOWS_FOLDER};
use folderskin_core::painted::{self, Border, MIN_PAINTED_FIT};
use image::{GrayImage, Luma, Rgba, RgbaImage};
use serde_json::Value;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/painted")
}

fn picture(name: &str) -> RgbaImage {
    image::open(fixtures().join(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .to_rgba8()
}

fn expected() -> Value {
    serde_json::from_slice(&std::fs::read(fixtures().join("expected.json")).unwrap()).unwrap()
}

/// FolderSkin's silhouette in the fixtures' frame, as `folderskin template --mask` writes it.
fn silhouette(w: u32, h: u32) -> GrayImage {
    let cut = folderskin_core::compositor::blank_template_cutout(w, h);
    GrayImage::from_fn(w, h, |x, y| Luma([cut.get_pixel(x, y).0[3]]))
}

#[test]
fn margins_match_the_reference() {
    let expected = expected();
    for name in ["print-on-paper", "torn-watercolour", "studio-backdrop"] {
        let found = painted::find_border(&picture(&format!("{name}.png")));
        let want = match &expected[name]["border"] {
            Value::Null => None,
            widths => {
                let w: Vec<u32> = serde_json::from_value(widths.clone()).unwrap();
                Some(Border {
                    top: w[0],
                    bottom: w[1],
                    left: w[2],
                    right: w[3],
                })
            }
        };
        assert_eq!(found, want, "{name}");
    }
}

#[test]
fn trimmed_pictures_match_the_reference() {
    let expected = expected();
    for name in ["print-on-paper", "torn-watercolour"] {
        let (_, trimmed) = painted::trim_border(&picture(&format!("{name}.png"))).unwrap();
        let want = &expected[name];
        let n = (trimmed.width() * trimmed.height()) as f64;
        for c in 0..3 {
            let mean = trimmed.pixels().map(|p| p.0[c] as f64).sum::<f64>() / n;
            let reference = want["trimmed_mean"][c].as_f64().unwrap();
            // Pillow's Lanczos and the image crate's weigh their taps a little differently.
            assert!(
                (mean - reference).abs() < 1.0,
                "{name} channel {c}: {mean} against {reference}"
            );
        }
        for sample in want["trimmed_samples"].as_array().unwrap() {
            let s: Vec<u32> = serde_json::from_value(sample.clone()).unwrap();
            let p = trimmed.get_pixel(s[0], s[1]).0;
            for c in 0..3 {
                assert!(
                    (p[c] as i32 - s[2 + c] as i32).abs() <= 12,
                    "{name} at {},{}: {p:?} against {:?}",
                    s[0],
                    s[1],
                    &s[2..]
                );
            }
        }
    }
}

#[test]
fn a_repainted_folder_is_cut_and_a_reshaped_one_is_not() {
    let img = picture("painted-folder.png");
    let cut = painted::cut_along_silhouette(&img, &silhouette(img.width(), img.height()));
    assert!(cut.fit >= MIN_PAINTED_FIT, "fit {}", cut.fit);
    assert!(cut.image.is_some());
    let img = picture("reshaped-folder.png");
    let cut = painted::cut_along_silhouette(&img, &silhouette(img.width(), img.height()));
    assert!(cut.fit < MIN_PAINTED_FIT, "fit {}", cut.fit);
    assert!(cut.image.is_none());
}

#[test]
fn the_cut_out_folder_matches_the_reference() {
    let img = picture("painted-folder.png");
    let ours = painted::cut_along_silhouette(&img, &silhouette(img.width(), img.height()))
        .image
        .unwrap();
    let theirs = picture("painted-folder.cut.png");
    assert_eq!(ours.dimensions(), theirs.dimensions());
    // The reference cut a few pixels inside the painted folder's edge; ours follows it, so only
    // the insides are held to it.
    let (mut opaque, mut colour_diff) = (0usize, 0f64);
    for (a, b) in ours.pixels().zip(theirs.pixels()) {
        if a.0[3] == 255 && b.0[3] == 255 {
            opaque += 1;
            colour_diff += (0..3).map(|c| a.0[c].abs_diff(b.0[c]) as f64).sum::<f64>() / 3.0;
        }
    }
    let n = (ours.width() * ours.height()) as f64;
    assert!(
        opaque as f64 > n * 0.6,
        "only {opaque} pixels opaque in both"
    );
    assert!(colour_diff / (opaque as f64) < 1.5, "colours differ");
}

/// klein's whole folders, and the shape each was painted on.
const KLEIN: &[(&str, &Base)] = &[
    ("klein-lighthouse.jpg", &MAC_FOLDER),
    ("klein-canyon.jpg", &MAC_FOLDER),
    ("klein-neon.jpg", &MAC_FOLDER),
    ("klein-flamingo.jpg", &MAC_FOLDER),
    ("klein-galaxy-windows.jpg", &WINDOWS_FOLDER),
];

fn silhouette_of(base: &Base, w: u32, h: u32) -> GrayImage {
    let cut = base.blank_cutout(w, h).unwrap();
    GrayImage::from_fn(w, h, |x, y| Luma([cut.get_pixel(x, y).0[3]]))
}

#[test]
fn klein_folders_that_kept_their_shape_are_cut_out() {
    for (name, base) in KLEIN {
        let img = picture(name);
        let (w, h) = img.dimensions();
        let cut = painted::cut_along_silhouette(&img, &silhouette_of(base, w, h));
        assert!(cut.fit >= MIN_PAINTED_FIT, "{name}: fit {}", cut.fit);
        let out = cut.image.unwrap();
        assert_eq!(out.get_pixel(1, 1).0[3], 0, "{name}: the backdrop is gone");
        let (x, y) = (w / 2, h * 2 / 3);
        assert_eq!(
            out.get_pixel(x, y).0,
            {
                let p = img.get_pixel(x, y).0;
                [p[0], p[1], p[2], 255]
            },
            "{name}: the painting stays as painted"
        );
    }
}

#[test]
fn no_magenta_is_left_along_the_flamingos_edge() {
    let img = picture("klein-flamingo.jpg");
    let (w, h) = img.dimensions();
    let out = painted::cut_along_silhouette(&img, &silhouette_of(&MAC_FOLDER, w, h))
        .image
        .unwrap();
    let magenta = out
        .pixels()
        .filter(|p| p.0[3] > 64 && i32::from(p.0[0].min(p.0[2])) - i32::from(p.0[1]) > 100)
        .count();
    assert!(magenta < 20, "{magenta} magenta pixels");
}

/// The box around `silhouette`'s body, and the rows its tab alone takes up: from its top down to
/// where the back panel starts, three quarters of the way across.
fn tab(silhouette: &GrayImage) -> ((u32, u32, u32, u32), std::ops::Range<u32>) {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in silhouette.enumerate_pixels() {
        if p.0[0] > 127 {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
        }
    }
    let probe = x0 + (x1 - x0) * 3 / 4;
    let panel = (y0..y1)
        .find(|&y| silhouette.get_pixel(probe, y).0[0] > 127)
        .unwrap();
    ((x0, y0, x1, y1), y0..panel)
}

#[test]
fn a_klein_folder_made_into_a_card_or_given_another_tab_is_left_alone() {
    for (name, base) in KLEIN {
        let img = picture(name);
        let (w, h) = img.dimensions();
        let sil = silhouette_of(base, w, h);
        let ((x0, _, x1, _), rows) = tab(&sil);
        let backdrop = Backdrop::measure(&img).unwrap();
        let panel = rows.end;
        // The strip beside the tab painted like the panel below it: a plain card.
        let mut card = img.clone();
        // The tab painted out: a folder without one.
        let mut untabbed = img.clone();
        // The tab moved to the other end.
        let mut moved = img.clone();
        for y in rows.clone() {
            for x in x0..x1 {
                if sil.get_pixel(x, y).0[0] <= 127 {
                    let below = (panel + (panel - y) + 4).min(h - 1);
                    card.put_pixel(x, y, *img.get_pixel(x, below));
                }
                moved.put_pixel(x, y, *img.get_pixel(x1 - 1 - (x - x0), y));
            }
        }
        // All of it, edge and all: the model's tab is a pixel or two off the template's.
        for y in 0..panel {
            for x in 0..w {
                let k = backdrop.at(x, y);
                untabbed.put_pixel(x, y, Rgba([k[0], k[1], k[2], 255]));
            }
        }
        if let Ok(dir) = std::env::var("SYNTH_OUT") {
            untabbed.save(format!("{dir}/untabbed-{name}.png")).unwrap();
            card.save(format!("{dir}/card-{name}.png")).unwrap();
        }
        for (change, made) in [
            ("a card", card),
            ("no tab", untabbed),
            ("the tab moved", moved),
        ] {
            let cut = painted::cut_along_silhouette(&made, &sil);
            assert!(
                cut.fit < MIN_PAINTED_FIT,
                "{name} with {change}: fit {}",
                cut.fit
            );
            assert!(cut.image.is_none(), "{name} with {change}");
        }
    }
}

#[test]
fn no_fixture_is_blank() {
    for name in ["print-on-paper.png", "painted-folder.png"] {
        assert!(!painted::is_blank(&picture(name)), "{name}");
    }
}
