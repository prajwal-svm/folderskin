//! Drives on Windows. A drive is a drive letter's root, such as `D:\`; `GetDriveType` says a
//! network drive, a disc drive or removable media, and for a disk its bus says the rest. A share
//! by its `\\server\share` path has no letter, and Windows gives it no icon of its own, so it
//! stays a folder.

use super::super::DriveKind;
#[cfg(target_os = "windows")]
use super::{Drive, Volume};
#[cfg(target_os = "windows")]
use std::path::Path;

// What `GetDriveTypeW` answers.
pub const DRIVE_REMOVABLE: u32 = 2;
pub const DRIVE_FIXED: u32 = 3;
pub const DRIVE_REMOTE: u32 = 4;
pub const DRIVE_CDROM: u32 = 5;
pub const DRIVE_RAMDISK: u32 = 6;

// The buses `IOCTL_STORAGE_QUERY_PROPERTY` names (`STORAGE_BUS_TYPE`) that tell a disk apart.
pub const BUS_RAID: i32 = 8;
pub const BUS_USB: i32 = 7;
pub const BUS_SD: i32 = 12;
pub const BUS_MMC: i32 = 13;
pub const BUS_FILE_BACKED_VIRTUAL: i32 = 15;
pub const BUS_SPACES: i32 = 16;

/// The letter of the drive whose root `path` is: `D` for `D:\`, `d:`, `D:/` or `\\?\D:\`. `None`
/// for anything else, a folder on a drive included.
pub fn root_letter(path: &str) -> Option<char> {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let mut chars = path.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    if chars.next() != Some(':') {
        return None;
    }
    match chars.as_str() {
        "" | "\\" | "/" => Some(letter.to_ascii_uppercase()),
        _ => None,
    }
}

/// The kind of drive a drive letter is, from what `GetDriveTypeW` answers, the disk's bus when it
/// could be asked, and whether it's `%SystemDrive%`. `None` for a letter with no drive behind it.
pub fn kind(drive_type: u32, bus: Option<i32>, system: bool) -> Option<DriveKind> {
    let card = matches!(bus, Some(BUS_SD | BUS_MMC));
    Some(match drive_type {
        DRIVE_REMOTE => DriveKind::Network,
        DRIVE_CDROM => DriveKind::Optical,
        DRIVE_REMOVABLE if card => DriveKind::Card,
        DRIVE_REMOVABLE => DriveKind::Removable,
        DRIVE_FIXED | DRIVE_RAMDISK if system => DriveKind::Startup,
        DRIVE_FIXED | DRIVE_RAMDISK => match bus {
            Some(BUS_USB) => DriveKind::External,
            Some(BUS_SD | BUS_MMC) => DriveKind::Card,
            Some(BUS_FILE_BACKED_VIRTUAL) => DriveKind::DiskImage,
            Some(BUS_RAID | BUS_SPACES) => DriveKind::MultiDisk,
            _ => DriveKind::Internal,
        },
        _ => return None,
    })
}

/// The letter `%SystemDrive%` names, `C` as a rule.
#[cfg(target_os = "windows")]
fn system_letter() -> Option<char> {
    root_letter(&std::env::var("SystemDrive").ok()?)
}

/// The UTF-16, NUL-terminated form of `s` that Win32 takes.
#[cfg(target_os = "windows")]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A volume's label, its file system flags and its file system's name; `None` when Windows
/// can't say, as for a disc drive with no disc in it.
#[cfg(target_os = "windows")]
fn volume_information(root: &str) -> Option<(String, u32, String)> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
    let root = wide(root);
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    let mut flags = 0u32;
    // SAFETY: `root` is NUL-terminated and both buffers are writable for the lengths given.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut flags,
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    let text = |buf: &[u16]| {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    };
    (ok != 0).then(|| (text(&label), flags, text(&fs)))
}

/// `FILE_READ_ONLY_VOLUME`, among the flags `GetVolumeInformationW` gives.
#[cfg(target_os = "windows")]
const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;

/// The drive whose root `path` is.
#[cfg(target_os = "windows")]
pub fn volume_at(path: &Path) -> Option<Volume> {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let letter = root_letter(&path.to_string_lossy())?;
    let root = format!(r"{letter}:\");
    // SAFETY: the root is NUL-terminated.
    let drive_type = unsafe { GetDriveTypeW(wide(&root).as_ptr()) };
    kind(drive_type, None, false)?;
    let (_, flags, fs_type) = volume_information(&root).unwrap_or_default();
    Some(Volume {
        root: root.into(),
        startup: system_letter() == Some(letter),
        read_only: flags & FILE_READ_ONLY_VOLUME != 0,
        network: drive_type == DRIVE_REMOTE,
        fs_type,
        device: String::new(),
    })
}

/// The drive `volume` is: its kind from its type and its disk's bus, and its label.
#[cfg(target_os = "windows")]
pub fn drive(volume: Volume) -> Drive {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let letter = volume.letter().unwrap_or('C');
    let root = format!(r"{letter}:\");
    // SAFETY: the root is NUL-terminated.
    let drive_type = unsafe { GetDriveTypeW(wide(&root).as_ptr()) };
    let bus = matches!(drive_type, DRIVE_FIXED | DRIVE_REMOVABLE)
        .then(|| bus_type(letter))
        .flatten();
    let kind = kind(drive_type, bus, volume.startup).unwrap_or(DriveKind::Internal);
    let name = volume_information(&root)
        .map(|(label, _, _)| label)
        .unwrap_or_default();
    Drive::new(volume, kind, name)
}

/// The bus the disk behind drive `letter` is on (`STORAGE_BUS_TYPE`), asked of the volume
/// itself, which needs no rights to read.
#[cfg(target_os = "windows")]
fn bus_type(letter: char) -> Option<i32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY,
        STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let device = wide(&format!(r"\\.\{letter}:"));
    // SAFETY: `device` is NUL-terminated; no access is asked for, only the right to query.
    let handle = unsafe {
        CreateFileW(
            device.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut descriptor = STORAGE_DEVICE_DESCRIPTOR::default();
    let mut returned = 0u32;
    // SAFETY: the query and the descriptor are the sizes given, and `handle` is open.
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            std::ptr::from_ref(&query).cast(),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            std::ptr::from_mut(&mut descriptor).cast(),
            std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    // SAFETY: `handle` was opened above and is closed once.
    unsafe { CloseHandle(handle) };
    (ok != 0).then_some(descriptor.BusType)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_letter_is_only_a_drive_at_its_root() {
        for (path, letter) in [
            (r"D:\", Some('D')),
            ("d:", Some('D')),
            ("E:/", Some('E')),
            (r"\\?\F:\", Some('F')),
            (r"D:\Photos", None),
            (r"D:Photos", None),
            (r"\\server\share\", None),
            (r"\\?\UNC\server\share", None),
            ("/Volumes/Stick", None),
            ("1:\\", None),
            ("", None),
        ] {
            assert_eq!(root_letter(path), letter, "{path}");
        }
    }

    #[test]
    fn a_drive_is_told_apart_by_its_type_and_its_bus() {
        assert_eq!(kind(DRIVE_FIXED, None, true), Some(DriveKind::Startup));
        assert_eq!(
            kind(DRIVE_FIXED, Some(11), false),
            Some(DriveKind::Internal)
        );
        assert_eq!(kind(DRIVE_FIXED, None, false), Some(DriveKind::Internal));
        assert_eq!(
            kind(DRIVE_FIXED, Some(BUS_USB), false),
            Some(DriveKind::External)
        );
        assert_eq!(
            kind(DRIVE_FIXED, Some(BUS_FILE_BACKED_VIRTUAL), false),
            Some(DriveKind::DiskImage)
        );
        assert_eq!(
            kind(DRIVE_FIXED, Some(BUS_SPACES), false),
            Some(DriveKind::MultiDisk)
        );
        assert_eq!(
            kind(DRIVE_REMOVABLE, Some(BUS_USB), false),
            Some(DriveKind::Removable)
        );
        assert_eq!(
            kind(DRIVE_REMOVABLE, Some(BUS_SD), false),
            Some(DriveKind::Card)
        );
        assert_eq!(kind(DRIVE_REMOTE, None, false), Some(DriveKind::Network));
        assert_eq!(kind(DRIVE_CDROM, None, false), Some(DriveKind::Optical));
        assert_eq!(kind(0, None, false), None, "DRIVE_UNKNOWN");
        assert_eq!(kind(1, None, false), None, "DRIVE_NO_ROOT_DIR");
    }
}
