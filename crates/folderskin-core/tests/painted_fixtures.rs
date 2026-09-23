//! `painted` against the pictures in tests/fixtures/painted and what the reference implementation
//! (the local-generation skill's `fsgen.py`, which these functions replaced) made of them.
//!
//! `print-on-paper.png` is a real FLUX.2 [klein] picture, shrunk: pop art printed on paper with
//! a frame line. The others are drawn to the same recipe a model follows: a watercolour on paper
//! with one torn edge, a subject on a studio backdrop (no margin), FolderSkin's blank folder
//! repainted 2% small on a drifting purple with a shadow, and one reshaped into a plain card.
//! `expected.json` holds the reference's results: each margin, the trimmed picture's mean colour
//! and a grid of its pixels, and each whole folder's silhouette fit, plus the reference's own
//! cut-out of the painted folder.

use folderskin_core::painted::{self, Border, MIN_SILHOUETTE_FIT};
use image::{GrayImage, Luma, RgbaImage};
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
fn whole_folders_fit_like_the_reference() {
    let expected = expected();
    for name in ["painted-folder", "reshaped-folder"] {
        let img = picture(&format!("{name}.png"));
        let cut = painted::cut_along_silhouette(&img, &silhouette(img.width(), img.height()));
        let reference = expected[name]["fit"].as_f64().unwrap();
        assert!(
            (cut.fit - reference).abs() < 0.002,
            "{name}: fit {} against {reference}",
            cut.fit
        );
        assert_eq!(
            cut.image.is_some(),
            reference >= MIN_SILHOUETTE_FIT,
            "{name}"
        );
    }
}

#[test]
fn the_cut_out_folder_matches_the_reference() {
    let img = picture("painted-folder.png");
    let ours = painted::cut_along_silhouette(&img, &silhouette(img.width(), img.height()))
        .image
        .unwrap();
    let theirs = picture("painted-folder.cut.png");
    assert_eq!(ours.dimensions(), theirs.dimensions());
    let (mut alpha_off, mut colour_diff, mut opaque) = (0usize, 0f64, 0usize);
    for (a, b) in ours.pixels().zip(theirs.pixels()) {
        if a.0[3].abs_diff(b.0[3]) > 32 {
            alpha_off += 1;
        }
        if a.0[3] > 200 && b.0[3] > 200 {
            opaque += 1;
            colour_diff += (0..3).map(|c| a.0[c].abs_diff(b.0[c]) as f64).sum::<f64>() / 3.0;
        }
    }
    let n = (ours.width() * ours.height()) as f64;
    assert!(
        (alpha_off as f64) < n * 0.005,
        "{alpha_off} pixels' alpha differs"
    );
    assert!(colour_diff / (opaque as f64) < 1.5, "colours differ");
}

#[test]
fn no_fixture_is_blank() {
    for name in ["print-on-paper.png", "painted-folder.png"] {
        assert!(!painted::is_blank(&picture(name)), "{name}");
    }
}
