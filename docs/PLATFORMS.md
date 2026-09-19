# Platforms

Every operating system has its own idea of what a "custom folder icon" is. FolderSkin
renders the same pixels everywhere (see [ARCHITECTURE.md](ARCHITECTURE.md)) and then hands
them to whichever mechanism the desktop actually reads. This page says exactly what is
written, what revert undoes, and where each mechanism falls short.

## What FolderSkin writes

| OS | apply | revert | files touched |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon(None, path)` then `setIcon(image, path)` with an `NSImage` carrying the 16–1024 px representations, then `noteFileSystemChanged` on the folder and the one around it | `setIcon(None)`, and the same notes | the invisible `Icon\r` file that macOS itself keeps inside the folder |
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
inside the folder. Finder is slow to notice a new one when it replaces another: it keeps drawing
the old icon, on the Desktop and in its windows, until the folder is opened. So the icon is
cleared first and then set, which Finder does redraw straight away (Apple's workaround,
developer.apple.com/forums/thread/788252), and Finder is told the folder and the folder around
it changed. A folder counts as having a custom icon, and offers **Remove custom icon**, when the
custom-icon flag is set in its Finder info (the `com.apple.FinderInfo` extended attribute),
whoever set it. On Windows and Linux only FolderSkin's own files count (and, on Linux, a GIO
custom icon), since those are all a revert takes off. Setting an icon needs write access
to the folder, so folders on read-only volumes and inside some sandboxed locations are
refused with the reason the OS gave.

`NSWorkspace.setIcon` works off the main thread, so the app calls it there and the window keeps
animating while a slow disk finishes the write. It isn't safe on two threads at once, though:
overlapping calls garble each other's `Icon\r` (a 37 KB icon came out as 286 bytes) or fail, so
every change of a folder's icon takes one lock for the whole process, and each runs in its own
autorelease pool so a long run over subfolders doesn't hold on to every folder's icon data.

The window is transparent over the system's sidebar material (`NSVisualEffectView`), which is
what makes the sidebar translucent; the light/dark switch sets the window's appearance so the
material follows it. Transparent windows need Tauri's `macOSPrivateApi`, which rules out the
Mac App Store; FolderSkin ships as a DMG. Windows and Linux keep an opaque window.

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

## A folder and its subfolders

With **Include subfolders** on, every folder in the tree gets exactly what a single apply
writes, as if each had been applied on its own: the `Icon\r` on macOS, `desktop.ini` and
`folderskin.ico` on Windows, `.directory` and `.folderskin.png` on Linux. Each folder keeps its
own copy, so the space adds up: on macOS a painted skin takes about 2.7 MB a folder, because
macOS stores the icon in its own, larger encoding, and the confirmation before a run over more
than ten folders gives the total. On Windows and Linux a copy is the icon file plus a disk block
for the text file.

The run leaves alone, along with everything inside them: symlinks and junctions, folders whose
names start with a dot, folders the OS hides (Finder's hidden flag; Windows' hidden or system
attribute), packages such as apps, photo and music libraries, Xcode projects and Keynote or
Pages documents, and the system locations listed under Known limits. At most 5,000 folders go in
one run. On a Mac a folder takes about a tenth of a second to skin and a thousandth to revert.

Reverting a run takes off exactly the folders it changed. Reverting the whole tree instead
(**Remove custom icons**, with the switch on) takes the custom icon off every folder that has one:
on macOS that is any custom icon, whoever set it, as with a single folder; on Windows and Linux
it is only FolderSkin's own files, as always. Cloud-synced trees sync every folder's copy.

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
