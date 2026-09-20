//! Apply / revert a folder icon on the current OS.
//!
//! Every OS does this differently — macOS attaches an `NSImage` to the folder, Windows drops a
//! `desktop.ini` and an `.ico` in it, Linux writes a `.directory` and a GIO attribute — so each
//! gets its own writer behind the same two functions.
//!
//! The Windows and Linux writers put their content on disk through *pure* functions that
//! compile on every OS. Those are the parts that can eat a file the user already had, and this
//! way they are unit-tested on the dev machine rather than only on the OS that ships them; the
//! OS-specific halves are `cfg`-gated and compile-checked against the other targets.
//!
//! Encoding the icon for the OS is the slow part of applying it, and the same for every folder,
//! so it is its own step: [`prepare_icon`] once, then [`apply_prepared`] to as many folders as
//! wanted (a whole tree of them; see [`tree`]). [`apply_icon`] is the two together.

use crate::compositor::IconSet;
use std::path::{Path, PathBuf};

pub mod linux;
pub mod paths;
pub mod tree;
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

/// User-facing failure reasons (the `Display` text is shown in the UI as-is).
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("{0} is not a folder")]
    NotADirectory(PathBuf),
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Platform(String),
}

pub use paths::validate_folder;

/// Why nothing can be applied on an OS without a writer.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
const UNSUPPORTED: &str = "custom folder icons are not supported on this OS";

/// An icon encoded for the current OS's writer, ready to go on any number of folders with
/// [`apply_prepared`]: the `NSImage` on macOS, the bytes of `folderskin.ico` on Windows and of
/// `.folderskin.png` on Linux.
///
/// On macOS it holds an AppKit object and so isn't `Send`: prepare it on the thread that
/// applies it.
pub struct PreparedIcon {
    #[cfg(target_os = "macos")]
    image: objc2::rc::Retained<objc2_app_kit::NSImage>,
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    bytes: Vec<u8>,
}

impl std::fmt::Debug for PreparedIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedIcon").finish_non_exhaustive()
    }
}

/// Encodes the rendered icon set for the current OS, once, for [`apply_prepared`].
pub fn prepare_icon(icons: &IconSet) -> Result<PreparedIcon, ApplyError> {
    #[cfg(target_os = "macos")]
    {
        macos::prepare(icons).map(|image| PreparedIcon { image })
    }
    #[cfg(target_os = "windows")]
    {
        windows::prepare(icons).map(|bytes| PreparedIcon { bytes })
    }
    #[cfg(target_os = "linux")]
    {
        linux::prepare(icons).map(|bytes| PreparedIcon { bytes })
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = icons;
        Err(ApplyError::Platform(UNSUPPORTED.into()))
    }
}

/// Applies the rendered icon set to `folder` using the current OS mechanism.
pub fn apply_icon(folder: &Path, icons: &IconSet) -> Result<(), ApplyError> {
    // Checked before the icon is encoded, the slow part, so a folder that can't be used fails
    // at once, as it always has.
    validate_folder(folder)?;
    apply_prepared(folder, &prepare_icon(icons)?)
}

/// Applies a [`prepare_icon`]d icon to `folder`, which is checked exactly as [`apply_icon`]
/// checks it.
pub fn apply_prepared(folder: &Path, icon: &PreparedIcon) -> Result<(), ApplyError> {
    let folder = validate_folder(folder)?;
    #[cfg(target_os = "macos")]
    {
        macos::apply(&folder, &icon.image)
    }
    #[cfg(target_os = "windows")]
    {
        windows::apply(&folder, &icon.bytes)
    }
    #[cfg(target_os = "linux")]
    {
        linux::apply(&folder, &icon.bytes)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (folder, icon);
        Err(ApplyError::Platform(UNSUPPORTED.into()))
    }
}

/// Room a `desktop.ini` or `.directory` takes beside the icon file: a few lines of text, but a
/// disk block of its own.
#[cfg(any(target_os = "windows", target_os = "linux"))]
const TEXT_FILE_BYTES: u64 = 4096;

/// How much disk one folder's copy of `icon` takes.
///
/// On Windows and Linux that is the icon file FolderSkin writes plus a block for the
/// `desktop.ini` or `.directory` beside it. macOS encodes the icon itself as it attaches it, into
/// the folder's `Icon\r` file, and that comes out several times the size of the PNGs; so it is
/// measured exactly: attached to a scratch folder in the temp directory, weighed, and thrown
/// away.
pub fn bytes_per_folder(icon: &PreparedIcon) -> Result<u64, ApplyError> {
    #[cfg(target_os = "macos")]
    {
        let scratch = TempDir::new("folderskin-measure")?;
        macos::apply(&scratch, &icon.image)?;
        match macos::icon_file_bytes(&scratch) {
            Some(bytes) if bytes > 0 => Ok(bytes),
            _ => Err(ApplyError::Platform(
                "couldn't measure the icon macOS wrote".into(),
            )),
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        Ok(icon.bytes.len() as u64 + TEXT_FILE_BYTES)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = icon;
        Err(ApplyError::Platform(UNSUPPORTED.into()))
    }
}

/// Restores the OS default icon for `folder`.
pub fn revert_icon(folder: &Path) -> Result<(), ApplyError> {
    let folder = validate_folder(folder)?;
    #[cfg(target_os = "macos")]
    {
        macos::revert(&folder)
    }
    #[cfg(target_os = "windows")]
    {
        windows::revert(&folder)
    }
    #[cfg(target_os = "linux")]
    {
        linux::revert(&folder)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = folder;
        Err(ApplyError::Platform(
            "custom folder icons are not supported on this OS".into(),
        ))
    }
}

/// Asks the file manager to draw folder icons again, once a whole operation has finished.
///
/// Only Windows needs it, and only because the Desktop repaints for nothing else; see
/// [`windows::refresh_shell_icons`] for what was tried first. It is deliberately called once per
/// operation rather than once per folder, since it refreshes every view, so a run over a tree
/// costs one refresh. macOS and Linux file managers act on the per-folder notifications the
/// writers already send, and do nothing here.
pub fn refresh_shell_icons() {
    #[cfg(target_os = "windows")]
    windows::refresh_shell_icons();
}

/// True when `folder` wears an icon of its own that [`revert_icon`] would take off: any custom
/// icon on macOS, FolderSkin's on Windows, and FolderSkin's or a GIO custom icon on Linux. False
/// for a folder FolderSkin wouldn't touch at all, since a revert there is refused.
pub fn has_custom_icon(folder: &Path) -> bool {
    let Ok(folder) = validate_folder(folder) else {
        return false;
    };
    #[cfg(target_os = "macos")]
    {
        macos::has_custom_icon(&folder)
    }
    #[cfg(target_os = "windows")]
    {
        windows::has_custom_icon(&folder)
    }
    #[cfg(target_os = "linux")]
    {
        linux::has_custom_icon(&folder)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = folder;
        false
    }
}

/// A unique scratch directory under the system temp dir, removed when the guard drops.
///
/// Derefs to `Path`, so it can be passed straight to anything taking `&Path`.
#[cfg(any(test, target_os = "macos"))]
pub(crate) struct TempDir(PathBuf);

#[cfg(any(test, target_os = "macos"))]
impl TempDir {
    /// Creates `<temp>/<prefix>-<process>-<n>`. Unique per process and per call, so callers
    /// running in parallel never share one.
    fn new(prefix: &str) -> std::io::Result<TempDir> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);

        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path)?;
        Ok(TempDir(path))
    }
}

#[cfg(any(test, target_os = "macos"))]
impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

#[cfg(any(test, target_os = "macos"))]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Creates a scratch directory for a test.
#[cfg(test)]
pub(crate) fn tempfile_dir() -> TempDir {
    TempDir::new("folderskin-test").expect("create the scratch directory")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat-colour icon set at `sizes`.
    fn solid(sizes: &[u32]) -> IconSet {
        IconSet {
            sizes: sizes
                .iter()
                .map(|&size| {
                    let pixel = image::Rgba([0x2E, 0x7D, 0x5B, 0xFF]);
                    (size, image::RgbaImage::from_pixel(size, size, pixel))
                })
                .collect(),
        }
    }

    // ------------------------------------------------------------- prepared icons

    #[test]
    fn the_windows_icon_packs_every_size_explorer_uses() {
        let ico = windows::prepare(&solid(&crate::compositor::ICON_SIZES)).unwrap();
        let widths: Vec<u32> = crate::ico::read_ico_header(&ico)
            .unwrap()
            .iter()
            .map(|&(width, _, _)| width)
            .collect();
        assert_eq!(widths, windows::ICO_SIZES);
        assert!(
            windows::prepare(&solid(&[512, 1024])).is_err(),
            "no size Windows can use"
        );
    }

    #[test]
    fn the_linux_icon_is_its_512_px_png() {
        let png = linux::prepare(&solid(&[1024, 512, 256])).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (512, 512));
        assert!(linux::prepare(&solid(&[256, 1024])).is_err());
    }

    #[test]
    fn a_prepared_icon_is_checked_against_the_folder_as_apply_icon_does() {
        let icons = solid(&[16, 32, 256, 512, 1024]);
        let icon = prepare_icon(&icons).unwrap();
        let tmp = tempfile_dir();
        let file = tmp.join("file.txt");
        std::fs::write(&file, b"x").unwrap();
        for result in [apply_prepared(&file, &icon), apply_icon(&file, &icons)] {
            assert!(matches!(result, Err(ApplyError::NotADirectory(_))));
        }
        for result in [
            apply_prepared(Path::new("/"), &icon),
            apply_icon(Path::new("/"), &icons),
        ] {
            assert!(matches!(result, Err(ApplyError::Refused(_))));
        }
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn one_prepared_icon_goes_on_several_folders_and_comes_off_again() {
        let icons = solid(&crate::compositor::ICON_SIZES);
        let icon = prepare_icon(&icons).unwrap();
        let (name, bytes) = if cfg!(windows) {
            let bytes = windows::prepare(&icons).unwrap();
            (windows::ico_file_name(&bytes), bytes)
        } else {
            (linux::PNG_NAME.to_string(), linux::prepare(&icons).unwrap())
        };
        assert_eq!(
            bytes_per_folder(&icon).unwrap(),
            bytes.len() as u64 + TEXT_FILE_BYTES
        );

        let folders = [tempfile_dir(), tempfile_dir()];
        for folder in &folders {
            apply_prepared(folder, &icon).unwrap();
            assert_eq!(std::fs::read(folder.join(&name)).unwrap(), bytes);
            assert!(has_custom_icon(folder));
        }
        for folder in &folders {
            revert_icon(folder).unwrap();
            assert!(!has_custom_icon(folder));
        }
    }

    /// The whole point of the hashed icon name: a second skin is a second path, so Explorer has
    /// nothing cached against it, and the folder is never left holding both.
    #[cfg(target_os = "windows")]
    #[test]
    fn a_second_skin_gets_its_own_icon_file_and_the_first_one_goes() {
        let folder = tempfile_dir();
        let one = prepare_icon(&solid(&crate::compositor::ICON_SIZES)).unwrap();
        let mut other = solid(&crate::compositor::ICON_SIZES);
        for (_, img) in other.sizes.iter_mut() {
            *img = image::RgbaImage::from_pixel(
                img.width(),
                img.height(),
                image::Rgba([0xD6, 0x28, 0x28, 0xFF]),
            );
        }
        let two = prepare_icon(&other).unwrap();

        let icos = |dir: &std::path::Path| -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".ico"))
                .collect();
            names.sort();
            names
        };

        apply_prepared(&folder, &one).unwrap();
        let first = icos(&folder);
        assert_eq!(first.len(), 1, "one icon file: {first:?}");

        // Re-applying the same skin resolves to the same name: nothing new, nothing left over.
        apply_prepared(&folder, &one).unwrap();
        assert_eq!(icos(&folder), first, "the same skin must not churn");

        apply_prepared(&folder, &two).unwrap();
        let second = icos(&folder);
        assert_eq!(second.len(), 1, "the old icon file must go: {second:?}");
        assert_ne!(second, first, "a different skin must be a different path");
        // desktop.ini names the file that is actually there.
        let ini = std::fs::read_to_string(folder.join(windows::INI_NAME)).unwrap();
        assert!(ini.contains(&second[0]), "{ini}");

        revert_icon(&folder).unwrap();
        assert!(icos(&folder).is_empty());
        assert!(!has_custom_icon(&folder));
    }

    // ------------------------------------------------------------------ desktop.ini

    /// The icon file name for a skin, as `apply` derives it from the packed `.ico`.
    fn ico_name() -> String {
        windows::ico_file_name(b"the packed icon")
    }

    #[test]
    fn the_icon_file_is_named_after_its_own_contents() {
        let one = windows::ico_file_name(b"first skin");
        let two = windows::ico_file_name(b"second skin");
        assert_ne!(one, two, "different pictures, different paths");
        assert_eq!(one, windows::ico_file_name(b"first skin"), "and stable");
        assert!(
            one.starts_with("folderskin-") && one.ends_with(".ico"),
            "{one}"
        );
        assert_eq!(one.len(), "folderskin-".len() + 16 + ".ico".len());
        assert!(windows::is_our_ico_name(&one) && windows::is_our_ico_name(&two));
    }

    #[test]
    fn only_our_own_icon_file_names_are_claimed() {
        // Ours: the hashed name, the fixed one older versions wrote, either case.
        for name in [
            "folderskin.ico",
            "FolderSkin.ICO",
            "folderskin-0123456789abcdef.ico",
            "FolderSkin-0123456789ABCDEF.ico",
        ] {
            assert!(windows::is_our_ico_name(name), "{name}");
        }
        // Not ours — revert deletes what this matches, so a file the user named must survive.
        for name in [
            "folderskin-mine.ico",
            "folderskin-0123456789abcdef.png",
            "folderskin-0123456789abcde.ico",   // 15 hex
            "folderskin-0123456789abcdef0.ico", // 17 hex
            "folderskin-0123456789abcdeg.ico",  // not hex
            "folderskin2.ico",
            "my-folderskin.ico",
            "folder.ico",
        ] {
            assert!(!windows::is_our_ico_name(name), "{name}");
        }
    }

    #[test]
    fn desktop_ini_is_created_with_marker_and_icon_resource() {
        let name = ico_name();
        let s = windows::desktop_ini_contents(None, &name);
        assert!(s.starts_with("[.ShellClassInfo]\r\n"));
        assert!(s.contains(&format!("IconResource={name},0\r\n")), "{s}");
        assert!(s.contains(windows::MARKER));
    }

    #[test]
    fn desktop_ini_preserves_foreign_keys_and_replaces_icon_resource() {
        let existing = "[.ShellClassInfo]\r\nInfoTip=hello\r\nIconResource=other.ico,0\r\n";
        let s = windows::desktop_ini_contents(Some(existing), &ico_name());
        assert!(s.contains("InfoTip=hello"));
        assert!(!s.contains("other.ico"));
        assert_eq!(s.matches("IconResource=").count(), 1);
    }

    /// A folder skinned by an older version, re-skinned: the fixed name it used to point at is
    /// replaced by the hashed one, and only one IconResource is left.
    #[test]
    fn desktop_ini_replaces_the_name_an_older_version_wrote() {
        let old = windows::desktop_ini_contents(None, windows::ICO_NAME);
        let name = ico_name();
        let new = windows::desktop_ini_contents(Some(&old), &name);
        assert!(new.contains(&format!("IconResource={name},0")), "{new}");
        assert!(!new.contains("IconResource=folderskin.ico"), "{new}");
        assert_eq!(new.matches("IconResource=").count(), 1);
        assert_eq!(new.matches(windows::MARKER).count(), 1);
        assert_eq!(windows::desktop_ini_without_ours(&new), None);
    }

    #[test]
    fn revert_removes_only_our_lines_or_whole_file() {
        let ours = windows::desktop_ini_contents(None, &ico_name());
        assert_eq!(windows::desktop_ini_without_ours(&ours), None);
        let mixed = windows::desktop_ini_contents(
            Some("[.ShellClassInfo]\r\nInfoTip=keep\r\n"),
            &ico_name(),
        );
        let left = windows::desktop_ini_without_ours(&mixed).unwrap();
        assert!(
            left.contains("InfoTip=keep")
                && !left.contains("IconResource")
                && !left.contains(windows::MARKER)
        );
    }

    #[test]
    fn desktop_ini_keeps_other_sections_and_stays_idempotent() {
        let existing = "[.ShellClassInfo]\r\nInfoTip=hello\r\n[ViewState]\r\nMode=\r\nVid=\r\n";
        let once = windows::desktop_ini_contents(Some(existing), &ico_name());
        assert!(once.contains("[ViewState]\r\nMode=\r\nVid=\r\n"));
        // Our lines land at the end of the section we own, not at the end of the file.
        assert!(once.contains("InfoTip=hello\r\n; managed by FolderSkin\r\n"));

        let twice = windows::desktop_ini_contents(Some(&once), &ico_name());
        assert_eq!(once, twice, "re-applying must not stack up our lines");

        let left = windows::desktop_ini_without_ours(&twice).unwrap();
        assert_eq!(left, existing, "revert must restore the file we found");
    }

    #[test]
    fn desktop_ini_adds_our_section_when_there_is_none() {
        let s = windows::desktop_ini_contents(Some("[ViewState]\r\nMode=\r\n"), &ico_name());
        assert!(s.starts_with("[.ShellClassInfo]\r\n"));
        assert!(s.contains("[ViewState]\r\nMode=\r\n"));
    }

    /// What `folder_icon.rs` reads to show the icon a folder already wears.
    #[test]
    fn the_icon_a_folder_wears_is_read_back_out_of_its_ini() {
        let name = ico_name();
        let ours = windows::desktop_ini_contents(None, &name);
        assert_eq!(windows::icon_resource_of(&ours), Some((name, 0)));

        // Someone else's icon, which is the case the app was showing the default folder for.
        assert_eq!(
            windows::icon_resource_of("[.ShellClassInfo]\r\nIconResource=theirs.ico,0\r\n"),
            Some(("theirs.ico".to_string(), 0))
        );
        assert_eq!(
            windows::icon_resource_of(
                "[.ShellClassInfo]\r\nIconResource=%SystemRoot%\\system32\\imageres.dll,-184\r\n"
            ),
            Some(("%SystemRoot%\\system32\\imageres.dll".to_string(), -184))
        );
        // Explorer prefers the legacy pair, so this must too, or it would name the wrong icon.
        assert_eq!(
            windows::icon_resource_of(
                "[.ShellClassInfo]\r\nIconResource=new.ico,0\r\nIconFile=old.dll\r\nIconIndex=4\r\n"
            ),
            Some(("old.dll".to_string(), 4))
        );
        // IconFile with no index is index 0.
        assert_eq!(
            windows::icon_resource_of("[.ShellClassInfo]\r\nIconFile=old.dll\r\n"),
            Some(("old.dll".to_string(), 0))
        );
        // A path with no index at all, and a drive letter's colon, which is not a separator.
        assert_eq!(
            windows::icon_resource_of("[.ShellClassInfo]\r\nIconResource=C:\\art\\a.ico\r\n"),
            Some(("C:\\art\\a.ico".to_string(), 0))
        );

        // Nothing to draw.
        for none in [
            "",
            "[ViewState]\r\nMode=\r\n",
            // The key belongs to [.ShellClassInfo]; the same name elsewhere means something else.
            "[ViewState]\r\nIconResource=nope.ico,0\r\n",
            "[.ShellClassInfo]\r\nInfoTip=hello\r\n",
            "[.ShellClassInfo]\r\nIconResource=\r\n",
        ] {
            assert_eq!(windows::icon_resource_of(none), None, "{none:?}");
        }
    }

    #[test]
    fn desktop_ini_revert_leaves_a_foreign_icon_resource_alone() {
        let foreign = "[.ShellClassInfo]\r\nIconResource=theirs.ico,0\r\n";
        let left = windows::desktop_ini_without_ours(foreign).unwrap();
        assert_eq!(left, foreign);
        assert!(!windows::is_ours(foreign));
    }

    #[test]
    fn desktop_ini_keeps_a_byte_order_mark() {
        let s = windows::desktop_ini_contents(
            Some("\u{feff}[.ShellClassInfo]\r\nInfoTip=hi\r\n"),
            &ico_name(),
        );
        assert!(s.starts_with("\u{feff}[.ShellClassInfo]\r\n"));
        assert_eq!(s.matches("[.ShellClassInfo]").count(), 1);
    }

    #[test]
    fn a_folder_counts_as_skinned_only_when_revert_would_change_it() {
        let ours = windows::desktop_ini_contents(None, &ico_name());
        assert!(windows::would_revert(Some(&ours), false));
        assert!(windows::would_revert(None, true));
        let theirs = "[.ShellClassInfo]\r\nIconResource=theirs.ico,0\r\n";
        assert!(!windows::would_revert(Some(theirs), false));
        assert!(!windows::would_revert(None, false));

        let png = std::path::Path::new("/home/me/Pics/.folderskin.png");
        assert!(linux::would_revert(
            Some(&linux::directory_file_contents(png)),
            false
        ));
        assert!(linux::would_revert(None, true));
        let edited = linux::directory_file_with_icon("[Desktop Entry]\nName=Pics\n", png);
        assert!(linux::would_revert(Some(&edited), false));
        let theirs = "[Desktop Entry]\nIcon=folder-pictures\n";
        assert!(!linux::would_revert(Some(theirs), false));
        assert!(!linux::would_revert(None, false));
    }

    // -------------------------------------------------------------------- .directory

    #[test]
    fn directory_file_points_at_absolute_icon() {
        let s =
            linux::directory_file_contents(std::path::Path::new("/home/me/Pics/.folderskin.png"));
        assert_eq!(
            s,
            "[Desktop Entry]\nIcon=/home/me/Pics/.folderskin.png\n# managed by FolderSkin\n"
        );
        assert!(linux::is_ours(&s));
        assert!(!linux::is_ours("[Desktop Entry]\nIcon=folder\n"));
    }

    #[test]
    fn directory_file_edits_a_foreign_entry_in_place() {
        let png = std::path::Path::new("/srv/box/.folderskin.png");
        let existing = "[Desktop Entry]\nName=Box\nIcon=folder-blue\n[Extra]\nKey=1\n";
        let s = linux::directory_file_with_icon(existing, png);
        assert!(s.contains("Name=Box"));
        assert!(s.contains("[Extra]\nKey=1\n"));
        assert!(!s.contains("folder-blue"));
        assert_eq!(s.matches("Icon=").count(), 1);
        // A file with someone else's keys in it is not ours to delete, marker or not.
        assert!(!linux::is_ours(&s));

        assert_eq!(linux::directory_file_with_icon(&s, png), s, "idempotent");
        // Revert takes our line out and leaves every other key. It cannot put `folder-blue`
        // back — replacing the key is the whole point of applying — so the folder falls back
        // to the file manager's default icon rather than the one it had before.
        assert_eq!(
            linux::directory_file_without_ours(&s).unwrap(),
            "[Desktop Entry]\nName=Box\n[Extra]\nKey=1\n"
        );
    }

    #[test]
    fn directory_file_revert_deletes_a_file_that_is_only_ours() {
        let ours = linux::directory_file_contents(std::path::Path::new("/a/.folderskin.png"));
        assert_eq!(linux::directory_file_without_ours(&ours), None);
    }

    #[test]
    fn directory_file_revert_leaves_a_foreign_icon_alone() {
        let foreign = "[Desktop Entry]\nIcon=folder-red\n";
        assert_eq!(
            linux::directory_file_without_ours(foreign).unwrap(),
            foreign
        );
    }

    #[test]
    fn directory_file_adds_the_group_when_there_is_none() {
        let s = linux::directory_file_with_icon(
            "[Something Else]\nKey=1\n",
            std::path::Path::new("/a/.folderskin.png"),
        );
        assert!(s.starts_with("[Desktop Entry]\nIcon=/a/.folderskin.png\n"));
        assert!(s.contains("[Something Else]\nKey=1\n"));
    }

    // ----------------------------------------------------------------------- paths

    #[test]
    fn validate_folder_rejects_files_roots_and_missing() {
        let tmp = tempfile_dir();
        assert!(validate_folder(&tmp).is_ok());
        let f = tmp.join("file.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(matches!(
            validate_folder(&f),
            Err(ApplyError::NotADirectory(_))
        ));
        assert!(matches!(
            validate_folder(std::path::Path::new("/")),
            Err(ApplyError::Refused(_))
        ));
        assert!(validate_folder(&tmp.join("nope")).is_err());
    }

    #[test]
    fn validate_folder_refuses_system_locations() {
        use super::paths::is_system_location;
        for p in [
            "/System/Library/Fonts",
            "/usr/local/bin",
            "/Applications/Safari.app/Contents/Resources",
            "/Library/Preferences",
            "C:/Windows/System32",
            "c:/Program Files/Whatever",
        ] {
            assert!(
                is_system_location(std::path::Path::new(p)),
                "{p} should be refused"
            );
        }
        for p in [
            "/Users/me/Desktop/readme",
            "/home/me/pictures",
            "C:/Users/me/Documents",
        ] {
            assert!(
                !is_system_location(std::path::Path::new(p)),
                "{p} should be allowed"
            );
        }
        // A scratch folder in the OS temp directory is not a system location.
        let tmp = tempfile_dir();
        assert!(!is_system_location(&tmp.canonicalize().unwrap()));
        assert!(validate_folder(&tmp).is_ok());
    }

    #[test]
    fn validate_folder_returns_a_canonical_path() {
        let tmp = tempfile_dir();
        let nested = tmp.join("a");
        std::fs::create_dir(&nested).unwrap();
        let round_about = tmp.join("a").join(".").join("..").join("a");
        // Canonical, less the `\\?\` prefix canonicalize adds on Windows, which validate_folder
        // strips (see strip_verbatim_prefix). On other systems there is no prefix to strip.
        let canonical = nested.canonicalize().unwrap();
        let expected =
            PathBuf::from(paths::strip_verbatim_prefix(&canonical.to_string_lossy()).as_ref());
        assert_eq!(validate_folder(&round_about).unwrap(), expected);
    }

    #[test]
    fn validate_folder_refuses_the_home_directory_itself() {
        let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let Some(home) = std::env::var_os(var) else {
            return;
        };
        let home = PathBuf::from(home);
        if !home.is_dir() {
            return;
        }
        assert!(matches!(
            validate_folder(&home),
            Err(ApplyError::Refused(_))
        ));
    }

    #[test]
    fn verbatim_prefixes_are_stripped() {
        assert_eq!(
            paths::strip_verbatim_prefix(r"\\?\C:\Users\me"),
            r"C:\Users\me"
        );
        assert_eq!(
            paths::strip_verbatim_prefix(r"\\?\UNC\server\share\dir"),
            r"\\server\share\dir"
        );
        assert_eq!(paths::strip_verbatim_prefix("/home/me"), "/home/me");
    }

    #[test]
    fn write_atomic_replaces_a_file_and_leaves_no_temp_behind() {
        let tmp = tempfile_dir();
        let target = tmp.join("desktop.ini");
        paths::write_atomic(&target, b"first").unwrap();
        paths::write_atomic(&target, b"second").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");

        let names: Vec<String> = std::fs::read_dir(&*tmp)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["desktop.ini".to_string()]);
    }

    #[test]
    fn read_text_if_present_reports_a_missing_file_and_refuses_binary() {
        let tmp = tempfile_dir();
        let path = tmp.join("desktop.ini");
        assert_eq!(paths::read_text_if_present(&path).unwrap(), None);

        std::fs::write(&path, b"[.ShellClassInfo]\r\n").unwrap();
        assert!(paths::read_text_if_present(&path).unwrap().is_some());

        std::fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(
            paths::read_text_if_present(&path),
            Err(ApplyError::Refused(_))
        ));
    }
}
