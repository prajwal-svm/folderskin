//! Will this picture land cleanly on a folder? What the app will make of it, and what to fix
//! before it does.

use folderskin_core::compositor::{self, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte::{self, Surround, MAGENTA};
use folderskin_core::pack::MAX_PICTURE_BYTES;
use folderskin_core::painted;
use image::RgbaImage;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Fine.
    Ok,
    /// Works, but looks worse than it could.
    Warning,
    /// Won't look right on a folder.
    Problem,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub verdict: Verdict,
    pub what: String,
    /// The command or step that fixes it.
    pub fix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    /// "artwork" (wrapped onto FolderSkin's folder) or "folder" (used as it is).
    pub kind: &'static str,
    pub width: u32,
    pub height: u32,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn problems(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.verdict == Verdict::Problem)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.verdict == Verdict::Warning)
            .count()
    }
}

fn ok(what: impl Into<String>) -> Finding {
    Finding {
        verdict: Verdict::Ok,
        what: what.into(),
        fix: None,
    }
}

fn warning(what: impl Into<String>, fix: impl Into<String>) -> Finding {
    Finding {
        verdict: Verdict::Warning,
        what: what.into(),
        fix: Some(fix.into()),
    }
}

fn problem(what: impl Into<String>, fix: impl Into<String>) -> Finding {
    Finding {
        verdict: Verdict::Problem,
        what: what.into(),
        fix: Some(fix.into()),
    }
}

/// How close a pixel must be to #FF00FF, in every channel, to count as the key colour.
const KEY_CLOSE: u8 = 24;

fn is_key(p: &[u8; 4]) -> bool {
    p[3] > 0 && p[0] >= 255 - KEY_CLOSE && p[1] <= KEY_CLOSE && p[2] >= 255 - KEY_CLOSE
}

/// Checks `img` (whose file is `bytes` long) as FolderSkin will use it. `name` is how the fixes
/// refer to it.
pub fn check(img: &RgbaImage, bytes: usize, name: &str) -> Report {
    let (w, h) = img.dimensions();
    let mut findings = Vec::new();
    if matte::alpha_bounds(img, 8).is_none() {
        findings.push(problem(
            "It is completely transparent: there is nothing to put on a folder.",
            "Use another picture.",
        ));
        return Report {
            kind: "artwork",
            width: w,
            height: h,
            findings,
        };
    }
    if painted::is_blank(img) {
        // A warning, not a problem: a plain colour is a skin people choose on purpose (whole
        // packs are made of them, and `packs check` passes them), but a painting that came out
        // flat is usually a failed one.
        findings.push(warning(
            "It is one flat colour, so the folder will look plain.",
            "Fine if a plain colour is what you want; otherwise paint it again or use another picture.",
        ));
    }
    let report = match matte::finished_cutout(img, MAGENTA) {
        Some(cut) => folder(img, &cut, name, &mut findings),
        None => artwork(img, name, &mut findings),
    };
    if bytes > MAX_PICTURE_BYTES {
        // Only a pack has a size limit, and making one shrinks the picture anyway.
        findings.push(ok(format!(
            "The file is {}: fine for your own folders, and `folderskin packs make` \
             shrinks it for a pack, whose limit is 2 MB.",
            crate::images::file_size(bytes)
        )));
    }
    if w.max(h) > 4096 {
        findings.push(warning(
            format!("At {w} × {h} it is far bigger than needed; icons stop at 1024 px."),
            format!("Crop or shrink it: folderskin image crop {name}"),
        ));
    }
    Report {
        kind: report,
        width: w,
        height: h,
        findings,
    }
}

fn artwork(img: &RgbaImage, name: &str, findings: &mut Vec<Finding>) -> &'static str {
    let (w, h) = img.dimensions();
    let short = w.min(h);
    if short < 256 {
        findings.push(problem(
            format!("At {w} × {h} it is too small: it will look blurred on the folder."),
            "Use a picture at least 1024 × 958.",
        ));
    } else if short < 512 {
        findings.push(warning(
            format!("At {w} × {h} it will look soft at the largest icon sizes."),
            "1024 × 958 is ideal; paint or export it bigger.",
        ));
    } else {
        findings.push(ok(format!("{w} × {h} is big enough.")));
    }

    // The folder cover-fits the picture to 1024 x 958, so a different shape loses its sides or
    // its top and bottom.
    let target = SKIN_WIDTH as f64 / SKIN_HEIGHT as f64;
    let aspect = w as f64 / h as f64;
    let kept = if aspect > target {
        target / aspect
    } else {
        aspect / target
    };
    let lost = ((1.0 - kept) * 100.0).round();
    if lost >= 25.0 {
        let sides = if aspect > target {
            "left and right"
        } else {
            "top and bottom"
        };
        findings.push(warning(
            format!(
                "Its shape is far from the folder's: {lost:.0}% of it is cropped off the {sides}."
            ),
            format!("Choose what stays: folderskin image crop {name} --focus 0.5,0.5"),
        ));
    } else {
        findings.push(ok(
            "Its shape is close to the folder's, so little is cropped.",
        ));
    }

    match painted::find_border(img).or_else(|| painted::find_bands(img)) {
        Some(b) => findings.push(warning(
            format!(
                "It sits on paper ({}) that would show as blank bands on the folder.",
                b.describe()
            ),
            format!("folderskin image trim {name}"),
        )),
        None => findings.push(ok("It fills the frame, with no paper margin.")),
    }

    // The top eighth becomes the tab and the strip beside the paper: busy detail there is cut
    // up by the folder's shape.
    let top = h / 8;
    let (top_detail, all_detail) = (detail(img, 0, top), detail(img, 0, h));
    if top > 4 && all_detail > 0.0 && top_detail > all_detail * 1.6 {
        findings.push(warning(
            "A lot happens in the top eighth, which becomes the folder's tab and the strip beside \
             the paper.",
            format!("See how it lands: folderskin render {name} --out preview.png"),
        ));
    } else {
        findings.push(ok("The top eighth, where the tab goes, is quiet."));
    }

    let key = img.pixels().filter(|p| is_key(&p.0)).count();
    let share = key as f64 / (w as f64 * h as f64);
    if share > 0.02 {
        findings.push(warning(
            format!(
                "{:.0}% of it is the magenta key colour (#FF00FF). Where that reaches the edges, \
                 the app may take the picture for a folder on a backdrop and cut it away.",
                share * 100.0
            ),
            format!("Shift the colour a little: folderskin image adjust {name} --hue 12"),
        ));
    }
    if img.pixels().any(|p| p.0[3] < 255) {
        findings.push(warning(
            "Parts of it are see-through; the plain folder shows there.",
            "Fill them in, or cut the subject out fully if it should be a whole folder.",
        ));
    }
    "artwork"
}

fn folder(
    img: &RgbaImage,
    cut: &RgbaImage,
    name: &str,
    findings: &mut Vec<Finding>,
) -> &'static str {
    let keyed = matte::surround(img, MAGENTA) == Surround::Keyed;
    findings.push(ok(if keyed {
        "It is a finished folder on the magenta key colour: the app cuts it out and uses it as it is."
    } else {
        "It is a finished folder, cut out: the app uses it as it is."
    }));
    let (w, h) = cut.dimensions();
    if w.max(h) < 256 {
        findings.push(problem(
            format!("The folder is only {w} × {h}: it will look blurred."),
            "Use a bigger picture; the folder should be at least 512 px wide.",
        ));
    } else if w.max(h) < 512 {
        findings.push(warning(
            format!("The folder is {w} × {h}: it will look soft at the largest icon sizes."),
            "Paint or export it bigger.",
        ));
    } else {
        findings.push(ok(format!("The folder is {w} × {h}, big enough.")));
    }
    // Magenta left inside the cut-out: either keyed away into holes, or a pink rim.
    let leftover = cut
        .pixels()
        .filter(|p| p.0[3] > 128 && is_key(&p.0))
        .count();
    if leftover > (w as usize * h as usize) / 500 {
        findings.push(warning(
            format!("{leftover} pixels of the magenta key colour are left inside the folder."),
            format!(
                "Cut it along FolderSkin's silhouette instead of by colour: folderskin image clip {name}"
            ),
        ));
    }
    let fit = silhouette_fit(cut);
    if fit < painted::MIN_SILHOUETTE_FIT {
        findings.push(warning(
            format!(
                "Its outline differs from FolderSkin's folder (fit {fit:.2}), so it won't line up \
                 with your other folders."
            ),
            "Fine if that is the look you want; otherwise paint it from the template: folderskin ai gen \"…\" --shape folder",
        ));
    } else {
        findings.push(ok(format!(
            "Its outline matches FolderSkin's folder (fit {fit:.2})."
        )));
    }
    "folder"
}

/// How much of `cut`'s outline agrees with FolderSkin's silhouette stretched over the same box:
/// intersection over union of the two shapes, over the whole box and over its top fifth, where
/// the tab makes the folder a folder, whichever is lower. Over the whole box alone, a plain card
/// would pass: the tab is a small part of the area.
pub fn silhouette_fit(cut: &RgbaImage) -> f64 {
    let (w, h) = cut.dimensions();
    if w < 8 || h < 8 {
        return 0.0;
    }
    let template = compositor::blank_template_cutout(w * 2, h * 2);
    let Some((x0, y0, x1, y1)) = matte::alpha_bounds(&template, 128) else {
        return 0.0;
    };
    let trimmed = image::imageops::crop_imm(&template, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
    let ours = image::imageops::resize(&trimmed, w, h, image::imageops::FilterType::Triangle);
    let iou = |rows: std::ops::Range<u32>| {
        let (mut both, mut either) = (0u64, 0u64);
        for y in rows {
            for x in 0..w {
                let a = cut.get_pixel(x, y).0[3] > 127;
                let b = ours.get_pixel(x, y).0[3] > 127;
                both += u64::from(a && b);
                either += u64::from(a || b);
            }
        }
        both as f64 / either.max(1) as f64
    };
    iou(0..h).min(iou(0..h / 5))
}

/// Mean difference between neighbouring pixels in rows `y0..y1`: how busy that band is.
fn detail(img: &RgbaImage, y0: u32, y1: u32) -> f64 {
    let (w, h) = img.dimensions();
    let (mut sum, mut n) = (0u64, 0u64);
    for y in y0..y1.min(h) {
        for x in 1..w {
            let (a, b) = (img.get_pixel(x - 1, y).0, img.get_pixel(x, y).0);
            sum += (0..3).map(|c| u64::from(a[c].abs_diff(b[c]))).sum::<u64>();
            n += 1;
        }
    }
    sum as f64 / n.max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn painting(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let v = (x / 32 + y / 32) % 2;
            Rgba([
                (60 + v * 80 + y / 8) as u8,
                (120 + (x / 16) % 40) as u8,
                (90 + y / 12) as u8,
                255,
            ])
        })
    }

    fn verdicts(r: &Report) -> Vec<(Verdict, &str)> {
        r.findings
            .iter()
            .map(|f| (f.verdict, f.what.as_str()))
            .collect()
    }

    #[test]
    fn good_artwork_passes() {
        let r = check(&painting(1024, 958), 900_000, "a.png");
        assert_eq!(r.kind, "artwork");
        assert_eq!((r.problems(), r.warnings()), (0, 0), "{:?}", verdicts(&r));
    }

    #[test]
    fn a_tiny_letterbox_is_called_out() {
        let r = check(&painting(400, 150), 10_000, "a.png");
        assert_eq!(r.problems(), 1, "{:?}", verdicts(&r));
        assert!(r
            .findings
            .iter()
            .any(|f| f.what.contains("cropped off the left and right")));
    }

    #[test]
    fn a_paper_margin_points_at_trim() {
        let art = painting(640, 600);
        let img = RgbaImage::from_fn(640, 600, |x, y| {
            let edge = x.min(y).min(639 - x).min(599 - y);
            if edge < 20 {
                Rgba([244, 238, 222, 255])
            } else {
                *art.get_pixel(x, y)
            }
        });
        let r = check(&img, 10_000, "print.png");
        let margin = r
            .findings
            .iter()
            .find(|f| {
                f.what
                    .contains("sits on paper (top 23, bottom 23, left 23, right 23 px)")
            })
            .expect("the margin is found");
        assert_eq!(
            margin.fix.as_deref(),
            Some("folderskin image trim print.png")
        );
    }

    #[test]
    fn a_folder_on_magenta_is_checked_as_a_folder() {
        let template = compositor::blank_template(1024, 960, MAGENTA);
        let r = check(&template, 10_000, "f.png");
        assert_eq!(r.kind, "folder");
        assert!(
            r.findings
                .iter()
                .any(|f| f.what.contains("matches FolderSkin's folder")),
            "{:?}",
            verdicts(&r)
        );
        assert_eq!(r.problems(), 0);
    }

    #[test]
    fn a_reshaped_folder_and_empty_pictures_are_caught() {
        // A plain card on magenta: finished, but not FolderSkin's shape.
        let card = RgbaImage::from_fn(800, 800, |x, y| {
            if (100..700).contains(&x) && (150..650).contains(&y) {
                Rgba([30, 90, 200, 255])
            } else {
                Rgba([255, 0, 255, 255])
            }
        });
        let r = check(&card, 10_000, "card.png");
        assert_eq!(r.kind, "folder");
        assert!(r
            .findings
            .iter()
            .any(|f| f.what.contains("outline differs")));
        let clear = check(&RgbaImage::new(64, 64), 100, "c.png");
        assert_eq!(clear.problems(), 1);
        let flat = check(
            &RgbaImage::from_pixel(1024, 958, Rgba([200, 30, 30, 255])),
            100,
            "f.png",
        );
        assert!(flat
            .findings
            .iter()
            .any(|f| f.what.contains("one flat colour") && f.verdict == Verdict::Warning));
        assert_eq!(
            flat.problems(),
            0,
            "a plain colour is a skin, as packs check agrees"
        );
    }

    #[test]
    fn big_files_and_leftover_key_colour_are_warned_about() {
        let mut img = painting(1024, 958);
        for y in 400..600 {
            for x in 400..700 {
                img.put_pixel(x, y, Rgba([255, 0, 255, 255]));
            }
        }
        let r = check(&img, 3_000_000, "k.png");
        assert!(r
            .findings
            .iter()
            .any(|f| f.what.contains("magenta key colour")));
        assert!(r.findings.iter().any(|f| f.what.contains("limit is 2 MB")));
        assert_eq!((r.problems(), r.warnings()), (0, 1));
    }
}
