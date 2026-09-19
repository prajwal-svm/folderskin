//! macOS folder icons via `NSWorkspace.setIcon(_:forFile:options:)`.
//!
//! macOS does not take a file of icons; it takes one `NSImage` carrying a representation per
//! size and writes the whole thing into the folder itself (as the invisible `Icon\r` file and a
//! custom-icon Finder flag). So every rendered size up to 1024 px is added as an
//! `NSBitmapImageRep` whose `size` is its pixel size — that is what tells AppKit "this rep *is*
//! the 32 px artwork" rather than a scaled-down 1024. Reverting is the same call with `nil`,
//! which clears the flag and removes the file.
//!
//! Finder is slow to notice. When one custom icon replaces another it keeps drawing the old
//! one, on the Desktop and in its windows, until the folder is opened. Apple's workaround is to
//! clear the icon first, so every apply goes from no icon to the new one, which Finder does
//! redraw (developer.apple.com/forums/thread/788252). After any change Finder is also told the
//! folder and the folder around it changed.
//!
//! Building the image ([`prepare`]) is separate from attaching it ([`apply`]), so a whole tree
//! of folders shares one image rather than encoding the same icon again for every folder.
//!
//! `setIcon` works off the main thread, but not on two threads at once: calls that overlap
//! garble each other's `Icon\r` (a 37 KB icon came out as 286 bytes) or fail outright. An apply,
//! a tree of them and a measurement can all be running together, so every change here takes one
//! lock for the whole process.

use super::ApplyError;
use crate::compositor::IconSet;
use objc2::rc::{autoreleasepool, Retained};
use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageRep, NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
use objc2_foundation::{NSData, NSSize, NSString};
use std::path::Path;
use std::sync::Mutex;

/// Point size of the image the reps hang off; the largest rep is the 1:1 one.
const IMAGE_POINTS: f64 = 1024.0;
/// Largest representation macOS has any use for in a folder icon.
const MAX_REP: u32 = 1024;

/// The icon as the one `NSImage` macOS wants: a representation per rendered size up to
/// [`MAX_REP`].
pub fn prepare(icons: &IconSet) -> Result<Retained<NSImage>, ApplyError> {
    // The PNG round trip leaves temporaries in the current pool; drain them here rather than
    // whenever the calling thread happens to end.
    autoreleasepool(|_| {
        let image =
            NSImage::initWithSize(NSImage::alloc(), NSSize::new(IMAGE_POINTS, IMAGE_POINTS));

        let mut reps = 0usize;
        for &(size, _) in &icons.sizes {
            if size > MAX_REP {
                continue;
            }
            let png = icons
                .png(size)
                .ok_or_else(|| ApplyError::Platform(format!("the icon has no {size} px size")))?;
            let data = NSData::with_bytes(&png);
            let rep = NSBitmapImageRep::imageRepWithData(&data).ok_or_else(|| {
                ApplyError::Platform(format!("macOS could not read the {size} px icon"))
            })?;
            // Without this the rep reports its own pixel size in points and AppKit treats the
            // small reps as tiny images rather than as the small-size artwork.
            rep.setSize(NSSize::new(f64::from(size), f64::from(size)));
            image.addRepresentation(&rep);
            reps += 1;
        }
        if reps == 0 {
            return Err(ApplyError::Platform(
                "the rendered icon has no size macOS can use".into(),
            ));
        }
        Ok(image)
    })
}

/// Attaches a [`prepare`]d image to `folder` as its custom Finder icon.
pub fn apply(folder: &Path, image: &NSImage) -> Result<(), ApplyError> {
    set_icon(folder, Some(image))
}

/// Clears `folder`'s custom icon, putting the system folder icon back.
pub fn revert(folder: &Path) -> Result<(), ApplyError> {
    set_icon(folder, None)
}

/// True when Finder draws a custom icon for `folder`: the custom-icon flag in its Finder info,
/// which a revert clears whoever set it.
pub fn has_custom_icon(folder: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
        return false;
    };
    let mut info = [0u8; 32];
    // SAFETY: both names are NUL-terminated and `info` is writable for its whole length.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            FINDER_INFO.as_ptr(),
            info.as_mut_ptr().cast(),
            info.len(),
            0,
            0,
        )
    };
    // The Finder flags are the big-endian u16 at bytes 8 and 9, and kHasCustomIcon is 0x0400.
    read >= 10 && info[8] & 0x04 != 0
}

/// The extended attribute macOS keeps a file's or folder's Finder flags in.
const FINDER_INFO: &std::ffi::CStr = c"com.apple.FinderInfo";

/// Held for every change of a folder's icon: `setIcon` calls must not overlap.
static SET_ICON: Mutex<()> = Mutex::new(());

/// The one call that changes a folder's icon; `None` reverts it.
///
/// Each change holds [`SET_ICON`], and runs in its own autorelease pool: the icon data AppKit
/// encodes for a folder is megabytes, and a tree of folders is changed one after another on one
/// thread, which would otherwise hold every folder's copy until the thread ends.
fn set_icon(folder: &Path, image: Option<&NSImage>) -> Result<(), ApplyError> {
    // A panic while holding the lock leaves nothing half-done that the next caller could trip
    // over, so a poisoned lock is simply taken.
    let _one_at_a_time = SET_ICON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let path = NSString::from_str(&folder.to_string_lossy());
        let options = NSWorkspaceIconCreationOptions::empty();
        if image.is_some() {
            // Clearing an icon that isn't there is harmless, and if clearing fails, so does
            // the set.
            workspace.setIcon_forFile_options(None, &path, options);
        }
        let ok = workspace.setIcon_forFile_options(image, &path, options);
        if !ok {
            return Err(ApplyError::Platform(
                "macOS refused to change this folder's icon (is it writable?)".into(),
            ));
        }
        workspace.noteFileSystemChanged_(&path);
        if let Some(parent) = folder.parent() {
            workspace.noteFileSystemChanged_(&NSString::from_str(&parent.to_string_lossy()));
        }
        Ok(())
    })
}

/// Bytes of the icon data macOS keeps for `folder`: the resource fork of its `Icon\r` file,
/// which is where `setIcon` puts the whole image. `None` when there is no such file.
pub(crate) fn icon_file_bytes(folder: &Path) -> Option<u64> {
    let icon = folder.join("Icon\r");
    // The fork reads as a file of its own under this name, on APFS and HFS+ alike.
    if let Ok(meta) = std::fs::metadata(icon.join("..namedfork/rsrc")) {
        return Some(meta.len());
    }
    // Otherwise the same bytes, seen as the extended attribute macOS maps the fork to.
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(icon.as_os_str().as_bytes()).ok()?;
    // SAFETY: both names are NUL-terminated; a null buffer of size 0 only asks for the size.
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            RESOURCE_FORK.as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    u64::try_from(size).ok()
}

/// The extended attribute a file's resource fork is visible as.
const RESOURCE_FORK: &std::ffi::CStr = c"com.apple.ResourceFork";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::{apply_icon, revert_icon, tempfile_dir};

    /// A flat-colour icon set at the sizes a real render produces.
    fn solid_icons(sizes: &[u32]) -> IconSet {
        IconSet {
            sizes: sizes
                .iter()
                .map(|&size| {
                    (
                        size,
                        image::RgbaImage::from_pixel(
                            size,
                            size,
                            image::Rgba([0xF2, 0x6B, 0x21, 0xFF]),
                        ),
                    )
                })
                .collect(),
        }
    }

    /// `GetFileInfo -a` prints the Finder attribute string, e.g. `avbstclinmedz`. The letter
    /// is uppercase when the flag is set, so `C` means "has a custom icon".
    fn attributes(folder: &Path) -> String {
        let out = std::process::Command::new("/usr/bin/GetFileInfo")
            .arg("-a")
            .arg(folder)
            .output()
            .expect("GetFileInfo (part of the Xcode command line tools)");
        assert!(
            out.status.success(),
            "GetFileInfo failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Writes `folder`'s 32 bytes of Finder info.
    fn set_finder_info(folder: &Path, info: &[u8; 32]) {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(folder.as_os_str().as_bytes()).unwrap();
        // SAFETY: both names are NUL-terminated and `info` is readable for its whole length.
        let status = unsafe {
            libc::setxattr(
                path.as_ptr(),
                FINDER_INFO.as_ptr(),
                info.as_ptr().cast(),
                info.len(),
                0,
                0,
            )
        };
        assert_eq!(status, 0, "setxattr failed");
    }

    #[test]
    fn a_custom_icon_is_read_from_the_finder_flags() {
        let folder = tempfile_dir();
        assert!(!has_custom_icon(&folder), "a new folder has no Finder info");

        let mut info = [0u8; 32];
        info[8] = 0x04;
        set_finder_info(&folder, &info);
        assert!(has_custom_icon(&folder));

        // Other flags alone don't make a custom icon.
        info[8] = 0x40;
        info[9] = 0x10;
        set_finder_info(&folder, &info);
        assert!(!has_custom_icon(&folder));
    }

    #[test]
    #[ignore = "touches a real folder and needs a desktop session: cargo test -p folderskin-core -- --ignored macos_roundtrip"]
    fn macos_roundtrip() {
        let folder = tempfile_dir();

        let before = attributes(&folder);
        assert!(
            before.contains('c') && !before.contains('C'),
            "a fresh folder should have no custom icon: {before}"
        );

        apply_icon(
            &folder,
            &solid_icons(&[16, 24, 32, 48, 64, 128, 256, 512, 1024]),
        )
        .unwrap();
        let after = attributes(&folder);
        assert!(
            after.contains('C'),
            "no custom-icon flag after apply: {after}"
        );
        assert!(has_custom_icon(&folder));

        // A second skin over the first: the icon is cleared on the way, but never left cleared.
        apply_icon(&folder, &solid_icons(&[16, 32, 128, 512])).unwrap();
        let again = attributes(&folder);
        assert!(
            again.contains('C'),
            "no custom-icon flag after a second apply: {again}"
        );
        assert!(
            folder.join("Icon\r").exists(),
            "the Icon file is missing after a second apply"
        );

        revert_icon(&folder).unwrap();
        let reverted = attributes(&folder);
        assert!(
            reverted.contains('c') && !reverted.contains('C'),
            "custom-icon flag survived revert: {reverted}"
        );
        assert!(!has_custom_icon(&folder));

        println!("GetFileInfo -a: before={before} after={after} reverted={reverted}");
    }

    #[test]
    #[ignore = "touches real folders and needs a desktop session: cargo test -p folderskin-core -- --ignored prepared"]
    fn one_prepared_icon_goes_on_several_folders_and_is_measured() {
        use crate::apply::{apply_prepared, bytes_per_folder, prepare_icon};

        let icon = prepare_icon(&solid_icons(&[16, 32, 64, 128, 256, 512, 1024])).unwrap();
        let folders = [tempfile_dir(), tempfile_dir(), tempfile_dir()];
        for folder in &folders {
            apply_prepared(folder, &icon).unwrap();
            assert!(has_custom_icon(folder));
        }

        let measured = bytes_per_folder(&icon).unwrap();
        assert_eq!(
            Some(measured),
            icon_file_bytes(&folders[0]),
            "the scratch folder's copy weighs what a real folder's does"
        );

        for folder in &folders {
            revert_icon(folder).unwrap();
            assert!(!has_custom_icon(folder));
        }
        println!("bytes per folder: {measured}");
    }
}
