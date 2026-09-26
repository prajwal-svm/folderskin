//! macOS folder icons via `NSWorkspace.setIcon(_:forFile:options:)`, and copies of what it wrote.
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
//! Even so, `setIcon` encodes the image again for every folder, and writes it twice (clearing
//! first): about 110 ms and 2.5 MB a folder, and on a network share much longer. So in a run over
//! a tree only the first folder goes through AppKit, and the rest get what it wrote: on an APFS
//! disk a clone of the first folder's `Icon\r`, which shares its blocks, so a folder costs about
//! 30 KB and under a millisecond; anywhere else (a network share, HFS+, exFAT) the icon
//! written straight into the folder, in a few milliseconds, without its 1024 px size, so each
//! holds about 1 MB rather than 2.5 (Finder only draws that size for the largest icons on a
//! Retina screen). When a copy can't be made, the folder goes through AppKit after all, which
//! says what's wrong with it as a single folder always has.
//!
//! Some network shares keep a custom icon's bytes but refuse `setIcon` itself, both setting and
//! clearing: a NAS measured on 2026-09-26 (SMB 3.1.1) took a 2 MB `Icon\r` written straight in,
//! and Finder drew it, while `setIcon` returned false and left an empty file behind. So when
//! AppKit refuses a folder, the icon it would have written goes in by hand, and a revert it
//! refuses takes the icon off by hand.
//!
//! `setIcon` works off the main thread, but not on two threads at once: calls that overlap
//! garble each other's `Icon\r` (a 37 KB icon came out as 286 bytes) or fail outright. An apply,
//! a tree of them and a measurement can all be running together, so every change here takes one
//! lock for the whole process.

use super::{ApplyError, TempDir};
use crate::compositor::IconSet;
use objc2::rc::{autoreleasepool, Retained};
use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageRep, NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
use objc2_foundation::{NSData, NSSize, NSString};
use std::cell::{Cell, OnceCell, RefCell};
use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Point size of the image the reps hang off; the largest rep is the 1:1 one.
const IMAGE_POINTS: f64 = 1024.0;
/// Largest representation macOS has any use for in a folder icon.
const MAX_REP: u32 = 1024;
/// Largest representation in the icon a run writes straight into folders it can't clone the
/// first one's for.
const LIGHT_MAX_REP: u32 = 512;
/// Disk one cloned `Icon\r` takes on APFS: its file record, its extended attributes and the
/// folder's Finder info. 14 to 29 KB measured over runs of 200 and 300 folders, rounded up.
pub(crate) const CLONE_BYTES: u64 = 32 * 1024;
/// The invisible file a folder's custom icon lives in.
const ICON_FILE: &str = "Icon\r";
/// kHasCustomIcon in the high byte of the big-endian Finder flags (byte 8 of the Finder info).
const HAS_CUSTOM_ICON: u8 = 0x04;

/// An icon ready for any number of folders: the image AppKit takes, and what a run makes of the
/// first folder it gives it to, for the rest.
///
/// It keeps AppKit objects and remembers that first folder, so it stays on the thread that
/// prepared it, as `PreparedIcon` says.
pub struct Prepared {
    /// Every rendered size up to [`MAX_REP`], as PNG, for the light image made when it's needed.
    pngs: Vec<(u32, Vec<u8>)>,
    image: Retained<NSImage>,
    /// The first folder this icon went on through AppKit, whose `Icon\r` the rest are cloned from.
    first: RefCell<Option<First>>,
    /// The icon without its 1024 px size as AppKit writes it, read back once from a scratch
    /// folder, for folders it's written straight into. The error, if that couldn't be done.
    light: OnceCell<Result<IconFile, String>>,
    /// The whole icon as AppKit writes it, likewise, for a folder AppKit refuses.
    whole: OnceCell<Result<IconFile, String>>,
}

/// The folder a run's copies come from.
struct First {
    icon: PathBuf,
    device: u64,
    /// False once its disk has said it can't clone.
    clones: Cell<bool>,
}

/// An `Icon\r` as AppKit writes it: its resource fork, which holds the icon, and its Finder info,
/// which hides it.
struct IconFile {
    fork: Vec<u8>,
    info: Vec<u8>,
}

/// The icon as the one `NSImage` macOS wants, a representation per rendered size up to
/// [`MAX_REP`], ready for [`apply`].
pub fn prepare(icons: &IconSet) -> Result<Prepared, ApplyError> {
    let pngs: Vec<(u32, Vec<u8>)> = icons
        .sizes
        .iter()
        .filter(|(size, _)| *size <= MAX_REP)
        .map(|&(size, _)| {
            icons
                .png(size)
                .map(|png| (size, png))
                .ok_or_else(|| ApplyError::Platform(format!("the icon has no {size} px size")))
        })
        .collect::<Result<_, _>>()?;
    let image = image_of(&pngs, MAX_REP)?;
    Ok(Prepared {
        pngs,
        image,
        first: RefCell::new(None),
        light: OnceCell::new(),
        whole: OnceCell::new(),
    })
}

/// One `NSImage` with a representation for each of `pngs` up to `max` px.
fn image_of(pngs: &[(u32, Vec<u8>)], max: u32) -> Result<Retained<NSImage>, ApplyError> {
    // The PNG round trip leaves temporaries in the current pool; drain them here rather than
    // whenever the calling thread happens to end.
    autoreleasepool(|_| {
        let image =
            NSImage::initWithSize(NSImage::alloc(), NSSize::new(IMAGE_POINTS, IMAGE_POINTS));

        let mut reps = 0usize;
        for (size, png) in pngs {
            let size = *size;
            if size > max {
                continue;
            }
            let data = NSData::with_bytes(png);
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

/// Gives `folder` the [`prepare`]d icon as its custom Finder icon.
///
/// The first folder an icon goes on gets it through AppKit; every later one, a copy of that (see
/// the module notes), or AppKit again if no copy can be made.
pub fn apply(folder: &Path, icon: &Prepared) -> Result<(), ApplyError> {
    let _one_at_a_time = one_at_a_time();
    let copied = match icon.first.borrow().as_ref() {
        Some(first) => copy_first(folder, icon, first).is_ok(),
        None => false,
    };
    if copied {
        return Ok(());
    }
    if let Err(refused) = set_icon(folder, Some(&icon.image)) {
        write_whole(folder, icon).map_err(|_| refused)?;
    }
    let mut first = icon.first.borrow_mut();
    if first.is_none() {
        *first = First::of(folder);
    }
    Ok(())
}

/// Clears `folder`'s custom icon, putting the system folder icon back.
pub fn revert(folder: &Path) -> Result<(), ApplyError> {
    let _one_at_a_time = one_at_a_time();
    if let Err(refused) = set_icon(folder, None) {
        // A share that refuses setIcon refuses clearing too, and can leave the icon half gone.
        take_off(folder, &folder.join(ICON_FILE)).map_err(|_| refused)?;
    }
    Ok(())
}

/// True when Finder draws a custom icon for `folder`: the custom-icon flag in its Finder info,
/// which a revert clears whoever set it.
pub fn has_custom_icon(folder: &Path) -> bool {
    finder_info(folder).is_some_and(|info| info[8] & HAS_CUSTOM_ICON != 0)
}

/// Disk each folder of a run takes for `icon`: a clone of the first folder's `Icon\r` when
/// `folder` is on a local APFS disk, the icon written straight in anywhere else. `None` for
/// `folder` means wherever the icon goes, which counts as anywhere else.
pub fn bytes_per_folder(icon: &Prepared, folder: Option<&Path>) -> Result<u64, ApplyError> {
    if folder.is_some_and(clones_there) {
        return Ok(CLONE_BYTES);
    }
    let _one_at_a_time = one_at_a_time();
    icon.light()
        .map(|file| file.fork.len() as u64)
        .map_err(|e| ApplyError::Platform(format!("couldn't measure the icon macOS writes: {e}")))
}

impl First {
    /// `folder`, which has just been given the icon through AppKit, as the one to copy from.
    fn of(folder: &Path) -> Option<First> {
        use std::os::unix::fs::MetadataExt;
        let device = std::fs::metadata(folder).ok()?.dev();
        Some(First {
            icon: folder.join(ICON_FILE),
            device,
            clones: Cell::new(true),
        })
    }
}

impl Prepared {
    /// The light icon as AppKit writes it, made the first time it's asked for. Called with
    /// [`SET_ICON`] held.
    fn light(&self) -> io::Result<&IconFile> {
        self.light
            .get_or_init(|| {
                image_of(&self.pngs, LIGHT_MAX_REP)
                    .map_err(|e| e.to_string())
                    .and_then(|image| written_by_appkit(&image).map_err(|e| e.to_string()))
            })
            .as_ref()
            .map_err(|e| io::Error::other(e.clone()))
    }

    /// The whole icon as AppKit writes it, made the first time it's asked for. Called with
    /// [`SET_ICON`] held.
    fn whole(&self) -> io::Result<&IconFile> {
        self.whole
            .get_or_init(|| written_by_appkit(&self.image).map_err(|e| e.to_string()))
            .as_ref()
            .map_err(|e| io::Error::other(e.clone()))
    }
}

/// What AppKit writes for `image`: attached to a scratch folder in the temp directory, read
/// back, and thrown away. Called with [`SET_ICON`] held.
fn written_by_appkit(image: &NSImage) -> Result<IconFile, ApplyError> {
    let scratch = TempDir::new("folderskin-icon")?;
    set_icon(&scratch, Some(image))?;
    let icon = scratch.join(ICON_FILE);
    let fork = std::fs::read(icon.join("..namedfork/rsrc"))?;
    let info = xattr(&icon, FINDER_INFO).unwrap_or_default();
    if fork.is_empty() || info.len() != 32 {
        return Err(ApplyError::Platform(
            "macOS wrote an icon FolderSkin couldn't read back".into(),
        ));
    }
    Ok(IconFile { fork, info })
}

/// Gives `folder` a copy of what `first` was given: a clone of its `Icon\r` when the two are on
/// one disk that can clone, otherwise the light icon written straight in.
fn copy_first(folder: &Path, icon: &Prepared, first: &First) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let target = folder.join(ICON_FILE);
    take_off(folder, &target)?;
    if first.clones.get() && std::fs::metadata(folder)?.dev() == first.device {
        match clone(&first.icon, &target) {
            Ok(()) => return put_on(folder),
            Err(e) if cannot_clone(&e) => first.clones.set(false),
            Err(e) => return Err(e),
        }
    }
    let light = icon.light()?;
    if let Err(e) = write_icon_file(&target, light) {
        let _ = std::fs::remove_file(&target);
        return Err(e);
    }
    put_on(folder)
}

/// Gives `folder`, which AppKit refused, the whole icon as AppKit would have written it.
fn write_whole(folder: &Path, icon: &Prepared) -> io::Result<()> {
    let whole = icon.whole()?;
    let target = folder.join(ICON_FILE);
    // A refused setIcon can leave an empty `Icon\r` behind.
    take_off(folder, &target)?;
    if let Err(e) = write_icon_file(&target, whole) {
        let _ = std::fs::remove_file(&target);
        return Err(e);
    }
    put_on(folder)
}

/// Takes any custom icon `folder` has off, as AppKit's clearing does, so Finder draws the new one
/// rather than keeping the old one it had drawn.
fn take_off(folder: &Path, icon: &Path) -> io::Result<()> {
    let removed = match std::fs::remove_file(icon) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    let flagged = set_custom_flag(folder, false)?;
    if removed || flagged {
        note_changed(folder);
    }
    Ok(())
}

/// Turns the custom-icon flag on for `folder`, whose `Icon\r` is in place, and tells Finder.
fn put_on(folder: &Path) -> io::Result<()> {
    set_custom_flag(folder, true)?;
    note_changed(folder);
    if let Some(parent) = folder.parent() {
        note_changed(parent);
    }
    Ok(())
}

/// Writes `file` as the `Icon\r` at `target`: an empty file with the icon in its resource fork,
/// hidden by its Finder info and by the flag `setIcon` also sets.
fn write_icon_file(target: &Path, file: &IconFile) -> io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    // The fork reads and writes as a file of its own under this name, on APFS, HFS+ and the
    // network shares macOS mounts alike.
    std::fs::write(target.join("..namedfork/rsrc"), &file.fork)?;
    set_xattr(target, FINDER_INFO, &file.info)?;
    let path = c_path(target)?;
    // SAFETY: `path` is NUL-terminated.
    if unsafe { libc::chflags(path.as_ptr(), libc::UF_HIDDEN) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Clones `from` to `to`, a new file sharing its blocks, with its extended attributes and flags.
fn clone(from: &Path, to: &Path) -> io::Result<()> {
    /// `CLONE_NOFOLLOW` from `<sys/clonefile.h>`: clone a link itself rather than what it names.
    const CLONE_NOFOLLOW: u32 = 0x0001;
    let (from, to) = (c_path(from)?, c_path(to)?);
    // SAFETY: both paths are NUL-terminated.
    if unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), CLONE_NOFOLLOW) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// True for a clone that failed because the disk can't clone, or not between these two folders:
/// not APFS, a network share, another volume.
fn cannot_clone(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(libc::ENOTSUP | libc::EXDEV | libc::EINVAL | libc::ENOTTY)
    )
}

/// True when `folder` is on a local APFS disk, where a run clones the first folder's icon.
fn clones_there(folder: &Path) -> bool {
    let Ok(path) = c_path(folder) else {
        return false;
    };
    // SAFETY: zeroed is a valid `statfs`, `path` is NUL-terminated and `stats` writable.
    let mut stats: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(path.as_ptr(), &mut stats) } != 0 {
        return false;
    }
    // SAFETY: the kernel NUL-terminates the file system's name.
    let kind = unsafe { CStr::from_ptr(stats.f_fstypename.as_ptr()) };
    kind.to_bytes() == b"apfs" && stats.f_flags & libc::MNT_LOCAL as u32 != 0
}

/// Turns `folder`'s custom-icon flag `on` or off, keeping the rest of its Finder info. True when
/// that changed anything.
fn set_custom_flag(folder: &Path, on: bool) -> io::Result<bool> {
    let mut info = finder_info(folder).unwrap_or([0; 32]);
    let before = info[8];
    if on {
        info[8] |= HAS_CUSTOM_ICON;
    } else {
        info[8] &= !HAS_CUSTOM_ICON;
    }
    if info[8] == before {
        return Ok(false);
    }
    set_xattr(folder, FINDER_INFO, &info)?;
    Ok(true)
}

/// `path`'s 32 bytes of Finder info, if it has any.
fn finder_info(path: &Path) -> Option<[u8; 32]> {
    let bytes = xattr(path, FINDER_INFO)?;
    let mut info = [0u8; 32];
    let n = bytes.len().min(32);
    info[..n].copy_from_slice(&bytes[..n]);
    Some(info)
}

/// The extended attribute `name` of `path` itself (never what a link names).
fn xattr(path: &Path, name: &CStr) -> Option<Vec<u8>> {
    let path = c_path(path).ok()?;
    // SAFETY: both names are NUL-terminated; a null buffer of size 0 only asks for the size.
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    let mut value = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: as above, and `value` is writable for its whole length.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    value.truncate(usize::try_from(read).ok()?);
    Some(value)
}

/// Sets the extended attribute `name` of `path` itself.
fn set_xattr(path: &Path, name: &CStr, value: &[u8]) -> io::Result<()> {
    let path = c_path(path)?;
    // SAFETY: both names are NUL-terminated and `value` is readable for its whole length.
    let status = unsafe {
        libc::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)
}

/// Tells Finder `path` changed, so it draws its icon again.
fn note_changed(path: &Path) {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .noteFileSystemChanged_(&NSString::from_str(&path.to_string_lossy()));
    });
}

/// The extended attribute macOS keeps a file's or folder's Finder flags in.
const FINDER_INFO: &CStr = c"com.apple.FinderInfo";

/// Held for every change of a folder's icon: `setIcon` calls must not overlap.
static SET_ICON: Mutex<()> = Mutex::new(());

/// Takes [`SET_ICON`]. A panic while holding it leaves nothing half-done that the next caller
/// could trip over, so a poisoned lock is simply taken.
fn one_at_a_time() -> MutexGuard<'static, ()> {
    SET_ICON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The one AppKit call that changes a folder's icon; `None` reverts it. Called with [`SET_ICON`]
/// held.
///
/// Each change runs in its own autorelease pool: the icon data AppKit encodes for a folder is
/// megabytes, and a tree of folders is changed one after another on one thread, which would
/// otherwise hold every folder's copy until the thread ends.
fn set_icon(folder: &Path, image: Option<&NSImage>) -> Result<(), ApplyError> {
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
#[cfg(test)]
pub(crate) fn icon_file_bytes(folder: &Path) -> Option<u64> {
    std::fs::metadata(folder.join(ICON_FILE).join("..namedfork/rsrc"))
        .ok()
        .map(|meta| meta.len())
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

    /// Writes `folder`'s 32 bytes of Finder info.
    fn set_finder_info(folder: &Path, info: &[u8; 32]) {
        set_xattr(folder, FINDER_INFO, info).expect("setxattr");
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
    fn the_custom_icon_flag_changes_alone() {
        let folder = tempfile_dir();
        // A label colour and another flag the folder had stay as they were.
        let mut info = [0u8; 32];
        info[8] = 0x40;
        info[9] = 0x0E;
        info[20] = 0x7F;
        set_finder_info(&folder, &info);

        assert!(set_custom_flag(&folder, true).unwrap());
        let on = finder_info(&folder).unwrap();
        assert_eq!(on[8], 0x44);
        assert_eq!((on[9], on[20]), (0x0E, 0x7F));
        assert!(!set_custom_flag(&folder, true).unwrap(), "already on");

        assert!(set_custom_flag(&folder, false).unwrap());
        assert_eq!(finder_info(&folder).unwrap(), info);
        assert!(!set_custom_flag(&folder, false).unwrap(), "already off");
    }

    #[test]
    fn a_folder_without_finder_info_gets_the_flag_alone() {
        let folder = tempfile_dir();
        assert!(
            !set_custom_flag(&folder, false).unwrap(),
            "nothing to clear"
        );
        assert!(finder_info(&folder).is_none(), "clearing wrote nothing");
        assert!(set_custom_flag(&folder, true).unwrap());
        let mut expected = [0u8; 32];
        expected[8] = HAS_CUSTOM_ICON;
        assert_eq!(finder_info(&folder).unwrap(), expected);
    }

    #[test]
    fn a_disk_that_cannot_clone_is_told_from_other_failures() {
        for code in [libc::ENOTSUP, libc::EXDEV] {
            assert!(cannot_clone(&io::Error::from_raw_os_error(code)));
        }
        for code in [libc::EACCES, libc::EPERM, libc::ENOSPC, libc::EROFS] {
            assert!(!cannot_clone(&io::Error::from_raw_os_error(code)));
        }
    }

    #[test]
    fn the_temp_directory_is_a_disk_that_clones() {
        // The system temp directory is on the startup disk, APFS on every Mac FolderSkin runs on.
        assert!(clones_there(&tempfile_dir()));
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

    /// The kinds of image in the icon a fork holds (`ic10` is the 1024 px one), read from its
    /// `icns`: a four-letter kind and a big-endian length for each.
    fn icns_kinds(fork: &[u8]) -> Vec<String> {
        let at = fork
            .windows(4)
            .position(|w| w == b"icns")
            .expect("an icns in the fork");
        let len = |i: usize| u32::from_be_bytes(fork[i..i + 4].try_into().unwrap()) as usize;
        let end = at + len(at + 4);
        let mut kinds = Vec::new();
        let mut i = at + 8;
        while i + 8 <= end {
            kinds.push(String::from_utf8_lossy(&fork[i..i + 4]).into_owned());
            i += len(i + 4).max(8);
        }
        kinds
    }

    #[test]
    #[ignore = "touches real folders and needs a desktop session: cargo test -p folderskin-core -- --ignored copies"]
    fn a_run_copies_the_first_folders_icon_to_the_rest() {
        use crate::apply::{apply_prepared, prepare_icon};

        let icon = prepare_icon(&solid_icons(&[16, 32, 64, 128, 256, 512, 1024])).unwrap();
        let tree = tempfile_dir();
        let folders: Vec<PathBuf> = (0..6).map(|i| tree.join(format!("f{i}"))).collect();
        for folder in &folders {
            std::fs::create_dir(folder).unwrap();
        }
        // One of them wore another icon already: it's taken off on the way.
        apply_icon(&folders[3], &solid_icons(&[16, 32, 128])).unwrap();

        for folder in &folders {
            apply_prepared(folder, &icon).unwrap();
        }
        let first = std::fs::read(folders[0].join("Icon\r/..namedfork/rsrc")).unwrap();
        for folder in &folders {
            assert!(has_custom_icon(folder), "{}", folder.display());
            let fork = std::fs::read(folder.join("Icon\r/..namedfork/rsrc")).unwrap();
            assert_eq!(fork, first, "a clone holds the first folder's icon");
            assert!(
                attributes(&folder.join("Icon\r")).contains('V'),
                "the Icon file is invisible"
            );
            assert!(
                icns_kinds(&fork).iter().any(|k| k == "ic10"),
                "the whole icon, 1024 px and all"
            );
        }

        // Where it can't clone, the light icon goes straight in.
        icon.prepared
            .first
            .borrow()
            .as_ref()
            .unwrap()
            .clones
            .set(false);
        let written = tree.join("written");
        std::fs::create_dir(&written).unwrap();
        apply_prepared(&written, &icon).unwrap();
        assert!(has_custom_icon(&written));
        let fork = std::fs::read(written.join("Icon\r/..namedfork/rsrc")).unwrap();
        assert!(fork.len() < first.len(), "the light icon is smaller");
        let kinds = icns_kinds(&fork);
        assert!(
            !kinds.iter().any(|k| k == "ic10"),
            "the light icon stops at 512 px: {kinds:?}"
        );
        assert!(
            kinds.iter().any(|k| k == "ic09"),
            "and still has its 512 px: {kinds:?}"
        );

        for folder in folders.iter().chain([&written]) {
            revert_icon(folder).unwrap();
            assert!(!has_custom_icon(folder));
            assert!(!folder.join("Icon\r").exists());
        }
    }

    #[test]
    #[ignore = "touches a real folder and needs a desktop session: cargo test -p folderskin-core -- --ignored appkit_refuses"]
    fn a_folder_appkit_refuses_gets_the_icon_written_in_and_taken_off_by_hand() {
        use crate::apply::prepare_icon;

        let icon = prepare_icon(&solid_icons(&[16, 32, 64, 128, 256, 512, 1024])).unwrap();
        let folder = tempfile_dir();
        // What a share that refuses setIcon leaves: an empty Icon file.
        std::fs::File::create(folder.join(ICON_FILE)).unwrap();
        {
            let _one_at_a_time = one_at_a_time();
            write_whole(&folder, &icon.prepared).unwrap();
        }
        assert!(has_custom_icon(&folder));
        let fork = std::fs::read(folder.join("Icon\r/..namedfork/rsrc")).unwrap();
        let whole = icon.prepared.whole().unwrap();
        assert_eq!(fork, whole.fork, "the icon AppKit would have written");
        assert!(
            icns_kinds(&fork).iter().any(|k| k == "ic10"),
            "all of it, 1024 px too"
        );
        assert!(
            attributes(&folder.join("Icon\r")).contains('V'),
            "the Icon file is invisible"
        );

        // Taking it off by hand leaves the folder as a revert does.
        take_off(&folder, &folder.join(ICON_FILE)).unwrap();
        assert!(!has_custom_icon(&folder));
        assert!(!folder.join(ICON_FILE).exists());
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

        // Anywhere a run can't clone, a folder weighs what the light icon does.
        let light = bytes_per_folder(&icon, None).unwrap();
        let written = tempfile_dir();
        icon.prepared
            .first
            .borrow()
            .as_ref()
            .unwrap()
            .clones
            .set(false);
        apply_prepared(&written, &icon).unwrap();
        assert_eq!(Some(light), icon_file_bytes(&written));
        // On APFS, a clone.
        assert_eq!(
            bytes_per_folder(&icon, Some(&folders[0])).unwrap(),
            CLONE_BYTES
        );

        for folder in folders.iter().map(|f| &**f).chain([&*written]) {
            revert_icon(folder).unwrap();
            assert!(!has_custom_icon(folder));
        }
        println!("bytes per folder written: {light}");
    }
}
