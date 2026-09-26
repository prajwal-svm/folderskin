//! Which drive a picked path is, if any: the root of a mounted volume, and what kind of drive it
//! is (docs/DRIVES.md, "How FolderSkin tells what was picked").
//!
//! It comes in two steps. [`volume_at`] says whether a path is a volume's root, and is cheap
//! enough to ask for every folder an apply or a revert reaches: `statfs` on macOS, the path itself
//! on Windows, the device numbers and `/proc/self/mounts` on Linux. [`drive_at`] then asks the
//! system what kind of drive it is, which takes longer (`diskutil` on macOS, the drive's bus on
//! Windows, sysfs on Linux) and is only done for the path that was picked.
//!
//! What each system says is turned into a kind by a pure function in that system's file, which
//! compiles and is tested on every machine; only the asking is for its own system.

pub mod linux;
pub mod mac;
pub mod windows;

use super::{DriveKind, DriveShape, DriveStyle};
use std::path::{Path, PathBuf};

/// The root of a mounted volume, as [`volume_at`] finds it: all an apply or a revert needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// Its mount point, or a drive letter's root such as `D:\`.
    pub root: PathBuf,
    /// The volume the system runs from: `/`, or Windows' `%SystemDrive%`.
    pub startup: bool,
    /// Mounted read-only: a disc, a locked card, a disk image attached read-only.
    pub read_only: bool,
    /// On another computer: an SMB, AFP, NFS or WebDAV share, or a mapped network drive.
    pub network: bool,
    /// The file system as the system names it: `apfs`, `smbfs`, `ext4`, `NTFS`.
    pub fs_type: String,
    /// What is mounted there: `/dev/disk4s1`, `//me@nas/Media`, `/dev/sdb1`. Empty on Windows.
    pub device: String,
}

impl Volume {
    /// The drive letter of a Windows drive, such as `D`.
    pub fn letter(&self) -> Option<char> {
        windows::root_letter(&self.root.to_string_lossy())
    }
}

/// A drive: a volume's root, what kind of drive it is, the shape it's drawn as here and the name
/// its system gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    pub volume: Volume,
    pub kind: DriveKind,
    /// The shape a skin goes on for it on this system: its kind's ([`DriveShape::for_kind`]), or
    /// for Linux's startup disk the disk it's on, which is how Linux draws it.
    pub shape: DriveShape,
    /// Its own name, as its system shows it: `Macintosh HD`, `DEV-PSV`, the label of a USB
    /// stick. Empty when it has none, as a Windows drive without a label or Linux's `/` often do.
    pub name: String,
}

impl Drive {
    /// A drive of `kind` on `volume`, drawn as this system draws that kind.
    fn new(volume: Volume, kind: DriveKind, name: String) -> Drive {
        Drive {
            volume,
            kind,
            shape: DriveShape::for_kind(DriveStyle::current(), kind),
            name,
        }
    }
}

/// The volume whose root `path` is, or `None` for anything else: a folder, a file, a path that
/// isn't there, or a mount the system keeps out of sight (macOS's `nobrowse` volumes, Linux's
/// `/proc`, `/boot` and `/home`).
pub fn volume_at(path: &Path) -> Option<Volume> {
    #[cfg(target_os = "macos")]
    {
        mac::volume_at(path)
    }
    #[cfg(target_os = "windows")]
    {
        windows::volume_at(path)
    }
    #[cfg(target_os = "linux")]
    {
        linux::volume_at(path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = path;
        None
    }
}

/// The drive whose root `path` is, with its kind and name; `None` when it isn't one
/// ([`volume_at`]).
pub fn drive_at(path: &Path) -> Option<Drive> {
    let volume = volume_at(path)?;
    #[cfg(target_os = "macos")]
    {
        Some(mac::drive(volume))
    }
    #[cfg(target_os = "windows")]
    {
        Some(windows::drive(volume))
    }
    #[cfg(target_os = "linux")]
    {
        Some(linux::drive(volume))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Some(Drive::new(volume, DriveKind::Internal, String::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::tempfile_dir;

    #[test]
    fn a_folder_or_a_file_is_not_a_drive() {
        let dir = tempfile_dir();
        let file = dir.join("notes.txt");
        std::fs::write(&file, b"x").unwrap();
        for path in [&*dir, &file, &dir.join("missing")] {
            assert_eq!(volume_at(path), None, "{}", path.display());
            assert_eq!(drive_at(path), None, "{}", path.display());
        }
    }

    /// The startup disk is a drive wherever FolderSkin runs, drawn as that system draws it.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn the_startup_disk_is_a_drive() {
        let drive = drive_at(Path::new("/")).expect("/ is the startup disk");
        assert!(drive.volume.startup);
        assert_eq!(drive.volume.root, Path::new("/"));
        assert_eq!(drive.kind, DriveKind::Startup);
        assert_eq!(drive.shape.style(), DriveStyle::current());
        #[cfg(target_os = "macos")]
        {
            assert_eq!(drive.shape.kind(), DriveKind::Startup);
            assert!(drive.volume.read_only, "macOS seals its startup disk");
            assert!(!drive.name.is_empty(), "Finder names it");
        }
    }

    /// Prints what FolderSkin makes of every drive connected now, and of the paths in
    /// `FOLDERSKIN_DRIVES` (separated as `PATH` is). Only reads.
    #[test]
    #[ignore = "reads this computer's drives: cargo test -p folderskin-core -- --ignored drives_here --nocapture"]
    fn drives_here() {
        let mut roots = vec![PathBuf::from("/")];
        if let Some(more) = std::env::var_os("FOLDERSKIN_DRIVES") {
            roots.extend(std::env::split_paths(&more));
        }
        for dir in ["/Volumes", "/media", "/run/media", "/mnt"] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                roots.extend(entries.flatten().map(|e| e.path()));
            }
        }
        roots.extend(('A'..='Z').map(|c| PathBuf::from(format!(r"{c}:\"))));
        for root in roots {
            if let Some(drive) = drive_at(&root) {
                println!(
                    "{}: {:?} as {}, {:?}, read-only {}, network {}, {} on {}",
                    root.display(),
                    drive.kind,
                    drive.shape.id(),
                    drive.name,
                    drive.volume.read_only,
                    drive.volume.network,
                    drive.volume.fs_type,
                    drive.volume.device
                );
            }
        }
    }

    #[test]
    fn a_drive_letter_is_read_from_the_root() {
        let volume = |root: &str| Volume {
            root: PathBuf::from(root),
            startup: false,
            read_only: false,
            network: false,
            fs_type: String::new(),
            device: String::new(),
        };
        assert_eq!(volume(r"E:\").letter(), Some('E'));
        assert_eq!(volume("/Volumes/Stick").letter(), None);
    }
}
