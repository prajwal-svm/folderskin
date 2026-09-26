//! Drives on Linux. `/proc/self/mounts` lists every mount point, but only some are drives to
//! the person using the computer: the ones the file manager shows as drives. GIO shows `/`, what
//! is mounted under `/media/`, `/run/media/` and the home folder, and network shares; `/mnt/` is
//! where people mount a disk by hand. `/boot`, `/home` on a partition of its own and the kernel's
//! own file systems are folders, or nothing, as far as anyone looking at them can tell.
//!
//! The file system's type says a network share or a disc, and for a disk the device and sysfs
//! say the rest: a loop device is a disk image, an `md` device a RAID set, an `mmcblk` device a
//! memory card, and sysfs says whether the disk's media can be removed, whether it is on USB and
//! whether it spins.
//!
//! GNOME mounts a share through GVfs rather than the kernel, as a folder under
//! `/run/user/<uid>/gvfs/`, and that folder is the share's root: a network drive too.

use super::super::DriveKind;
#[cfg(target_os = "linux")]
use super::super::{DriveShape, DriveStyle};
#[cfg(target_os = "linux")]
use super::{Drive, Volume};
use std::path::{Path, PathBuf};

/// Network file systems, the kernel's and FUSE's.
pub const NETWORK_FS: &[&str] = &[
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "ncpfs",
    "afs",
    "ceph",
    "glusterfs",
    "fuse.sshfs",
    "fuse.davfs2",
    "davfs",
    "fuse.rclone",
    "fuse.s3fs",
    "fuse.afpfs",
];

/// The file systems of discs.
pub const DISC_FS: &[&str] = &["iso9660", "udf"];

/// The kernel's and the system's own file systems, which are never a drive.
pub const SYSTEM_FS: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "tmpfs",
    "ramfs",
    "cgroup",
    "cgroup2",
    "securityfs",
    "pstore",
    "bpf",
    "debugfs",
    "tracefs",
    "configfs",
    "fusectl",
    "mqueue",
    "hugetlbfs",
    "autofs",
    "binfmt_misc",
    "rpc_pipefs",
    "nsfs",
    "efivarfs",
    "selinuxfs",
    "overlay",
    "squashfs",
    "nfsd",
    "fuse.gvfsd-fuse",
    "fuse.portal",
];

/// The GVfs schemes of network shares, as the first part of a share's folder name
/// (`smb-share:server=nas,share=media`).
const GVFS_NETWORK: &[&str] = &[
    "smb-share",
    "sftp",
    "ftp",
    "ftps",
    "dav",
    "davs",
    "afp-volume",
    "nfs",
];

/// One line of `/proc/self/mounts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    pub device: String,
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub read_only: bool,
}

/// Every mount in `/proc/self/mounts`'s text, in its order. The kernel writes a space, a tab, a
/// line break or a backslash in a field as an octal escape (`\040`), which is undone here.
pub fn parse_mounts(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            let device = unescape(fields.next()?);
            let mount_point = PathBuf::from(unescape(fields.next()?));
            let fs_type = unescape(fields.next()?);
            let options = fields.next()?;
            Some(Mount {
                device,
                mount_point,
                fs_type,
                read_only: options.split(',').any(|o| o == "ro"),
            })
        })
        .collect()
}

/// A field of `/proc/self/mounts` with its octal escapes undone.
pub fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'\\')
            .then(|| bytes.get(i + 1..i + 4))
            .flatten()
            .filter(|digits| digits.iter().all(|b| (b'0'..=b'7').contains(b)))
            .and_then(|digits| {
                let value = digits.iter().fold(0u32, |v, d| v * 8 + u32::from(d - b'0'));
                u8::try_from(value).ok()
            });
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// True for a network file system, by its type; a FUSE mount of a web address counts too.
pub fn is_network(fs_type: &str, device: &str) -> bool {
    NETWORK_FS.contains(&fs_type)
        || (fs_type.starts_with("fuse") && (device.starts_with("http") || device.contains("://")))
}

/// True for a mount the file manager shows as a drive: `/`, anything under `/media/`,
/// `/run/media/`, `/mnt/` or the home folder, and a network share anywhere. The kernel's own file
/// systems never are.
pub fn is_shown(mount: &Mount, home: Option<&Path>) -> bool {
    if SYSTEM_FS.contains(&mount.fs_type.as_str()) {
        return false;
    }
    let at = mount.mount_point.as_path();
    if at == Path::new("/") || is_network(&mount.fs_type, &mount.device) {
        return true;
    }
    let under = |dir: &Path| at.starts_with(dir) && at != dir;
    under(Path::new("/media"))
        || under(Path::new("/run/media"))
        || under(Path::new("/mnt"))
        || home.is_some_and(under)
}

/// The GVfs scheme of a share GNOME mounted, when `path` is one's root: `smb-share` for
/// `/run/user/1000/gvfs/smb-share:server=nas,share=media`. `None` for anything else, GVfs's
/// phones and cameras included, which aren't drives FolderSkin can change.
pub fn gvfs_share(path: &Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    let gvfs = path.parent()?;
    let user = gvfs.parent()?;
    let is_gvfs = gvfs.file_name()? == "gvfs"
        && user.parent()? == Path::new("/run/user")
        && user
            .file_name()?
            .to_str()?
            .bytes()
            .all(|b| b.is_ascii_digit());
    let scheme = name.split_once(':')?.0;
    (is_gvfs && GVFS_NETWORK.contains(&scheme)).then_some(scheme)
}

/// The disk a block device's partition is on, by the kernel's naming: `sdb` for `sdb1`,
/// `nvme0n1` for `nvme0n1p2`, `mmcblk0` for `mmcblk0p1`. A whole disk is its own.
pub fn disk_of(name: &str) -> String {
    let numbered = ["nvme", "mmcblk", "loop", "nbd", "md"];
    if numbered.iter().any(|p| name.starts_with(p)) {
        // These end in a digit whole, so a partition is marked with a `p` after the disk's digits.
        if let Some(at) = name.rfind('p') {
            let (disk, part) = name.split_at(at);
            let numbered = disk.ends_with(|c: char| c.is_ascii_digit());
            if numbered && part.len() > 1 && part[1..].bytes().all(|b| b.is_ascii_digit()) {
                return disk.to_string();
            }
        }
        return name.to_string();
    }
    if name.starts_with("sr") {
        return name.to_string();
    }
    name.trim_end_matches(|c: char| c.is_ascii_digit())
        .to_string()
}

/// What sysfs says about the disk a volume is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Disk {
    /// `removable`: its media comes out, as a USB stick's does.
    pub removable: bool,
    /// It's connected through USB.
    pub usb: bool,
    /// `queue/rotational`: it spins. `None` when sysfs doesn't say.
    pub spins: Option<bool>,
}

/// The kind of drive a volume is on Linux, from its mount and, for a disk, what sysfs says.
pub fn kind(mount: &Mount, disk: Option<Disk>) -> DriveKind {
    let device = mount.device.as_str();
    let name = device.rsplit('/').next().unwrap_or(device);
    if mount.mount_point == Path::new("/") {
        return DriveKind::Startup;
    }
    if is_network(&mount.fs_type, device) {
        return DriveKind::Network;
    }
    if DISC_FS.contains(&mount.fs_type.as_str()) || name.starts_with("sr") {
        return DriveKind::Optical;
    }
    if name.starts_with("loop") {
        return DriveKind::DiskImage;
    }
    if name.starts_with("md") {
        return DriveKind::MultiDisk;
    }
    if name.starts_with("mmcblk") {
        return DriveKind::Card;
    }
    let disk = disk.unwrap_or_default();
    if disk.removable {
        DriveKind::Removable
    } else if disk.usb {
        DriveKind::External
    } else if disk.spins == Some(false) {
        DriveKind::SolidState
    } else {
        DriveKind::Internal
    }
}

/// The mount `path` is the root of, from `/proc/self/mounts`, when the file manager shows it as
/// a drive. A folder on the same device as the folder it's in isn't a mount point at all, which
/// is known without reading the list.
#[cfg(target_os = "linux")]
pub fn volume_at(path: &Path) -> Option<Volume> {
    use std::os::unix::fs::MetadataExt;
    let canonical = path.canonicalize().ok()?;
    if gvfs_share(&canonical).is_some() {
        return Some(Volume {
            device: canonical.file_name()?.to_string_lossy().into_owned(),
            root: canonical,
            startup: false,
            read_only: false,
            network: true,
            fs_type: "gvfs".into(),
        });
    }
    if let Some(parent) = canonical.parent() {
        let here = std::fs::metadata(&canonical).ok()?.dev();
        if std::fs::metadata(parent).ok()?.dev() == here {
            return None;
        }
    }
    let mount = the_mount_at(&canonical)?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if !is_shown(&mount, home.as_deref()) {
        return None;
    }
    Some(Volume {
        startup: canonical == Path::new("/"),
        read_only: mount.read_only,
        network: is_network(&mount.fs_type, &mount.device),
        fs_type: mount.fs_type,
        device: mount.device,
        root: canonical,
    })
}

/// The mount at `at`: the last in the list, since a later mount at the same place hides the
/// earlier ones.
#[cfg(target_os = "linux")]
fn the_mount_at(at: &Path) -> Option<Mount> {
    let text = std::fs::read_to_string("/proc/self/mounts").ok()?;
    parse_mounts(&text)
        .into_iter()
        .rev()
        .find(|m| m.mount_point == at)
}

/// The drive `volume` is, with sysfs asked about its disk.
#[cfg(target_os = "linux")]
pub fn drive(volume: Volume) -> Drive {
    let mount = Mount {
        device: volume.device.clone(),
        mount_point: volume.root.clone(),
        fs_type: volume.fs_type.clone(),
        read_only: volume.read_only,
    };
    let disk = volume
        .device
        .starts_with("/dev/")
        .then(|| disk_facts(&volume.device))
        .flatten();
    let kind = if volume.fs_type == "gvfs" {
        DriveKind::Network
    } else {
        kind(&mount, disk)
    };
    // The startup disk is drawn as the disk it's on, as Linux draws it.
    let own = if kind == DriveKind::Startup {
        match disk.and_then(|d| d.spins) {
            Some(false) => DriveKind::SolidState,
            _ => DriveKind::Internal,
        }
    } else {
        kind
    };
    let name = if volume.startup || volume.fs_type == "gvfs" {
        String::new()
    } else {
        volume
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut drive = Drive::new(volume, kind, name);
    drive.shape = DriveShape::for_kind(DriveStyle::current(), own);
    drive
}

/// What sysfs says about the disk `device` is on. A device-mapper device (an encrypted or LVM
/// volume) is followed down to the disk under it.
#[cfg(target_os = "linux")]
fn disk_facts(device: &str) -> Option<Disk> {
    let real = Path::new(device).canonicalize().ok()?;
    let mut name = real.file_name()?.to_string_lossy().into_owned();
    for _ in 0..4 {
        if !name.starts_with("dm-") {
            break;
        }
        let slaves = Path::new("/sys/class/block").join(&name).join("slaves");
        name = std::fs::read_dir(slaves)
            .ok()?
            .flatten()
            .next()?
            .file_name()
            .to_string_lossy()
            .into_owned();
    }
    // sysfs knows a partition's disk: the folder the partition's own is in.
    let class = Path::new("/sys/class/block").join(&name);
    let disk = if class.join("partition").exists() {
        class
            .canonicalize()
            .ok()?
            .parent()?
            .file_name()?
            .to_string_lossy()
            .into_owned()
    } else if class.exists() {
        name
    } else {
        disk_of(&name)
    };
    let sys = Path::new("/sys/block").join(&disk);
    let read = |file: &str| std::fs::read_to_string(sys.join(file)).ok();
    Some(Disk {
        removable: read("removable").is_some_and(|s| s.trim() == "1"),
        usb: sys
            .canonicalize()
            .is_ok_and(|p| p.to_string_lossy().contains("/usb")),
        spins: read("queue/rotational").map(|s| s.trim() == "1"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "\
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
/dev/nvme0n1p1 /boot/efi vfat rw,relatime,fmask=0077,dmask=0077 0 0
tmpfs /run tmpfs rw,nosuid,nodev,size=1631640k,mode=755 0 0
/dev/sdb1 /media/ana/My\\040Stick vfat rw,nosuid,nodev,relatime,uid=1000 0 0
/dev/sr0 /media/ana/HOLIDAY\\040DVD iso9660 ro,nosuid,nodev,relatime 0 0
//nas/media /mnt/media cifs rw,relatime,vers=3.1.1 0 0
gvfsd-fuse /run/user/1000/gvfs fuse.gvfsd-fuse rw,nosuid,nodev,relatime 0 0
/dev/loop7 /snap/core22/1380 squashfs ro,nodev,relatime 0 0
";

    #[test]
    fn the_mount_list_is_read_with_its_escapes_undone() {
        let mounts = parse_mounts(MOUNTS);
        assert_eq!(mounts.len(), 10);
        let stick = &mounts[5];
        assert_eq!(stick.device, "/dev/sdb1");
        assert_eq!(stick.mount_point, Path::new("/media/ana/My Stick"));
        assert_eq!(stick.fs_type, "vfat");
        assert!(!stick.read_only);
        assert!(mounts[6].read_only, "a disc is mounted read-only");
        assert_eq!(unescape(r"a\011b\012c\134d"), "a\tb\nc\\d");
        assert_eq!(unescape(r"not\7an\escape\"), r"not\7an\escape\");
    }

    #[test]
    fn only_what_the_file_manager_shows_is_a_drive() {
        let mounts = parse_mounts(MOUNTS);
        let home = Path::new("/home/ana");
        let shown: Vec<&Path> = mounts
            .iter()
            .filter(|m| is_shown(m, Some(home)))
            .map(|m| m.mount_point.as_path())
            .collect();
        assert_eq!(
            shown,
            [
                Path::new("/"),
                Path::new("/media/ana/My Stick"),
                Path::new("/media/ana/HOLIDAY DVD"),
                Path::new("/mnt/media"),
            ]
        );
        let fuse = Mount {
            device: "sshfs#ana@server:".into(),
            mount_point: "/home/ana/server".into(),
            fs_type: "fuse.sshfs".into(),
            read_only: false,
        };
        assert!(is_shown(&fuse, Some(home)));
        let own = Mount {
            fs_type: "ext4".into(),
            mount_point: "/home".into(),
            ..fuse
        };
        assert!(
            !is_shown(&own, Some(home)),
            "/home on a partition of its own"
        );
    }

    #[test]
    fn a_gvfs_share_is_known_by_where_gnome_mounts_it() {
        let share = Path::new("/run/user/1000/gvfs/smb-share:server=nas,share=media");
        assert_eq!(gvfs_share(share), Some("smb-share"));
        assert_eq!(
            gvfs_share(Path::new("/run/user/1000/gvfs/sftp:host=server")),
            Some("sftp")
        );
        assert_eq!(
            gvfs_share(Path::new("/run/user/1000/gvfs/mtp:host=Phone")),
            None,
            "a phone isn't a drive FolderSkin can change"
        );
        assert_eq!(gvfs_share(&share.join("Photos")), None, "a folder in it");
        assert_eq!(
            gvfs_share(Path::new("/home/ana/gvfs/smb-share:server=nas")),
            None
        );
    }

    #[test]
    fn a_partition_is_on_its_disk() {
        for (part, disk) in [
            ("sdb1", "sdb"),
            ("sda", "sda"),
            ("sdaa12", "sdaa"),
            ("nvme0n1p2", "nvme0n1"),
            ("nvme0n1", "nvme0n1"),
            ("mmcblk0p1", "mmcblk0"),
            ("mmcblk0", "mmcblk0"),
            ("loop7", "loop7"),
            ("sr0", "sr0"),
            ("vdb3", "vdb"),
        ] {
            assert_eq!(disk_of(part), disk, "{part}");
        }
    }

    fn mounted(device: &str, at: &str, fs: &str) -> Mount {
        Mount {
            device: device.into(),
            mount_point: at.into(),
            fs_type: fs.into(),
            read_only: false,
        }
    }

    #[test]
    fn a_drive_is_told_apart_by_its_mount_and_its_disk() {
        let stick = mounted("/dev/sdb1", "/media/ana/Stick", "vfat");
        assert_eq!(
            kind(&mounted("/dev/nvme0n1p2", "/", "ext4"), None),
            DriveKind::Startup
        );
        assert_eq!(
            kind(
                &stick,
                Some(Disk {
                    removable: true,
                    usb: true,
                    spins: Some(true)
                })
            ),
            DriveKind::Removable
        );
        assert_eq!(
            kind(
                &stick,
                Some(Disk {
                    usb: true,
                    ..Disk::default()
                })
            ),
            DriveKind::External
        );
        assert_eq!(
            kind(
                &stick,
                Some(Disk {
                    spins: Some(false),
                    ..Disk::default()
                })
            ),
            DriveKind::SolidState
        );
        assert_eq!(
            kind(
                &stick,
                Some(Disk {
                    spins: Some(true),
                    ..Disk::default()
                })
            ),
            DriveKind::Internal
        );
        assert_eq!(kind(&stick, None), DriveKind::Internal);
        assert_eq!(
            kind(&mounted("/dev/sr0", "/media/ana/DVD", "iso9660"), None),
            DriveKind::Optical
        );
        assert_eq!(
            kind(&mounted("/dev/loop3", "/media/ana/Installer", "ext4"), None),
            DriveKind::DiskImage
        );
        assert_eq!(
            kind(&mounted("/dev/md0", "/mnt/raid", "ext4"), None),
            DriveKind::MultiDisk
        );
        assert_eq!(
            kind(
                &mounted("/dev/mmcblk0p1", "/media/ana/CAMERA", "exfat"),
                None
            ),
            DriveKind::Card
        );
        assert_eq!(
            kind(&mounted("//nas/media", "/mnt/media", "cifs"), None),
            DriveKind::Network
        );
        assert_eq!(
            kind(
                &mounted("https://dav.example.com/", "/mnt/dav", "fuse"),
                None
            ),
            DriveKind::Network
        );
    }
}
