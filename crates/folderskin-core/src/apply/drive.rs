//! A drive's own icon (docs/DRIVES.md, "How FolderSkin sets a drive's icon").
//!
//! Some drives can't have an icon of their own, and the stage says so before anything is tried:
//! [`refusal`] says which, and why.

use crate::drive::detect::Volume;
use crate::drive::DriveStyle;

/// Why FolderSkin can't give a drive an icon of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// macOS's startup disk, which macOS keeps sealed.
    StartupSealed,
    /// Linux's startup disk, which only the system writes to.
    StartupSystem,
    /// A drive mounted read-only, on a system that keeps a drive's icon on the drive.
    ReadOnly,
}

impl Refusal {
    /// How the webview names it, to say it in its own words: `startup-sealed`, `startup-system`,
    /// `read-only`.
    pub fn code(self) -> &'static str {
        match self {
            Refusal::StartupSealed => "startup-sealed",
            Refusal::StartupSystem => "startup-system",
            Refusal::ReadOnly => "read-only",
        }
    }

    /// The sentence an apply that is refused fails with.
    pub fn sentence(self) -> &'static str {
        match self {
            Refusal::StartupSealed => {
                "macOS keeps the startup disk sealed, so its icon can't be changed"
            }
            Refusal::StartupSystem => {
                "only the system can write to the startup disk, so its icon can't be changed"
            }
            Refusal::ReadOnly => "this drive is read-only, so its icon can't be changed",
        }
    }
}

/// Why FolderSkin can't change `volume`'s icon on this system; `None` when it can try.
pub fn refusal(volume: &Volume) -> Option<Refusal> {
    refusal_on(DriveStyle::current(), volume)
}

/// [`refusal`] on the system `system` names. macOS and Linux keep a drive's icon on the drive,
/// so a drive that can't be written to can't have one, and neither can the startup disk. Windows
/// keeps it in the user's own registry, so any drive letter can.
pub fn refusal_on(system: DriveStyle, volume: &Volume) -> Option<Refusal> {
    match system {
        DriveStyle::Windows => None,
        DriveStyle::Mac if volume.startup => Some(Refusal::StartupSealed),
        DriveStyle::Linux if volume.startup => Some(Refusal::StartupSystem),
        _ if volume.read_only => Some(Refusal::ReadOnly),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn volume(startup: bool, read_only: bool) -> Volume {
        Volume {
            root: PathBuf::from(if startup { "/" } else { "/Volumes/Stick" }),
            startup,
            read_only,
            network: false,
            fs_type: "apfs".into(),
            device: String::new(),
        }
    }

    #[test]
    fn a_drive_that_cant_be_written_to_is_refused_where_its_icon_lives_on_it() {
        let usb = volume(false, false);
        let disc = volume(false, true);
        let startup = volume(true, true);
        for system in DriveStyle::ALL {
            assert_eq!(refusal_on(system, &usb), None, "{system:?}");
        }
        assert_eq!(
            refusal_on(DriveStyle::Mac, &startup),
            Some(Refusal::StartupSealed)
        );
        assert_eq!(
            refusal_on(DriveStyle::Linux, &startup),
            Some(Refusal::StartupSystem)
        );
        assert_eq!(refusal_on(DriveStyle::Mac, &disc), Some(Refusal::ReadOnly));
        assert_eq!(
            refusal_on(DriveStyle::Linux, &disc),
            Some(Refusal::ReadOnly)
        );
        // Windows keeps a drive's icon in the registry, so a disc or the system drive can have one.
        assert_eq!(refusal_on(DriveStyle::Windows, &disc), None);
        assert_eq!(refusal_on(DriveStyle::Windows, &startup), None);
    }

    #[test]
    fn a_refusal_has_a_code_for_the_stage_and_a_sentence_for_an_apply() {
        for (refusal, code) in [
            (Refusal::StartupSealed, "startup-sealed"),
            (Refusal::StartupSystem, "startup-system"),
            (Refusal::ReadOnly, "read-only"),
        ] {
            assert_eq!(refusal.code(), code);
            assert!(refusal.sentence().ends_with("its icon can't be changed"));
        }
    }
}
