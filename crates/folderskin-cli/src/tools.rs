//! The commands folderskin-tools has always had, on the same library code, with errors that say
//! what to do: `apply`, `revert`, `render`, `template` and `packs`.

use crate::cli::{ApplyArgs, RenderArgs, TemplateArgs};
use crate::error::CliError;
use crate::out::Out;
use crate::{images, preview};
use folderskin_core::apply::{refresh_shell_icons, revert_icon};
use folderskin_core::compositor::{self, render_preview_png, Artwork, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte;
use folderskin_tools::cli::PacksCommand;
use folderskin_tools::{make, packs};
use image::RgbaImage;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn apply(args: &ApplyArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let what = preview::apply(&args.folder, &args.image, args.focus.unwrap_or((0.5, 0.5)))?;
    out.result(
        Some(&args.folder),
        "applied",
        json!({"folder": args.folder, "picture": args.image}),
        &format!("applied to {}: {what}", args.folder.display()),
        false,
    );
    Ok(())
}

pub fn revert(folder: &Path, out: &Arc<Out>) -> Result<(), CliError> {
    revert_icon(folder).map_err(|e| preview::apply_error(folder, e))?;
    refresh_shell_icons();
    out.result(
        Some(folder),
        "reverted",
        json!({"folder": folder}),
        &format!("reverted {}", folder.display()),
        false,
    );
    Ok(())
}

/// `RRGGBB`, with or without a leading `#`.
pub fn rgb(hex: &str) -> Result<[u8; 3], CliError> {
    let digits = hex.trim().trim_start_matches('#');
    u32::from_str_radix(digits, 16)
        .ok()
        .filter(|_| digits.len() == 6)
        .map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8])
        .ok_or_else(|| {
            CliError::usage(
                format!("{hex:?} isn't a colour."),
                "A colour is six hex digits, RRGGBB, like 2A9D8F.",
            )
        })
}

pub fn render(args: &RenderArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let focus = args.focus.unwrap_or((0.5, 0.5));
    let (png, what) = match (&args.image, &args.solid) {
        (Some(path), _) => {
            let (img, _) = images::load(path)?;
            let skin = preview::skin(img, focus, path)?;
            (skin.preview_png(args.size), skin.describe())
        }
        (None, Some(hex)) => {
            let [r, g, b] = rgb(hex)?;
            let art = Artwork {
                rgba: RgbaImage::from_pixel(SKIN_WIDTH, SKIN_HEIGHT, image::Rgba([r, g, b, 255])),
                focus,
            };
            (
                render_preview_png(&art, args.size),
                "artwork on FolderSkin's folder",
            )
        }
        (None, None) => {
            return Err(CliError::usage(
                "There is nothing to render.",
                "Give a picture, or a colour with --solid.",
            )
            .fix("folderskin render picture.png --out preview.png"))
        }
    };
    std::fs::write(&args.out, png).map_err(|e| CliError::io("save the preview", &args.out, &e))?;
    out.result(
        Some(&args.out),
        "render",
        json!({"size": args.size, "becomes": what}),
        &format!(
            "wrote {} ({size}×{size}): {what}",
            args.out.display(),
            size = args.size
        ),
        false,
    );
    Ok(())
}

pub fn template(args: &TemplateArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let backdrop = rgb(&args.backdrop)?;
    let (w, h) = (args.width, args.height);
    let cut = compositor::blank_template_cutout(w, h);
    let write = |path: &Path, img: &RgbaImage| {
        std::fs::write(path, folderskin_core::raster::encode_png(img))
            .map_err(|e| CliError::io("save the template", path, &e))
    };
    write(&args.out, &matte::flatten(&cut, backdrop))?;
    out.result(
        Some(&args.out),
        "template",
        json!({"width": w, "height": h}),
        &format!("wrote {} ({w}×{h}): the blank folder", args.out.display()),
        false,
    );
    if let Some(mask) = &args.mask {
        let silhouette = RgbaImage::from_fn(w, h, |x, y| {
            let a = cut.get_pixel(x, y).0[3];
            image::Rgba([a, a, a, 255])
        });
        write(mask, &silhouette)?;
        out.result(
            Some(mask),
            "silhouette",
            json!({"width": w, "height": h}),
            &format!("wrote {} ({w}×{h}): its silhouette", mask.display()),
            false,
        );
    }
    Ok(())
}

pub fn packs(command: PacksCommand, out: &Arc<Out>) -> Result<(), CliError> {
    match command {
        PacksCommand::Check { dir, max_kb } => {
            let report = match max_kb {
                Some(kb) => packs::check_within(&dir, kb * 1024),
                None => packs::check(&dir),
            }
            .map_err(|why| unreadable_packs(&dir, why))?;
            if report.problems.is_empty() {
                out.result(
                    Some(&dir),
                    "packs",
                    json!({"packs": report.packs.len(), "problems": []}),
                    &report.summary(),
                    false,
                );
                return Ok(());
            }
            let mut e = CliError::fixable(
                "pack_problems",
                format!("The packs aren't ready: {}.", report.summary()),
                report.problems.join("\n"),
            );
            e = e.fix("Fix each problem listed, then check again.");
            Err(e)
        }
        PacksCommand::Index { dir } => {
            let report = packs::check(&dir).map_err(|why| unreadable_packs(&dir, why))?;
            for problem in &report.problems {
                out.warn(problem);
            }
            let changes = packs::write_index(&dir, &report).map_err(|why| {
                CliError::fixable("index_failed", "The index couldn't be written.", why)
            })?;
            let unchanged = if changes.is_empty() {
                ", nothing changed"
            } else {
                ""
            };
            let mut lines: Vec<String> = changes
                .removed
                .iter()
                .map(|p| format!("removed {}", p.display()))
                .chain(
                    changes
                        .written
                        .iter()
                        .map(|p| format!("wrote {}", p.display())),
                )
                .collect();
            lines.push(format!("{} indexed{unchanged}", report.totals()));
            out.result(
                Some(&dir),
                "index",
                json!({"written": changes.written, "removed": changes.removed}),
                &lines.join("\n"),
                false,
            );
            Ok(())
        }
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
            // cwebp from the PATH, or the one `ai setup` put beside the models on Windows.
            let cwebp = make::find_cwebp().or_else(folderskin_local::paths::cwebp);
            if cwebp.is_none() {
                out.warn("cwebp isn't installed, so finished folders are saved as PNG, which is bigger (folderskin ai setup installs it on Windows)");
            }
            let opts = make::MakeOptions {
                id,
                name,
                tags,
                author,
                license,
                dir,
                max_bytes: max_kb * 1024,
                cwebp,
                flat_backdrop,
            };
            make_pack(&pictures, &opts, preview.as_deref(), out)
        }
    }
}

fn unreadable_packs(dir: &Path, why: String) -> CliError {
    CliError::fixable("packs_unreadable", "The packs can't be read.", why).fix(format!(
        "Check that {} holds a packs folder, or pass --dir",
        dir.display()
    ))
}

fn make_pack(
    pictures: &[PathBuf],
    opts: &make::MakeOptions,
    preview: Option<&Path>,
    out: &Arc<Out>,
) -> Result<(), CliError> {
    let (folder, made) = make::make(pictures, opts).map_err(|why| {
        CliError::fixable("pack_not_made", "The pack couldn't be made.", why)
            .fix("Nothing was left behind; fix what it says and run it again.")
    })?;
    let mut lines: Vec<String> = made
        .iter()
        .map(|m| {
            format!(
                "{}  {:>4} KB  {}  \"{}\"  from {}",
                if m.folder { "folder " } else { "artwork" },
                m.bytes.div_ceil(1024),
                m.file,
                m.name,
                m.source.display()
            )
        })
        .collect();
    let total: usize = made.iter().map(|m| m.bytes).sum();
    let folders = made.iter().filter(|m| m.folder).count();
    lines.push(format!(
        "wrote {}: {} skins ({folders} finished folders, {} artwork), {} KB",
        folder.display(),
        made.len(),
        made.len() - folders,
        total.div_ceil(1024)
    ));
    if let Some(path) = preview {
        let manifest = std::fs::read(folder.join(folderskin_core::pack::MANIFEST_FILE))
            .map_err(|e| CliError::io("read the new pack", &folder, &e))?;
        let pack = folderskin_core::pack::Pack::parse(&manifest).map_err(|problems| {
            CliError::bug("The new pack doesn't read back.", problems.join("; "))
        })?;
        let sheet = packs::contact_sheet(&folder, &pack, 256, 6).map_err(|why| {
            CliError::fixable(
                "preview_failed",
                "The pack's preview couldn't be drawn.",
                why,
            )
        })?;
        sheet.save(path).map_err(|e| {
            CliError::fixable(
                "preview_failed",
                "The pack's preview couldn't be saved.",
                e.to_string(),
            )
        })?;
        lines.push(format!("preview: {}", path.display()));
    }
    lines.push("Rename the skins in pack.json if their file names don't make good names.".into());
    out.result(
        Some(&folder),
        "pack",
        json!({"skins": made.len(), "folders": folders, "bytes": total}),
        &lines.join("\n"),
        false,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_six_hex_digits() {
        assert_eq!(rgb("#2A9D8F").unwrap(), [0x2A, 0x9D, 0x8F]);
        assert_eq!(rgb("ff00ff").unwrap(), [255, 0, 255]);
        for bad in ["FFF", "FF00FF00", "#GG00FF", "", "teal"] {
            assert_eq!(rgb(bad).unwrap_err().code, "usage", "{bad:?}");
        }
    }
}
