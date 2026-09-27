//! macOS `.icns` container.
//!
//! An `.icns` is the four bytes `icns` and the file's whole length, then one element per image:
//! a four-letter kind, the element's length (header included) and the image data. Everything is
//! big-endian. Most kinds hold a whole PNG file, so their data is exactly the PNG bytes the
//! compositor produced, as in [`crate::ico`]. The 16 and 32 px kinds hold `ARGB` instead, each
//! channel run-length compressed, as Apple's own `iconutil` writes them: AppKit reads a PNG in
//! the older 16 and 32 px kinds (`icp4`, `icp5`) too, but `iconutil` takes it for noise. Written
//! here rather than by AppKit, so an icon can be saved as `.icns` on any system.

/// How an element holds its image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Data {
    /// A whole PNG file.
    Png,
    /// `ARGB`, then the alpha, red, green and blue channels, each compressed by [`rle`].
    Argb,
}

/// The kinds each size is stored as. A size can fill two kinds: 32 px is the 32 point icon and
/// the 16 point one on a Retina screen. Sizes not listed are left out.
const KINDS: [(u32, &[u8; 4], Data); 10] = [
    (16, b"ic04", Data::Argb),
    (32, b"ic05", Data::Argb),
    (32, b"ic11", Data::Png),
    (64, b"ic12", Data::Png),
    (128, b"ic07", Data::Png),
    (256, b"ic08", Data::Png),
    (256, b"ic13", Data::Png),
    (512, b"ic09", Data::Png),
    (512, b"ic14", Data::Png),
    (1024, b"ic10", Data::Png),
];

/// Packs `(size, png bytes)` entries into an `.icns`, each size under every kind macOS reads it
/// from, smallest first. Sizes macOS has no kind for (such as 24, 48 or 2048) are skipped, so
/// callers can hand over the whole icon ladder, and so is a 16 or 32 px entry that isn't a PNG
/// of that size.
pub fn write_icns(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (size, kind, data) in KINDS {
        let Some((_, png)) = entries.iter().find(|(s, _)| *s == size) else {
            continue;
        };
        let bytes = match data {
            Data::Png => png.clone(),
            Data::Argb => match argb(png, size) {
                Some(bytes) => bytes,
                None => continue,
            },
        };
        body.extend_from_slice(kind);
        body.extend_from_slice(&(8 + bytes.len() as u32).to_be_bytes());
        body.extend_from_slice(&bytes);
    }
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(8 + body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// A PNG `size` px square as an `ARGB` element's data: straight (not premultiplied) colour,
/// one channel after another.
fn argb(png: &[u8], size: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    if img.dimensions() != (size, size) {
        return None;
    }
    let mut out = b"ARGB".to_vec();
    for channel in [3, 0, 1, 2] {
        let plane: Vec<u8> = img.pixels().map(|p| p.0[channel]).collect();
        rle(&plane, &mut out);
    }
    Some(out)
}

/// Compresses `bytes` onto `out` the way icns compresses a channel: a byte under 0x80 is followed
/// by that many bytes and one more, as they are; one from 0x80 is followed by a byte repeated
/// three times more than it is over 0x80.
fn rle(bytes: &[u8], out: &mut Vec<u8>) {
    // A byte repeated this many times or more is worth a run.
    const RUN: usize = 3;
    let run_at = |i: usize| {
        let b = bytes[i];
        bytes[i..].iter().take(130).take_while(|&&x| x == b).count()
    };
    let mut i = 0;
    while i < bytes.len() {
        let run = run_at(i);
        if run >= RUN {
            out.push(0x80 + (run - RUN) as u8);
            out.push(bytes[i]);
            i += run;
            continue;
        }
        // As they are, up to 128 bytes or where a run starts.
        let start = i;
        while i < bytes.len() && i - start < 128 && (i == start || run_at(i) < RUN) {
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&bytes[start..i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The elements of an `.icns`: each kind and its data.
    fn elements(icns: &[u8]) -> Vec<(String, Vec<u8>)> {
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize,
            icns.len()
        );
        let mut out = Vec::new();
        let mut at = 8;
        while at < icns.len() {
            let kind = String::from_utf8(icns[at..at + 4].to_vec()).unwrap();
            let len = u32::from_be_bytes(icns[at + 4..at + 8].try_into().unwrap()) as usize;
            out.push((kind, icns[at + 8..at + len].to_vec()));
            at += len;
        }
        assert_eq!(at, icns.len(), "the elements fill the file exactly");
        out
    }

    /// What [`rle`] compressed, `count` bytes of it, and how many bytes that took.
    fn unrle(data: &[u8], count: usize) -> (Vec<u8>, usize) {
        let (mut out, mut i) = (Vec::new(), 0);
        while out.len() < count {
            let n = data[i] as usize;
            i += 1;
            if n < 0x80 {
                out.extend_from_slice(&data[i..i + n + 1]);
                i += n + 1;
            } else {
                out.extend(std::iter::repeat_n(data[i], n - 0x80 + 3));
                i += 1;
            }
        }
        assert_eq!(out.len(), count, "no run goes past the end");
        (out, i)
    }

    /// A picture `size` px square with runs, steps and see-through corners, as a PNG.
    fn picture(size: u32) -> (image::RgbaImage, Vec<u8>) {
        let img = image::RgbaImage::from_fn(size, size, |x, y| {
            let alpha = match (x < size / 4 && y < size / 4, y % 5) {
                (true, _) => 0,
                (false, 0) => 128,
                _ => 255,
            };
            image::Rgba([(x * 255 / size) as u8, 0x9D, (y % 3 * 60) as u8, alpha])
        });
        let png = crate::raster::encode_png(&img);
        (img, png)
    }

    #[test]
    fn runs_and_the_bytes_between_them_come_back_as_they_were() {
        let mut bytes = vec![7u8; 200];
        bytes.extend((0..300).map(|i| (i * 37 % 251) as u8));
        bytes.extend([1, 1, 2, 2, 3, 3, 3, 9]);
        bytes.extend(vec![0u8; 131]);
        for sample in [&bytes[..], &[], &[5], &[5, 5], &[5, 5, 5], &bytes[..130]] {
            let mut packed = Vec::new();
            rle(sample, &mut packed);
            let (back, used) = unrle(&packed, sample.len());
            assert_eq!((back.as_slice(), used), (sample, packed.len()));
        }
    }

    #[test]
    fn every_size_macos_reads_goes_in_under_each_of_its_kinds() {
        let ladder: Vec<(u32, Vec<u8>)> = [2048, 1024, 512, 256, 128, 64, 48, 32, 24, 16]
            .iter()
            .map(|&s| (s, picture(s).1))
            .collect();
        let found = elements(&write_icns(&ladder));
        let kinds: Vec<&str> = found.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            kinds,
            ["ic04", "ic05", "ic11", "ic12", "ic07", "ic08", "ic13", "ic09", "ic14", "ic10"]
        );
        let data = |kind: &str| found.iter().find(|(k, _)| k == kind).unwrap().1.clone();
        assert_eq!(data("ic11"), picture(32).1);
        assert_eq!(data("ic10"), picture(1024).1);
        // The small ones are the same picture, channel by channel.
        for (kind, size) in [("ic04", 16u32), ("ic05", 32)] {
            let d = data(kind);
            assert_eq!(&d[..4], b"ARGB");
            let (img, _) = picture(size);
            let mut at = 4;
            for channel in [3, 0, 1, 2] {
                let (plane, used) = unrle(&d[at..], (size * size) as usize);
                let want: Vec<u8> = img.pixels().map(|p| p.0[channel]).collect();
                assert_eq!(plane, want, "{kind} channel {channel}");
                at += used;
            }
            assert_eq!(at, d.len(), "{kind}: the four channels fill it");
        }
    }

    #[test]
    fn nothing_to_pack_is_an_empty_icns() {
        assert_eq!(write_icns(&[]), b"icns\0\0\0\x08");
        assert_eq!(write_icns(&[(48, vec![1, 2, 3])]), b"icns\0\0\0\x08");
        // A small size that isn't a PNG of that size is left out rather than written wrong.
        assert_eq!(write_icns(&[(16, vec![1, 2, 3])]), b"icns\0\0\0\x08");
        assert_eq!(write_icns(&[(16, picture(32).1)]), b"icns\0\0\0\x08");
    }

    /// macOS's own tool reads it back into every size, and the PNG ones with the same pixels.
    /// (`iconutil` writes the `ARGB` ones out wrong, Apple's own included: it takes the colour as
    /// premultiplied and loses the last run, so those are checked the way Finder draws them, in
    /// [`appkit_reads_the_small_sizes_as_they_are`].)
    #[cfg(target_os = "macos")]
    #[test]
    fn iconutil_reads_it() {
        let ladder: Vec<(u32, Vec<u8>)> = [1024, 512, 256, 128, 64, 32, 16]
            .iter()
            .map(|&s| (s, picture(s).1))
            .collect();
        let dir = std::env::temp_dir().join(format!("folderskin-icns-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let icns = dir.join("test.icns");
        std::fs::write(&icns, write_icns(&ladder)).unwrap();
        let set = dir.join("test.iconset");
        let status = std::process::Command::new("iconutil")
            .args(["-c", "iconset", "-o"])
            .arg(&set)
            .arg(&icns)
            .status();
        let Ok(status) = status else {
            return; // no iconutil on this Mac
        };
        assert!(status.success(), "iconutil couldn't read it");
        for name in ["icon_16x16.png", "icon_32x32.png", "icon_256x256.png"] {
            assert!(set.join(name).is_file(), "{name} missing");
        }
        for (name, size) in [
            ("icon_16x16@2x.png", 32),
            ("icon_32x32@2x.png", 64),
            ("icon_512x512@2x.png", 1024),
        ] {
            let back = image::open(set.join(name))
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .to_rgba8();
            assert_same(&back, &picture(size).0, name);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// AppKit, which Finder draws icons with, reads the 16 and 32 px `ARGB` images as they were.
    #[cfg(target_os = "macos")]
    #[test]
    fn appkit_reads_the_small_sizes_as_they_are() {
        use objc2::rc::Retained;
        use objc2::AllocAnyThread;
        use objc2_app_kit::{NSBitmapFormat, NSBitmapImageRep, NSImage};
        use objc2_foundation::NSData;

        let ladder: Vec<(u32, Vec<u8>)> = [32, 16].iter().map(|&s| (s, picture(s).1)).collect();
        let data = NSData::with_bytes(&write_icns(&ladder));
        let icon = NSImage::initWithData(NSImage::alloc(), &data).expect("AppKit reads it");
        let reps = icon.representations();
        for size in [16u32, 32] {
            // The 1x image: as many points as pixels.
            let rep = (0..reps.count())
                .map(|i| reps.objectAtIndex(i))
                .find(|r| r.pixelsWide() == size as isize && r.size().width == f64::from(size))
                .unwrap_or_else(|| panic!("no {size} px image"));
            let bitmap: Retained<NSBitmapImageRep> = rep.downcast().expect("a bitmap");
            let format = bitmap.bitmapFormat();
            let premultiplied = !format.contains(NSBitmapFormat::AlphaNonpremultiplied);
            let alpha_first = format.contains(NSBitmapFormat::AlphaFirst);
            assert_eq!((bitmap.samplesPerPixel(), bitmap.bitsPerSample()), (4, 8));
            let seen = image::RgbaImage::from_fn(size, size, |x, y| {
                let mut p = [0usize; 4];
                // SAFETY: four samples of a four-sample pixel inside the bitmap.
                unsafe {
                    bitmap.getPixel_atX_y(
                        std::ptr::NonNull::new(p.as_mut_ptr()).unwrap(),
                        x as isize,
                        y as isize,
                    )
                };
                let [r, g, b, a] = if alpha_first {
                    [p[1], p[2], p[3], p[0]]
                } else {
                    p
                };
                let straight = |c: usize| match (premultiplied, a) {
                    (true, 1..) => ((c * 255 + a / 2) / a).min(255),
                    _ => c,
                };
                image::Rgba([
                    straight(r) as u8,
                    straight(g) as u8,
                    straight(b) as u8,
                    a as u8,
                ])
            });
            assert_same(&seen, &picture(size).0, &format!("{size} px"));
        }
    }

    /// `back` is `want`: see-through in the same places, and the rest a level or two off at most
    /// (colour management, or unpremultiplying half-clear pixels), where noise is far off.
    #[cfg(target_os = "macos")]
    fn assert_same(back: &image::RgbaImage, want: &image::RgbaImage, what: &str) {
        assert_eq!(back.dimensions(), want.dimensions(), "{what}");
        for (x, y, w) in want.enumerate_pixels() {
            let b = back.get_pixel(x, y);
            assert!(
                b.0[3].abs_diff(w.0[3]) <= 1,
                "{what} ({x}, {y}): {b:?} for {w:?}"
            );
            if w.0[3] >= 128 {
                let off = (0..3).map(|i| b.0[i].abs_diff(w.0[i])).max().unwrap();
                assert!(off <= 4, "{what} ({x}, {y}): {b:?} for {w:?}");
            }
        }
    }
}
