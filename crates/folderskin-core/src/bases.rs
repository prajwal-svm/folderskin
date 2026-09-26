//! Every base a skin can be drawn on, in one list: FolderSkin's folders in their three looks,
//! every drive shape, and none at all.
//!
//! The composer starts a design on one of these, and anything else that lets someone pick what a
//! picture is painted on (the AI view's skeleton) reads the same list, so a new shape shows up
//! everywhere at once. An entry is data: an id that never changes, the key the webview names it
//! by, the system whose look it is drawn in, whether it is a folder or a drive (and which kind of
//! drive), and a way to draw the bare shape.

use crate::compositor::{self, Style};
use crate::drive::{DriveKind, DriveShape};
use image::RgbaImage;

/// One base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Base {
    /// A folder in one of its looks.
    Folder(Style),
    /// A drive shape.
    Drive(DriveShape),
    /// No base: the picture is the whole icon.
    Free,
}

impl Base {
    /// Every base: the folders in the order the looks are offered, then the drives system by
    /// system, then none.
    pub fn all() -> Vec<Base> {
        Style::ALL
            .into_iter()
            .map(Base::Folder)
            .chain(DriveShape::all().into_iter().map(Base::Drive))
            .chain(std::iter::once(Base::Free))
            .collect()
    }

    /// `folder-mac`, `drive-linux-solid-state`, `free`. Never changes once shipped: designs and
    /// settings keep it.
    pub fn id(self) -> String {
        match self {
            Base::Folder(style) => format!("folder-{}", style.id()),
            Base::Drive(shape) => format!("drive-{}", shape.id()),
            Base::Free => "free".into(),
        }
    }

    /// The base an [`Base::id`] names, or `None` for anything else.
    pub fn from_id(id: &str) -> Option<Base> {
        if id == "free" {
            return Some(Base::Free);
        }
        if let Some(style) = id.strip_prefix("folder-") {
            return Style::from_id(style).map(Base::Folder);
        }
        id.strip_prefix("drive-")
            .and_then(DriveShape::from_id)
            .map(Base::Drive)
    }

    /// The message the webview names it by, in every language it speaks.
    pub fn label_key(self) -> String {
        format!("common.bases.{}", self.id())
    }

    /// `folder`, `drive` or `free`.
    pub fn kind_id(self) -> &'static str {
        match self {
            Base::Folder(_) => "folder",
            Base::Drive(_) => "drive",
            Base::Free => "free",
        }
    }

    /// The system whose look it is drawn in: `mac`, `windows` or `linux`, and none for no base.
    pub fn style_id(self) -> Option<&'static str> {
        match self {
            Base::Folder(style) => Some(style.id()),
            Base::Drive(shape) => Some(shape.style().id()),
            Base::Free => None,
        }
    }

    /// The kind of drive, for a drive.
    pub fn drive_kind(self) -> Option<DriveKind> {
        match self {
            Base::Drive(shape) => Some(shape.kind()),
            _ => None,
        }
    }

    /// The bare shape, `size` px square, through the same render as every icon: the folder in the
    /// colour its system gives it, the drive with nothing on its face, and for no base a square
    /// with nothing in it.
    pub fn render(self, size: u32) -> RgbaImage {
        let size = size.max(1);
        let icons = match self {
            Base::Folder(style) => compositor::render_icon_set_in(
                &compositor::default_folder_artwork_in(style),
                &[size],
                style,
            ),
            Base::Drive(shape) => compositor::render_drive_icon_set(None, &[size], shape),
            Base::Free => return RgbaImage::new(size, size),
        };
        icons
            .sizes
            .into_iter()
            .next()
            .expect("the size asked for")
            .1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_has_one_id_that_leads_back_to_it() {
        let all = Base::all();
        assert_eq!(all.len(), 3 + DriveShape::all().len() + 1);
        let mut ids: Vec<String> = all.iter().map(|b| b.id()).collect();
        for base in &all {
            assert_eq!(Base::from_id(&base.id()), Some(*base), "{}", base.id());
            assert_eq!(base.label_key(), format!("common.bases.{}", base.id()));
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "no two bases share an id");
        for id in [
            "folder-amiga",
            "drive-mac-solid-state",
            "drive-",
            "folder",
            "freehand",
            "",
        ] {
            assert_eq!(Base::from_id(id), None, "{id}");
        }
    }

    #[test]
    fn the_list_is_folders_then_drives_then_none() {
        let all = Base::all();
        assert_eq!(all[0], Base::Folder(Style::Mac));
        assert_eq!(all[1].id(), "folder-windows");
        assert_eq!(all[2].id(), "folder-linux");
        assert_eq!(all[3].id(), "drive-mac-startup");
        assert_eq!(all.last(), Some(&Base::Free));
        assert_eq!(Base::Free.style_id(), None);
        assert_eq!(Base::Free.kind_id(), "free");
        let ssd = Base::from_id("drive-linux-solid-state").unwrap();
        assert_eq!((ssd.kind_id(), ssd.style_id()), ("drive", Some("linux")));
        assert_eq!(ssd.drive_kind(), Some(DriveKind::SolidState));
        assert_eq!(Base::Folder(Style::Windows).drive_kind(), None);
    }

    #[test]
    fn a_base_draws_its_bare_shape_at_the_size_asked_for() {
        for id in ["folder-linux", "drive-windows-card", "free"] {
            let img = Base::from_id(id).unwrap().render(64);
            assert_eq!(img.dimensions(), (64, 64), "{id}");
            let shown = img.pixels().filter(|p| p.0[3] > 0).count();
            if id == "free" {
                assert_eq!(shown, 0, "nothing at all");
            } else {
                assert!(shown > 64 * 64 / 5, "{id} shows only {shown} pixels");
            }
        }
    }

    /// Every base plain and with a picture on it, at 256 px, and at 32 and 16 px as they are and
    /// shown four times over, for looking at: `FOLDERSKIN_SHEET=<folder> cargo test -p
    /// folderskin-core --lib -- --ignored base_sheet`. Each base's bare shape is written at 1024 px
    /// beside the sheets.
    #[test]
    #[ignore = "writes pictures to look at"]
    fn base_sheet() {
        let Some(dir) = std::env::var_os("FOLDERSKIN_SHEET") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let art = crate::compositor::Artwork {
            rgba: RgbaImage::from_fn(1024, 958, |x, y| {
                let (u, v) = (x as f32 / 1023.0, y as f32 / 957.0);
                let sky = [
                    (40.0 + 200.0 * v) as u8,
                    (90.0 + 120.0 * v) as u8,
                    (200.0 - 60.0 * v) as u8,
                    255,
                ];
                let sun = ((u - 0.62).powi(2) + (v - 0.42).powi(2)).sqrt() < 0.13;
                let hill = v > 0.72 - 0.12 * (u * 6.0).sin().abs();
                image::Rgba(if sun {
                    [255, 214, 90, 255]
                } else if hill {
                    [30, 90, 60, 255]
                } else {
                    sky
                })
            }),
            focus: (0.5, 0.5),
        };
        let bases: Vec<Base> = Base::all()
            .into_iter()
            .filter(|b| *b != Base::Free)
            .collect();
        let (cell, cols) = (256u32, 7u32);
        let rows = bases.len().div_ceil(cols as usize) as u32;
        for (name, with_art) in [("sheet.png", false), ("sheet-art.png", true)] {
            let mut sheet = RgbaImage::from_pixel(
                cols * cell,
                rows * (cell + 72),
                image::Rgba([236, 237, 240, 255]),
            );
            for (i, base) in bases.iter().enumerate() {
                let sizes = [1024, 256, 32, 16];
                let set = match (*base, with_art) {
                    (Base::Folder(style), true) => {
                        compositor::render_icon_set_in(&art, &sizes, style)
                    }
                    (Base::Drive(shape), true) => {
                        compositor::render_drive_icon_set(Some(&art), &sizes, shape)
                    }
                    (Base::Folder(style), false) => compositor::render_icon_set_in(
                        &compositor::default_folder_artwork_in(style),
                        &sizes,
                        style,
                    ),
                    (Base::Drive(shape), false) => {
                        compositor::render_drive_icon_set(None, &sizes, shape)
                    }
                    (Base::Free, _) => unreachable!(),
                };
                if !with_art {
                    set.sizes[0]
                        .1
                        .save(dir.join(format!("{}.png", base.id())))
                        .unwrap();
                }
                let (x, y) = ((i as u32 % cols) * cell, (i as u32 / cols) * (cell + 72));
                let (x, y) = (i64::from(x), i64::from(y));
                image::imageops::overlay(&mut sheet, &set.sizes[1].1, x, y);
                let big = |img: &RgbaImage| {
                    image::imageops::resize(img, 64, 64, image::imageops::FilterType::Nearest)
                };
                image::imageops::overlay(&mut sheet, &set.sizes[2].1, x + 8, y + 276);
                image::imageops::overlay(&mut sheet, &set.sizes[3].1, x + 48, y + 284);
                image::imageops::overlay(&mut sheet, &big(&set.sizes[2].1), x + 100, y + 260);
                image::imageops::overlay(&mut sheet, &big(&set.sizes[3].1), x + 176, y + 260);
            }
            sheet.save(dir.join(name)).unwrap();
        }
    }
}
