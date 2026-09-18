//! folderskin-tools: generate, import and check skins; render previews; apply from a terminal.

use clap::Parser;
use folderskin_core::apply::{apply_icon, revert_icon};
use folderskin_core::compositor::{render_icon_set, render_preview_png, Artwork, ICON_SIZES};
use folderskin_core::manifest::{Manifest, SkinEntry, MAX_SKIN_BYTES, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_tools::cli::{Cli, Command, PacksCommand, SkinCommand};
use folderskin_tools::{gen, packs};
use image::{ImageEncoder, RgbaImage};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const PREVIEW_SIZE: u32 = 512;
const JPEG_QUALITY: u8 = 90;

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Skin { command } => match command {
            SkinCommand::Gen { out, previews } => skin_gen(&out, &previews),
            SkinCommand::Add {
                image,
                id,
                name,
                collection,
                focus,
                lossless,
                dir,
                previews,
                author,
                license,
            } => skin_add(
                &image,
                &id,
                &name,
                &collection,
                focus.unwrap_or((0.5, 0.5)),
                lossless,
                &dir,
                &previews,
                &author,
                &license,
            ),
            SkinCommand::Check { dir } => skin_check(&dir),
            SkinCommand::Guide { out } => skin_guide(&out),
        },
        Command::Render {
            image,
            solid,
            out,
            size,
            focus,
        } => {
            let rgba = match (image, solid) {
                (Some(p), _) => load_picture(&p)?,
                (None, Some(hex)) => solid_image(&hex)?,
                (None, None) => return Err("give a picture path or --solid RRGGBB".into()),
            };
            let art = Artwork {
                rgba,
                focus: focus.unwrap_or((0.5, 0.5)),
            };
            std::fs::write(&out, render_preview_png(&art, size)).map_err(|e| e.to_string())?;
            println!("wrote {} ({size}×{size})", out.display());
            Ok(())
        }
        Command::AppIcon { input, out } => {
            let (w, h) = image::ImageReader::open(&input)
                .and_then(|r| r.with_guessed_format())
                .map_err(|e| e.to_string())?
                .into_dimensions()
                .map_err(|e| e.to_string())?;
            if (w, h) != (1024, 1024) {
                return Err(format!(
                    "{} is {w}×{h}; the app icon source must be 1024×1024",
                    input.display()
                ));
            }
            let img = image::open(&input).map_err(|e| e.to_string())?.to_rgba8();
            if !img.pixels().any(|p| p.0[3] == 0) {
                return Err(
                    "the app icon source has no transparent pixels; export it with alpha".into(),
                );
            }
            println!(
                "{} looks good. Build the icon set with:\n  pnpm tauri icon {} -o {}",
                input.display(),
                input.display(),
                out.display()
            );
            Ok(())
        }
        Command::Apply {
            folder,
            skin,
            image,
            dir,
            focus,
        } => {
            let art = match (skin, image) {
                (Some(id), _) => {
                    let m = Manifest::load(&dir).map_err(|e| e.to_string())?;
                    let entry =
                        m.skins.iter().find(|s| s.id == id).ok_or_else(|| {
                            format!("no skin with id {id:?} in {}", dir.display())
                        })?;
                    Artwork {
                        rgba: load_picture(&dir.join(&entry.file))?,
                        focus: focus.unwrap_or((entry.focus[0], entry.focus[1])),
                    }
                }
                (None, Some(p)) => Artwork {
                    rgba: load_picture(&p)?,
                    focus: focus.unwrap_or((0.5, 0.5)),
                },
                (None, None) => return Err("give --skin <id> or --image <path>".into()),
            };
            let icons = render_icon_set(&art, &ICON_SIZES);
            apply_icon(&folder, &icons).map_err(|e| e.to_string())?;
            println!("applied to {}", folder.display());
            Ok(())
        }
        Command::Revert { folder } => {
            revert_icon(&folder).map_err(|e| e.to_string())?;
            println!("reverted {}", folder.display());
            Ok(())
        }
        Command::Packs { command } => match command {
            PacksCommand::Check { dir } => packs_check(&dir),
            PacksCommand::Index { dir } => packs_index(&dir),
        },
    }
}

// ---------- helpers ----------

fn load_picture(path: &Path) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn solid_image(hex: &str) -> Result<RgbaImage, String> {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16)
        .map_err(|_| format!("{hex:?} is not RRGGBB"))?;
    Ok(RgbaImage::from_pixel(
        SKIN_WIDTH,
        SKIN_HEIGHT,
        image::Rgba([(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]),
    ))
}

/// Crops `img` to the skin aspect around the focus point and resizes to 1024×958.
pub fn crop_to_skin(img: &RgbaImage, focus: (f32, f32)) -> RgbaImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let target = SKIN_WIDTH as f32 / SKIN_HEIGHT as f32;
    let (cw, ch) = if w / h > target {
        (h * target, h)
    } else {
        (w, w / target)
    };
    let x0 = ((w - cw) * focus.0).round().clamp(0.0, w - cw) as u32;
    let y0 = ((h - ch) * focus.1).round().clamp(0.0, h - ch) as u32;
    let cropped =
        image::imageops::crop_imm(img, x0, y0, cw.round() as u32, ch.round() as u32).to_image();
    image::imageops::resize(
        &cropped,
        SKIN_WIDTH,
        SKIN_HEIGHT,
        image::imageops::FilterType::Lanczos3,
    )
}

fn encode_jpeg(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut buf,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Encodes as JPEG q90, or PNG when it is smaller (flat graphics) or when `lossless` is set.
fn encode_skin(img: &RgbaImage, lossless: bool) -> Result<(Vec<u8>, &'static str), String> {
    let png = encode_png(img)?;
    if lossless {
        return Ok((png, "png"));
    }
    let jpeg = encode_jpeg(img)?;
    Ok(if png.len() <= jpeg.len() {
        (png, "png")
    } else {
        (jpeg, "jpg")
    })
}

fn write_preview(dir: &Path, id: &str, art: &Artwork) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let p = dir.join(format!("{id}.png"));
    std::fs::write(&p, render_preview_png(art, PREVIEW_SIZE)).map_err(|e| e.to_string())?;
    Ok(p)
}

fn skin_gen(out: &Path, previews: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut manifest = Manifest::default();
    let mut total = 0usize;
    for (mut entry, img) in gen::builtin_skins() {
        let (bytes, ext) = encode_skin(&img, false)?;
        entry.file = format!("{}.{ext}", entry.id);
        std::fs::write(out.join(&entry.file), &bytes).map_err(|e| e.to_string())?;
        total += bytes.len();
        let preview = write_preview(
            previews,
            &entry.id,
            &Artwork {
                rgba: img,
                focus: (entry.focus[0], entry.focus[1]),
            },
        )?;
        println!(
            "{:<10} {:>4} KB  {}  preview {}",
            entry.id,
            bytes.len() / 1024,
            ext,
            preview.display()
        );
        manifest.upsert(entry);
    }
    manifest.save(out).map_err(|e| e.to_string())?;
    println!(
        "{} skins, {} KB total, manifest {}",
        manifest.skins.len(),
        total / 1024,
        Manifest::path_in(out).display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn skin_add(
    image: &Path,
    id: &str,
    name: &str,
    collection: &str,
    focus: (f32, f32),
    lossless: bool,
    dir: &Path,
    previews: &Path,
    author: &str,
    license: &str,
) -> Result<(), String> {
    if !Manifest::is_valid_id(id) {
        return Err(format!(
            "id {id:?} must be lowercase letters, digits and dashes"
        ));
    }
    let src = load_picture(image)?;
    let skin = crop_to_skin(&src, focus);
    let (bytes, ext) = encode_skin(&skin, lossless)?;
    if bytes.len() as u64 > MAX_SKIN_BYTES {
        return Err(format!("{id}: encoded size {} KB is above the {} KB limit; try a simpler picture or omit --lossless", bytes.len() / 1024, MAX_SKIN_BYTES / 1024));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut manifest = Manifest::load(dir).unwrap_or_default();
    if let Some(old) = manifest.skins.iter().find(|s| s.id == id) {
        if old.file != format!("{id}.{ext}") {
            let _ = std::fs::remove_file(dir.join(&old.file));
        }
    }
    let file = format!("{id}.{ext}");
    std::fs::write(dir.join(&file), &bytes).map_err(|e| e.to_string())?;
    let preview = write_preview(previews, id, &Artwork { rgba: skin, focus })?;
    manifest.upsert(SkinEntry {
        id: id.into(),
        name: name.into(),
        collection: collection.into(),
        file,
        focus: [focus.0, focus.1],
        author: author.into(),
        license: license.into(),
    });
    manifest.save(dir).map_err(|e| e.to_string())?;
    println!(
        "added {id} ({} KB, {ext}); look at {} and adjust --focus if the crop is off",
        bytes.len() / 1024,
        preview.display()
    );
    let problems = manifest.validate(dir);
    for p in &problems {
        println!("note: {p}");
    }
    Ok(())
}

fn skin_check(dir: &Path) -> Result<(), String> {
    let manifest = Manifest::load(dir).map_err(|e| e.to_string())?;
    let problems = manifest.validate(dir);
    println!(
        "{} skins, {} KB total",
        manifest.skins.len(),
        manifest.total_bytes(dir) / 1024
    );
    if problems.is_empty() {
        println!("ok");
        Ok(())
    } else {
        for p in &problems {
            println!("problem: {p}");
        }
        Err(format!("{} problem(s)", problems.len()))
    }
}

/// Checks every community pack, printing each problem on its own line.
fn packs_check(dir: &Path) -> Result<(), String> {
    let report = packs::check(dir)?;
    for problem in &report.problems {
        println!("{problem}");
    }
    if !report.problems.is_empty() {
        return Err(report.summary());
    }
    println!("{}", report.summary());
    Ok(())
}

/// Checks every community pack and, when all pass, brings the index and the previews up to date.
fn packs_index(dir: &Path) -> Result<(), String> {
    let report = packs::check(dir)?;
    for problem in &report.problems {
        println!("{problem}");
    }
    let changes = packs::write_index(dir, &report)?;
    for path in &changes.removed {
        println!("removed {}", path.display());
    }
    for path in &changes.written {
        println!("wrote {}", path.display());
    }
    let unchanged = if changes.is_empty() {
        ", nothing changed"
    } else {
        ""
    };
    println!("{} indexed{unchanged}", report.totals());
    Ok(())
}

/// A template showing which parts of a 1024×958 skin land in the tab, the back strip and the front panel.
fn skin_guide(out: &Path) -> Result<(), String> {
    use folderskin_core::geometry as g;
    let mut img = RgbaImage::from_pixel(SKIN_WIDTH, SKIN_HEIGHT, image::Rgba([246, 239, 228, 255]));
    // The skin is cover-fitted to BACK_BBOX (whole height) and FRONT (middle band); express both in skin pixels.
    let back_scale = g::BACK_BBOX.height() / SKIN_HEIGHT as f32;
    let front_top = ((g::FRONT.y0 - g::BACK_BBOX.y0) / back_scale).round() as u32;
    let paper_top = ((g::PAPER.y0 - g::BACK_BBOX.y0) / back_scale).round() as u32;
    let body_top = ((g::BACK_BODY.y0 - g::BACK_BBOX.y0) / back_scale).round() as u32;
    let front_scale = g::FRONT.width() / SKIN_WIDTH as f32;
    let front_visible_h = (g::FRONT.height() / front_scale).round() as u32;
    let front_crop = (SKIN_HEIGHT - front_visible_h) / 2;
    for (y, px) in img
        .enumerate_rows_mut()
        .flat_map(|(y, row)| row.map(move |(_, _, p)| (y, p)))
    {
        let tint = if y < body_top {
            [255, 214, 120] // tab
        } else if y < paper_top {
            [255, 232, 170] // back strip above the paper
        } else if y < front_top {
            [232, 232, 232] // hidden behind the paper strip
        } else {
            [200, 232, 200] // front panel
        };
        let in_front_crop = y >= front_crop && y < SKIN_HEIGHT - front_crop;
        let k = if in_front_crop { 0.55 } else { 0.85 };
        *px = image::Rgba([
            (px.0[0] as f32 * (1.0 - k) + tint[0] as f32 * k) as u8,
            (px.0[1] as f32 * (1.0 - k) + tint[1] as f32 * k) as u8,
            (px.0[2] as f32 * (1.0 - k) + tint[2] as f32 * k) as u8,
            255,
        ]);
    }
    for y in [
        body_top,
        paper_top,
        front_top,
        front_crop,
        SKIN_HEIGHT - front_crop,
    ] {
        for x in 0..SKIN_WIDTH {
            img.put_pixel(x, y.min(SKIN_HEIGHT - 1), image::Rgba([60, 40, 20, 255]));
        }
    }
    img.save(out).map_err(|e| e.to_string())?;
    println!(
        "wrote {}: rows 0–{} show in the tab, {}–{} above the paper, {}–{} are hidden by the paper, {}+ is the front panel; the front panel keeps rows {}–{} of the picture (centred on the focus)",
        out.display(),
        body_top,
        body_top,
        paper_top,
        paper_top,
        front_top,
        front_top,
        front_crop,
        SKIN_HEIGHT - front_crop
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_to_skin_keeps_aspect_and_focus() {
        let mut src = RgbaImage::from_pixel(2000, 1000, image::Rgba([0, 0, 0, 255]));
        for x in 1500..2000 {
            for y in 0..1000 {
                src.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let left = crop_to_skin(&src, (0.0, 0.5));
        let right = crop_to_skin(&src, (1.0, 0.5));
        assert_eq!(left.dimensions(), (SKIN_WIDTH, SKIN_HEIGHT));
        assert_eq!(left.get_pixel(1000, 500).0[0], 0);
        assert_eq!(right.get_pixel(1000, 500).0[0], 255);
    }

    #[test]
    fn encode_skin_prefers_the_smaller_container() {
        let flat = RgbaImage::from_pixel(SKIN_WIDTH, SKIN_HEIGHT, image::Rgba([10, 200, 90, 255]));
        let (_, ext) = encode_skin(&flat, false).unwrap();
        assert_eq!(ext, "png");
        let (_, ext) = encode_skin(&flat, true).unwrap();
        assert_eq!(ext, "png");
    }
}
