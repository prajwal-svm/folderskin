//! A picture the way the app takes it, so `render`, `apply` and the pack previews show exactly
//! what adding that picture to FolderSkin would.

use folderskin_core::compositor::{self, Artwork, IconSet, Style};
use folderskin_core::drive::{DriveShape, DriveStyle};
use folderskin_core::matte;
use folderskin_core::pack::PackShape;
use image::RgbaImage;

/// The drive a pack of drives' artwork is shown on in its previews: the Mac's external drive, as
/// a pack of folders' is shown on FolderSkin's folder. In the app each skin goes on the drive
/// picked.
pub fn preview_drive() -> DriveShape {
    DriveShape::default_for(DriveStyle::Mac)
}

/// A picture split the way the app's `prepare_import` splits it: a finished folder picture (cut
/// out, or on the magenta key) is the icon itself, and anything else is artwork for FolderSkin's
/// template.
pub enum Skin {
    /// A finished folder, trimmed, with any magenta backdrop keyed out.
    Folder(RgbaImage),
    /// Artwork for the template, cropped around its focus point.
    Artwork(Artwork),
}

impl Skin {
    /// Splits `rgba`. `focus` only matters for artwork. The error finishes a sentence that starts
    /// with the picture's name.
    pub fn from_picture(rgba: RgbaImage, focus: (f32, f32)) -> Result<Skin, String> {
        if matte::alpha_bounds(&rgba, 8).is_none() {
            return Err("is completely transparent".into());
        }
        Ok(match matte::finished_cutout(&rgba, matte::MAGENTA) {
            Some(cut) => Skin::Folder(cut),
            None => Skin::Artwork(Artwork { rgba, focus }),
        })
    }

    /// The icon as a PNG, `size` px square, through the render the app uses.
    pub fn preview_png(&self, size: u32) -> Vec<u8> {
        self.preview_png_in(size, Style::Mac)
    }

    /// [`Skin::preview_png`] for a skin of a pack of `shape`: a pack of drives' artwork on
    /// [`preview_drive`], and a finished drive as it is.
    pub fn preview_png_for(&self, size: u32, shape: PackShape) -> Vec<u8> {
        match (self, shape) {
            (Skin::Artwork(art), PackShape::Drive) => {
                compositor::render_drive_preview_png(Some(art), size, preview_drive())
            }
            _ => self.preview_png(size),
        }
    }

    /// [`Skin::preview_png`] with artwork on the folder of `style`, as the app draws it when
    /// that folder is chosen. A finished folder is its own shape on either.
    pub fn preview_png_in(&self, size: u32, style: Style) -> Vec<u8> {
        match self {
            Skin::Folder(cut) => compositor::preview_png_from_image(cut, size),
            Skin::Artwork(art) => compositor::render_preview_png_in(art, size, style),
        }
    }

    /// The icon at every size in `sizes`, as the app applies it.
    pub fn icon_set(&self, sizes: &[u32]) -> IconSet {
        self.icon_set_in(sizes, Style::Mac)
    }

    /// [`Skin::icon_set`] with artwork on the folder of `style`.
    pub fn icon_set_in(&self, sizes: &[u32], style: Style) -> IconSet {
        match self {
            Skin::Folder(cut) => compositor::icon_set_from_image(cut, sizes),
            Skin::Artwork(art) => compositor::render_icon_set_in(art, sizes, style),
        }
    }

    /// What the app does with it, for a report line.
    pub fn describe(&self) -> &'static str {
        match self {
            Skin::Folder(_) => "a finished folder, used as it is",
            Skin::Artwork(_) => "artwork on FolderSkin's folder",
        }
    }

    /// The icon at every size in `sizes` as a drive's own, the way the app gives one to the drive
    /// picked: artwork on the drive's `shape`, and a finished picture as it is.
    pub fn icon_set_on_drive(&self, sizes: &[u32], shape: DriveShape) -> IconSet {
        match self {
            Skin::Folder(cut) => compositor::icon_set_from_image(cut, sizes),
            Skin::Artwork(art) => compositor::render_drive_icon_set(Some(art), sizes, shape),
        }
    }

    /// What the app does with it on a drive, for a report line.
    pub fn describe_on_drive(&self) -> &'static str {
        match self {
            Skin::Folder(_) => "a finished picture, used as it is",
            Skin::Artwork(_) => "artwork on the drive's own shape",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A folder-shaped block on `around`.
    fn block(around: [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(300, 280, |x, y| {
            if (40..260).contains(&x) && (50..240).contains(&y) {
                Rgba([30, 90, 200, 255])
            } else {
                Rgba(around)
            }
        })
    }

    /// Artwork on a drive takes the drive's shape, as the app draws it for the drive picked: a
    /// USB stick is narrow where the folder is wide. A finished picture is used as it is.
    #[test]
    fn artwork_on_a_drive_takes_the_drives_shape() {
        let photo = RgbaImage::from_fn(320, 300, |x, y| Rgba([x as u8, y as u8, 90, 255]));
        let art = Skin::from_picture(photo, (0.5, 0.5)).unwrap();
        let stick = DriveShape::from_id("linux-removable").unwrap();
        let on_drive = art.icon_set_on_drive(&[256], stick);
        let on_folder = art.icon_set(&[256]);
        let alpha = |set: &IconSet| set.sizes[0].1.get_pixel(40, 128).0[3];
        assert_eq!(alpha(&on_drive), 0, "left of the stick is clear");
        assert!(alpha(&on_folder) > 0, "the folder reaches there");
        assert_eq!(art.describe_on_drive(), "artwork on the drive's own shape");

        let finished = Skin::from_picture(block([0, 0, 0, 0]), (0.5, 0.5)).unwrap();
        assert_eq!(
            finished.icon_set_on_drive(&[64], stick).sizes[0].1,
            finished.icon_set(&[64]).sizes[0].1
        );
    }

    #[test]
    fn a_picture_is_split_the_way_the_app_splits_it() {
        for (around, what) in [
            ([0, 0, 0, 0], "cut out"),
            ([255, 0, 255, 255], "on magenta"),
        ] {
            match Skin::from_picture(block(around), (0.5, 0.5)).unwrap() {
                Skin::Folder(cut) => assert_eq!(cut.dimensions(), (220, 190), "{what}"),
                Skin::Artwork(_) => panic!("a finished folder {what} became artwork"),
            }
        }
        let photo = RgbaImage::from_fn(320, 300, |x, y| Rgba([x as u8, y as u8, 90, 255]));
        match Skin::from_picture(photo, (0.5, 0.3)).unwrap() {
            Skin::Artwork(art) => assert_eq!(art.focus, (0.5, 0.3)),
            Skin::Folder(_) => panic!("a photo became a finished folder"),
        }
        let clear = RgbaImage::new(300, 300);
        assert_eq!(
            Skin::from_picture(clear, (0.5, 0.5)).err().as_deref(),
            Some("is completely transparent")
        );
    }

    #[test]
    fn both_kinds_render_at_the_size_asked_for() {
        for around in [[0, 0, 0, 0], [240, 200, 120, 255]] {
            let skin = Skin::from_picture(block(around), (0.5, 0.5)).unwrap();
            let png = image::load_from_memory(&skin.preview_png(64)).unwrap();
            assert_eq!((png.width(), png.height()), (64, 64));
            let set = skin.icon_set(&[32, 16]);
            assert_eq!(
                set.sizes.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
                [32, 16]
            );
        }
    }
}
