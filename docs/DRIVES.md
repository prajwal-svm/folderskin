# Drives

A drive is a volume you pick in FolderSkin instead of a folder: the startup disk, an external disk,
a USB stick, a memory card, a disc, a disk image or a network share. Each system draws drives in
its own shapes, and none of them look like folders, so FolderSkin gives a drive a drive-shaped icon.
This page is what each system shows and when, how FolderSkin tells what kind of drive was picked,
the drive shapes FolderSkin draws, and exactly how it sets and takes off a drive's icon on each
system.

## What each system shows

### macOS

Finder asks the storage driver for a volume's icon. The driver that publishes the disk names an
icon file in its own bundle (`IOMediaIcon` in the I/O Registry, for example `Internal.icns` from
IOStorageFamily), and Finder draws that unless the volume has a custom icon of its own.

| kind | what Finder draws | when |
|---|---|---|
| Startup disk | an upright silver disk with the Apple logo pressed into it | the volume at `/` |
| Internal disk | the same silver disk | a disk inside the Mac (`Internal`) |
| External disk | an amber disk with a cable plug on it | a disk that is neither internal nor removable media |
| Removable disk | a white disk with a down arrow | removable media, such as most USB sticks |
| SD card | an SD card | a card in the Mac's own card reader (`Secure Digital`) |
| CD, DVD or Blu-ray | a disc, marked DVD or BD for those | a disc in an optical drive |
| Disk image | the disk image icon | a mounted `.dmg` (`Disk Image`) |
| Network share | a cyan disk with a globe | an SMB, AFP, NFS or WebDAV share |
| Time Machine disk | the Time Machine disk | a disk Time Machine backs up to |

All the disks share one shape: an upright box with rounded top corners and a strip along its
bottom with a light in it. Colour and the mark pressed into the front say which kind it is.

### Windows 11

Explorer draws a drive by what `GetDriveType` says it is, and by how the disk is connected.

| kind | what Explorer draws | when |
|---|---|---|
| System drive | a grey drive seen from above, with the Windows logo on top | the drive Windows runs from, usually `C:` |
| Local disk | the same grey drive, without the logo | every other fixed disk, internal or external |
| USB drive | the drive with a connector at its side | removable media such as a USB stick |
| SD card | an SD card | a card in a card reader |
| CD, DVD or Blu-ray drive | the drive with a disc on it | an optical drive, and a mounted ISO |
| Network drive | the drive over a green network pipe | a network share mapped to a drive letter |

### Linux

GNOME Files and KDE Dolphin draw a mounted volume with a freedesktop icon name from the icon theme
(Adwaita and Breeze), picked by udisks from the drive's properties: how it is connected, whether
its media can be removed, what media it is and whether it spins.

| icon name | what it is | when |
|---|---|---|
| `drive-harddisk` | a hard disk | a fixed disk that spins |
| `drive-harddisk-solidstate` | a solid-state disk | a fixed disk with a rotation rate of 0 |
| `drive-harddisk-usb` | a hard disk on a USB cable | a fixed disk connected by USB |
| `drive-removable-media` | a USB stick | a drive whose media can be removed |
| `media-flash` | a memory card | SD, MMC and other flash cards |
| `drive-optical` | an optical drive | the drive itself |
| `media-optical` | a disc | a disc in it, which is what a mounted disc shows |
| `network-server` | a server | a server in the network view |
| `folder-remote` | a folder with a network mark | a mounted network share |
| `drive-multidisk` | a stack of disks | a RAID set |

## The kinds FolderSkin knows

FolderSkin puts every volume into one of these kinds. The stage says which one was picked
("External drive", "Network drive"), and the kind decides which drive shape a skin goes on.

| kind | id | on macOS | on Windows | on Linux |
|---|---|---|---|---|
| Startup disk | `startup` | the volume at `/` | the system drive | the volume at `/` |
| Internal drive | `internal` | an internal disk | a fixed disk | a fixed disk that spins |
| Solid-state drive | `solid-state` | drawn as internal | drawn as a local disk | a fixed disk that doesn't spin |
| External drive | `external` | a disk that isn't internal or removable | drawn as a local disk | a fixed disk on USB |
| USB drive | `removable` | removable media | removable media | removable media |
| Memory card | `card` | a card in the card reader | a card on the SD or MMC bus | an `mmcblk` device |
| Disc | `optical` | a disc | an optical drive | a disc |
| Disk image | `disk-image` | a mounted disk image | drawn as a local disk | a loop device |
| Network drive | `network` | a network share | a mapped network drive | a network file system |
| Time Machine disk | `time-machine` | a Time Machine backup disk | drawn as a local disk | drawn as an external drive |
| RAID set | `multi-disk` | drawn as an external drive | drawn as a local disk | an `md` device |

Two more shapes are only for designing: Linux's optical drive (`optical-drive`) and network server
(`server`).

## How FolderSkin tells what was picked

A drive is the root of a volume, its mount point. A folder inside a drive is still a folder. The
rules below are in `crates/folderskin-core/src/drive/detect/`: what each system says is turned into
a kind by a pure function, tested on any computer, and only the asking is done on the system
itself.

### On macOS

1. `statfs` names the mount point of the file system a path is on. The path is a drive when it is
   that mount point, and the mount isn't one macOS hides (`nobrowse`, as for
   `/System/Volumes/Data`).
2. `/` is the startup disk.
3. The file system type says a network share (`smbfs`, `afpfs`, `nfs`, `webdav`, `ftp`, or any
   mount that isn't local) and a disc (`cd9660`, `cddafs`, `udf`).
4. For a local disk, `diskutil info -plist` says the rest: a `BusProtocol` of `Disk Image` is a
   disk image and `Secure Digital` a memory card (as is a card reader, by its `MediaName`),
   `OpticalMediaType` is a disc, `Internal` is an internal drive, `RemovableMedia` a USB drive, and
   anything else an external drive.
5. A disk with `Backups.backupdb` at its root, or one Time Machine backs up to
   (`tmutil destinationinfo`), is a Time Machine disk.
6. Its name is the one Finder shows (`Macintosh HD` for `/`).

### On Windows

1. A drive is a drive letter's root, such as `D:\`. A network share by its `\\server\share` path
   has no letter, and Windows gives it no icon of its own, so it stays a folder. Its name is its
   label and its letter, `Backup (E:)`, or what kind of drive it is when it has no label,
   `USB drive (F:)`, as Explorer names it.
2. `GetDriveType` says a network drive (`DRIVE_REMOTE`), a disc drive (`DRIVE_CDROM`) and
   removable media (`DRIVE_REMOVABLE`).
3. A fixed disk is the startup disk when its letter is `%SystemDrive%`. Otherwise its bus
   (`IOCTL_STORAGE_QUERY_PROPERTY`) says the rest: USB is an external drive, SD and MMC a memory
   card, a file-backed virtual disk a disk image, Storage Spaces and RAID a RAID set, and anything
   else an internal drive.

### On Linux

1. `/proc/self/mounts` lists every mount point, but a drive is one the file manager shows as a
   drive: `/`, the startup disk, anything mounted under `/media/`, `/run/media/`, `/mnt/` or the
   home folder, and a network share anywhere. `/boot`, `/home` on a partition of its own and the
   kernel's own file systems stay folders. GNOME mounts a share through GVfs, as a folder under
   `/run/user/<uid>/gvfs/`, and that folder is a network drive too.
2. The file system type says a network drive (`nfs`, `nfs4`, `cifs`, `smb3`, `sshfs`, `davfs` and
   other network file systems) and a disc (`iso9660`, `udf`).
3. The device says the rest. `/dev/loop*` is a disk image, `/dev/md*` a RAID set and
   `/dev/mmcblk*` a memory card. For a disk such as `/dev/sdb1` or `/dev/nvme0n1p1`, sysfs says
   whether its media is `removable` (a USB drive), whether it is connected through USB (an
   external drive) and whether it spins (`queue/rotational`: a solid-state drive when it doesn't).

## FolderSkin's drive shapes

FolderSkin draws its own drive shapes, in code, the way it draws its folders
(`crates/folderskin-core/src/drive/`). They are in the spirit of each system's own drives, not
copies of Apple's, Microsoft's or GNOME's artwork, and they carry no system's logo. A logo can
still go on a drive from the composer's logo library.

| system | shapes |
|---|---|
| Mac | Startup disk, Internal drive, External drive, USB drive, Memory card, Disc, Disk image, Network drive, Time Machine disk |
| Windows | System drive, Local disk, USB drive, SD card, Disc drive, Network drive |
| Linux | Hard disk, Solid-state drive, USB hard disk, USB stick, Memory card, Optical drive, Disc, Network server, Network folder, RAID set |

Every shape is drawn on the same 1024-unit canvas as the folders and has a face: the part a
picture covers and where a design's layers sit. The rest of the drive (its sides, its base, its
connector, the hole in a disc) is drawn over and around the face, so a skinned drive still reads
as its kind.

- **Artwork** (a photo, a painting) is wrapped onto the drive: cover-fitted to the face and cut to
  it, on the shape for the system FolderSkin runs on and the kind of drive picked.
- **A finished icon**, a drive or a folder drawn whole, is used as drawn.
- **A design** from the composer is drawn on the shape it was made on, and saved as a drive skin.

The library puts drive skins first while a drive is picked, and after the folder skins while a
folder is. Every skin still goes on either.

## What the stage shows for a drive

The stage names the drive as its system does and says what kind it is, "External drive ·
/Volumes/Backup Disk", and shows its icon as it is now: its own, or the plain drive of its kind.
While it's picked, every card in the library shows its skin on that drive. A card's picture is
drawn the first time it comes into view, a few at a time, served to the webview under the
`fsdrive:` scheme (`src-tauri/src/drive_thumbs.rs`), and kept beside the skin as
`<id>.thumb-v3-drive-<shape>.png`, so it's drawn once. The plain drive stands in for it until it
arrives.

A drive whose icon can't be changed (the startup disk on macOS and Linux, a drive mounted
read-only on either) can still have skins tried on it. Its Apply button is off, and the line under
it says why. It offers no run over its folders either.

## How FolderSkin sets a drive's icon

The writers are in `crates/folderskin-core/src/apply/drive/`. `apply_icon`, `revert_icon` and
`has_custom_icon` hand a drive's root to them, so every caller that takes a folder takes a drive.

### macOS

| | |
|---|---|
| apply | `NSWorkspace.setIcon(image, forFile: <mount point>)`, the call Finder's own Get Info uses. For a volume it writes the icon as `.VolumeIcon.icns` at the root and sets the custom-icon flag on the root. Finder is told the volume changed |
| revert | `setIcon(nil, …)`, which takes both away again |
| refused | the startup disk (macOS keeps it sealed and read-only) and a read-only volume or disk image, before anything is tried, and a share whose server doesn't let FolderSkin write, each with a sentence that says so |

A test attaches a disk image made in the temp folder, applies an icon to it, checks for
`.VolumeIcon.icns` and the flag, reverts it and detaches it (`cargo test -p folderskin-core --
--ignored drive_image`), and one attached read-only is refused.

### Windows

| | |
|---|---|
| apply | writes the icon as `folderskin-drive-<letter>-<hash>.ico` in FolderSkin's own data folder (`%APPDATA%\app.folderskin.desktop\drive-icons\`), and points `HKEY_CURRENT_USER\Software\Classes\Applications\Explorer.exe\Drives\<letter>\DefaultIcon` at it. The letter's older icon files go. Explorer is told to draw icons again |
| revert | only when the key names FolderSkin's icon file: puts back the icon the key named before, or deletes that `DefaultIcon` key, and the letter's key when nothing else is in it, and then the letter's icon files |
| the drive's own | a `DefaultIcon` that was there before is kept aside in the same key, as `FolderSkinBefore`, and put back by revert |

The name carries a hash of the icon, as a folder's does, because Explorer caches an icon against
the path it came from: a new skin is a new path, so it shows at once. How the key is kept and given
back is pure logic over a stand-in registry, tested on any computer. Only the registry calls
themselves are Windows'.

The key is per user and needs no administrator. It works for fixed disks, removable drives and
mapped network drives alike, because Explorer looks it up by the drive letter. That is also its
limit: the icon belongs to the letter, not the disk. A different USB stick that gets the same
letter shows the same icon, and a drive that comes back under another letter shows its own. A
network share with no letter can't have an icon of its own at all.

FolderSkin doesn't write an `autorun.inf` to the drive, the other way Windows reads a drive icon.
Windows ignores it for network drives, and virus scanners are right to distrust a program that
writes one.

### Linux

| | |
|---|---|
| apply | writes `.folderskin.png` (512 px) at the drive's root, a `.directory` pointing at it for Dolphin, a `.xdg-volume-info` with `IconFile=.folderskin.png` for GNOME's volume list, and sets `metadata::custom-icon` on the mount point with `gio` for Nautilus, Nemo and Caja |
| revert | removes FolderSkin's lines from `.directory` and `.xdg-volume-info` (deleting each when nothing else is left in it), deletes `.folderskin.png`, and unsets the GIO attribute |
| refused | `/`, and a volume mounted read-only, before anything is tried |

GNOME reads `.xdg-volume-info` as it mounts a drive, so its sidebar shows the new icon the next time
the drive is mounted. A name already in `.xdg-volume-info` (`Name=`) stays. KDE draws the drive with
its device icon in the Places panel and with the `.directory` icon wherever it shows the mount point
as a folder.

## Include subfolders on a drive

With **Include subfolders** on, the drive gets its drive icon and every folder on it gets the skin
as a folder icon, the way a folder and its subfolders do. The drive's own icon goes on first, and
then the run goes through the folders on it, leaving the drive itself be, since its icon is drawn
for its shape (`apply_prepared`). Reverting, or removing custom icons, takes the drive's icon off
too.

A drive whose icon can't be changed offers no run. Neither do Windows' system drive, whose folders
are Windows' own and the user's home, and a mapped network drive, which Windows resolves to the
share's own path.

## Sources

- Apple: [NSWorkspace setIcon(_:forFile:options:)](https://developer.apple.com/documentation/appkit/nsworkspace/1529882-seticon),
  and `IOMediaIcon` as `ioreg -c IOMedia -l` shows it.
- Microsoft: [Assign a custom icon and label to a drive letter](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-assign-a-custom-icon-and-label-to-a-drive-letter),
  [GetDriveTypeW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew),
  [STORAGE_BUS_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ne-winioctl-storage_bus_type),
  and [How to set a custom drive icon in Windows](https://woshub.com/set-custom-drive-icon-windows/)
  for the per-user key.
- GNOME: [gvfsmountinfo.c](https://github.com/GNOME/gvfs/blob/master/common/gvfsmountinfo.c), which
  reads `.xdg-volume-info` and `autorun.inf`, and
  [udisksclient.c](https://github.com/storaged-project/udisks/blob/master/udisks/udisksclient.c),
  which picks the icon names.
- freedesktop.org: the [Icon Naming Specification](https://specifications.freedesktop.org/icon-naming-spec/latest/).
