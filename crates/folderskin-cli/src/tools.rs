//! The commands folderskin-tools has always had, on the same library code, with errors that say
//! what to do: `apply`, `revert`, `render`, `template` and `packs`.

use crate::cli::{ApplyArgs, RenderArgs, TemplateArgs};
use crate::error::CliError;
use crate::out::Out;
use crate::{images, preview};
use folderskin_core::apply::{has_custom_icon, refresh_shell_icons, revert_icon};
use folderskin_core::compositor::{
    self, render_preview_png_in, Artwork, Style, SKIN_HEIGHT, SKIN_WIDTH,
};
use folderskin_core::matte;
use folderskin_tools::cli::PacksCommand;
use folderskin_tools::{catalog, make, packs, rename};
use image::RgbaImage;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn apply(args: &ApplyArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let what = preview::apply(&args.folder, &args.image, args.focus.unwrap_or((0.5, 0.5)))?;
    focus_unused(args.focus, what, out);
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
    let (png, what) = rendered(args, preview::look())?;
    focus_unused(args.focus, what, out);
    images::write_as_named(&png, &args.out, "save the preview")?;
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

/// Says that `--focus` was left out when the picture turned out to be a finished folder, which is
/// used as it is and not cropped.
fn focus_unused(focus: Option<(f32, f32)>, became: &str, out: &Arc<Out>) {
    if focus.is_some() && became == FINISHED {
        out.warn("--focus is left out: a finished folder is used as it is, not cropped");
    }
}

/// What a finished folder becomes, as [`preview::becomes`] says it.
const FINISHED: &str = "a finished folder, used as it is";

/// The PNG `render` writes, with artwork on the folder of `style`, and what it became.
fn rendered(args: &RenderArgs, style: Style) -> Result<(Vec<u8>, &'static str), CliError> {
    let focus = args.focus.unwrap_or((0.5, 0.5));
    Ok(match (&args.image, &args.solid) {
        (Some(path), _) => {
            let (img, _) = images::load(path)?;
            let skin = preview::skin(img, focus, path)?;
            (
                skin.preview_png_in(args.size, style),
                preview::becomes(&skin, style),
            )
        }
        (None, Some(hex)) => {
            let [r, g, b] = rgb(hex)?;
            let art = Artwork {
                rgba: RgbaImage::from_pixel(SKIN_WIDTH, SKIN_HEIGHT, image::Rgba([r, g, b, 255])),
                focus,
            };
            let becomes = match style {
                Style::Mac => "artwork on FolderSkin's folder",
                Style::Windows => "artwork on Windows' folder",
            };
            (render_preview_png_in(&art, args.size, style), becomes)
        }
        (None, None) => {
            return Err(CliError::usage(
                "There is nothing to render.",
                "Give a picture, or a colour with --solid.",
            )
            .fix("folderskin render picture.png --out preview.png"))
        }
    })
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
        images::write_as_named(
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
        PacksCommand::Check {
            dir,
            max_kb,
            require_generated_ids,
        } => {
            let mut opts = packs::CheckOptions {
                require_generated_ids,
                ..packs::CheckOptions::default()
            };
            if let Some(kb) = max_kb {
                opts.max_bytes = kb * 1024;
            }
            let report =
                packs::check_with(&dir, &opts).map_err(|why| unreadable_packs(&dir, why))?;
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
            // Each pack is dated by the commit that added it, as the catalog dates them.
            let dates = catalog::git_dates(&dir, &report.moved);
            if dates.is_empty() && !report.packs.is_empty() {
                out.warn(
                    "no git history for these packs, so index.json can't say when each was added",
                );
            }
            let changes = packs::write_index(&dir, &report, &dates).map_err(|why| {
                CliError::fixable(
                    "index_failed",
                    "The index couldn't be written.",
                    sentence(&why),
                )
                .fix(format!(
                    "Check that {} can be written to, then run it again.",
                    dir.display()
                ))
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
        PacksCommand::Rename { dir, all, id, to } => rename_packs(&dir, all, id, to, out),
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
                out: folderskin_tools::cli::catalog_out(&dir, to),
                mirrors,
                cwebp: make::find_cwebp().or_else(folderskin_local::paths::cwebp),
                dates: catalog::git_dates(&dir, &report.moved),
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
                .fix(format!(
                    "Check that {} can be written to, then run it again.",
                    opts.out.display()
                ))
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

/// `packs rename`: generated ids for every pack that lacks one (`all`), or for pack `id`.
fn rename_packs(
    dir: &Path,
    all: bool,
    id: Option<String>,
    to: Option<String>,
    out: &Arc<Out>,
) -> Result<(), CliError> {
    let which = match (all, id) {
        (_, Some(id)) => rename::Which::One { id, to },
        (true, None) => rename::Which::All,
        (false, None) => {
            return Err(CliError::usage(
                "There is nothing to rename.",
                "Name a pack, or rename every pack with --all.",
            )
            .fix("folderskin packs rename --all"))
        }
    };
    let renamed = rename::rename(dir, &which).map_err(|why| {
        CliError::fixable(
            "rename_failed",
            "The packs couldn't be renamed.",
            sentence_about(&why, &[dir]),
        )
        .fix("Fix what it says and run it again: packs that moved already are left alone.")
    })?;
    let mut lines: Vec<String> = renamed
        .moved
        .iter()
        .map(|(old, new)| format!("moved {old} to {new}"))
        .chain(
            renamed
                .earlier
                .iter()
                .map(|(old, now)| format!("{old} moved to {now} already")),
        )
        .chain(
            renamed
                .written
                .iter()
                .map(|p| format!("wrote {}", p.display())),
        )
        .collect();
    lines.push(if renamed.moved.is_empty() {
        "nothing to rename".into()
    } else {
        format!(
            "renamed {} and staged it all: check the packs, then commit",
            crate::ai::count(renamed.moved.len(), "pack")
        )
    });
    let moves = |list: &[(String, String)]| {
        list.iter()
            .map(|(from, to)| json!({"from": from, "to": to}))
            .collect::<Vec<_>>()
    };
    out.result(
        Some(dir),
        "rename",
        json!({
            "moved": moves(&renamed.moved),
            "earlier": moves(&renamed.earlier),
            "kept": renamed.kept,
            "written": renamed.written,
        }),
        &lines.join("\n"),
        false,
    );
    Ok(())
}

fn unreadable_packs(dir: &Path, why: String) -> CliError {
    let packs = dir.join("packs");
    if !packs.is_dir() {
        return CliError::fixable(
            "packs_unreadable",
            "There are no packs here to read.",
            format!("There is no {} folder.", packs.display()),
        )
        .fix("Run it inside a checkout of github.com/prajwal-svm/folderskin-community, which holds packs/.")
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
    sentence_about(text, &[])
}

/// [`sentence`], except that a message starting with a path keeps it as it was written: one of
/// the `given` paths the command was handed, or a first word that is plainly a path. Capitalised,
/// `community\packs\x` read `Community\packs\x` and a picture called `nope` became `Nope`.
pub(crate) fn sentence_about(text: &str, given: &[&Path]) -> String {
    let text = text.trim();
    let first_word = text.split_whitespace().next().unwrap_or("");
    let starts_with_path = first_word.contains(['/', '\\'])
        || given.iter().any(|p| {
            let p = p.display().to_string();
            !p.is_empty() && text.starts_with(&p)
        });
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let s: String = if starts_with_path {
        text.to_string()
    } else {
        first.to_uppercase().chain(chars).collect()
    };
    if s.ends_with(['.', '!', '?']) {
        s
    } else {
        s + "."
    }
}

/// Pictures in the folders given that a pack leaves out: a pack takes PNG, JPEG and WebP only.
fn left_out(pictures: &[PathBuf]) -> Vec<String> {
    const OTHER_PICTURES: [&str; 11] = [
        "bmp", "gif", "tif", "tiff", "heic", "heif", "avif", "jxl", "ico", "svg", "psd",
    ];
    let mut names: Vec<String> = pictures
        .iter()
        .filter(|p| p.is_dir())
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flat_map(|entries| entries.flatten().map(|e| e.path()))
        .filter(|p| {
            let ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            p.is_file() && OTHER_PICTURES.contains(&ext.as_str())
        })
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

fn make_pack(
    pictures: &[PathBuf],
    opts: &make::MakeOptions,
    preview: Option<&Path>,
    out: &Arc<Out>,
) -> Result<(), CliError> {
    let skipped = left_out(pictures);
    if !skipped.is_empty() {
        out.warn(&format!(
            "left out {}: a pack takes PNG, JPEG and WebP pictures",
            skipped.join(", ")
        ));
    }
    let (folder, made) = make::make(pictures, opts).map_err(|why| {
        let given: Vec<&Path> = pictures
            .iter()
            .map(PathBuf::as_path)
            .chain([opts.dir.as_path()])
            .collect();
        CliError::fixable(
            "pack_not_made",
            "The pack couldn't be made.",
            sentence_about(&why, &given),
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
        "wrote {}: {} ({}, {} artwork), {} KB",
        folder.display(),
        crate::ai::count(made.len(), "skin"),
        crate::ai::count(folders, "finished folder"),
        made.len() - folders,
        total.div_ceil(1024)
    ));
    if opts.id.is_none() {
        let id = folder.file_name().unwrap_or_default().to_string_lossy();
        lines.push(format!(
            "its id is {id}, which stays the same whatever the pack is called later"
        ));
    }
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
    fn a_pack_says_which_pictures_it_leaves_out() {
        let dir = std::env::temp_dir().join(format!("fs-left-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "a.png",
            "b.jpg",
            "app.bmp",
            "c.GIF",
            ".hidden.bmp",
            "notes.txt",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        assert_eq!(left_out(std::slice::from_ref(&dir)), ["app.bmp", "c.GIF"]);
        // A picture named on its own is the pack maker's to take or refuse.
        assert!(left_out(&[dir.join("app.bmp")]).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
        // A path keeps its own letters.
        assert_eq!(
            sentence("community\\packs\\uat-test exists already; pick another id or remove it"),
            "community\\packs\\uat-test exists already; pick another id or remove it."
        );
        assert_eq!(
            sentence_about("nope doesn't exist", &[Path::new("nope")]),
            "nope doesn't exist."
        );
        assert_eq!(
            sentence_about(
                "community/packs/x exists already",
                &[Path::new("community")]
            ),
            "community/packs/x exists already."
        );
        assert_eq!(
            sentence_about("there are no pictures there", &[Path::new("nope")]),
            "There are no pictures there."
        );
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

    #[test]
    fn render_draws_artwork_on_the_folder_chosen() {
        let args = RenderArgs {
            image: None,
            solid: Some("2A9D8F".into()),
            out: "-".into(),
            size: 64,
            focus: None,
        };
        let (mac, _) = rendered(&args, Style::Mac).unwrap();
        let (windows, _) = rendered(&args, Style::Windows).unwrap();
        let art = Artwork {
            rgba: RgbaImage::from_pixel(
                SKIN_WIDTH,
                SKIN_HEIGHT,
                image::Rgba([0x2A, 0x9D, 0x8F, 255]),
            ),
            focus: (0.5, 0.5),
        };
        assert_eq!(windows, render_preview_png_in(&art, 64, Style::Windows));
        assert_ne!(mac, windows, "Windows' folder is another shape");
    }

    #[test]
    fn render_and_template_write_the_format_the_name_asks_for() {
        let dir = std::env::temp_dir().join(format!("fs-render-format-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = Out::new(true, false);
        let t = TemplateArgs {
            out: dir.join("t.webp"),
            width: 64,
            height: 64,
            backdrop: "FF00FF".into(),
            mask: Some(dir.join("m.jpg")),
        };
        template(&t, &out).unwrap();
        let r = RenderArgs {
            image: Some(t.out.clone()),
            solid: None,
            out: dir.join("p.jpg"),
            size: 32,
            focus: None,
        };
        render(&r, &out).unwrap();
        let format = |p: &Path| image::guess_format(&std::fs::read(p).unwrap()).unwrap();
        assert_eq!(format(&t.out), image::ImageFormat::WebP);
        assert_eq!(format(&dir.join("m.jpg")), image::ImageFormat::Jpeg);
        assert_eq!(format(&r.out), image::ImageFormat::Jpeg);
        // A name no picture format goes by is refused, as the image commands refuse it.
        let gif = RenderArgs {
            out: dir.join("p.gif"),
            ..r
        };
        assert_eq!(render(&gif, &out).unwrap_err().code, "unknown_format");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
