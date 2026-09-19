//! folderskin-tools: make and check community skin packs, render any picture as the folder the
//! app makes of it, write the composer's template layers, and apply or revert an icon from a
//! terminal.

use clap::Parser;
use folderskin_core::apply::{apply_icon, revert_icon};
use folderskin_core::compositor::{
    render_preview_png, Artwork, ICON_SIZES, SKIN_HEIGHT, SKIN_WIDTH,
};
use folderskin_core::raster;
use folderskin_tools::cli::{Cli, Command, PacksCommand};
use folderskin_tools::skin::Skin;
use folderskin_tools::{composer, make, packs};
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
        Command::Render {
            image,
            solid,
            out,
            size,
            focus,
        } => {
            let focus = focus.unwrap_or((0.5, 0.5));
            let (png, what) = match (image, solid) {
                (Some(p), _) => {
                    let skin = load_skin(&p, focus)?;
                    (skin.preview_png(size), skin.describe())
                }
                (None, Some(hex)) => {
                    let art = Artwork {
                        rgba: solid_image(&hex)?,
                        focus,
                    };
                    (
                        render_preview_png(&art, size),
                        "artwork on FolderSkin's folder",
                    )
                }
                (None, None) => return Err("give a picture path or --solid RRGGBB".into()),
            };
            std::fs::write(&out, png).map_err(|e| e.to_string())?;
            println!("wrote {} ({size}×{size}): {what}", out.display());
            Ok(())
        }
        Command::Guide { out } => guide(&out),
        Command::ComposerLayers { out, size } => composer_layers(&out, size),
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
            image,
            focus,
        } => {
            let skin = load_skin(&image, focus.unwrap_or((0.5, 0.5)))?;
            apply_icon(&folder, &skin.icon_set(&ICON_SIZES)).map_err(|e| e.to_string())?;
            println!("applied to {}: {}", folder.display(), skin.describe());
            Ok(())
        }
        Command::Revert { folder } => {
            revert_icon(&folder).map_err(|e| e.to_string())?;
            println!("reverted {}", folder.display());
            Ok(())
        }
        Command::Packs { command } => match command {
            PacksCommand::Check { dir, max_kb } => packs_check(&dir, max_kb),
            PacksCommand::Index { dir } => packs_index(&dir),
            PacksCommand::Make {
                pictures,
                id,
                name,
                tags,
                author,
                license,
                dir,
                max_kb,
                preview,
                flat_backdrop,
            } => {
                let opts = make::MakeOptions {
                    id,
                    name,
                    tags,
                    author,
                    license,
                    dir,
                    max_bytes: max_kb * 1024,
                    cwebp: make::find_cwebp(),
                    flat_backdrop,
                };
                packs_make(&pictures, &opts, preview.as_deref())
            }
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

/// A picture as the app takes it: a finished folder, or artwork around `focus`.
fn load_skin(path: &Path, focus: (f32, f32)) -> Result<Skin, String> {
    Skin::from_picture(load_picture(path)?, focus).map_err(|e| format!("{} {e}", path.display()))
}

/// Checks every pack in `dir`, printing each problem on its own line.
fn packs_check(dir: &Path, max_kb: Option<usize>) -> Result<(), String> {
    let report = match max_kb {
        Some(kb) => packs::check_within(dir, kb * 1024)?,
        None => packs::check(dir)?,
    };
    for problem in &report.problems {
        println!("{problem}");
    }
    if !report.problems.is_empty() {
        return Err(report.summary());
    }
    println!("{}", report.summary());
    Ok(())
}

/// Makes a pack from pictures and says what went into it: each skin's file, whether it is a
/// finished folder or artwork, and its size.
fn packs_make(
    pictures: &[PathBuf],
    opts: &make::MakeOptions,
    preview: Option<&Path>,
) -> Result<(), String> {
    if opts.cwebp.is_none() {
        println!("cwebp isn't installed, so finished folders are saved as PNG, which is bigger");
    }
    let (folder, made) = make::make(pictures, opts)?;
    for m in &made {
        let kind = if m.folder { "folder " } else { "artwork" };
        println!(
            "{kind}  {:>4} KB  {}  \"{}\"  from {}",
            m.bytes.div_ceil(1024),
            m.file,
            m.name,
            m.source.display()
        );
    }
    let total: usize = made.iter().map(|m| m.bytes).sum();
    let folders = made.iter().filter(|m| m.folder).count();
    println!(
        "wrote {}: {} skins ({folders} finished folders, {} artwork), {} KB",
        folder.display(),
        made.len(),
        made.len() - folders,
        total.div_ceil(1024)
    );
    if let Some(path) = preview {
        let pack = folderskin_core::pack::Pack::parse(
            &std::fs::read(folder.join(folderskin_core::pack::MANIFEST_FILE))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|problems| problems.join("; "))?;
        let sheet = packs::contact_sheet(&folder, &pack, 256, 6)?;
        sheet
            .save(path)
            .map_err(|e| format!("couldn't write {}: {e}", path.display()))?;
        println!("preview: {}", path.display());
    }
    println!("Rename the skins in pack.json if their file names don't make good names.");
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

/// Writes the layers the composer draws a design between into `out`, one PNG each.
fn composer_layers(out: &Path, size: u32) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("couldn't make {}: {e}", out.display()))?;
    for (file, layer) in composer::layer_files(size) {
        let path = out.join(file);
        std::fs::write(&path, raster::encode_png(&layer))
            .map_err(|e| format!("couldn't write {}: {e}", path.display()))?;
        println!("wrote {} ({size}×{size})", path.display());
    }
    Ok(())
}

/// A template showing which parts of a 1024×958 skin land in the tab, the back strip and the front panel.
fn guide(out: &Path) -> Result<(), String> {
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
    fn a_solid_colour_fills_a_skin() {
        let img = solid_image("#2A9D8F").unwrap();
        assert_eq!(img.dimensions(), (SKIN_WIDTH, SKIN_HEIGHT));
        assert_eq!(img.get_pixel(500, 400).0, [0x2A, 0x9D, 0x8F, 255]);
        assert!(solid_image("teal").is_err());
    }
}
