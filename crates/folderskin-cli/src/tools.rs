//! The commands folderskin-tools has always had, on the same library code, with errors that say
//! what to do: `apply`, `revert`, `render`, `template` and `packs`.

use crate::cli::{ApplyArgs, FormatArg, GlyphArgs, RenderArgs, TemplateArgs};
use crate::error::CliError;
use crate::out::Out;
use crate::{images, preview};
use folderskin_core::apply::{apply_icon, has_custom_icon, refresh_shell_icons_now, revert_icon};
use folderskin_core::compositor::{
    self, Artwork, IconSet, Style, ICON_SIZES, SKIN_HEIGHT, SKIN_WIDTH,
};
use folderskin_core::export::{self, Format};
use folderskin_core::glyph::{self, Fill};
use folderskin_core::matte;
use folderskin_tools::cli::PacksCommand;
use folderskin_tools::skin::Skin;
use folderskin_tools::{catalog, make, normalize, packs, rename};
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
        refresh_shell_icons_now();
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
    let style = preview::look();
    let skin = to_render(args)?;
    let what = preview::becomes(&skin, style);
    focus_unused(args.focus, what, out);
    let dest = destination(
        args.out.as_deref(),
        args.format.map(FormatArg::format),
        |f| {
            PathBuf::from(if f == Format::Favicon {
                "favicon"
            } else {
                "preview"
            })
        },
    )?;
    let icons = skin.icon_set_in(&dest.sizes(args.size), style);
    save(&icons, &dest, args.size, "save the preview")?;
    wrote(out, "render", &dest, args.size, what);
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

/// What `render` draws, as the app takes it: the picture given, or artwork of one colour.
fn to_render(args: &RenderArgs) -> Result<Skin, CliError> {
    let focus = args.focus.unwrap_or((0.5, 0.5));
    match (&args.image, &args.solid) {
        (Some(path), _) => {
            let (img, _) = images::load(path)?;
            preview::skin(img, focus, path)
        }
        (None, Some(hex)) => {
            let [r, g, b] = rgb(hex)?;
            Ok(Skin::Artwork(Artwork {
                rgba: RgbaImage::from_pixel(SKIN_WIDTH, SKIN_HEIGHT, image::Rgba([r, g, b, 255])),
                focus,
            }))
        }
        (None, None) => Err(CliError::usage(
            "There is nothing to render.",
            "Give a picture, or a colour with --solid.",
        )
        .fix("folderskin render picture.png --out preview.png")),
    }
}

/// Where `render` or `glyph` writes an icon, and as what.
#[derive(Debug, PartialEq)]
struct Dest {
    path: PathBuf,
    /// The kind of file, or `None` for a picture in whatever format its name asks for: `.webp`,
    /// or a PNG on standard output.
    format: Option<Format>,
}

impl Dest {
    /// The sizes to draw for it: a picture's `size`, or every size its kind holds.
    fn sizes(&self, size: u32) -> Vec<u32> {
        self.format.map_or_else(|| vec![size], |f| f.sizes(size))
    }
}

/// `--out` and `--format` read together. A name that ends as a kind does (`.icns`, `.iconset`)
/// is that kind, and `--format` says the kind where it doesn't, adding the ending. With no
/// `--out`, the icon is named by `default` and the ending of the kind asked for, PNG if none.
fn destination(
    out: Option<&Path>,
    asked: Option<Format>,
    default: impl FnOnce(Format) -> PathBuf,
) -> Result<Dest, CliError> {
    let Some(path) = out else {
        let format = asked.unwrap_or(Format::Png);
        return Ok(Dest {
            path: with_ending(default(format), format),
            format: Some(format),
        });
    };
    match (asked, Format::of(path)) {
        (None, named) => Ok(Dest {
            path: path.to_path_buf(),
            format: named,
        }),
        (Some(asked), Some(named)) if asked != named => Err(CliError::usage(
            "The name and --format don't agree.",
            format!(
                "{} is named as {}, and --format asks for {}.",
                path.display(),
                kind_of(named),
                kind_of(asked)
            ),
        )
        .fix("Change the name's ending, or leave --format out.")),
        (Some(Format::Png), None) if path.as_os_str() == "-" => Ok(Dest {
            path: path.to_path_buf(),
            format: None,
        }),
        (Some(asked), None) if path.as_os_str() == "-" => Err(CliError::usage(
            "Only a PNG can go to standard output.",
            format!("--format asks for {}, and --out is -.", kind_of(asked)),
        )
        .fix("Give it a name instead, such as --out icons")),
        (Some(asked), _) => Ok(Dest {
            path: with_ending(path.to_path_buf(), asked),
            format: Some(asked),
        }),
    }
}

/// `path` with the ending of `format`'s names added, unless it has it already. A website's
/// favicons are a folder named as it is.
fn with_ending(path: PathBuf, format: Format) -> PathBuf {
    match format.extension() {
        Some(ending) if Format::of(&path) != Some(format) => {
            let mut name = path.into_os_string();
            name.push(".");
            name.push(ending);
            name.into()
        }
        _ => path,
    }
}

/// A kind of file, as a sentence names it.
fn kind_of(format: Format) -> &'static str {
    match format {
        Format::Png => "a PNG",
        Format::Jpeg => "a JPEG",
        Format::Icns => "an .icns",
        Format::Ico => "an .ico",
        Format::Iconset => "an .iconset",
        Format::Ios => "an iOS app icon",
        Format::Favicon => "a website's favicons",
    }
}

/// Writes `icons` to `dest`: a picture in the format its name asks for, or the kind of file
/// `dest` names, whose folder is made first.
fn save(icons: &IconSet, dest: &Dest, size: u32, doing: &str) -> Result<(), CliError> {
    let Some(format) = dest.format else {
        let png = icons.png(size).ok_or_else(|| drawn_wrong(size))?;
        return images::write_as_named(&png, &dest.path, doing);
    };
    if !format.is_folder() && dest.path.is_dir() {
        return Err(CliError::folder_not_file(doing, &dest.path));
    }
    if format.is_folder() && dest.path.is_file() {
        return Err(CliError::fixable(
            "not_a_folder",
            format!(
                "{} is a folder of files, and a file has that name.",
                kind_of(format)
            ),
            format!("{} is a file.", dest.path.display()),
        )
        .fix("Give it a name nothing has yet."));
    }
    export::write(icons, format, size, &dest.path).map_err(|e| CliError::io(doing, &dest.path, &e))
}

/// The icon wasn't drawn at a size it was asked for: a bug.
fn drawn_wrong(size: u32) -> CliError {
    CliError::bug(
        "The icon wasn't drawn at the size it was asked for.",
        format!("There was no {size} px image to write."),
    )
}

/// Says what went to `dest`, and returns whether that was standard output.
fn wrote(out: &Arc<Out>, kind: &str, dest: &Dest, size: u32, what: &str) -> bool {
    let (place, stdout) = place(&dest.path);
    let format = dest.format.map_or_else(
        || {
            dest.path
                .extension()
                .map_or("png".into(), |e| e.to_string_lossy().to_ascii_lowercase())
        },
        |f| f.id().to_string(),
    );
    out.result(
        (!stdout).then_some(dest.path.as_path()),
        kind,
        json!({"size": size, "format": format, "becomes": what}),
        &format!("wrote {place} ({}): {what}", holds(dest.format, size)),
        stdout,
    );
    stdout
}

/// What a file of `format` holds, for a report line: a picture's size, or the sizes and files.
fn holds(format: Option<Format>, size: u32) -> String {
    match format {
        None | Some(Format::Png | Format::Jpeg) => format!("{size}×{size}"),
        Some(Format::Icns | Format::Iconset) => "every size from 16 to 1024 px".into(),
        Some(Format::Ico) => "every size from 16 to 256 px".into(),
        Some(Format::Ios) => "every iPhone and iPad size, and the App Store's".into(),
        Some(Format::Favicon) => {
            "favicon.ico, its PNGs, a web manifest and the lines for the page".into()
        }
    }
}

/// `glyph`: a mark pressed into the folder, written to a file, put on a folder, or both.
pub fn glyph(args: &GlyphArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let style = preview::look();
    if args.empty && style != Style::Mac {
        out.warn("--empty changes nothing here: only FolderSkin's folder has paper in it");
    }
    if let Some(folder) = &args.folder {
        if folderskin_core::drive::detect::drive_at(folder).is_some() {
            return Err(mark_on_a_drive(folder));
        }
    }
    let asked = args.format.map(FormatArg::format);
    // Where the icon goes: --out, or beside the picture, unless there is a folder to put it on
    // and no file was asked for. From standard input, it goes to standard output.
    let dest = match (&args.out, &args.folder, asked) {
        (None, Some(_), None) => None,
        (None, None, None) if args.picture.as_os_str() == "-" => Some(Dest {
            path: PathBuf::from("-"),
            format: None,
        }),
        (to, _, asked) => Some(destination(to.as_deref(), asked, |f| {
            beside(&args.picture, f)
        })?),
    };
    let (img, _) = images::load(&args.picture)?;
    let mark = glyph::coverage(&img, !args.no_trim).ok_or_else(|| {
        CliError::fixable(
            "image_empty",
            "There is no mark in that picture to press into a folder.",
            format!("{} is one flat colour all over.", args.picture.display()),
        )
        .fix("Give a picture of a symbol, a logo or a letter on a plain background.")
    })?;
    let fill = match &args.colour {
        Some(hex) => Fill::Solid(rgb(hex)?),
        None => Fill::Plain,
    };
    let design = glyph::design(&mark, fill, style, args.depth as f32);
    let mut sizes = dest.as_ref().map_or_else(Vec::new, |d| d.sizes(args.size));
    if args.folder.is_some() {
        sizes.extend(ICON_SIZES);
    }
    sizes.sort_unstable();
    sizes.dedup();
    let icons = compositor::render_placed_icon_set_with(&design, &sizes, style, !args.empty);
    let what = pressed_into(style);
    let mut stdout = false;
    if let Some(dest) = &dest {
        save(&icons, dest, args.size, "save the icon")?;
        stdout = wrote(out, "glyph", dest, args.size, what);
    }
    if let Some(folder) = &args.folder {
        // Only the sizes the app applies, whatever else the file above took.
        let applied = IconSet {
            sizes: icons
                .sizes
                .into_iter()
                .filter(|(size, _)| ICON_SIZES.contains(size))
                .collect(),
        };
        apply_icon(folder, &applied).map_err(|e| preview::apply_error(folder, e))?;
        refresh_shell_icons_now();
        out.result(
            Some(folder),
            "applied",
            json!({"folder": folder, "picture": args.picture}),
            &format!("applied to {}: {what}", folder.display()),
            stdout,
        );
    }
    Ok(())
}

/// Where a glyph from `picture` goes when nothing else is asked, without its ending: beside it,
/// as `logo-folder` (or `logo-favicon` for a website's favicons).
fn beside(picture: &Path, format: Format) -> PathBuf {
    let stem = match picture.file_stem() {
        Some(stem) if picture.as_os_str() != "-" => stem.to_string_lossy().into_owned(),
        _ => "glyph".into(),
    };
    let suffix = if format == Format::Favicon {
        "favicon"
    } else {
        "folder"
    };
    picture.with_file_name(format!("{stem}-{suffix}"))
}

/// What `glyph` makes, in words, on the folder of `style`.
fn pressed_into(style: Style) -> &'static str {
    match style {
        Style::Mac => "a mark pressed into FolderSkin's folder",
        Style::Windows => "a mark pressed into Windows' folder",
        Style::Linux => "a mark pressed into the Linux folder",
    }
}

/// A mark can't go on a drive's root: the app gives a drive an icon on the drive's own shape.
fn mark_on_a_drive(root: &Path) -> CliError {
    CliError::fixable(
        "glyph_on_drive",
        "A mark goes on a folder, not on a drive.",
        format!(
            "{} is a drive, and FolderSkin draws a drive's icon on the drive's own shape.",
            root.display()
        ),
    )
    .fix("Give a folder on the drive instead.")
    .fix("Or put a picture on the drive itself: folderskin apply <drive> --image picture.png")
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
            require_lossless,
            require_one_shape,
        } => {
            let mut opts = packs::CheckOptions {
                require_generated_ids,
                require_lossless,
                require_one_shape,
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
            keep_outliers,
            drives,
        } => {
            let opts = make::MakeOptions {
                id,
                name,
                tags,
                author,
                license,
                dir,
                max_bytes: max_kb * 1024,
                flat_backdrop,
                keep_outliers,
                shape: if drives {
                    folderskin_core::pack::PackShape::Drive
                } else {
                    folderskin_core::pack::PackShape::Folder
                },
            };
            make_pack(&pictures, &opts, preview.as_deref(), out)
        }
        PacksCommand::Normalize {
            dir,
            ids,
            tolerance,
            drop_outliers,
        } => {
            let opts = normalize::Options {
                tolerance,
                outliers: if drop_outliers {
                    normalize::Outliers::Drop
                } else {
                    normalize::Outliers::Keep
                },
            };
            normalize_packs(&dir, &ids, &opts, out)
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
                out.warn("cwebp isn't installed, so thumbnails are lossless WebP, which is bigger (`brew install webp`, or the webp package on Linux)");
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

/// `packs normalize`: one shape for the finished folders in every pack, or in the packs `ids`
/// names. What it did to each pack is the result; a pack it couldn't do is the error, after it.
fn normalize_packs(
    dir: &Path,
    ids: &[String],
    opts: &normalize::Options,
    out: &Arc<Out>,
) -> Result<(), CliError> {
    let results = normalize::normalize(dir, ids, opts).map_err(|why| unreadable_packs(dir, why))?;
    let packs: Vec<serde_json::Value> = results
        .iter()
        .map(|r| match &r.result {
            Ok(done) => json!({"id": r.id, "result": done}),
            Err(why) => json!({"id": r.id, "error": why}),
        })
        .collect();
    out.result(
        Some(dir),
        "normalize",
        json!({"packs": packs}),
        &normalize::report(&results, opts).join("\n"),
        false,
    );
    let failed: Vec<String> = results
        .iter()
        .filter_map(|r| {
            r.result
                .as_ref()
                .err()
                .map(|why| format!("{}: {why}", r.id))
        })
        .collect();
    if failed.is_empty() {
        return Ok(());
    }
    Err(CliError::fixable(
        "normalize_failed",
        format!(
            "{} couldn't be given one shape, and nothing in {} was changed.",
            crate::ai::count(failed.len(), "pack"),
            if failed.len() == 1 { "it" } else { "them" }
        ),
        failed.join("\n"),
    )
    .fix("Fix what it says and run it again: a pack at one shape already is left as it is."))
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
    let make::MadePack {
        folder,
        made,
        shape,
        left_out: too_far,
    } = make::make(pictures, opts).map_err(|why| {
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
            let scaled = m.scaled_to.map_or(String::new(), |side| {
                format!(", made {side} px to fit {} KB", opts.picture_limit() / 1024)
            });
            format!(
                "{}  {:>4} KB  {}  \"{}\"  from {}{scaled}{}",
                if m.folder { "folder " } else { "artwork" },
                m.bytes.div_ceil(1024),
                m.file,
                m.name,
                m.source.display(),
                m.shape_note()
            )
        })
        .collect();
    lines.extend(too_far.iter().map(make::LeftOut::describe));
    let total: usize = made.iter().map(|m| m.bytes).sum();
    let folders = made.iter().filter(|m| m.folder).count();
    let one_shape = shape.map_or(String::new(), |shape| {
        format!(", given one shape {}", normalize::times_as_wide(shape))
    });
    lines.push(format!(
        "wrote {}: {} ({}{one_shape}, {} artwork), {} KB",
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
    let left_out_json: Vec<serde_json::Value> = too_far
        .iter()
        .map(|l| json!({"source": l.source, "reshaping": l.reshaping}))
        .collect();
    out.result(
        Some(&folder),
        "pack",
        json!({
            "skins": made.len(),
            "folders": folders,
            "bytes": total,
            "shape": shape,
            "left_out": left_out_json,
        }),
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
            out: Some(dir.join("c").join("d").join("p.png")),
            format: None,
            size: 32,
            focus: None,
        };
        render(&r, &out).unwrap();
        for p in [&t.out, t.mask.as_ref().unwrap(), r.out.as_ref().unwrap()] {
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
            out: Some("-".into()),
            format: None,
            size: 64,
            focus: None,
        };
        let drawn = |style| {
            to_render(&args)
                .unwrap()
                .icon_set_in(&[64], style)
                .png(64)
                .unwrap()
        };
        let (mac, windows) = (drawn(Style::Mac), drawn(Style::Windows));
        let art = Artwork {
            rgba: RgbaImage::from_pixel(
                SKIN_WIDTH,
                SKIN_HEIGHT,
                image::Rgba([0x2A, 0x9D, 0x8F, 255]),
            ),
            focus: (0.5, 0.5),
        };
        assert_eq!(
            windows,
            compositor::render_preview_png_in(&art, 64, Style::Windows)
        );
        assert_ne!(mac, windows, "Windows' folder is another shape");
    }

    #[test]
    fn render_writes_an_icon_file_with_every_size_in_it() {
        let dir = std::env::temp_dir().join(format!("fs-render-icns-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = Out::new(true, false);
        for (name, magic) in [("p.icns", &b"icns"[..]), ("p.ICO", &[0, 0, 1, 0][..])] {
            let r = RenderArgs {
                image: None,
                solid: Some("2A9D8F".into()),
                out: Some(dir.join(name)),
                format: None,
                size: 64,
                focus: None,
            };
            render(&r, &out).unwrap();
            let bytes = std::fs::read(r.out.as_ref().unwrap()).unwrap();
            assert!(bytes.starts_with(magic), "{name}");
        }
        assert_eq!(
            holds(Some(Format::Icns), 64),
            "every size from 16 to 1024 px"
        );
        assert_eq!(holds(Some(Format::Ico), 64), "every size from 16 to 256 px");
        assert_eq!(holds(Some(Format::Png), 64), "64×64");
        assert_eq!(holds(None, 64), "64×64");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `glyph`'s options for `picture`, as it runs with none given.
    fn glyph_of(picture: PathBuf) -> GlyphArgs {
        GlyphArgs {
            picture,
            folder: None,
            out: None,
            format: None,
            colour: None,
            depth: 60.0,
            size: 128,
            no_trim: false,
            empty: false,
        }
    }

    #[test]
    fn a_glyph_goes_beside_its_picture_or_where_it_is_asked() {
        let dir = std::env::temp_dir().join(format!("fs-glyph-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = Out::new(true, false);
        // A black disc on white.
        let logo = dir.join("logo.png");
        RgbaImage::from_fn(64, 64, |x, y| {
            let d = (x as i32 - 32).pow(2) + (y as i32 - 32).pow(2);
            image::Rgba(if d < 400 { [0, 0, 0, 255] } else { [255; 4] })
        })
        .save(&logo)
        .unwrap();
        glyph(&glyph_of(logo.clone()), &out).unwrap();
        let beside = image::open(dir.join("logo-folder.png")).unwrap();
        assert_eq!((beside.width(), beside.height()), (128, 128));

        let icns = GlyphArgs {
            out: Some(dir.join("music.icns")),
            colour: Some("2A9D8F".into()),
            empty: true,
            ..glyph_of(logo)
        };
        glyph(&icns, &out).unwrap();
        assert!(std::fs::read(dir.join("music.icns"))
            .unwrap()
            .starts_with(b"icns"));

        let blank = dir.join("blank.png");
        RgbaImage::from_pixel(8, 8, image::Rgba([255; 4]))
            .save(&blank)
            .unwrap();
        assert_eq!(
            glyph(&glyph_of(blank), &out).unwrap_err().code,
            "image_empty"
        );
        let bad_colour = GlyphArgs {
            colour: Some("teal".into()),
            ..glyph_of(dir.join("logo.png"))
        };
        assert_eq!(glyph(&bad_colour, &out).unwrap_err().code, "usage");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn packs_normalize_gives_a_pack_one_shape_and_says_which_it_couldnt() {
        let dir = std::env::temp_dir().join(format!("fs-normalize-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pack = dir.join("packs").join("desk-k7q2mx");
        std::fs::create_dir_all(&pack).unwrap();
        // Two finished folders, 1.2 and 1.15 times as wide as tall.
        let folder = |w: u32| {
            RgbaImage::from_fn(w + 40, 340, |x, y| {
                let inside = (20..20 + w).contains(&x) && (20..320).contains(&y);
                image::Rgba(if inside {
                    [40, 90, 200, 255]
                } else {
                    [0, 0, 0, 0]
                })
            })
        };
        for (file, w) in [("a.png", 360), ("b.png", 345)] {
            let png = folderskin_core::raster::encode_png(&folder(w));
            std::fs::write(pack.join(file), png).unwrap();
        }
        std::fs::write(
            pack.join("pack.json"),
            r#"{ "version": 1, "name": "Desk", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["desk"], "skins": [{ "file": "a.png", "name": "A" }, { "file": "b.png", "name": "B" }] }"#,
        )
        .unwrap();
        let out = Out::new(true, false);
        let normalize = |ids: Vec<String>| {
            let command = PacksCommand::Normalize {
                dir: dir.clone(),
                ids,
                tolerance: 0.08,
                drop_outliers: false,
            };
            packs(command, &out)
        };
        normalize(Vec::new()).unwrap();
        let listed = std::fs::read_to_string(pack.join("pack.json")).unwrap();
        assert!(
            listed.contains("a.webp") && listed.contains("b.webp"),
            "{listed}"
        );
        assert!(!pack.join("a.png").exists());

        let e = normalize(vec!["desk-k7q2mx".into(), "nope-k7q2mx".into()]).unwrap_err();
        assert_eq!(e.code, "normalize_failed");
        assert!(e.why.contains("nope-k7q2mx: there's no pack"), "{}", e.why);
        std::fs::remove_dir_all(&dir).unwrap();
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
            out: Some(dir.join("p.jpg")),
            format: None,
            size: 32,
            focus: None,
        };
        render(&r, &out).unwrap();
        let webp = RenderArgs {
            out: Some(dir.join("p.webp")),
            ..r
        };
        render(&webp, &out).unwrap();
        let format = |p: &Path| image::guess_format(&std::fs::read(p).unwrap()).unwrap();
        assert_eq!(format(&t.out), image::ImageFormat::WebP);
        assert_eq!(format(&dir.join("m.jpg")), image::ImageFormat::Jpeg);
        assert_eq!(format(&dir.join("p.jpg")), image::ImageFormat::Jpeg);
        assert_eq!(format(&dir.join("p.webp")), image::ImageFormat::WebP);
        // A JPEG of an icon is on white, not the key colour.
        let jpeg = image::open(dir.join("p.jpg")).unwrap().to_rgb8();
        assert!(
            jpeg.get_pixel(0, 0).0.iter().all(|&c| c > 240),
            "{:?}",
            jpeg.get_pixel(0, 0)
        );
        // A name no picture format goes by is refused, as the image commands refuse it.
        let gif = RenderArgs {
            out: Some(dir.join("p.gif")),
            ..webp
        };
        assert_eq!(render(&gif, &out).unwrap_err().code, "unknown_format");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_name_or_format_says_where_and_as_what() {
        let default = |f: Format| {
            PathBuf::from(if f == Format::Favicon {
                "favicon"
            } else {
                "preview"
            })
        };
        let dest = |out: Option<&str>, asked: Option<Format>| {
            destination(out.map(Path::new), asked, default)
        };
        let is = |path: &str, format: Option<Format>| Dest {
            path: PathBuf::from(path),
            format,
        };
        // Nothing asked: a PNG called preview.png, or the kind asked for, named after it.
        assert_eq!(
            dest(None, None).unwrap(),
            is("preview.png", Some(Format::Png))
        );
        assert_eq!(
            dest(None, Some(Format::Ios)).unwrap(),
            is("preview.appiconset", Some(Format::Ios))
        );
        assert_eq!(
            dest(None, Some(Format::Favicon)).unwrap(),
            is("favicon", Some(Format::Favicon))
        );
        // The name says the kind, and --format adds the ending it lacks.
        assert_eq!(
            dest(Some("a/b.iconset"), None).unwrap(),
            is("a/b.iconset", Some(Format::Iconset))
        );
        assert_eq!(dest(Some("b.webp"), None).unwrap(), is("b.webp", None));
        assert_eq!(
            dest(Some("Photos"), Some(Format::Icns)).unwrap(),
            is("Photos.icns", Some(Format::Icns))
        );
        assert_eq!(
            dest(Some("My.Photos"), Some(Format::Ico)).unwrap(),
            is("My.Photos.ico", Some(Format::Ico))
        );
        assert_eq!(
            dest(Some("site"), Some(Format::Favicon)).unwrap(),
            is("site", Some(Format::Favicon))
        );
        assert_eq!(
            dest(Some("x.ICNS"), Some(Format::Icns)).unwrap(),
            is("x.ICNS", Some(Format::Icns))
        );
        // A name and a format that disagree, and anything but a PNG on standard output, are refused.
        assert_eq!(
            dest(Some("x.png"), Some(Format::Icns)).unwrap_err().code,
            "usage"
        );
        assert_eq!(
            dest(Some("-"), Some(Format::Iconset)).unwrap_err().code,
            "usage"
        );
        assert_eq!(dest(Some("-"), Some(Format::Png)).unwrap(), is("-", None));
    }

    #[test]
    fn render_writes_every_kind_of_file() {
        let dir = std::env::temp_dir().join(format!("fs-render-kinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = Out::new(true, false);
        for (format, name, check) in [
            (
                FormatArg::Iconset,
                "Photos",
                "Photos.iconset/icon_512x512@2x.png",
            ),
            (
                FormatArg::Ios,
                "AppIcon",
                "AppIcon.appiconset/Contents.json",
            ),
            (FormatArg::Favicon, "site", "site/favicon.ico"),
            (FormatArg::Jpeg, "Photos", "Photos.jpg"),
        ] {
            let r = RenderArgs {
                image: None,
                solid: Some("2A9D8F".into()),
                out: Some(dir.join(name)),
                format: Some(format),
                size: 64,
                focus: None,
            };
            render(&r, &out).unwrap();
            assert!(dir.join(check).is_file(), "{check}");
        }
        // A folder of files can't take the place of a file.
        std::fs::write(dir.join("taken.iconset"), b"x").unwrap();
        let r = RenderArgs {
            image: None,
            solid: Some("2A9D8F".into()),
            out: Some(dir.join("taken.iconset")),
            format: None,
            size: 64,
            focus: None,
        };
        assert_eq!(render(&r, &out).unwrap_err().code, "not_a_folder");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
