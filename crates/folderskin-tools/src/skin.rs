//! A picture the way the app takes it, so `render`, `apply` and the pack previews show exactly
//! what adding that picture to FolderSkin would.

use folderskin_core::compositor::{self, Artwork, IconSet};
use folderskin_core::matte;
use image::RgbaImage;

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
        match self {
            Skin::Folder(cut) => compositor::preview_png_from_image(cut, size),
            Skin::Artwork(art) => compositor::render_preview_png(art, size),
        }
    }

    /// The icon at every size in `sizes`, as the app applies it.
    pub fn icon_set(&self, sizes: &[u32]) -> IconSet {
        match self {
            Skin::Folder(cut) => compositor::icon_set_from_image(cut, sizes),
            Skin::Artwork(art) => compositor::render_icon_set(art, sizes),
        }
    }

    /// What the app does with it, for a report line.
    pub fn describe(&self) -> &'static str {
        match self {
            Skin::Folder(_) => "a finished folder, used as it is",
            Skin::Artwork(_) => "artwork on FolderSkin's folder",
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
