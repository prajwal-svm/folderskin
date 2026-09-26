//! A drive's own icon (docs/DRIVES.md, "How FolderSkin sets a drive's icon").
//!
//! Each system keeps it somewhere of its own:
//!
//! - **macOS** takes it with the same call as a folder's, on the drive's mount point, and writes
//!   it to the drive as `.VolumeIcon.icns` with the custom-icon flag on the root. A share that
//!   refuses that call gets the same written in by hand, as a folder there does.
//! - **Windows** looks it up by drive letter in the user's registry ([`windows`]), so nothing is
//!   written to the drive.
//! - **Linux** file managers draw a drive's root like any folder, and GNOME's list of drives reads
//!   `.xdg-volume-info` from it ([`linux`]).
//!
//! Some drives can't have an icon of their own, and the stage says so before anything is tried:
//! [`refusal`] says which, and why. [`apply`] refuses them all the same.

pub mod linux;
pub mod windows;

use super::ApplyError;
use crate::compositor::IconSet;
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

/// Why a share's server turned the icon down.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const SERVER_REFUSED: &str = "the server didn't let FolderSkin change this share's icon";
/// Why macOS turned a drive's icon down otherwise.
#[cfg(target_os = "macos")]
const MACOS_REFUSED: &str = "macOS refused to change this drive's icon";

/// Puts `icons` on the drive at `volume` as its own icon, refused first when it can't have one.
pub fn apply(volume: &Volume, icons: &IconSet) -> Result<(), ApplyError> {
    if let Some(refusal) = refusal(volume) {
        return Err(ApplyError::Refused(refusal.sentence().into()));
    }
    #[cfg(target_os = "macos")]
    {
        let image = super::macos::prepare(icons)?;
        super::macos::apply_volume(&volume.root, &image).map_err(|_| {
            ApplyError::Platform(
                if volume.network {
                    SERVER_REFUSED
                } else {
                    MACOS_REFUSED
                }
                .into(),
            )
        })
    }
    #[cfg(target_os = "windows")]
    {
        let letter = volume.letter().ok_or_else(|| {
            ApplyError::Refused("Windows gives an icon of its own only to a drive letter".into())
        })?;
        windows::apply(letter, &super::windows::prepare(icons)?)
    }
    #[cfg(target_os = "linux")]
    {
        let png = super::linux::prepare(icons)?;
        linux::apply(&volume.root, &png).map_err(|e| server_refused(volume, e))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = icons;
        Err(ApplyError::Platform(
            "custom drive icons are not supported on this OS".into(),
        ))
    }
}

/// Takes the drive's own icon off: any custom icon on macOS, as for a folder, and FolderSkin's on
/// Windows and Linux.
pub fn revert(volume: &Volume) -> Result<(), ApplyError> {
    if let Some(refusal) = refusal(volume) {
        return Err(ApplyError::Refused(refusal.sentence().into()));
    }
    #[cfg(target_os = "macos")]
    {
        super::macos::revert_volume(&volume.root).map_err(|_| {
            ApplyError::Platform(
                if volume.network {
                    SERVER_REFUSED
                } else {
                    MACOS_REFUSED
                }
                .into(),
            )
        })
    }
    #[cfg(target_os = "windows")]
    {
        match volume.letter() {
            Some(letter) => windows::revert(letter),
            None => Ok(()),
        }
    }
    #[cfg(target_os = "linux")]
    {
        linux::revert(&volume.root).map_err(|e| server_refused(volume, e))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = volume;
        Ok(())
    }
}

/// True when the drive wears an icon of its own that [`revert`] would take off.
pub fn has_custom_icon(volume: &Volume) -> bool {
    #[cfg(target_os = "macos")]
    {
        super::macos::has_custom_icon(&volume.root)
    }
    #[cfg(target_os = "windows")]
    {
        volume.letter().is_some_and(windows::has_custom_icon)
    }
    #[cfg(target_os = "linux")]
    {
        linux::has_custom_icon(&volume.root)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = volume;
        false
    }
}

/// A failure to write to a share, said as the server turning it down.
#[cfg(target_os = "linux")]
fn server_refused(volume: &Volume, e: ApplyError) -> ApplyError {
    let denied =
        matches!(&e, ApplyError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied);
    if volume.network && denied {
        ApplyError::Platform(SERVER_REFUSED.into())
    } else {
        e
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

    /// A disk image made for a test in the temp folder and attached there, where Finder shows it
    /// as a drive. It's detached, and its folder deleted, when the test ends, however it ends. No
    /// drive but this one is ever written to.
    #[cfg(target_os = "macos")]
    struct Image {
        mount: PathBuf,
        _dir: crate::apply::TempDir,
    }

    #[cfg(target_os = "macos")]
    impl Image {
        fn attach(read_only: bool) -> Image {
            let dir = crate::apply::tempfile_dir();
            let dmg = dir.join("drive.dmg");
            let mount = dir.join("mount");
            let hdiutil = |args: &[&std::ffi::OsStr]| {
                let out = std::process::Command::new("/usr/bin/hdiutil")
                    .args(args)
                    .output()
                    .expect("hdiutil");
                assert!(
                    out.status.success(),
                    "hdiutil {args:?}: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            };
            hdiutil(&[
                "create".as_ref(),
                "-size".as_ref(),
                "8m".as_ref(),
                "-fs".as_ref(),
                "APFS".as_ref(),
                "-volname".as_ref(),
                "FolderSkin Test".as_ref(),
                "-quiet".as_ref(),
                dmg.as_os_str(),
            ]);
            let mut attach = vec!["attach".as_ref(), "-quiet".as_ref(), "-mountpoint".as_ref()];
            attach.push(mount.as_os_str());
            if read_only {
                attach.push("-readonly".as_ref());
            }
            attach.push(dmg.as_os_str());
            hdiutil(&attach);
            Image { mount, _dir: dir }
        }
    }

    #[cfg(target_os = "macos")]
    impl Drop for Image {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/usr/bin/hdiutil")
                .args(["detach", "-force", "-quiet"])
                .arg(&self.mount)
                .status();
        }
    }

    #[cfg(target_os = "macos")]
    fn icons() -> IconSet {
        IconSet {
            sizes: [16, 32, 64, 128, 256, 512, 1024]
                .into_iter()
                .map(|size| {
                    let pixel = image::Rgba([0x35, 0x7A, 0xD6, 0xFF]);
                    (size, image::RgbaImage::from_pixel(size, size, pixel))
                })
                .collect(),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "attaches a disk image and needs a desktop session: cargo test -p folderskin-core -- --ignored drive_image"]
    fn a_drive_image_takes_an_icon_and_gives_it_back() {
        use crate::drive::detect::{drive_at, volume_at};
        use crate::drive::DriveKind;

        let image = Image::attach(false);
        let volume = volume_at(&image.mount).expect("an attached image is a drive");
        assert_eq!(
            drive_at(&image.mount).map(|d| d.kind),
            Some(DriveKind::DiskImage)
        );
        assert_eq!(refusal(&volume), None);
        assert!(
            !has_custom_icon(&volume),
            "a new image has no icon of its own"
        );

        apply(&volume, &icons()).unwrap();
        assert!(image.mount.join(".VolumeIcon.icns").is_file());
        assert!(has_custom_icon(&volume));
        // As a run over the drive's folders sees it too.
        assert!(crate::apply::has_custom_icon(&image.mount));

        // A second skin over the first leaves one icon, still on.
        apply(&volume, &icons()).unwrap();
        assert!(has_custom_icon(&volume));

        revert(&volume).unwrap();
        assert!(!image.mount.join(".VolumeIcon.icns").exists());
        assert!(!has_custom_icon(&volume));
    }

    /// A share that refuses `setIcon` refuses clearing too, and gets both done by hand, as a
    /// folder there does. On a drive image the icon written in is the one AppKit writes, and
    /// taking it off leaves the drive as a revert does.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "attaches a disk image and needs a desktop session: cargo test -p folderskin-core -- --ignored drive_image"]
    fn a_drive_image_takes_its_icon_by_hand_as_a_share_that_refuses_appkit_does() {
        use crate::apply::macos;

        let image = Image::attach(false);
        let volume = crate::drive::detect::volume_at(&image.mount).expect("a drive");
        let icon = macos::prepare(&icons()).unwrap();
        let file = image.mount.join(".VolumeIcon.icns");

        macos::apply_volume(&image.mount, &icon).unwrap();
        let appkit = std::fs::read(&file).unwrap();
        macos::revert_volume(&image.mount).unwrap();
        assert!(!file.exists());

        macos::apply_volume_by_hand(&image.mount, &icon).unwrap();
        assert_eq!(
            std::fs::read(&file).unwrap(),
            appkit,
            "the icon AppKit would have written"
        );
        assert!(has_custom_icon(&volume));

        macos::revert_volume_by_hand(&image.mount).unwrap();
        assert!(!file.exists());
        assert!(!has_custom_icon(&volume));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "attaches a disk image and needs a desktop session: cargo test -p folderskin-core -- --ignored drive_image"]
    fn a_read_only_drive_image_is_refused_before_anything_is_tried() {
        let image = Image::attach(true);
        let volume = crate::drive::detect::volume_at(&image.mount).expect("a drive");
        assert!(volume.read_only);
        assert_eq!(refusal(&volume), Some(Refusal::ReadOnly));
        let refused = apply(&volume, &icons()).unwrap_err();
        assert_eq!(refused.to_string(), Refusal::ReadOnly.sentence());
        assert!(!image.mount.join(".VolumeIcon.icns").exists());
    }
}
