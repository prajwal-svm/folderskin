//! The icon a folder currently shows in the OS file manager, as PNG bytes.
//!
//! macOS asks NSWorkspace, and Windows reads the folder's own `desktop.ini` and draws the icon
//! it names, so on both a folder that already wears an icon shows it — its own, not a stand-in.
//! Linux falls back to the compositor's plain default folder, as every platform but macOS used
//! to: it is what `commands::folder_icon` shows when this answers `None`.
//!
//! Windows was that fallback until recently, which is what made the app look broken there: a
//! folder dropped on the window showed the plain blue folder however it really looked, and
//! applying a skin left that same plain folder on screen, so nothing seemed to happen. Reading
//! the ini is also the one way to be sure: `SHGetFileInfo` would hand back whatever Explorer
//! has cached for the folder, which is the stale picture the hashed icon name exists to get
//! away from (`folderskin_core::apply::windows`).

use std::path::Path;

/// PNG bytes of the folder's current icon at about `size` px, or None if the OS cannot say.
#[cfg(target_os = "macos")]
pub fn current_icon_png(folder: &Path, size: u32) -> Option<Vec<u8>> {
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSGraphicsContext, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    let path = NSString::from_str(&folder.to_string_lossy());
    let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
    // Draw the icon into a fresh bitmap at the requested pixel size so we control the output.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            size as isize,
            size as isize,
            8,
            4,
            true,
            false,
            objc2_app_kit::NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    {
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        image.drawInRect_fromRect_operation_fraction(
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(size as f64, size as f64),
            ),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            objc2_app_kit::NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();
    }
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current_icon_png(_folder: &Path, _size: u32) -> Option<Vec<u8>> {
    None
}

/// The icon `folder`'s `desktop.ini` points Explorer at, drawn at about `size` px; `None` when
/// the folder has no icon of its own, or the one it names can't be read.
///
/// `None` is the answer for an ordinary folder, and the caller shows the compositor's plain
/// folder for it — the same picture Explorer draws, near enough, and far better than failing.
#[cfg(target_os = "windows")]
pub fn current_icon_png(folder: &Path, size: u32) -> Option<Vec<u8>> {
    use folderskin_core::apply::windows::{icon_resource_of, INI_NAME};

    // Not UTF-8 (a UTF-16 ini some other tool wrote) reads as no icon rather than a wrong one.
    let ini = std::fs::read_to_string(folder.join(INI_NAME)).ok()?;
    let (named, index) = icon_resource_of(&ini)?;
    let path = win::resolve_icon_path(folder, &named)?;
    let rgba = win::extract_icon(&path, index, size)?;
    Some(folderskin_core::raster::encode_png(&rgba))
}

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, RGBQUAD,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO,
    };

    /// Largest icon Windows stores. Asking for more only upscales, so the app's 512 px thumbnail
    /// is drawn from this and scaled by the webview, as Explorer scales it too.
    const MAX_ICON: u32 = 256;

    /// `%NAME%` replaced by the environment variable it names, as Explorer expands the paths in a
    /// `desktop.ini` (`%SystemRoot%\system32\imageres.dll` is how Windows writes its own).
    ///
    /// A name with no variable behind it is left as it was written, so a folder really called
    /// `%something%` still resolves. Pure, so it is unit-tested.
    pub fn expand_env(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        let mut rest = value;
        while let Some(start) = rest.find('%') {
            let (before, after) = rest.split_at(start);
            out.push_str(before);
            match after[1..].find('%') {
                Some(end) => {
                    let name = &after[1..=end];
                    match std::env::var(name) {
                        Ok(v) if !name.is_empty() => out.push_str(&v),
                        _ => {
                            out.push('%');
                            out.push_str(name);
                            out.push('%');
                        }
                    }
                    rest = &after[end + 2..];
                }
                // An unpaired %: nothing left to expand.
                None => {
                    out.push_str(after);
                    return out;
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// The file a `desktop.ini` icon value names, as a path that exists: relative values are the
    /// folder's own files, which is how FolderSkin writes its own.
    pub fn resolve_icon_path(folder: &Path, named: &str) -> Option<PathBuf> {
        let expanded = expand_env(named.trim_matches('"'));
        let path = Path::new(&expanded);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            folder.join(path)
        };
        path.is_file().then_some(path)
    }

    /// The icon at `index` in `path`, drawn at `size` px (capped at [`MAX_ICON`]), as straight
    /// RGBA. A negative index is a resource id, which is how Windows names its own icons.
    pub fn extract_icon(path: &Path, index: i32, size: u32) -> Option<image::RgbaImage> {
        let side = size.clamp(16, MAX_ICON) as i32;
        let wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut icon: HICON = std::ptr::null_mut();
        // SAFETY: `wide` is a NUL-terminated path that outlives the call, and one icon is asked
        // for into one slot. The returned count says whether `icon` was written.
        let got = unsafe {
            PrivateExtractIconsW(
                wide.as_ptr(),
                index,
                side,
                side,
                &mut icon,
                std::ptr::null_mut(),
                1,
                0,
            )
        };
        if got != 1 || icon.is_null() {
            return None;
        }
        let rgba = icon_to_rgba(icon);
        // SAFETY: `icon` came from PrivateExtractIconsW and is ours to free exactly once.
        unsafe { DestroyIcon(icon) };
        rgba
    }

    /// An `HICON`'s pixels as straight RGBA.
    fn icon_to_rgba(icon: HICON) -> Option<image::RgbaImage> {
        // SAFETY: `icon` is a live icon; GetIconInfo fills the struct and hands over two bitmaps
        // that this function owns and deletes before it returns.
        let mut info: ICONINFO = unsafe { std::mem::zeroed() };
        if unsafe { GetIconInfo(icon, &mut info) } == 0 {
            return None;
        }
        let result = colour_bitmap_rgba(&info);
        for bitmap in [info.hbmColor, info.hbmMask] {
            if !bitmap.is_null() {
                unsafe { DeleteObject(bitmap.cast()) };
            }
        }
        result
    }

    /// The colour bitmap of `info` as straight RGBA, with the mask filling in the alpha when the
    /// icon carries none of its own (every icon older than XP, and a few since).
    fn colour_bitmap_rgba(info: &ICONINFO) -> Option<image::RgbaImage> {
        if info.hbmColor.is_null() {
            return None;
        }
        let mut bm: BITMAP = unsafe { std::mem::zeroed() };
        let read = unsafe {
            GetObjectW(
                info.hbmColor.cast(),
                std::mem::size_of::<BITMAP>() as i32,
                std::ptr::from_mut(&mut bm).cast(),
            )
        };
        if read == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
            return None;
        }
        let (width, height) = (bm.bmWidth as u32, bm.bmHeight as u32);

        let mut pixels = read_dib(info.hbmColor, width, height, 32)?;
        // BGRA as GDI keeps it; the image crate wants RGBA.
        let (rgba, _) = pixels.as_chunks_mut::<4>();
        for px in rgba.iter_mut() {
            px.swap(0, 2);
        }
        if rgba.iter().all(|px| px[3] == 0) {
            apply_mask(&mut pixels, info.hbmMask, width, height);
        }
        image::RgbaImage::from_raw(width, height, pixels)
    }

    /// A `BITMAPINFO` with room for the colour table `GetDIBits` writes after the header.
    ///
    /// For any format of 8 bits per pixel or fewer, `GetDIBits` fills in a palette of
    /// `2 ^ biBitCount` entries — two of them for the 1-bpp mask. `windows_sys` declares
    /// `BITMAPINFO.bmiColors` as a single `RGBQUAD`, being the C struct translated literally, so
    /// handing GDI one of those is a write four bytes past the end of it. The `.ico` files that
    /// reach the mask path are the ones with no alpha channel, which is every 24-bit icon —
    /// `imageres.dll` and anything old — so this is a path folders really take. Two entries is
    /// all any depth this reads can ask for.
    #[repr(C)]
    struct DibInfo {
        header: BITMAPINFOHEADER,
        colours: [RGBQUAD; 2],
    }

    // The palette has to sit immediately after the header, which is how GDI appends one to a
    // `BITMAPINFO`. If either struct ever gains padding this stops compiling rather than going
    // quietly wrong again.
    const _: () = assert!(
        std::mem::size_of::<DibInfo>()
            == std::mem::size_of::<BITMAPINFOHEADER>() + 2 * std::mem::size_of::<RGBQUAD>()
    );
    const _: () = assert!(std::mem::align_of::<DibInfo>() == std::mem::align_of::<BITMAPINFO>());

    /// `bitmap`'s pixels as a top-down DIB at `bits` bits per pixel.
    fn read_dib(
        bitmap: windows_sys::Win32::Graphics::Gdi::HBITMAP,
        width: u32,
        height: u32,
        bits: u16,
    ) -> Option<Vec<u8>> {
        // Rows of a DIB are padded to 4 bytes; at 32 bpp that is already the case, and at 1 bpp
        // (the mask) it is not.
        let stride = (width as usize * bits as usize).div_ceil(32) * 4;
        let mut buffer = vec![0u8; stride * height as usize];

        let mut info = DibInfo {
            header: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                // Negative: top-down, so the rows come in the order the image crate wants.
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: bits,
                biCompression: BI_RGB,
                ..unsafe { std::mem::zeroed() }
            },
            // GDI writes the palette here.
            colours: unsafe { std::mem::zeroed() },
        };

        // SAFETY: a memory DC is created and freed here; `buffer` is exactly the size the header
        // describes, so GetDIBits writes inside it; and `info` is a BITMAPINFO followed by the
        // colour table GDI appends to one, so the palette lands inside it too.
        let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        if dc.is_null() {
            return None;
        }
        let lines = unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                height,
                buffer.as_mut_ptr().cast(),
                std::ptr::from_mut(&mut info).cast::<BITMAPINFO>(),
                DIB_RGB_COLORS,
            )
        };
        unsafe { DeleteDC(dc) };
        (lines > 0).then_some(buffer)
    }

    /// Fills in the alpha of `pixels` from a 1-bit AND mask: a set bit is a hole in the icon.
    fn apply_mask(
        pixels: &mut [u8],
        mask: windows_sys::Win32::Graphics::Gdi::HBITMAP,
        width: u32,
        height: u32,
    ) {
        // An icon with no alpha and no mask is opaque, which is also the sensible fallback when
        // the mask can't be read.
        let bits = if mask.is_null() {
            None
        } else {
            read_dib(mask, width, height, 1)
        };
        let stride = (width as usize).div_ceil(32) * 4;
        for y in 0..height as usize {
            for x in 0..width as usize {
                let transparent = bits.as_ref().is_some_and(|bits| {
                    bits.get(y * stride + x / 8)
                        .is_some_and(|byte| byte >> (7 - (x % 8)) & 1 == 1)
                });
                let at = (y * width as usize + x) * 4 + 3;
                if let Some(alpha) = pixels.get_mut(at) {
                    *alpha = if transparent { 0 } else { 255 };
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn environment_variables_in_an_icon_path_are_expanded() {
            std::env::set_var("FOLDERSKIN_TEST_DIR", "C:\\Art");
            assert_eq!(expand_env("%FOLDERSKIN_TEST_DIR%\\a.ico"), "C:\\Art\\a.ico");
            assert_eq!(
                expand_env("x %FOLDERSKIN_TEST_DIR% y %FOLDERSKIN_TEST_DIR%"),
                "x C:\\Art y C:\\Art"
            );
            // Left alone: no variable of that name, an unpaired %, an empty name, none at all.
            assert_eq!(
                expand_env("%NOT_A_REAL_VAR_XYZ%\\a.ico"),
                "%NOT_A_REAL_VAR_XYZ%\\a.ico"
            );
            assert_eq!(expand_env("100% of it"), "100% of it");
            assert_eq!(expand_env("%%"), "%%");
            assert_eq!(expand_env("folderskin.ico"), "folderskin.ico");
            assert_eq!(expand_env(""), "");
        }

        #[test]
        fn an_icon_path_resolves_against_the_folder_and_must_exist() {
            let dir = std::env::temp_dir().join(format!("folderskin-icon-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let ico = dir.join("folderskin-0123456789abcdef.ico");
            std::fs::write(&ico, b"not really an icon").unwrap();

            // Relative: the folder's own file, which is what FolderSkin writes.
            assert_eq!(
                resolve_icon_path(&dir, "folderskin-0123456789abcdef.ico"),
                Some(ico.clone())
            );
            // Absolute, and quoted as some tools write it.
            let quoted = format!("\"{}\"", ico.display());
            assert_eq!(resolve_icon_path(&dir, &quoted), Some(ico));
            // Nothing there.
            assert_eq!(resolve_icon_path(&dir, "gone.ico"), None);
            std::fs::remove_dir_all(&dir).unwrap();
        }

        /// The whole Windows path, end to end: a folder wearing an icon gives back that icon's
        /// pixels, and a folder wearing none gives back nothing (the caller draws its own).
        ///
        /// This is the bug the Windows build shipped with — the panel drew the plain folder
        /// whatever a folder really looked like — so it is worth a test that goes through the
        /// ini, the file it names, and the icon inside it.
        #[test]
        fn a_folder_wearing_an_icon_gives_back_that_icons_pixels() {
            use folderskin_core::apply::windows::{desktop_ini_contents, ico_file_name, Before};

            let dir = std::env::temp_dir().join(format!("folderskin-wears-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();

            // A plain folder has no icon of its own.
            assert_eq!(super::super::current_icon_png(&dir, 256), None);

            // A real `.ico`: one flat teal image at each size Explorer picks from.
            let teal = image::Rgba([0x2E, 0x7D, 0x5B, 0xFF]);
            let entries: Vec<(u32, Vec<u8>)> = [32u32, 256]
                .iter()
                .map(|&px| {
                    (
                        px,
                        folderskin_core::raster::encode_png(&image::RgbaImage::from_pixel(
                            px, px, teal,
                        )),
                    )
                })
                .collect();
            let ico = folderskin_core::ico::write_ico(&entries);
            let name = ico_file_name(&ico);
            std::fs::write(dir.join(&name), &ico).unwrap();
            std::fs::write(
                dir.join("desktop.ini"),
                desktop_ini_contents(None, &name, Before::default()).as_bytes(),
            )
            .unwrap();

            let png = super::super::current_icon_png(&dir, 256).expect("the icon it wears");
            let img = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(img.dimensions(), (256, 256));
            let middle = img.get_pixel(128, 128).0;
            assert_eq!(&middle[..3], &teal.0[..3], "the icon's own colour");
            assert_eq!(middle[3], 255, "opaque where the icon is");

            // An ini that names a file which isn't there falls back rather than failing.
            std::fs::write(
                dir.join("desktop.ini"),
                desktop_ini_contents(None, "folderskin-ffffffffffffffff.ico", Before::default())
                    .as_bytes(),
            )
            .unwrap();
            assert_eq!(super::super::current_icon_png(&dir, 256), None);

            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}

/// Shows the app's own icon in the Dock even for the unbundled dev binary.
#[cfg(target_os = "macos")]
pub fn set_dock_icon(png: &[u8]) {
    use objc2::AnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(png);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon(_png: &[u8]) {}
