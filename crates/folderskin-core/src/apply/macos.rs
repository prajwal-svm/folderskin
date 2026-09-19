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

use super::ApplyError;
use crate::compositor::IconSet;
use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageRep, NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
use objc2_foundation::{NSData, NSSize, NSString};
use std::path::Path;

/// Point size of the image the reps hang off; the largest rep is the 1:1 one.
const IMAGE_POINTS: f64 = 1024.0;
/// Largest representation macOS has any use for in a folder icon.
const MAX_REP: u32 = 1024;

/// Attaches `icons` to `folder` as its custom Finder icon.
pub fn apply(folder: &Path, icons: &IconSet) -> Result<(), ApplyError> {
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(IMAGE_POINTS, IMAGE_POINTS));

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

    set_icon(folder, Some(&image))
}

/// Clears `folder`'s custom icon, putting the system folder icon back.
pub fn revert(folder: &Path) -> Result<(), ApplyError> {
    set_icon(folder, None)
}

/// The one call that changes a folder's icon; `None` reverts it.
fn set_icon(folder: &Path, image: Option<&NSImage>) -> Result<(), ApplyError> {
    let workspace = NSWorkspace::sharedWorkspace();
    let path = NSString::from_str(&folder.to_string_lossy());
    let options = NSWorkspaceIconCreationOptions::empty();
    if image.is_some() {
        // Clearing an icon that isn't there is harmless, and if clearing fails, so does the set.
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
}

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

        println!("GetFileInfo -a: before={before} after={after} reverted={reverted}");
    }
}
