//! An icon saved as a file of its own, to use anywhere: macOS's `.icns`, Windows' `.ico`, or a
//! PNG of one size.

use crate::compositor::IconSet;
use std::path::Path;

/// The kinds of file an icon is saved as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// macOS's icon file, with every size from 16 to 1024 px ([`crate::icns`]).
    Icns,
    /// Windows' icon file, with every size Explorer picks from, 16 to 256 px ([`crate::ico`]).
    Ico,
    /// A picture of one size.
    Png,
}

/// The sizes an `.icns` holds: one for each kind macOS reads.
pub const ICNS_SIZES: [u32; 7] = [1024, 512, 256, 128, 64, 32, 16];

impl Format {
    /// The format a file name asks for by its extension, in any case: `.icns`, `.ico`, and a
    /// PNG for anything else.
    pub fn of(path: &Path) -> Format {
        let extension = path.extension().and_then(|e| e.to_str());
        match extension.map(str::to_ascii_lowercase).as_deref() {
            Some("icns") => Format::Icns,
            Some("ico") => Format::Ico,
            _ => Format::Png,
        }
    }

    /// The sizes a file of this format holds; a PNG holds only `png`.
    pub fn sizes(self, png: u32) -> Vec<u32> {
        match self {
            Format::Icns => ICNS_SIZES.to_vec(),
            Format::Ico => crate::apply::windows::ICO_SIZES.to_vec(),
            Format::Png => vec![png],
        }
    }

    /// The file name extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Icns => "icns",
            Format::Ico => "ico",
            Format::Png => "png",
        }
    }
}

/// The bytes of a file of `format` holding `icons` at each of its sizes ([`Format::sizes`]),
/// or `None` when `icons` has none of them.
pub fn encode(icons: &IconSet, format: Format, png: u32) -> Option<Vec<u8>> {
    let entries: Vec<(u32, Vec<u8>)> = format
        .sizes(png)
        .into_iter()
        .filter_map(|size| icons.png(size).map(|bytes| (size, bytes)))
        .collect();
    if entries.is_empty() {
        return None;
    }
    Some(match format {
        Format::Icns => crate::icns::write_icns(&entries),
        Format::Ico => crate::ico::write_ico(&entries),
        Format::Png => entries.into_iter().next()?.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder(sizes: &[u32]) -> IconSet {
        IconSet {
            sizes: sizes
                .iter()
                .map(|&s| {
                    (
                        s,
                        image::RgbaImage::from_pixel(s, s, image::Rgba([9, 8, 7, 255])),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_name_says_the_format() {
        assert_eq!(Format::of(Path::new("a/Photos.icns")), Format::Icns);
        assert_eq!(Format::of(Path::new("Photos.ICO")), Format::Ico);
        assert_eq!(Format::of(Path::new("Photos.png")), Format::Png);
        assert_eq!(Format::of(Path::new("Photos")), Format::Png);
        assert_eq!(Format::of(Path::new("-")), Format::Png);
    }

    #[test]
    fn each_file_holds_its_own_sizes() {
        let icons = ladder(&crate::compositor::ICON_SIZES);
        let icns = encode(&icons, Format::Icns, 0).unwrap();
        assert_eq!(&icns[..4], b"icns");
        let mut elements = 0;
        let mut at = 8;
        while at < icns.len() {
            elements += 1;
            at += u32::from_be_bytes(icns[at + 4..at + 8].try_into().unwrap()) as usize;
        }
        // Seven sizes in ten elements: 32, 256 and 512 px each go in under two kinds.
        assert_eq!((elements, at), (10, icns.len()));
        let ico = encode(&icons, Format::Ico, 0).unwrap();
        assert_eq!(
            u16::from_le_bytes([ico[4], ico[5]]),
            7,
            "seven sizes, 16 to 256 px"
        );
        let png = encode(&ladder(&[300]), Format::Png, 300).unwrap();
        let back = image::load_from_memory(&png).unwrap();
        assert_eq!((back.width(), back.height()), (300, 300));
    }

    #[test]
    fn nothing_at_the_sizes_a_file_holds_is_no_file() {
        assert!(encode(&ladder(&[48]), Format::Icns, 0).is_none());
        assert!(encode(&ladder(&[2048]), Format::Ico, 0).is_none());
        assert!(encode(&ladder(&[512]), Format::Png, 256).is_none());
    }
}
