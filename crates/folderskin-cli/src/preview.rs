//! Pictures as the folders the app makes of them: one preview each, a contact sheet of many,
//! and putting one on a folder.

use crate::error::CliError;
use folderskin_core::apply::{apply_icon, refresh_shell_icons, ApplyError};
use folderskin_core::compositor::{Style, ICON_SIZES};
use folderskin_core::matte;
use folderskin_tools::skin::Skin;
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The side of a preview, in pixels.
pub const PREVIEW_SIZE: u32 = 512;

/// Whether artwork goes on Windows' folder rather than the Mac's, for this run: `--look`, or
/// the folder chosen in the app. Set once, before the command runs.
static WINDOWS_LOOK: AtomicBool = AtomicBool::new(false);

/// Puts artwork on the folder of `style` for the rest of the run.
pub fn set_look(style: Style) {
    WINDOWS_LOOK.store(style == Style::Windows, Ordering::Relaxed);
}

/// The folder artwork goes on in this run, as the app would put it.
pub fn look() -> Style {
    if WINDOWS_LOOK.load(Ordering::Relaxed) {
        Style::Windows
    } else {
        Style::Mac
    }
}

/// A contact sheet's cells and columns.
const CELL: u32 = 256;
const COLUMNS: u32 = 4;
/// Light grey, not transparency, which most viewers draw black: a cut-out that went into a dark
/// coat would look whole there.
const SHEET_GREY: [u8; 3] = [236, 236, 236];

pub fn read_picture(path: &Path) -> Result<RgbaImage, CliError> {
    if path.is_dir() {
        return Err(CliError::folder_not_file("read the picture", path));
    }
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|e| unreadable(path, &e))
}

pub fn unreadable(path: &Path, e: &image::ImageError) -> CliError {
    match e {
        // The decoder ran out of picture: the file is there, just cut short.
        image::ImageError::IoError(io) if io.kind() == std::io::ErrorKind::UnexpectedEof => {
            CliError::fixable(
                "image_unreadable",
                "That picture can't be read.",
                format!("{} ends early: the file is cut short.", path.display()),
            )
            .fix("Download, copy or export it again, then run the command again.")
        }
        image::ImageError::IoError(io) => CliError::io("read the picture", path, io),
        image::ImageError::Unsupported(_) => CliError::fixable(
            "image_unreadable",
            "That picture can't be read.",
            format!("{} isn't a PNG, JPEG or WebP picture.", path.display()),
        )
        .fix("Save it as PNG, JPEG or WebP and try again."),
        other => CliError::fixable(
            "image_unreadable",
            "That picture can't be read.",
            format!("{}: {other}.", path.display()),
        )
        .fix("Open it in an image editor and save it again as PNG."),
    }
}

/// A picture the way the app takes it; the error says why it can't be one.
pub fn skin(img: RgbaImage, focus: (f32, f32), path: &Path) -> Result<Skin, CliError> {
    Skin::from_picture(img, focus).map_err(|why| {
        CliError::fixable(
            "image_empty",
            "There is nothing in that picture to put on a folder.",
            format!("{} {why}.", path.display()),
        )
    })
}

/// Draws `picture` as the folder the app makes of it into `dir`/`<its name>.png`, and says what
/// the app would do with it.
pub fn preview(picture: &Path, dir: &Path) -> Result<(PathBuf, &'static str), CliError> {
    let skin = skin(read_picture(picture)?, (0.5, 0.5), picture)?;
    std::fs::create_dir_all(dir).map_err(|e| CliError::io("make the previews folder", dir, &e))?;
    let dest = dir
        .join(picture.file_name().unwrap_or_default())
        .with_extension("png");
    std::fs::write(&dest, skin.preview_png_in(PREVIEW_SIZE, look()))
        .map_err(|e| CliError::io("save the preview", &dest, &e))?;
    Ok((dest, skin.describe()))
}

/// Every preview in `previews`, `CELL` px square, `COLUMNS` to a row, in the order given.
pub fn contact_sheet(previews: &[PathBuf], dest: &Path) -> Result<(), CliError> {
    if previews.is_empty() {
        return Ok(());
    }
    let n = previews.len() as u32;
    let columns = COLUMNS.min(n);
    let rows = n.div_ceil(columns);
    let [r, g, b] = SHEET_GREY;
    let mut sheet = RgbaImage::from_pixel(CELL * columns, CELL * rows, image::Rgba([r, g, b, 255]));
    for (i, path) in previews.iter().enumerate() {
        let img = read_picture(path)?;
        let tile = image::imageops::resize(&img, CELL, CELL, image::imageops::FilterType::Lanczos3);
        let (col, row) = (i as u32 % columns, i as u32 / columns);
        image::imageops::replace(
            &mut sheet,
            &matte::flatten(&tile, SHEET_GREY),
            i64::from(col * CELL),
            i64::from(row * CELL),
        );
    }
    sheet.save(dest).map_err(|e| match e {
        image::ImageError::IoError(io) => CliError::io("save the contact sheet", dest, &io),
        other => CliError::bug("The contact sheet couldn't be encoded.", other.to_string()),
    })
}

/// Puts `picture` on `folder`, as the app does, and says what it became. `-` reads the picture
/// from standard input, as every image command does.
pub fn apply(folder: &Path, picture: &Path, focus: (f32, f32)) -> Result<&'static str, CliError> {
    let (img, _) = crate::images::load(picture)?;
    let skin = skin(img, focus, picture)?;
    apply_icon(folder, &skin.icon_set_in(&ICON_SIZES, look()))
        .map_err(|e| apply_error(folder, e))?;
    refresh_shell_icons();
    Ok(skin.describe())
}

pub fn apply_error(folder: &Path, e: ApplyError) -> CliError {
    match e {
        ApplyError::NotADirectory(_) => CliError::fixable(
            "not_a_folder",
            "That isn't a folder.",
            format!("{} doesn't exist or isn't a folder.", folder.display()),
        )
        .fix("Give the path of a folder that exists."),
        ApplyError::Refused(why) => CliError::fixable(
            "apply_refused",
            "FolderSkin won't change that folder's icon.",
            why,
        ),
        ApplyError::Io(io) => CliError::io("change the folder's icon", folder, &io),
        ApplyError::Platform(why) => {
            CliError::environment("apply_failed", "The system didn't take the new icon.", why)
                .fix("Try again; if it keeps happening, apply it from the app, which says more.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_has_a_cell_for_every_preview() {
        let dir = std::env::temp_dir().join(format!("fs-sheet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut previews = Vec::new();
        for i in 0..5u8 {
            let p = dir.join(format!("{i}.png"));
            RgbaImage::from_pixel(64, 64, image::Rgba([i * 40, 90, 200, 255]))
                .save(&p)
                .unwrap();
            previews.push(p);
        }
        contact_sheet(&previews, &dir.join("_sheet.png")).unwrap();
        let sheet = image::open(dir.join("_sheet.png")).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (CELL * 4, CELL * 2));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_picture_that_isnt_there_says_so() {
        let e = read_picture(Path::new("/definitely/not/here.png")).unwrap_err();
        assert_eq!(e.code, "io");
        assert!(e.why.contains("here.png"), "{e:?}");
    }
}
