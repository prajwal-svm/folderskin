//! An icon saved on its own, to use anywhere: macOS's `.icns` and `.iconset`, Windows' `.ico`,
//! an iOS app icon, the favicons a website needs, or one PNG or JPEG.

use crate::apply::paths::write_atomic;
use crate::compositor::IconSet;
use crate::raster::encode_png;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use serde_json::json;
use std::io;
use std::path::Path;

/// The kinds of file an icon is saved as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// One picture, see-through around the icon.
    Png,
    /// One picture on white, since a JPEG can't be see-through.
    Jpeg,
    /// macOS's icon file, with every size from 16 to 1024 px ([`crate::icns`]).
    Icns,
    /// Windows' icon file, with every size Explorer picks from, 16 to 256 px ([`crate::ico`]).
    Ico,
    /// A folder of PNGs named the way macOS's `iconutil` and Xcode read them, 16 to 1024 px.
    Iconset,
    /// An iOS app icon: a folder Xcode takes into an asset catalog, every size on white and with
    /// no transparency, as the App Store wants them.
    Ios,
    /// What a website needs: `favicon.ico`, the PNGs browsers and phones ask for, a web manifest
    /// and the lines that go in the page's head.
    Favicon,
}

/// The sizes an `.icns` holds: one for each kind macOS reads.
pub const ICNS_SIZES: [u32; 7] = [1024, 512, 256, 128, 64, 32, 16];

/// The files of an `.iconset`: each one's name and size.
const ICONSET: [(&str, u32); 10] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
];

/// Every image an iOS app icon set lists: the device, the size in points, the scale and its
/// size in pixels. iPhone and iPad as Xcode lays them out, and the App Store's own 1024 px.
const IOS: [(&str, &str, &str, u32); 18] = [
    ("iphone", "20x20", "2x", 40),
    ("iphone", "20x20", "3x", 60),
    ("iphone", "29x29", "2x", 58),
    ("iphone", "29x29", "3x", 87),
    ("iphone", "40x40", "2x", 80),
    ("iphone", "40x40", "3x", 120),
    ("iphone", "60x60", "2x", 120),
    ("iphone", "60x60", "3x", 180),
    ("ipad", "20x20", "1x", 20),
    ("ipad", "20x20", "2x", 40),
    ("ipad", "29x29", "1x", 29),
    ("ipad", "29x29", "2x", 58),
    ("ipad", "40x40", "1x", 40),
    ("ipad", "40x40", "2x", 80),
    ("ipad", "76x76", "1x", 76),
    ("ipad", "76x76", "2x", 152),
    ("ipad", "83.5x83.5", "2x", 167),
    ("ios-marketing", "1024x1024", "1x", 1024),
];

/// The sizes in a website's `favicon.ico`.
const FAVICON_ICO: [u32; 3] = [16, 32, 48];
/// The PNGs a website's favicons come with: each one's name, its size and whether it's on white
/// (an iPhone draws a see-through touch icon on black).
const FAVICON_PNGS: [(&str, u32, bool); 5] = [
    ("favicon-16x16.png", 16, false),
    ("favicon-32x32.png", 32, false),
    ("apple-touch-icon.png", 180, true),
    ("icon-192.png", 192, false),
    ("icon-512.png", 512, false),
];

/// The lines that go in a page's head for the favicons.
const FAVICON_HEAD: &str = r#"<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
<link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
"#;

/// White, under whatever can't be see-through.
const WHITE: [u8; 3] = [255, 255, 255];

impl Format {
    /// Every format, in the order they're offered.
    pub const ALL: [Format; 7] = [
        Format::Png,
        Format::Jpeg,
        Format::Icns,
        Format::Ico,
        Format::Iconset,
        Format::Ios,
        Format::Favicon,
    ];

    /// The format a name asks for by its extension, in any case, if it names one: `.png`,
    /// `.jpg` or `.jpeg`, `.icns`, `.ico`, `.iconset` and `.appiconset`.
    pub fn of(path: &Path) -> Option<Format> {
        let extension = path.extension().and_then(|e| e.to_str());
        match extension.map(str::to_ascii_lowercase).as_deref() {
            Some("png") => Some(Format::Png),
            Some("jpg" | "jpeg") => Some(Format::Jpeg),
            Some("icns") => Some(Format::Icns),
            Some("ico") => Some(Format::Ico),
            Some("iconset") => Some(Format::Iconset),
            Some("appiconset") => Some(Format::Ios),
            _ => None,
        }
    }

    /// Its name for the command line and the app: png, jpeg, icns, ico, iconset, ios or favicon.
    pub fn id(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpeg",
            Format::Icns => "icns",
            Format::Ico => "ico",
            Format::Iconset => "iconset",
            Format::Ios => "ios",
            Format::Favicon => "favicon",
        }
    }

    /// The format called `id` ([`Format::id`]).
    pub fn from_id(id: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.id() == id)
    }

    /// The extension its name ends in, without the dot. A website's favicons are a folder with
    /// no extension of its own.
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Format::Png => Some("png"),
            Format::Jpeg => Some("jpg"),
            Format::Icns => Some("icns"),
            Format::Ico => Some("ico"),
            Format::Iconset => Some("iconset"),
            Format::Ios => Some("appiconset"),
            Format::Favicon => None,
        }
    }

    /// Whether it's a folder of files rather than one file.
    pub fn is_folder(self) -> bool {
        matches!(self, Format::Iconset | Format::Ios | Format::Favicon)
    }

    /// The sizes it's drawn at, each once; a picture only at `size`.
    pub fn sizes(self, size: u32) -> Vec<u32> {
        let mut sizes: Vec<u32> = match self {
            Format::Png | Format::Jpeg => vec![size],
            Format::Icns => ICNS_SIZES.to_vec(),
            Format::Ico => crate::apply::windows::ICO_SIZES.to_vec(),
            Format::Iconset => ICONSET.iter().map(|&(_, px)| px).collect(),
            Format::Ios => IOS.iter().map(|&(.., px)| px).collect(),
            Format::Favicon => FAVICON_ICO
                .iter()
                .copied()
                .chain(FAVICON_PNGS.iter().map(|&(_, px, _)| px))
                .collect(),
        };
        sizes.sort_unstable();
        sizes.dedup();
        sizes
    }
}

/// The bytes of a one-file `format` holding `icons` at its sizes ([`Format::sizes`]), a picture
/// `size` px square, or `None` for a folder of files or when `icons` lacks a size it needs.
pub fn encode(icons: &IconSet, format: Format, size: u32) -> Option<Vec<u8>> {
    let pngs = |sizes: &[u32]| -> Option<Vec<(u32, Vec<u8>)>> {
        sizes
            .iter()
            .map(|&px| image_at(icons, px).map(|img| (px, encode_png(img))))
            .collect()
    };
    match format {
        Format::Png => image_at(icons, size).map(encode_png),
        Format::Jpeg => image_at(icons, size).map(jpeg),
        Format::Icns => pngs(&ICNS_SIZES).map(|entries| crate::icns::write_icns(&entries)),
        Format::Ico => {
            pngs(&crate::apply::windows::ICO_SIZES).map(|entries| crate::ico::write_ico(&entries))
        }
        Format::Iconset | Format::Ios | Format::Favicon => None,
    }
}

/// Writes `icons`, drawn at the sizes of `format` ([`Format::sizes`]), to `dest`: one file, or a
/// folder of them, with its files replaced. The folders on the way are made if they aren't there.
pub fn write(icons: &IconSet, format: Format, size: u32, dest: &Path) -> io::Result<()> {
    let at = |px: u32| image_at(icons, px).ok_or_else(|| missing(px));
    if !format.is_folder() {
        let bytes = encode(icons, format, size).ok_or_else(|| missing(size))?;
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        return write_atomic(dest, &bytes);
    }
    std::fs::create_dir_all(dest)?;
    let put = |name: &str, bytes: &[u8]| write_atomic(&dest.join(name), bytes);
    match format {
        Format::Iconset => {
            for (name, px) in ICONSET {
                put(name, &encode_png(at(px)?))?;
            }
        }
        Format::Ios => {
            let mut images = Vec::new();
            for (idiom, points, scale, px) in IOS {
                let name = format!("Icon-{px}.png");
                if !images
                    .iter()
                    .any(|i: &serde_json::Value| i["filename"] == name)
                {
                    put(&name, &png_on_white(at(px)?))?;
                }
                images.push(json!({
                    "filename": name,
                    "idiom": idiom,
                    "scale": scale,
                    "size": points,
                }));
            }
            let contents = json!({
                "images": images,
                "info": {"author": "xcode", "version": 1},
            });
            put("Contents.json", pretty(&contents).as_bytes())?;
        }
        Format::Favicon => {
            let ico: Vec<(u32, Vec<u8>)> = FAVICON_ICO
                .iter()
                .map(|&px| at(px).map(|img| (px, encode_png(img))))
                .collect::<io::Result<_>>()?;
            put("favicon.ico", &crate::ico::write_ico(&ico))?;
            for (name, px, white) in FAVICON_PNGS {
                let img = at(px)?;
                put(
                    name,
                    &if white {
                        png_on_white(img)
                    } else {
                        encode_png(img)
                    },
                )?;
            }
            let manifest = json!({
                "icons": [
                    {"src": "/icon-192.png", "sizes": "192x192", "type": "image/png"},
                    {"src": "/icon-512.png", "sizes": "512x512", "type": "image/png"},
                ],
            });
            put("site.webmanifest", pretty(&manifest).as_bytes())?;
            put("favicon.html", FAVICON_HEAD.as_bytes())?;
        }
        Format::Png | Format::Jpeg | Format::Icns | Format::Ico => unreachable!("one file"),
    }
    Ok(())
}

/// The image of `px` px in `icons`.
fn image_at(icons: &IconSet, px: u32) -> Option<&RgbaImage> {
    icons
        .sizes
        .iter()
        .find(|(s, _)| *s == px)
        .map(|(_, img)| img)
}

fn missing(px: u32) -> io::Error {
    io::Error::other(format!("the icon wasn't drawn at {px} px"))
}

/// `img` on white as a JPEG, at a quality that shows no blocks on flat colour.
fn jpeg(img: &RgbaImage) -> Vec<u8> {
    let rgb = image::DynamicImage::ImageRgba8(crate::matte::flatten(img, WHITE)).to_rgb8();
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, 92)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .expect("encoding a JPEG into memory cannot fail");
    buf
}

/// `img` on white as a PNG with no alpha channel at all, which the App Store insists on.
fn png_on_white(img: &RgbaImage) -> Vec<u8> {
    let rgb = image::DynamicImage::ImageRgba8(crate::matte::flatten(img, WHITE)).to_rgb8();
    let mut buf = Vec::new();
    PngEncoder::new_with_quality(&mut buf, CompressionType::Best, FilterType::Adaptive)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .expect("encoding a PNG into memory cannot fail");
    buf
}

/// `value` as JSON laid out the way Xcode and people read it, ending in a new line.
fn pretty(value: &serde_json::Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("JSON values always serialize");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A see-through square with an opaque block in its middle, at every size in `sizes`.
    fn ladder(sizes: &[u32]) -> IconSet {
        IconSet {
            sizes: sizes
                .iter()
                .map(|&s| {
                    let img = RgbaImage::from_fn(s, s, |x, y| {
                        let inside =
                            (s / 4..s - s / 4).contains(&x) && (s / 4..s - s / 4).contains(&y);
                        image::Rgba(if inside {
                            [9, 80, 200, 255]
                        } else {
                            [0, 0, 0, 0]
                        })
                    });
                    (s, img)
                })
                .collect(),
        }
    }

    /// A fresh, empty folder for one test.
    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("folderskin-export-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The files in `dir`, sorted.
    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_name_says_the_format() {
        assert_eq!(Format::of(Path::new("a/Photos.icns")), Some(Format::Icns));
        assert_eq!(Format::of(Path::new("Photos.ICO")), Some(Format::Ico));
        assert_eq!(Format::of(Path::new("Photos.png")), Some(Format::Png));
        assert_eq!(Format::of(Path::new("Photos.JPEG")), Some(Format::Jpeg));
        assert_eq!(
            Format::of(Path::new("Photos.iconset")),
            Some(Format::Iconset)
        );
        assert_eq!(
            Format::of(Path::new("AppIcon.appiconset")),
            Some(Format::Ios)
        );
        assert_eq!(Format::of(Path::new("Photos.webp")), None);
        assert_eq!(Format::of(Path::new("Photos")), None);
        assert_eq!(Format::of(Path::new("-")), None);
        for format in Format::ALL {
            assert_eq!(Format::from_id(format.id()), Some(format));
        }
        let folders: Vec<Format> = Format::ALL.into_iter().filter(|f| f.is_folder()).collect();
        assert_eq!(folders, [Format::Iconset, Format::Ios, Format::Favicon]);
    }

    #[test]
    fn each_file_holds_its_own_sizes() {
        let icons = ladder(&Format::Icns.sizes(0));
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
        let ico = encode(&ladder(&Format::Ico.sizes(0)), Format::Ico, 0).unwrap();
        assert_eq!(
            u16::from_le_bytes([ico[4], ico[5]]),
            7,
            "seven sizes, 16 to 256 px"
        );
        let png = encode(&ladder(&[300]), Format::Png, 300).unwrap();
        let back = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (300, 300));
        assert_eq!(back.get_pixel(0, 0).0[3], 0, "see-through around it");
    }

    #[test]
    fn a_jpeg_is_on_white() {
        let jpeg = encode(&ladder(&[64]), Format::Jpeg, 64).unwrap();
        assert_eq!(
            image::guess_format(&jpeg).unwrap(),
            image::ImageFormat::Jpeg
        );
        let back = image::load_from_memory(&jpeg).unwrap().to_rgb8();
        assert!(
            back.get_pixel(1, 1).0.iter().all(|&c| c > 245),
            "{:?}",
            back.get_pixel(1, 1)
        );
        let middle = back.get_pixel(32, 32).0;
        assert!(middle[2] > 170 && middle[0] < 40, "{middle:?}");
    }

    #[test]
    fn nothing_at_the_sizes_a_file_holds_is_no_file() {
        assert!(encode(&ladder(&[48]), Format::Icns, 0).is_none());
        assert!(encode(&ladder(&[2048]), Format::Ico, 0).is_none());
        assert!(encode(&ladder(&[512]), Format::Png, 256).is_none());
        assert!(
            encode(&ladder(&[512]), Format::Iconset, 512).is_none(),
            "a folder, not a file"
        );
        let dir = temp_dir("missing");
        let e = write(&ladder(&[16]), Format::Iconset, 0, &dir.join("x.iconset")).unwrap_err();
        assert!(e.to_string().contains("32 px"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_iconset_is_every_file_iconutil_reads() {
        let dir = temp_dir("iconset");
        let set = dir.join("Photos.iconset");
        write(&ladder(&Format::Iconset.sizes(0)), Format::Iconset, 0, &set).unwrap();
        assert_eq!(files(&set).len(), 10);
        for (name, px) in ICONSET {
            let img = image::open(set.join(name)).unwrap();
            assert_eq!((img.width(), img.height()), (px, px), "{name}");
        }
        #[cfg(target_os = "macos")]
        {
            let icns = dir.join("Photos.icns");
            let made = std::process::Command::new("iconutil")
                .args(["-c", "icns", "-o"])
                .arg(&icns)
                .arg(&set)
                .status();
            if let Ok(status) = made {
                assert!(status.success(), "iconutil took the iconset");
                assert!(icns.is_file());
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_ios_icon_is_every_size_xcode_lists_on_white() {
        let dir = temp_dir("ios");
        let set = dir.join("AppIcon.appiconset");
        write(&ladder(&Format::Ios.sizes(0)), Format::Ios, 0, &set).unwrap();
        let contents: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(set.join("Contents.json")).unwrap())
                .unwrap();
        let images = contents["images"].as_array().unwrap();
        assert_eq!(images.len(), 18);
        assert_eq!(contents["info"]["author"], "xcode");
        for image in images {
            let file = set.join(image["filename"].as_str().unwrap());
            let bytes = std::fs::read(&file).unwrap();
            let img = image::load_from_memory(&bytes).unwrap();
            // No alpha channel at all, and white where the icon was see-through.
            assert_eq!(img.color(), image::ColorType::Rgb8, "{}", file.display());
            assert_eq!(img.to_rgb8().get_pixel(0, 0).0, [255, 255, 255]);
            // The scale times the size in points is the size in pixels.
            let points: f32 = image["size"]
                .as_str()
                .unwrap()
                .split('x')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let scale: f32 = image["scale"]
                .as_str()
                .unwrap()
                .trim_end_matches('x')
                .parse()
                .unwrap();
            assert_eq!(img.width(), (points * scale) as u32, "{image}");
        }
        assert!(images
            .iter()
            .any(|i| i["idiom"] == "ios-marketing" && i["size"] == "1024x1024"));
        // One file for each size in pixels, shared by the entries that need it.
        assert_eq!(files(&set).len(), 13 + 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_website_gets_its_favicons_a_manifest_and_the_head_lines() {
        let dir = temp_dir("favicon");
        let site = dir.join("favicon");
        write(
            &ladder(&Format::Favicon.sizes(0)),
            Format::Favicon,
            0,
            &site,
        )
        .unwrap();
        assert_eq!(
            files(&site),
            [
                "apple-touch-icon.png",
                "favicon-16x16.png",
                "favicon-32x32.png",
                "favicon.html",
                "favicon.ico",
                "icon-192.png",
                "icon-512.png",
                "site.webmanifest",
            ]
        );
        let ico = std::fs::read(site.join("favicon.ico")).unwrap();
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), 3, "16, 32 and 48 px");
        let touch = image::open(site.join("apple-touch-icon.png")).unwrap();
        assert_eq!(
            (touch.width(), touch.to_rgba8().get_pixel(0, 0).0[3]),
            (180, 255)
        );
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(site.join("site.webmanifest")).unwrap())
                .unwrap();
        assert_eq!(manifest["icons"][1]["sizes"], "512x512");
        let head = std::fs::read_to_string(site.join("favicon.html")).unwrap();
        for file in [
            "favicon.ico",
            "favicon-32x32.png",
            "apple-touch-icon.png",
            "site.webmanifest",
        ] {
            assert!(head.contains(file), "{file}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writing_again_replaces_what_was_there() {
        let dir = temp_dir("again");
        let file = dir.join("Photos.icns");
        write(&ladder(&ICNS_SIZES), Format::Icns, 0, &file).unwrap();
        write(&ladder(&ICNS_SIZES), Format::Icns, 0, &file).unwrap();
        let set = dir.join("Photos.iconset");
        write(&ladder(&Format::Iconset.sizes(0)), Format::Iconset, 0, &set).unwrap();
        write(&ladder(&Format::Iconset.sizes(0)), Format::Iconset, 0, &set).unwrap();
        assert_eq!(
            files(&dir),
            ["Photos.icns", "Photos.iconset"],
            "no temporary files left"
        );
        assert_eq!(files(&set).len(), 10);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
