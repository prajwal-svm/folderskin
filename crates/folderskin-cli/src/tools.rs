//! The commands folderskin-tools has always had, on the same library code, with errors that say
//! what to do: `apply`, `revert`, `render`, `template` and `packs`.

use crate::cli::{ApplyArgs, RenderArgs, TemplateArgs};
use crate::error::CliError;
use crate::out::Out;
use crate::{images, preview};
use folderskin_core::apply::{has_custom_icon, refresh_shell_icons, revert_icon};
use folderskin_core::compositor::{self, render_preview_png, Artwork, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte;
use folderskin_tools::cli::PacksCommand;
use folderskin_tools::{catalog, make, packs};
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
    // Asked first: a folder with nothing to take off is left exactly as it is, and says so.
    let had = has_custom_icon(folder);
    revert_icon(folder).map_err(|e| preview::apply_error(folder, e))?;
    if had {
        refresh_shell_icons();
    }
    let human = if had {
        format!("reverted {}", folder.display())
    } else {
        format!(
            "nothing to revert: {} doesn't wear an icon FolderSkin can take off",
            folder.display()
        )
    };
    out.result(
        Some(folder),
        "reverted",
        json!({"folder": folder, "changed": had}),
        &human,
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
    images::write_png(&png, &args.out, "save the preview")?;
    let (place, stdout) = place(&args.out);
    out.result(
        (!stdout).then_some(args.out.as_path()),
        "render",
        json!({"size": args.size, "becomes": what}),
        &format!("wrote {place} ({size}×{size}): {what}", size = args.size),
        stdout,
    );
    Ok(())
}

/// How a result's destination reads, and whether it is standard output (`-`).
fn place(path: &Path) -> (String, bool) {
    if path.as_os_str() == "-" {
        ("standard output".into(), true)
    } else {
        (path.display().to_string(), false)
    }
}

pub fn template(args: &TemplateArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let backdrop = rgb(&args.backdrop)?;
    let is_stdout = |p: &Path| p.as_os_str() == "-";
    if is_stdout(&args.out) && args.mask.as_deref().is_some_and(is_stdout) {
        return Err(CliError::usage(
            "Only one picture can go to standard output.",
            "Both --out and --mask are -.",
        )
        .fix("Write the silhouette to a file: --mask mask.png"));
    }
    let (w, h) = (args.width, args.height);
    let cut = compositor::blank_template_cutout(w, h);
    let write = |path: &Path, img: &RgbaImage| {
        images::write_png(
            &folderskin_core::raster::encode_png(img),
            path,
            "save the template",
        )
    };
    write(&args.out, &matte::flatten(&cut, backdrop))?;
    let (place_out, stdout) = place(&args.out);
    out.result(
        (!stdout).then_some(args.out.as_path()),
        "template",
        json!({"width": w, "height": h}),
        &format!("wrote {place_out} ({w}×{h}): the blank folder"),
        stdout,
    );
    if let Some(mask) = &args.mask {
        let silhouette = RgbaImage::from_fn(w, h, |x, y| {
            let a = cut.get_pixel(x, y).0[3];
            image::Rgba([a, a, a, 255])
        });
        write(mask, &silhouette)?;
        let (place_mask, mask_stdout) = place(mask);
        out.result(
            (!mask_stdout).then_some(mask.as_path()),
            "silhouette",
            json!({"width": w, "height": h}),
            &format!("wrote {place_mask} ({w}×{h}): its silhouette"),
            stdout || mask_stdout,
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
                CliError::fixable(
                    "index_failed",
                    "The index couldn't be written.",
                    sentence(&why),
                )
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
        PacksCommand::Catalog {
            dir,
            out: to,
            mirrors,
        } => {
            let report = packs::check(&dir).map_err(|why| unreadable_packs(&dir, why))?;
            for problem in &report.problems {
                out.warn(problem);
            }
            let opts = catalog::CatalogOptions {
                out: to,
                mirrors,
                cwebp: make::find_cwebp().or_else(folderskin_local::paths::cwebp),
                dates: catalog::git_dates(&dir),
            };
            if opts.cwebp.is_none() {
                out.warn("cwebp isn't installed, so thumbnails are lossless WebP, which is bigger (folderskin ai setup installs it on Windows)");
            }
            if opts.dates.is_empty() && !report.packs.is_empty() {
                out.warn("no git history for these packs, so Newest can't tell them apart");
            }
            let built = catalog::write_catalog(&dir, &report, &opts).map_err(|why| {
                CliError::fixable(
                    "catalog_failed",
                    "The catalog couldn't be written.",
                    sentence(&why),
                )
            })?;
            let unchanged = if built.changes.is_empty() {
                ", nothing changed"
            } else {
                ""
            };
            out.result(
                Some(&opts.out),
                "catalog",
                json!({
                    "generation": built.head.generation,
                    "written": built.changes.written,
                    "removed": built.changes.removed,
                }),
                &format!(
                    "{}: generation {}, a {} KB catalog; {} files written, {} removed{unchanged}",
                    report.totals(),
                    built.head.generation,
                    built.head.catalog.bytes.div_ceil(1024),
                    built.changes.written.len(),
                    built.changes.removed.len(),
                ),
                false,
            );
            Ok(())
        }
    }
}

fn unreadable_packs(dir: &Path, why: String) -> CliError {
    let packs = dir.join("packs");
    if !packs.is_dir() {
        return CliError::fixable(
            "packs_unreadable",
            "There are no packs here to read.",
            format!("There is no {} folder.", packs.display()),
        )
        .fix("Run it from a FolderSkin checkout, where they are in community/packs.")
        .fix("Or point it at the folder that holds packs/: --dir <folder>");
    }
    CliError::fixable(
        "packs_unreadable",
        "The packs can't be read.",
        sentence(&why),
    )
    .fix(format!(
        "Check that {} is readable, then run it again.",
        packs.display()
    ))
}

/// folderskin-tools' plain messages as a sentence: a capital letter first, a full stop last.
fn sentence(text: &str) -> String {
    let mut chars = text.trim().chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let s: String = first.to_uppercase().chain(chars).collect();
    if s.ends_with(['.', '!', '?']) {
        s
    } else {
        s + "."
    }
}

fn make_pack(
    pictures: &[PathBuf],
    opts: &make::MakeOptions,
    preview: Option<&Path>,
    out: &Arc<Out>,
) -> Result<(), CliError> {
    let (folder, made) = make::make(pictures, opts).map_err(|why| {
        CliError::fixable(
            "pack_not_made",
            "The pack couldn't be made.",
            sentence(&why),
        )
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

    #[test]
    fn no_packs_folder_is_said_in_a_sentence() {
        let dir = std::env::temp_dir().join(format!("fs-no-packs-{}", std::process::id()));
        let e = unreadable_packs(&dir, "couldn't read x: os error 3".into());
        assert_eq!(e.what, "There are no packs here to read.");
        assert_eq!(
            e.why,
            format!("There is no {} folder.", dir.join("packs").display())
        );
        assert!(e.fix.iter().any(|f| f.contains("--dir")));
        assert_eq!(sentence("couldn't read it"), "Couldn't read it.");
        assert_eq!(sentence("Done!"), "Done!");
    }

    #[test]
    fn render_and_template_make_the_folder_they_write_into() {
        let dir = std::env::temp_dir().join(format!("fs-render-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = Out::new(true, false);
        let t = TemplateArgs {
            out: dir.join("a").join("t.png"),
            width: 64,
            height: 64,
            backdrop: "FF00FF".into(),
            mask: Some(dir.join("b").join("m.png")),
        };
        template(&t, &out).unwrap();
        let r = RenderArgs {
            image: Some(t.out.clone()),
            solid: None,
            out: dir.join("c").join("d").join("p.png"),
            size: 32,
            focus: None,
        };
        render(&r, &out).unwrap();
        for p in [&t.out, t.mask.as_ref().unwrap(), &r.out] {
            assert!(image::open(p).is_ok(), "{}", p.display());
        }
        let both = TemplateArgs {
            out: "-".into(),
            mask: Some("-".into()),
            ..t
        };
        assert_eq!(template(&both, &out).unwrap_err().code, "usage");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
