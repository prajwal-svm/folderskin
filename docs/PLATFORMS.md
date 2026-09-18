# Platforms

Every operating system has its own idea of what a "custom folder icon" is. FolderSkin
renders the same pixels everywhere (see [ARCHITECTURE.md](ARCHITECTURE.md)) and then hands
them to whichever mechanism the desktop actually reads. This page says exactly what is
written, what revert undoes, and where each mechanism falls short.

## What FolderSkin writes

| OS | apply | revert | files touched |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon(image, path)` with an `NSImage` carrying the 16–1024 px representations | `setIcon(None)` | the invisible `Icon\r` file that macOS itself keeps inside the folder |
| Windows | writes `folderskin.ico` and a `desktop.ini` with `[.ShellClassInfo]` / `IconResource=folderskin.ico,0`, marks both hidden + system, sets the folder's read-only attribute, then calls `SHChangeNotify` | removes FolderSkin's lines from `desktop.ini` (and deletes the file if FolderSkin created it), deletes `folderskin.ico`, clears read-only | `desktop.ini`, `folderskin.ico` |
| Linux | writes `.folderskin.png` (512 px) and a `.directory` with `[Desktop Entry]` / `Icon=/abs/path/.folderskin.png`, and runs `gio set <folder> metadata::custom-icon file://…` when `gio` is installed | deletes both files when they are FolderSkin's, and runs `gio set -t unset` on the metadata | `.directory`, `.folderskin.png` |

All three writers validate that the path is an existing directory, refuse filesystem
roots, and write atomically (temp file, then rename), so a failure never leaves a folder
half-skinned.

### The marker lines

On Windows and Linux the icon lives in a text file that the user may also own, so
FolderSkin marks its own work and never deletes anything else:

```ini
[.ShellClassInfo]
; managed by FolderSkin
IconResource=folderskin.ico,0
```

```ini
[Desktop Entry]
Icon=/home/you/Pictures/.folderskin.png
# managed by FolderSkin
```

On Windows the legacy `IconFile` and `IconIndex` keys are dropped from `[.ShellClassInfo]`
when FolderSkin writes its own icon, because Explorer prefers that pair over `IconResource`
and the folder would otherwise keep its old icon.

Revert parses the file, drops FolderSkin's marker and the `IconResource` or `Icon` line it
owns, and keeps every other key. If nothing else is left and FolderSkin created the file,
the file goes too. Revert is idempotent: running it on a folder that was never skinned
succeeds and changes nothing.

## Per-platform notes

### macOS

The icon is stored by the system, not by us — macOS writes an invisible `Icon\r` file
inside the folder and Finder picks it up immediately. Setting an icon needs write access
to the folder, so folders on read-only volumes and inside some sandboxed locations are
refused with the reason the OS gave.

The `setIcon` call must run on the main thread; the app marshals it there and awaits the
result, which is why apply can take a moment on a slow disk.

### Windows

Explorer only reads `desktop.ini` for folders that carry the read-only attribute. That
attribute is the mechanism, not a mistake: the folder's contents stay writable and revert
clears it again.

`SHChangeNotify` asks Explorer to refresh, but Explorer keeps its own icon cache and
sometimes ignores the hint. If the folder still shows the old icon, press `F5` in the
window, or close and reopen it. A machine-wide cache rebuild (`ie4uinit.exe -show`) is
never needed for a folder we just wrote.

`desktop.ini` and `folderskin.ico` are hidden + system, so they do not show up unless
"Show hidden files" and "Hide protected operating system files" are both switched.

### Linux

There is no single standard, so FolderSkin writes both mechanisms:

| file manager | reads |
|---|---|
| Dolphin, Konqueror (KDE) | `.directory` |
| Nautilus (GNOME), Nemo (Cinnamon), Caja (MATE) | GIO metadata, set through `gio` |
| Thunar (XFCE) | its own per-folder metadata; not covered — set the icon from its properties dialog |
| PCManFM (LXDE/LXQt) | `.directory` in most builds |

`gio` ships with GLib and is present on nearly every desktop install. When it is missing,
FolderSkin still writes `.directory` and `.folderskin.png` and the GNOME family keeps the
default icon until `gio` is available.

`Icon=` must be an absolute path, so a skinned folder that is moved or renamed loses its
icon. Apply again after moving it.

## Known limits

- **Explorer's icon cache.** Windows may keep showing the previous icon until the folder
  view is refreshed with `F5`.
- **Cloud-synced folders.** iCloud Drive, OneDrive, Dropbox and Google Drive see
  `desktop.ini`, `folderskin.ico`, `.directory`, `.folderskin.png` and `Icon\r` as ordinary
  files and will sync them to your other machines, where a different OS ignores them. They
  are small, but they are visible in the sync history. OneDrive's "Files On-Demand" can
  also block the read-only attribute Windows needs.
- **File managers that read neither mechanism.** Most tiling and minimal managers (ranger,
  lf, nnn, and Thunar's own metadata store) show the default folder icon whatever is on
  disk. FolderSkin still writes the files, so the icon appears if you later switch to a
  manager that honours them.
- **Network and read-only volumes.** SMB, NFS and read-only mounts often refuse the
  attribute or the hidden flag; the failure surfaces in the drop zone with the reason from
  the OS.
- **Folders inside an app bundle or a system directory.** Refused before anything is written:
  `/System`, `/Library`, `/usr`, `/bin`, `/sbin`, `/etc`, `/var`, anything inside a `.app`
  bundle, and on Windows the `Windows`, `Program Files` and `ProgramData` trees. Drive roots
  and your home folder itself are refused too.

## Reverting by hand

If you uninstall FolderSkin before reverting, the icon is easy to remove yourself:

- macOS: select the folder, `Cmd+I`, click the icon at the top of the info window, press
  `Backspace`.
- Windows: delete `desktop.ini` and `folderskin.ico` from inside the folder, then clear the
  folder's read-only attribute (`attrib -r <folder>`).
- Linux: delete `.directory` and `.folderskin.png`, then run
  `gio set -t unset <folder> metadata::custom-icon`.
