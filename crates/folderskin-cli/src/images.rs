//! `folderskin image …`: look at a picture, clean it up and adjust it. Every command reads a file
//! or standard input (`-`) and writes a file or standard output (`--out -`), so they chain:
//!
//! ```text
//! folderskin image trim raw.png --out - | folderskin image saturate - 20 --out final.png
//! ```

use crate::check::{self, Verdict};
use crate::cli::{AmountArgs, CropArgs, CutoutArgs, ImageCommand, OneImage};
use crate::error::{shell_path, CliError};
use crate::out::Out;
use crate::{preview, tools};
use folderskin_core::adjust::{self, Fx};
use folderskin_core::compositor::{SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte::{self, Surround, MAGENTA};
use folderskin_core::painted;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::{ImageEncoder, RgbaImage};
use serde_json::json;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn run(command: ImageCommand, out: &Arc<Out>) -> Result<(), CliError> {
    match command {
        ImageCommand::Crop(args) => crop(&args, out),
        ImageCommand::Trim(args) => trim(&args, out),
        ImageCommand::Clip(args) => clip(&args, out),
        ImageCommand::Cutout(args) => cutout(&args, out),
        ImageCommand::Check { input } => check(&input, out),
        ImageCommand::Saturate(a) => amount(&a, out, "saturate", |v| Fx {
            saturation: v,
            ..Fx::default()
        }),
        ImageCommand::Brightness(a) => amount(&a, out, "brightness", |v| Fx {
            brightness: v,
            ..Fx::default()
        }),
        ImageCommand::Contrast(a) => amount(&a, out, "contrast", |v| Fx {
            contrast: v,
            ..Fx::default()
        }),
        ImageCommand::Invert(a) => adjusted(
            &a.image,
            Fx {
                invert: a.amount,
                ..Fx::default()
            },
            "invert",
            out,
        ),
        ImageCommand::Adjust(a) => {
            let fx = Fx {
                brightness: a.brightness.unwrap_or(0.0),
                contrast: a.contrast.unwrap_or(0.0),
                saturation: a.saturation.unwrap_or(0.0),
                hue: a.hue.unwrap_or(0.0),
                grayscale: a.grayscale.unwrap_or(0.0),
                sepia: a.sepia.unwrap_or(0.0),
                invert: a.invert.unwrap_or(0.0),
            };
            if fx.is_none() {
                return Err(CliError::usage(
                    "There is nothing to adjust.",
                    "No adjustment was given.",
                )
                .fix("Name at least one, e.g. folderskin image adjust in.png --saturation 20 --hue -10"));
            }
            adjusted(&a.image, fx, "adjusted", out)
        }
        ImageCommand::Info { input } => info(&input, out),
        ImageCommand::Render(args) => tools::render(&args, out),
        ImageCommand::Template(args) => tools::template(&args, out),
    }
}

fn is_stdio(path: &Path) -> bool {
    path.as_os_str() == "-"
}

/// A picture from a file, or from standard input for `-`, with its size in bytes.
pub fn load(path: &Path) -> Result<(RgbaImage, usize), CliError> {
    load_with_bytes(path).map(|(img, bytes)| (img, bytes.len()))
}

/// [`load`], keeping the file's bytes, which say what format it is when standard input can't.
fn load_with_bytes(path: &Path) -> Result<(RgbaImage, Vec<u8>), CliError> {
    if !is_stdio(path) {
        if path.is_dir() {
            return Err(CliError::folder_not_file("read the picture", path));
        }
        let bytes = std::fs::read(path).map_err(|e| CliError::io("read the picture", path, &e))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| preview::unreadable(path, &e))?
            .to_rgba8();
        return Ok((img, bytes));
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|e| CliError::io("read the picture", Path::new("standard input"), &e))?;
    if bytes.is_empty() {
        return Err(CliError::fixable(
            "image_unreadable",
            "No picture came in.",
            "Standard input was empty.",
        )
        .fix("Pipe a picture in, e.g. folderskin image trim a.png --out - | folderskin image check -"));
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| preview::unreadable(Path::new("standard input"), &e))?
        .to_rgba8();
    Ok((img, bytes))
}

/// A file's size as a person reads it: kilobytes below a megabyte, so a small picture isn't
/// "0.00 MB".
pub fn file_size(bytes: usize) -> String {
    if bytes < 1_000_000 {
        format!("{} KB", bytes.div_ceil(1000))
    } else {
        format!("{:.1} MB", bytes as f64 / 1e6)
    }
}

/// Where a command's picture goes: the `--out` given, standard output when the picture came from
/// standard input, or `<name>-<suffix>.png` beside the input.
pub fn target(input: &Path, out: Option<&Path>, suffix: &str) -> PathBuf {
    match out {
        Some(p) => p.to_path_buf(),
        None if is_stdio(input) => PathBuf::from("-"),
        None => {
            let stem = input.file_stem().unwrap_or_default().to_string_lossy();
            input.with_file_name(format!("{stem}-{suffix}.png"))
        }
    }
}

/// PNG bytes: RGB when every pixel is opaque, which is smaller; RGBA otherwise.
pub fn encode_png(img: &RgbaImage) -> Vec<u8> {
    if img.pixels().any(|p| p.0[3] < 255) {
        return folderskin_core::raster::encode_png(img);
    }
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let mut buf = Vec::new();
    let _ = PngEncoder::new_with_quality(&mut buf, CompressionType::Best, PngFilter::Adaptive)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        );
    buf
}

/// Writes `bytes`, a PNG, to `target`: standard output for `-`, otherwise the file, making its
/// folder first.
pub fn write_png(bytes: &[u8], target: &Path, doing: &str) -> Result<(), CliError> {
    if is_stdio(target) {
        let mut stdout = std::io::stdout().lock();
        return stdout
            .write_all(bytes)
            .and_then(|()| stdout.flush())
            .map_err(|e| CliError::io(doing, Path::new("standard output"), &e));
    }
    if target.is_dir() {
        return Err(CliError::folder_not_file(doing, target));
    }
    make_parent(target)?;
    std::fs::write(target, bytes).map_err(|e| CliError::io(doing, target, &e))
}

/// Writes `png`, a rendered PNG, to `target` in the format its name asks for, as [`save`] does:
/// the PNG as it is for a `.png` name (or none, or standard output), encoded again for `.jpg` and
/// `.webp`, and refused for any other.
pub fn write_as_named(png: &[u8], target: &Path, doing: &str) -> Result<(), CliError> {
    let named_png = is_stdio(target)
        || image::ImageFormat::from_path(target).map_or(true, |f| f == image::ImageFormat::Png);
    if named_png {
        return write_png(png, target, doing);
    }
    let img = image::load_from_memory(png)
        .map_err(|e| CliError::bug("A rendered picture didn't read back.", e.to_string()))?
        .to_rgba8();
    save(&img, target)
}

fn make_parent(target: &Path) -> Result<(), CliError> {
    if let Some(dir) = target.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| CliError::io("make the folder", dir, &e))?;
    }
    Ok(())
}

/// Writes `img` to `target`: standard output for `-` (always PNG), otherwise in the format the
/// file name asks for.
pub fn save(img: &RgbaImage, target: &Path) -> Result<(), CliError> {
    if is_stdio(target) {
        return write_png(&encode_png(img), target, "write the picture");
    }
    if target.is_dir() {
        return Err(CliError::folder_not_file("save the picture", target));
    }
    make_parent(target)?;
    let format = image::ImageFormat::from_path(target).unwrap_or(image::ImageFormat::Png);
    let written = match format {
        image::ImageFormat::Png => {
            std::fs::write(target, encode_png(img)).map_err(image::ImageError::IoError)
        }
        // JPEG has no transparency. Dropping the alpha would leave whatever colour the clear
        // pixels happen to hold, black as often as not; on the key colour instead, a cut-out
        // folder stays a folder the app cuts out again.
        image::ImageFormat::Jpeg => image::DynamicImage::ImageRgba8(matte::flatten(img, MAGENTA))
            .to_rgb8()
            .save_with_format(target, format),
        image::ImageFormat::WebP => img.save_with_format(target, format),
        _ => {
            return Err(CliError::fixable(
                "unknown_format",
                "FolderSkin can't write that kind of file.",
                format!("{} isn't a .png, .jpg or .webp name.", target.display()),
            )
            .fix("End the name with .png (best for folders), .jpg or .webp."))
        }
    };
    written.map_err(|e| match e {
        image::ImageError::IoError(io) => CliError::io("save the picture", target, &io),
        other => CliError::bug("The picture couldn't be encoded.", other.to_string()),
    })
}

/// Saves the result and says what happened. The message goes to standard error when the picture
/// itself is on standard output.
fn done(
    img: &RgbaImage,
    target: &Path,
    what: &str,
    out: &Arc<Out>,
    meta: serde_json::Value,
) -> Result<(), CliError> {
    save(img, target)?;
    let stdout = is_stdio(target);
    let place = if stdout {
        "standard output".to_string()
    } else {
        target.display().to_string()
    };
    let mut meta = meta;
    meta["width"] = json!(img.width());
    meta["height"] = json!(img.height());
    out.result(
        (!stdout).then_some(target),
        "image",
        meta,
        &format!("wrote {place} ({}×{}): {what}", img.width(), img.height()),
        stdout,
    );
    Ok(())
}

fn crop(args: &CropArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, _) = load(&args.image.input)?;
    let (w, h) = img.dimensions();
    let dest = target(&args.image.input, args.image.out.as_deref(), "cropped");
    if let Some((x, y, bw, bh)) = args.crop_box {
        if x.saturating_add(bw) > w || y.saturating_add(bh) > h {
            return Err(CliError::fixable(
                "box_outside",
                "That box goes outside the picture.",
                format!(
                    "The picture is {w} × {h}; the box ends at {} × {}.",
                    u64::from(x) + u64::from(bw),
                    u64::from(y) + u64::from(bh)
                ),
            )
            .fix("Keep X + WIDTH and Y + HEIGHT within the picture."));
        }
        let cut = image::imageops::crop_imm(&img, x, y, bw, bh).to_image();
        return done(
            &cut,
            &dest,
            &format!("the box at {x},{y}"),
            out,
            json!({"box": [x, y, bw, bh]}),
        );
    }
    let focus = args.focus.unwrap_or((0.5, 0.5));
    let (cut, what) = match args.aspect.to_lowercase().as_str() {
        "artwork" => (
            matte::crop_to_aspect(&img, SKIN_WIDTH, SKIN_HEIGHT, focus),
            "the folder's artwork shape, 1024 × 958".to_string(),
        ),
        "square" => (crop_aspect(&img, 1.0, focus), "square".to_string()),
        other => {
            let ratio = other
                .split_once(':')
                .and_then(|(a, b)| {
                    Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?))
                })
                .filter(|(a, b)| *a > 0.0 && *b > 0.0)
                .ok_or_else(|| {
                    CliError::usage(
                        format!("{other:?} isn't a shape FolderSkin can crop to."),
                        "An aspect is artwork, square, or width:height such as 16:9.",
                    )
                })?;
            (
                crop_aspect(&img, ratio.0 / ratio.1, focus),
                other.to_string(),
            )
        }
    };
    done(
        &cut,
        &dest,
        &what,
        out,
        json!({"aspect": args.aspect, "focus": [focus.0, focus.1]}),
    )
}

/// `img` cropped (not resized) to `aspect` (width over height) around `focus`.
fn crop_aspect(img: &RgbaImage, aspect: f64, focus: (f32, f32)) -> RgbaImage {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let (cw, ch) = if w / h > aspect {
        ((h * aspect).round().max(1.0), h)
    } else {
        (w, (w / aspect).round().max(1.0))
    };
    let x0 = ((w - cw) * focus.0.clamp(0.0, 1.0) as f64).round() as u32;
    let y0 = ((h - ch) * focus.1.clamp(0.0, 1.0) as f64).round() as u32;
    image::imageops::crop_imm(img, x0, y0, cw as u32, ch as u32).to_image()
}

/// Whether the app will take `img` as a finished folder (cut out, or on the key colour) rather
/// than artwork to wrap onto its folder. Around a finished folder is backdrop, never paper.
fn is_finished_folder(img: &RgbaImage) -> bool {
    matte::finished_cutout(img, MAGENTA).is_some()
}

fn trim(args: &OneImage, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, _) = load(&args.input)?;
    let dest = target(&args.input, args.out.as_deref(), "trimmed");
    if is_finished_folder(&img) {
        // Its backdrop would read as a paper margin, and cutting it away would cut up the folder.
        return done(
            &img,
            &dest,
            "left unchanged: it is a finished folder, and what surrounds it is its backdrop, not \
             paper (folderskin image clip or image cutout take a backdrop away)",
            out,
            json!({"border": null, "finished_folder": true}),
        );
    }
    match painted::trim_paper(&img) {
        Some((b, trimmed)) => done(
            &trimmed,
            &dest,
            &format!(
                "cut away its paper ({}) and filled the frame again",
                b.describe()
            ),
            out,
            json!({"border": [b.top, b.bottom, b.left, b.right]}),
        ),
        None => done(
            &img,
            &dest,
            "no paper margin found, so it is unchanged",
            out,
            json!({"border": null}),
        ),
    }
}

fn clip(args: &OneImage, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, _) = load(&args.input)?;
    let (w, h) = img.dimensions();
    let dest = target(&args.input, args.out.as_deref(), "clipped");
    // Already cut out along FolderSkin's outline, as `image check` finds it: there is nothing to
    // cut. The cut below expects the template's own frame around the folder, so it would take a
    // trimmed cut-out for a folder of another shape.
    if matte::surround(&img, MAGENTA) == Surround::Transparent {
        if let Some(cut) = matte::finished_cutout(&img, MAGENTA) {
            let fit = check::silhouette_fit(&cut);
            if fit >= painted::MIN_SILHOUETTE_FIT {
                return done(
                    &cut,
                    &dest,
                    &format!(
                        "already cut out along FolderSkin's silhouette (fit {fit:.2}), so only \
                         trimmed to it"
                    ),
                    out,
                    json!({"fit": fit, "already_cut_out": true}),
                );
            }
        }
    }
    let silhouette = folderskin_local::generate::folder_silhouette(w, h);
    let cut = painted::cut_along_silhouette(&img, &silhouette);
    match cut.image {
        Some(clipped) => done(
            &clipped,
            &dest,
            &format!("cut out along FolderSkin's silhouette (fit {:.3})", cut.fit),
            out,
            json!({"fit": cut.fit}),
        ),
        None => {
            let why = if cut.fit == 0.0 {
                "Nothing in it stands out from its backdrop.".to_string()
            } else {
                format!(
                    "Only {:.0}% of it lines up with FolderSkin's silhouette; below 95% a cut along \
                     ours would cut into the painting.",
                    cut.fit * 100.0
                )
            };
            let input = shell_path(&args.input);
            Err(CliError::fixable(
                "folder_reshaped",
                "That isn't FolderSkin's folder shape.",
                why,
            )
            .fix(format!(
                "If it is artwork, it needs no cutting: the app wraps it onto its folder (see it with folderskin render {input})"
            ))
            .fix(format!(
                "If it is a folder on a flat colour, key the colour out instead: folderskin image cutout {input} --flat-backdrop"
            ))
            .fix("If a model painted it, paint it again with another seed."))
        }
    }
}

fn cutout(args: &CutoutArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, _) = load(&args.image.input)?;
    if matte::alpha_bounds(&img, 8).is_none() {
        // Trimming nothing to its subject would write the same empty picture and call it done.
        return Err(CliError::fixable(
            "image_empty",
            "There is nothing in that picture to cut out.",
            format!("{} is completely transparent.", args.image.input.display()),
        )
        .fix("Use another picture."));
    }
    let dest = target(&args.image.input, args.image.out.as_deref(), "cutout");
    let (cut, what) = if args.flat_backdrop {
        let Some(key) = matte::flat_backdrop(&img) else {
            return Err(no_backdrop(
                &args.image.input,
                "It isn't on one flat colour.",
                true,
            ));
        };
        (
            matte::cutout_connected(&img, key),
            format!(
                "keyed out its flat #{:02X}{:02X}{:02X} backdrop",
                key[0], key[1], key[2]
            ),
        )
    } else {
        match matte::surround(&img, MAGENTA) {
            Surround::Keyed => (
                matte::cutout(&img, MAGENTA, matte::KeyOptions::default()),
                "keyed out its magenta backdrop".to_string(),
            ),
            Surround::Transparent => (
                matte::autocrop(&img, 0),
                "trimmed to its subject".to_string(),
            ),
            Surround::Opaque => {
                return Err(no_backdrop(
                    &args.image.input,
                    "It isn't on the magenta key colour (#FF00FF).",
                    false,
                ))
            }
        }
    };
    done(&cut, &dest, &what, out, json!({}))
}

/// Nothing to cut away from `input`, and what to try instead: never the `--flat-backdrop` just
/// tried (`flat`), which would only fail the same way.
fn no_backdrop(input: &Path, why: &str, flat: bool) -> CliError {
    let input = shell_path(input);
    let mut e = CliError::fixable("no_backdrop", "There is no backdrop to cut away.", why);
    if !flat {
        e = e.fix(format!(
            "For a backdrop of another flat colour: folderskin image cutout {input} --flat-backdrop"
        ));
    }
    e = e.fix(format!(
        "For a folder repainted from FolderSkin's template: folderskin image clip {input}"
    ));
    if flat {
        e = e.fix(format!(
            "If it is artwork, it needs no cutting: the app wraps it onto its folder (see it with folderskin render {input})"
        ));
    }
    e
}

fn amount(
    args: &AmountArgs,
    out: &Arc<Out>,
    what: &str,
    fx: impl Fn(f64) -> Fx,
) -> Result<(), CliError> {
    let one = OneImage {
        input: args.input.clone(),
        out: args.out.clone(),
    };
    adjusted(&one, fx(args.amount), what, out)
}

fn adjusted(args: &OneImage, fx: Fx, what: &str, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, _) = load(&args.input)?;
    let suffix = what.split_whitespace().next().unwrap_or("adjusted");
    let dest = target(&args.input, args.out.as_deref(), suffix);
    let (result, cut_out) = adjust_picture(&img, &fx);
    let settings: Vec<String> = [
        ("brightness", fx.brightness),
        ("contrast", fx.contrast),
        ("saturation", fx.saturation),
        ("hue", fx.hue),
        ("grayscale", fx.grayscale),
        ("sepia", fx.sepia),
        ("invert", fx.invert),
    ]
    .iter()
    .filter(|(_, v)| *v != 0.0)
    .map(|(k, v)| format!("{k} {v:+}"))
    .collect();
    let mut said = settings.join(", ");
    if said.is_empty() {
        said.push_str("unchanged: the amount is 0");
    }
    if cut_out {
        said.push_str(
            " (a finished folder on the key colour: cut out first, so the backdrop stays one the \
             app recognises)",
        );
    }
    done(
        &result,
        &dest,
        &said,
        out,
        json!({"brightness": fx.brightness, "contrast": fx.contrast, "saturation": fx.saturation,
               "hue": fx.hue, "grayscale": fx.grayscale, "sepia": fx.sepia, "invert": fx.invert,
               "cut_out": cut_out}),
    )
}

/// `img` with `fx` applied, and whether it was cut out first. A finished folder on the magenta
/// key colour is: adjusted as it is, inverting turns the backdrop green and desaturating turns it
/// grey, and the app would then wrap the whole picture, backdrop and all, onto its folder as
/// artwork.
pub fn adjust_picture(img: &RgbaImage, fx: &Fx) -> (RgbaImage, bool) {
    if matte::surround(img, MAGENTA) == Surround::Keyed {
        if let Some(cut) = matte::finished_cutout(img, MAGENTA) {
            return (adjust::adjust(&cut, fx), true);
        }
    }
    (adjust::adjust(img, fx), false)
}

fn check(input: &Path, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, bytes) = load(input)?;
    let (name, pasted) = if is_stdio(input) {
        ("<picture>".to_string(), "<picture>".to_string())
    } else {
        (input.display().to_string(), shell_path(input))
    };
    // The fixes are commands to paste, so they name it the way a shell reads it.
    let report = check::check(&img, bytes, &pasted);
    let mut lines = vec![format!(
        "{name}: {}, {} × {}, {}",
        report.kind,
        report.width,
        report.height,
        file_size(bytes)
    )];
    for f in &report.findings {
        let label = match f.verdict {
            Verdict::Ok => "ok",
            Verdict::Warning => "warning",
            Verdict::Problem => "problem",
        };
        lines.push(format!("  {label:8} {}", f.what));
        if let Some(fix) = &f.fix {
            lines.push(format!("           fix: {fix}"));
        }
    }
    let (problems, warnings) = (report.problems(), report.warnings());
    lines.push(match (problems, warnings) {
        (0, 0) => "Ready for a folder.".to_string(),
        (0, w) => format!(
            "Ready for a folder; {w} thing{} could be better.",
            if w == 1 { "" } else { "s" }
        ),
        (p, _) => format!("{p} thing{} to fix first.", if p == 1 { "" } else { "s" }),
    });
    let meta = serde_json::to_value(&report).unwrap_or_default();
    out.result(
        (!is_stdio(input)).then_some(input),
        "check",
        meta,
        &lines.join("\n"),
        false,
    );
    if problems > 0 {
        let first = report
            .findings
            .iter()
            .find(|f| f.verdict == Verdict::Problem)
            .map(|f| f.what.clone())
            .unwrap_or_default();
        let mut e = CliError::fixable(
            "check_failed",
            format!("{name} won't look right on a folder yet."),
            first,
        );
        for f in report
            .findings
            .iter()
            .filter(|f| f.verdict == Verdict::Problem)
        {
            if let Some(fix) = &f.fix {
                e = e.fix(fix.clone());
            }
        }
        return Err(e);
    }
    Ok(())
}

fn info(input: &Path, out: &Arc<Out>) -> Result<(), CliError> {
    let (img, data) = load_with_bytes(input)?;
    let bytes = data.len();
    let (w, h) = img.dimensions();
    // From the bytes themselves, so standard input says what it is as a file does.
    let format = image::guess_format(&data)
        .ok()
        .map(|f| format!("{f:?}").to_uppercase());
    // What render and apply refuse, so nothing to become.
    let empty = matte::alpha_bounds(&img, 8).is_none();
    let surround = matte::surround(&img, MAGENTA);
    let transparent = img.pixels().filter(|p| p.0[3] < 255).count();
    let finished = matte::finished_cutout(&img, MAGENTA);
    // Only artwork can sit on paper; around a finished folder is its backdrop.
    let border = match finished {
        Some(_) => None,
        None => painted::find_border(&img).or_else(|| painted::find_bands(&img)),
    };
    let kind = match &finished {
        _ if empty => {
            "nothing: it is completely transparent, so it can't go on a folder".to_string()
        }
        Some(cut) => format!(
            "a finished folder ({} × {} once cut out): used as it is",
            cut.width(),
            cut.height()
        ),
        None => "artwork: wrapped onto FolderSkin's folder".to_string(),
    };
    let n = (w as f64 * h as f64).max(1.0);
    let mean: Vec<f64> = (0..3)
        .map(|c| img.pixels().map(|p| p.0[c] as f64).sum::<f64>() / n)
        .collect();
    let mut lines = vec![
        format!(
            "size:        {w} × {h}{}, {}",
            format
                .as_deref()
                .map(|f| format!(" {f}"))
                .unwrap_or_default(),
            file_size(bytes)
        ),
        format!("becomes:     {kind}"),
        format!(
            "around it:   {}",
            match surround {
                Surround::Transparent => "transparency",
                Surround::Keyed => "the magenta key colour",
                Surround::Opaque => "the picture itself",
            }
        ),
        format!(
            "see-through: {:.1}% of pixels",
            transparent as f64 / n * 100.0
        ),
        format!(
            "mean colour: #{:02X}{:02X}{:02X}",
            mean[0].round() as u8,
            mean[1].round() as u8,
            mean[2].round() as u8
        ),
    ];
    if let Some(b) = border {
        lines.push(format!(
            "margin:      paper, top {} bottom {} left {} right {} px (folderskin image trim)",
            b.top, b.bottom, b.left, b.right
        ));
    }
    if painted::is_blank(&img) {
        lines.push("note:        it is one flat colour".into());
    }
    let meta = json!({
        "width": w, "height": h, "bytes": bytes, "format": format,
        "kind": if empty { "empty" } else if finished.is_some() { "folder" } else { "artwork" },
        "surround": format!("{surround:?}").to_lowercase(),
        "transparent_share": transparent as f64 / n,
        "mean_rgb": mean.iter().map(|v| v.round() as u8).collect::<Vec<_>>(),
        "border": border.map(|b| [b.top, b.bottom, b.left, b.right]),
        "blank": painted::is_blank(&img),
    });
    out.result(
        (!is_stdio(input)).then_some(input),
        "info",
        meta,
        &lines.join("\n"),
        false,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn results_go_beside_the_input_unless_told() {
        assert_eq!(
            target(Path::new("renders/koi.jpg"), None, "trimmed"),
            PathBuf::from("renders/koi-trimmed.png")
        );
        assert_eq!(target(Path::new("-"), None, "trimmed"), PathBuf::from("-"));
        assert_eq!(
            target(Path::new("a.png"), Some(Path::new("b.webp")), "x"),
            PathBuf::from("b.webp")
        );
    }

    #[test]
    fn crops_keep_the_focus_in_the_middle() {
        let img = RgbaImage::from_fn(400, 100, |x, _| Rgba([(x / 2) as u8, 0, 0, 255]));
        let left = crop_aspect(&img, 1.0, (0.0, 0.5));
        assert_eq!(left.dimensions(), (100, 100));
        assert_eq!(left.get_pixel(0, 0).0[0], 0);
        let right = crop_aspect(&img, 1.0, (1.0, 0.5));
        assert_eq!(right.get_pixel(99, 0).0[0], 199);
        let tall = crop_aspect(&img, 0.5, (0.5, 0.5));
        assert_eq!(tall.dimensions(), (50, 100));
    }

    #[test]
    fn opaque_pictures_are_written_without_alpha() {
        let opaque = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        let png = encode_png(&opaque);
        let back = image::load_from_memory(&png).unwrap();
        assert_eq!(back.color(), image::ColorType::Rgb8);
        let mut clear = opaque.clone();
        clear.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        let back = image::load_from_memory(&encode_png(&clear)).unwrap();
        assert_eq!(back.color(), image::ColorType::Rgba8);
    }

    #[test]
    fn saving_follows_the_file_name() {
        let dir = std::env::temp_dir().join(format!("fs-save-{}", std::process::id()));
        let img = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 200]));
        for name in ["a.png", "a.jpg", "a.webp"] {
            save(&img, &dir.join(name)).unwrap();
            assert!(image::open(dir.join(name)).is_ok(), "{name}");
        }
        let e = save(&img, &dir.join("a.gif")).unwrap_err();
        assert_eq!(e.code, "unknown_format");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-images-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn template() -> RgbaImage {
        folderskin_core::compositor::blank_template(640, 600, MAGENTA)
    }

    #[test]
    fn a_finished_folders_backdrop_is_never_taken_for_paper() {
        // Both find a "margin" in a folder on magenta and in a cut-out one; cutting it away used
        // to leave a slab of grey with a strip of magenta.
        let keyed = template();
        let clipped = matte::finished_cutout(&keyed, MAGENTA).unwrap();
        let fixture = image::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../folderskin-core/tests/fixtures/painted/painted-folder.cut.png"
        ))
        .unwrap()
        .to_rgba8();
        for img in [&keyed, &clipped, &fixture] {
            assert!(is_finished_folder(img));
        }

        let dir = temp_dir("trim");
        let (input, output) = (dir.join("t.png"), dir.join("t-trimmed.png"));
        std::fs::write(&input, encode_png(&keyed)).unwrap();
        let args = OneImage {
            input,
            out: Some(output.clone()),
        };
        trim(&args, &Out::new(true, false)).unwrap();
        let trimmed = image::open(&output).unwrap().to_rgba8();
        assert_eq!(trimmed, keyed, "trim leaves a finished folder as it is");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn adjusting_a_folder_on_the_key_colour_keeps_it_a_folder() {
        let keyed = template();
        for fx in [
            Fx {
                invert: 100.0,
                ..Fx::default()
            },
            Fx {
                saturation: -60.0,
                ..Fx::default()
            },
        ] {
            let (adjusted, cut_out) = adjust_picture(&keyed, &fx);
            assert!(cut_out);
            assert_eq!(matte::surround(&adjusted, MAGENTA), Surround::Transparent);
            assert_eq!(
                check::check(&adjusted, 10_000, "a.png").kind,
                "folder",
                "{fx:?}"
            );
        }
        // Artwork is adjusted as it is.
        let art = RgbaImage::from_fn(64, 60, |x, y| Rgba([x as u8 * 3, y as u8 * 4, 90, 255]));
        let (adjusted, cut_out) = adjust_picture(
            &art,
            &Fx {
                invert: 100.0,
                ..Fx::default()
            },
        );
        assert!(!cut_out);
        assert_eq!(adjusted.dimensions(), art.dimensions());
        assert_eq!(adjusted.get_pixel(0, 0).0, [255, 255, 165, 255]);
    }

    #[test]
    fn a_cut_out_saved_as_jpeg_goes_back_on_the_key_colour() {
        let dir = temp_dir("jpeg");
        let clipped = matte::finished_cutout(&template(), MAGENTA).unwrap();
        save(&clipped, &dir.join("f.jpg")).unwrap();
        let back = image::open(dir.join("f.jpg")).unwrap().to_rgba8();
        assert_eq!(matte::surround(&back, MAGENTA), Surround::Keyed);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sizes_read_in_kilobytes_below_a_megabyte() {
        assert_eq!(file_size(33_000), "33 KB");
        assert_eq!(file_size(1), "1 KB");
        assert_eq!(file_size(999_999), "1000 KB");
        assert_eq!(file_size(2_345_678), "2.3 MB");
    }

    #[test]
    fn an_empty_picture_is_not_cut_out_or_called_artwork() {
        let dir = temp_dir("empty");
        let input = dir.join("clear.png");
        std::fs::write(&input, encode_png(&RgbaImage::new(64, 64))).unwrap();
        let args = CutoutArgs {
            image: OneImage {
                input: input.clone(),
                out: Some(dir.join("clear-cutout.png")),
            },
            flat_backdrop: false,
        };
        let e = cutout(&args, &Out::new(true, false)).unwrap_err();
        assert_eq!(e.code, "image_empty");
        assert!(!dir.join("clear-cutout.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_already_cut_out_is_clipped_to_itself() {
        let dir = temp_dir("clip");
        let cut = matte::finished_cutout(&template(), MAGENTA).unwrap();
        let (input, output) = (dir.join("cut.png"), dir.join("cut-clipped.png"));
        std::fs::write(&input, encode_png(&cut)).unwrap();
        let args = OneImage {
            input,
            out: Some(output.clone()),
        };
        clip(&args, &Out::new(true, false)).unwrap();
        assert_eq!(image::open(&output).unwrap().to_rgba8(), cut);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn advice_never_repeats_the_command_that_failed_and_quotes_its_paths() {
        let wide = Path::new("my wide shot.png");
        let after_flat = no_backdrop(wide, "It isn't on one flat colour.", true);
        assert!(
            !after_flat.fix.iter().any(|f| f.contains("--flat-backdrop")),
            "{after_flat:?}"
        );
        let after_magenta = no_backdrop(wide, "It isn't on the magenta key colour.", false);
        assert!(after_magenta.fix[0].ends_with("cutout \"my wide shot.png\" --flat-backdrop"));
        for e in [after_flat, after_magenta] {
            assert!(
                e.fix.iter().all(|f| !f.contains(" my wide")),
                "every path is quoted: {e:?}"
            );
        }
    }

    #[test]
    fn a_folder_or_a_cut_short_file_is_named_for_what_it_is() {
        let dir = temp_dir("not-a-file");
        let folder = dir.join("afolder.png");
        std::fs::create_dir(&folder).unwrap();
        let img = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        for e in [
            load(&folder).unwrap_err(),
            preview::read_picture(&folder).unwrap_err(),
            save(&img, &folder).unwrap_err(),
            write_png(b"\x89PNG", &folder, "save the preview").unwrap_err(),
        ] {
            assert_eq!(e.code, "not_a_file", "{e:?}");
            assert!(
                e.why.ends_with("afolder.png is a folder, not a file."),
                "{e:?}"
            );
        }
        let whole = encode_png(&RgbaImage::from_fn(64, 64, |x, y| {
            Rgba([x as u8 * 4, y as u8 * 4, 90, 255])
        }));
        let short = dir.join("truncated.png");
        std::fs::write(&short, &whole[..whole.len() / 2]).unwrap();
        for e in [
            load(&short).unwrap_err(),
            preview::read_picture(&short).unwrap_err(),
        ] {
            assert_eq!(e.code, "image_unreadable", "{e:?}");
            assert!(e.why.contains("cut short"), "{e:?}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pngs_go_to_standard_output_or_into_folders_made_for_them() {
        let dir = temp_dir("write");
        let deep = dir.join("x").join("y").join("z.png");
        write_png(b"\x89PNG", &deep, "save the preview").unwrap();
        assert_eq!(std::fs::read(&deep).unwrap(), b"\x89PNG");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
