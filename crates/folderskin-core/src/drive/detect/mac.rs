//! Drives on macOS. `statfs` names the mount point a path is on, so a path is a drive when it is
//! that mount point; the file system's type says a network share or a disc, and for a disk
//! `diskutil info -plist` says how it's connected and what its media is. Time Machine names the
//! disks it backs up to.

use super::super::DriveKind;
#[cfg(target_os = "macos")]
use super::{Drive, Volume};
#[cfg(target_os = "macos")]
use std::path::{Path, PathBuf};

/// The file systems of network shares.
pub const NETWORK_FS: &[&str] = &["smbfs", "afpfs", "nfs", "webdav", "ftp", "cifs"];
/// The file systems of discs.
pub const DISC_FS: &[&str] = &["cd9660", "cddafs", "udf"];

/// What macOS says about a volume, which [`kind`] turns into a kind of drive.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Mounted at `/`.
    pub startup: bool,
    /// On this Mac rather than another computer (`MNT_LOCAL`).
    pub local: bool,
    /// `statfs`'s file system type: `apfs`, `hfs`, `exfat`, `smbfs`, `cd9660`.
    pub fs_type: String,
    /// diskutil's `BusProtocol`: `Apple Fabric`, `USB`, `Thunderbolt`, `Disk Image`,
    /// `Secure Digital`.
    pub bus: String,
    /// diskutil's `Internal`: the disk is inside the Mac.
    pub internal: bool,
    /// diskutil's `RemovableMedia`: its media comes out, as a USB stick's does as far as macOS
    /// can tell.
    pub removable_media: bool,
    /// diskutil gives it an `OpticalMediaType`: it's a disc.
    pub optical: bool,
    /// diskutil's `MediaName`, which is how a card in a USB card reader shows.
    pub media_name: String,
    /// Time Machine backs up to it.
    pub time_machine: bool,
}

/// The kind of drive a volume is on macOS, from what the system says about it.
pub fn kind(f: &Facts) -> DriveKind {
    let fs = f.fs_type.to_ascii_lowercase();
    if f.startup {
        DriveKind::Startup
    } else if !f.local || NETWORK_FS.contains(&fs.as_str()) {
        DriveKind::Network
    } else if f.optical || DISC_FS.contains(&fs.as_str()) {
        DriveKind::Optical
    } else if f.time_machine {
        DriveKind::TimeMachine
    } else if f.bus == "Disk Image" {
        DriveKind::DiskImage
    } else if f.bus == "Secure Digital" || is_card_reader(&f.media_name) {
        DriveKind::Card
    } else if f.internal {
        DriveKind::Internal
    } else if f.removable_media {
        DriveKind::Removable
    } else {
        DriveKind::External
    }
}

/// True for the media name of a card in a card reader: "SD Card Reader", "USB3.0 CRW -SD",
/// "Apple SDXC Reader Media". Words are matched whole, so "SanDisk" isn't one.
pub fn is_card_reader(media_name: &str) -> bool {
    const CARDS: &[&str] = &["SD", "SDHC", "SDXC", "microSD", "MMC", "CF", "CFexpress"];
    media_name.to_ascii_lowercase().contains("card")
        || media_name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| CARDS.contains(&word))
}

/// The volume whose mount point `path` is, as `statfs` has it. macOS's own hidden volumes
/// (`nobrowse`, as `/System/Volumes/Data` is) are left out: Finder doesn't show them as drives.
#[cfg(target_os = "macos")]
pub fn volume_at(path: &Path) -> Option<Volume> {
    let canonical = path.canonicalize().ok()?;
    let st = statfs(&canonical)?;
    if canonical != Path::new(&c_chars(&st.f_mntonname)) {
        return None;
    }
    let flags = st.f_flags as i32;
    if flags & libc::MNT_DONTBROWSE != 0 {
        return None;
    }
    let fs_type = c_chars(&st.f_fstypename);
    let network = flags & libc::MNT_LOCAL == 0 || NETWORK_FS.contains(&fs_type.as_str());
    Some(Volume {
        root: canonical,
        startup: flags & libc::MNT_ROOTFS != 0,
        read_only: flags & libc::MNT_RDONLY != 0,
        network,
        fs_type,
        device: c_chars(&st.f_mntfromname),
    })
}

/// The drive `volume` is: `diskutil` and Time Machine are asked about a disk, and Finder for the
/// name it shows.
#[cfg(target_os = "macos")]
pub fn drive(volume: Volume) -> Drive {
    let mut facts = Facts {
        startup: volume.startup,
        local: !volume.network,
        fs_type: volume.fs_type.clone(),
        ..Facts::default()
    };
    let mut volume_name = String::new();
    if facts.local && !facts.startup {
        if let Some(info) = diskutil_info(&volume.root) {
            let text = |key: &str| {
                info.get(key)
                    .and_then(|v| v.as_string())
                    .unwrap_or_default()
                    .to_string()
            };
            let yes = |key: &str| info.get(key).and_then(|v| v.as_boolean()) == Some(true);
            facts.bus = text("BusProtocol");
            facts.internal = yes("Internal");
            facts.removable_media = yes("RemovableMedia");
            facts.optical = info.contains_key("OpticalMediaType");
            facts.media_name = text("MediaName");
            volume_name = text("VolumeName");
        }
        facts.time_machine = volume.root.join("Backups.backupdb").is_dir()
            || time_machine_mounts().contains(&volume.root);
    }
    let name = finder_name(&volume.root)
        .filter(|n| !n.is_empty())
        .unwrap_or(volume_name);
    let kind = kind(&facts);
    Drive::new(volume, kind, name)
}

#[cfg(target_os = "macos")]
fn statfs(path: &Path) -> Option<libc::statfs> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: `c` is NUL-terminated and `st` is written whole by a successful call.
    let ok = unsafe { libc::statfs(c.as_ptr(), st.as_mut_ptr()) } == 0;
    // SAFETY: only read once statfs has filled it in.
    ok.then(|| unsafe { st.assume_init() })
}

/// A NUL-terminated C string held in a fixed array, as `statfs` fills them in.
#[cfg(target_os = "macos")]
fn c_chars(chars: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// What `diskutil info -plist` says about the disk mounted at `root`.
#[cfg(target_os = "macos")]
fn diskutil_info(root: &Path) -> Option<plist::Dictionary> {
    let out = std::process::Command::new("/usr/sbin/diskutil")
        .args(["info", "-plist"])
        .arg(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    plist::Value::from_reader_xml(&out.stdout[..])
        .ok()?
        .into_dictionary()
}

/// The mount points of the disks Time Machine backs up to that are connected now.
#[cfg(target_os = "macos")]
fn time_machine_mounts() -> Vec<PathBuf> {
    let Ok(out) = std::process::Command::new("/usr/bin/tmutil")
        .args(["destinationinfo", "-X"])
        .output()
    else {
        return Vec::new();
    };
    time_machine_mount_points(&out.stdout)
}

/// The `MountPoint`s of the destinations `tmutil destinationinfo -X` lists.
#[cfg(target_os = "macos")]
fn time_machine_mount_points(plist_xml: &[u8]) -> Vec<PathBuf> {
    let Some(info) = plist::Value::from_reader_xml(plist_xml)
        .ok()
        .and_then(plist::Value::into_dictionary)
    else {
        return Vec::new();
    };
    info.get("Destinations")
        .and_then(|d| d.as_array())
        .into_iter()
        .flatten()
        .filter_map(|d| d.as_dictionary()?.get("MountPoint")?.as_string())
        .map(PathBuf::from)
        .collect()
}

/// The name Finder shows for the volume at `root`: `Macintosh HD` for `/`.
#[cfg(target_os = "macos")]
fn finder_name(root: &Path) -> Option<String> {
    use objc2_foundation::{NSFileManager, NSString};
    let path = NSString::from_str(root.to_str()?);
    Some(
        NSFileManager::defaultManager()
            .displayNameAtPath(&path)
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(bus: &str) -> Facts {
        Facts {
            local: true,
            fs_type: "apfs".into(),
            bus: bus.into(),
            ..Facts::default()
        }
    }

    #[test]
    fn a_volume_is_told_apart_by_what_macos_says_about_it() {
        assert_eq!(
            kind(&Facts {
                startup: true,
                local: true,
                internal: true,
                ..disk("Apple Fabric")
            }),
            DriveKind::Startup
        );
        assert_eq!(
            kind(&Facts {
                internal: true,
                ..disk("Apple Fabric")
            }),
            DriveKind::Internal
        );
        assert_eq!(kind(&disk("Thunderbolt")), DriveKind::External);
        assert_eq!(kind(&disk("USB")), DriveKind::External);
        assert_eq!(
            kind(&Facts {
                removable_media: true,
                ..disk("USB")
            }),
            DriveKind::Removable
        );
        assert_eq!(
            kind(&Facts {
                removable_media: true,
                ..disk("Disk Image")
            }),
            DriveKind::DiskImage,
            "a disk image says its media is removable too"
        );
        assert_eq!(
            kind(&Facts {
                removable_media: true,
                ..disk("Secure Digital")
            }),
            DriveKind::Card
        );
        assert_eq!(
            kind(&Facts {
                removable_media: true,
                media_name: "USB3.0 CRW -SD".into(),
                ..disk("USB")
            }),
            DriveKind::Card,
            "a card in a USB card reader"
        );
        assert_eq!(
            kind(&Facts {
                time_machine: true,
                ..disk("USB")
            }),
            DriveKind::TimeMachine
        );
        assert_eq!(
            kind(&Facts {
                fs_type: "cd9660".into(),
                removable_media: true,
                ..disk("USB")
            }),
            DriveKind::Optical
        );
        assert_eq!(
            kind(&Facts {
                optical: true,
                fs_type: "hfs".into(),
                ..disk("SATA")
            }),
            DriveKind::Optical
        );
    }

    #[test]
    fn a_share_is_a_network_drive_whatever_its_file_system() {
        for fs in ["smbfs", "afpfs", "nfs", "webdav"] {
            assert_eq!(
                kind(&Facts {
                    fs_type: fs.into(),
                    local: false,
                    ..Facts::default()
                }),
                DriveKind::Network,
                "{fs}"
            );
        }
        assert_eq!(
            kind(&Facts {
                fs_type: "macfuse".into(),
                local: false,
                ..Facts::default()
            }),
            DriveKind::Network,
            "any mount that isn't local"
        );
    }

    #[test]
    fn a_card_reader_is_known_by_its_name() {
        for name in [
            "SD Card Reader",
            "USB3.0 CRW -SD",
            "Apple SDXC Reader Media",
            "Multi-Card",
            "microSD",
        ] {
            assert!(is_card_reader(name), "{name}");
        }
        for name in [
            "SanDisk Ultra",
            "Samsung Portable SSD T7",
            "",
            "WD Elements",
        ] {
            assert!(!is_card_reader(name), "{name}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn time_machine_names_the_disks_it_backs_up_to() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Destinations</key>
	<array>
		<dict>
			<key>Kind</key>
			<string>Local</string>
			<key>MountPoint</key>
			<string>/Volumes/Backups</string>
			<key>Name</key>
			<string>Backups</string>
		</dict>
		<dict>
			<key>Kind</key>
			<string>Network</string>
			<key>Name</key>
			<string>NAS</string>
		</dict>
	</array>
</dict>
</plist>"#;
        assert_eq!(
            time_machine_mount_points(xml),
            vec![PathBuf::from("/Volumes/Backups")]
        );
        assert!(time_machine_mount_points(b"<plist><dict/></plist>").is_empty());
        assert!(time_machine_mount_points(b"not a plist").is_empty());
    }
}
