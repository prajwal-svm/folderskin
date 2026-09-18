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

use std::path::{Path, PathBuf};

pub mod linux;
pub mod paths;
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

/// Applies the rendered icon set to `folder` using the current OS mechanism.
pub fn apply_icon(folder: &Path, icons: &crate::compositor::IconSet) -> Result<(), ApplyError> {
    let folder = validate_folder(folder)?;
    #[cfg(target_os = "macos")]
    {
        macos::apply(&folder, icons)
    }
    #[cfg(target_os = "windows")]
    {
        windows::apply(&folder, icons)
    }
    #[cfg(target_os = "linux")]
    {
        linux::apply(&folder, icons)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (folder, icons);
        Err(ApplyError::Platform(
            "custom folder icons are not supported on this OS".into(),
        ))
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

/// A unique scratch directory under the system temp dir, removed when the guard drops.
///
/// Derefs to `Path`, so it can be passed straight to anything taking `&Path`.
#[cfg(test)]
pub(crate) struct TempDir(PathBuf);

#[cfg(test)]
impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Creates a scratch directory for a test. Unique per process and per call, so tests running
/// in parallel never share one.
#[cfg(test)]
pub(crate) fn tempfile_dir() -> TempDir {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let path = std::env::temp_dir().join(format!(
        "folderskin-test-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("create the scratch directory");
    TempDir(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------ desktop.ini

    #[test]
    fn desktop_ini_is_created_with_marker_and_icon_resource() {
        let s = windows::desktop_ini_contents(None);
        assert!(s.starts_with("[.ShellClassInfo]\r\n"));
        assert!(s.contains("IconResource=folderskin.ico,0\r\n"));
        assert!(s.contains(windows::MARKER));
    }

    #[test]
    fn desktop_ini_preserves_foreign_keys_and_replaces_icon_resource() {
        let existing = "[.ShellClassInfo]\r\nInfoTip=hello\r\nIconResource=other.ico,0\r\n";
        let s = windows::desktop_ini_contents(Some(existing));
        assert!(s.contains("InfoTip=hello"));
        assert!(!s.contains("other.ico"));
        assert_eq!(s.matches("IconResource=").count(), 1);
    }

    #[test]
    fn revert_removes_only_our_lines_or_whole_file() {
        let ours = windows::desktop_ini_contents(None);
        assert_eq!(windows::desktop_ini_without_ours(&ours), None);
        let mixed = windows::desktop_ini_contents(Some("[.ShellClassInfo]\r\nInfoTip=keep\r\n"));
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
        let once = windows::desktop_ini_contents(Some(existing));
        assert!(once.contains("[ViewState]\r\nMode=\r\nVid=\r\n"));
        // Our lines land at the end of the section we own, not at the end of the file.
        assert!(once.contains("InfoTip=hello\r\n; managed by FolderSkin\r\n"));

        let twice = windows::desktop_ini_contents(Some(&once));
        assert_eq!(once, twice, "re-applying must not stack up our lines");

        let left = windows::desktop_ini_without_ours(&twice).unwrap();
        assert_eq!(left, existing, "revert must restore the file we found");
    }

    #[test]
    fn desktop_ini_adds_our_section_when_there_is_none() {
        let s = windows::desktop_ini_contents(Some("[ViewState]\r\nMode=\r\n"));
        assert!(s.starts_with("[.ShellClassInfo]\r\n"));
        assert!(s.contains("[ViewState]\r\nMode=\r\n"));
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
        let s = windows::desktop_ini_contents(Some("\u{feff}[.ShellClassInfo]\r\nInfoTip=hi\r\n"));
        assert!(s.starts_with("\u{feff}[.ShellClassInfo]\r\n"));
        assert_eq!(s.matches("[.ShellClassInfo]").count(), 1);
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
    fn validate_folder_returns_a_canonical_path() {
        let tmp = tempfile_dir();
        let nested = tmp.join("a");
        std::fs::create_dir(&nested).unwrap();
        let round_about = tmp.join("a").join(".").join("..").join("a");
        assert_eq!(
            validate_folder(&round_about).unwrap(),
            nested.canonicalize().unwrap()
        );
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
