//! Windows folder icons: a `folderskin-<hash>.ico` beside a `desktop.ini` that points at it.
//!
//! Explorer reads `[.ShellClassInfo] IconResource=` out of a folder's `desktop.ini`. The file
//! is shared — other tools and Windows itself keep `InfoTip`, `LocalizedResourceName`,
//! `ConfirmFileOp` and whole `[ViewState]` sections in it — so the contents are produced by
//! [`desktop_ini_contents`] and [`desktop_ini_without_ours`], which edit only our own two
//! lines and leave every other key where it was. Those two functions are pure and compile on
//! every OS, so the logic that could eat a user's file is unit-tested everywhere rather than
//! only on Windows. So is [`prepare`], which packs the `.ico` once for any number of folders.
//!
//! The icon file is named after its own contents ([`ico_file_name`]). Explorer caches an icon
//! against the *path* it came from, so writing a second skin over a fixed `folderskin.ico` left
//! the folder wearing the first one until the cache was cleared or the machine restarted — the
//! "press F5" of the old platform note. A new picture is now a new path, which the shell has
//! nothing cached for, and re-applying the same skin resolves to the same name and so changes
//! nothing at all. [`is_our_ico_name`] is what keeps revert honest across both: it matches the
//! fixed name older versions wrote as well as the hashed ones.

use super::ApplyError;
use crate::compositor::IconSet;

/// The icon file FolderSkin wrote before the name carried a hash. Still recognised, so a folder
/// skinned by an older version reverts cleanly.
pub const ICO_NAME: &str = "folderskin.ico";
/// Start of the icon file FolderSkin writes now: `folderskin-<hash>.ico`.
const ICO_PREFIX: &str = "folderskin-";
const ICO_SUFFIX: &str = ".ico";
/// Hex characters of the content hash in the name. 16 is 64 bits: far past any chance of two
/// skins colliding, and short enough to stay readable in Explorer.
const ICO_HASH_HEX: usize = 16;
/// The Explorer-read ini FolderSkin writes into the folder.
pub const INI_NAME: &str = "desktop.ini";
/// Comment line that identifies the lines FolderSkin owns.
pub const MARKER: &str = "; managed by FolderSkin";
/// The ini section Explorer reads folder appearance from.
pub const SECTION: &str = "[.ShellClassInfo]";
/// Sizes packed into the `.ico`; Explorer picks per view, so all of them are shipped.
pub const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// The key inside [`SECTION`] that names the folder's icon.
const ICON_RESOURCE: &str = "IconResource";
/// The older pair Explorer prefers over [`ICON_RESOURCE`] when a folder carries both.
const ICON_FILE: &str = "IconFile";
const ICON_INDEX: &str = "IconIndex";

/// The name of the icon file holding `ico_bytes`: `folderskin-<16 hex of its SHA-256>.ico`.
///
/// Naming it after its contents is what makes a changed skin show up at once; see the module
/// note. The same bytes always give the same name, so re-applying a skin rewrites one identical
/// file rather than leaving a trail of them.
pub fn ico_file_name(ico_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(ico_bytes);
    let mut name = String::with_capacity(ICO_PREFIX.len() + ICO_HASH_HEX + ICO_SUFFIX.len());
    name.push_str(ICO_PREFIX);
    for byte in &digest[..ICO_HASH_HEX / 2] {
        use std::fmt::Write;
        let _ = write!(name, "{byte:02x}");
    }
    name.push_str(ICO_SUFFIX);
    name
}

/// True for a file name FolderSkin writes as a folder's icon: the hashed name, or the fixed one
/// older versions wrote.
///
/// Deliberately strict. Revert deletes what this matches, so it must not claim a file the user
/// named themselves: only `folderskin.ico` and `folderskin-` plus exactly [`ICO_HASH_HEX`]
/// lowercase hex digits qualify. Windows file names are case-insensitive, so the name is folded
/// before it is checked, but the hex must still be hex.
pub fn is_our_ico_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if name == ICO_NAME {
        return true;
    }
    let Some(rest) = name.strip_prefix(ICO_PREFIX) else {
        return false;
    };
    let Some(hash) = rest.strip_suffix(ICO_SUFFIX) else {
        return false;
    };
    hash.len() == ICO_HASH_HEX && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The `IconResource` line FolderSkin writes for the icon file called `ico_name`.
fn our_icon_line(ico_name: &str) -> String {
    format!("{ICON_RESOURCE}={ico_name},0")
}

/// The bytes of `folderskin.ico` for `icons`: every size in [`ICO_SIZES`] the set has.
pub fn prepare(icons: &IconSet) -> Result<Vec<u8>, ApplyError> {
    let entries: Vec<(u32, Vec<u8>)> = ICO_SIZES
        .iter()
        .filter_map(|&size| icons.png(size).map(|png| (size, png)))
        .collect();
    if entries.is_empty() {
        return Err(ApplyError::Platform(
            "the rendered icon has no size Windows can use".into(),
        ));
    }
    Ok(crate::ico::write_ico(&entries))
}

/// `desktop.ini` contents that point Explorer at the icon file called `ico_name`.
///
/// `existing` is the file already in the folder, if any; its keys and sections are preserved
/// and our marker plus `IconResource` are (re)written at the end of `[.ShellClassInfo]`.
/// Re-applying is idempotent: our old lines are dropped before the new ones go in. Line
/// endings are CRLF, which is what every other writer of this file uses.
pub fn desktop_ini_contents(existing: Option<&str>, ico_name: &str) -> String {
    let icon_line = our_icon_line(ico_name);

    let Some(existing) = existing else {
        return join_lines("", &[SECTION.to_string(), MARKER.to_string(), icon_line]);
    };

    let (bom, body) = split_bom(existing);
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut seen_section = false;
    let mut inserted = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if is_header(trimmed) {
            // Our lines belong at the end of [.ShellClassInfo], so they go in as the next
            // section starts.
            if in_section && !inserted {
                out.push(MARKER.to_string());
                out.push(icon_line.clone());
                inserted = true;
            }
            in_section = is_our_section(trimmed);
            seen_section |= in_section;
            out.push(line.to_string());
            continue;
        }

        // Drop our own marker wherever it sits, and any IconResource inside the section we
        // own — that key is exactly what we are replacing.
        if trimmed == MARKER {
            continue;
        }
        if in_section
            && key_of(trimmed).is_some_and(|k| {
                k.eq_ignore_ascii_case(ICON_RESOURCE)
                    || k.eq_ignore_ascii_case(ICON_FILE)
                    || k.eq_ignore_ascii_case(ICON_INDEX)
            })
        {
            // Explorer prefers the legacy IconFile/IconIndex pair over IconResource, so an old
            // pair would silently win over ours.
            continue;
        }
        out.push(line.to_string());
    }

    if in_section && !inserted {
        out.push(MARKER.to_string());
        out.push(icon_line.clone());
        inserted = true;
    }
    if !seen_section {
        // No [.ShellClassInfo] at all: ours leads the file, where Explorer expects it.
        let mut head = vec![SECTION.to_string(), MARKER.to_string(), icon_line];
        head.append(&mut out);
        out = head;
        inserted = true;
    }
    debug_assert!(inserted, "our lines must end up somewhere");

    join_lines(bom, &out)
}

/// `existing` with FolderSkin's lines removed, or `None` when the file should be deleted.
///
/// Only our marker and an `IconResource` that names our own `folderskin.ico` are removed, so a
/// user who later pointed the key at their own icon keeps it. `None` is returned when nothing
/// but section headers would be left — there is no reason to keep an ini with no keys in it.
pub fn desktop_ini_without_ours(existing: &str) -> Option<String> {
    let (bom, body) = split_bom(existing);
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if is_header(trimmed) {
            in_section = is_our_section(trimmed);
            out.push(line.to_string());
            continue;
        }
        if trimmed == MARKER {
            continue;
        }
        if in_section && is_our_icon_resource(trimmed) {
            continue;
        }
        out.push(line.to_string());
    }

    let empty = out
        .iter()
        .all(|line| line.trim().is_empty() || is_header(line.trim()));
    if empty {
        return None;
    }
    Some(join_lines(bom, &out))
}

/// True when `contents` still carries FolderSkin's marker.
pub fn is_ours(contents: &str) -> bool {
    contents.lines().any(|line| line.trim() == MARKER)
}

/// True when a revert would take something off: FolderSkin's lines in `desktop.ini` (its text,
/// if the folder has one) or its icon file. Someone else's `IconResource` doesn't count, since a
/// revert leaves it alone.
pub fn would_revert(desktop_ini: Option<&str>, ico_exists: bool) -> bool {
    ico_exists
        || desktop_ini.is_some_and(|ini| desktop_ini_without_ours(ini).as_deref() != Some(ini))
}

/// The icon file a folder's `desktop.ini` points Explorer at, and which icon in it, as the
/// values were written: `("folderskin-1a2b….ico", 0)`, `("%SystemRoot%\\system32\\imageres.dll",
/// 3)`, `("..\\shared\\theirs.ico", 0)`.
///
/// Explorer takes the older `IconFile`/`IconIndex` pair over `IconResource` when a folder carries
/// both, which is why [`desktop_ini_contents`] drops the pair when it writes ours — so this reads
/// it the same way round, and answers with the icon actually on screen. Only
/// `[.ShellClassInfo]` is looked at; a key of the same name in another section means something
/// else. Pure, so the parsing is unit-tested on every OS; resolving and drawing what it names is
/// the caller's job (`src-tauri/src/folder_icon.rs`).
pub fn icon_resource_of(desktop_ini: &str) -> Option<(String, i32)> {
    let (_, body) = split_bom(desktop_ini);
    let mut in_section = false;
    let (mut resource, mut file, mut index) = (None, None, None);

    for line in body.lines() {
        let trimmed = line.trim();
        if is_header(trimmed) {
            in_section = is_our_section(trimmed);
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key.eq_ignore_ascii_case(ICON_RESOURCE) {
            resource = Some(value);
        } else if key.eq_ignore_ascii_case(ICON_FILE) {
            file = Some(value);
        } else if key.eq_ignore_ascii_case(ICON_INDEX) {
            index = value.parse::<i32>().ok();
        }
    }

    // `IconFile` names the file alone; `IconResource` carries the index after a comma.
    if let Some(file) = file.filter(|f| !f.is_empty()) {
        return Some((file.to_string(), index.unwrap_or(0)));
    }
    let resource = resource.filter(|r| !r.is_empty())?;
    match resource.rsplit_once(',') {
        Some((path, idx)) if !path.is_empty() => {
            Some((path.trim().to_string(), idx.trim().parse().unwrap_or(0)))
        }
        _ => Some((resource.to_string(), 0)),
    }
}

/// True for a `[Section]` header line.
fn is_header(trimmed: &str) -> bool {
    trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2
}

/// True for the `[.ShellClassInfo]` header, whatever its casing.
fn is_our_section(trimmed: &str) -> bool {
    trimmed.eq_ignore_ascii_case(SECTION)
}

/// The key of a `key=value` line, trimmed; `None` for anything else.
fn key_of(trimmed: &str) -> Option<&str> {
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

/// True for an `IconResource` line whose value names an icon file of ours.
fn is_our_icon_resource(trimmed: &str) -> bool {
    let Some((key, value)) = trimmed.split_once('=') else {
        return false;
    };
    if !key.trim().eq_ignore_ascii_case(ICON_RESOURCE) {
        return false;
    }
    let file = value.split(',').next().unwrap_or("").trim();
    is_our_ico_name(file)
}

/// Splits a leading UTF-8 byte-order mark off `contents` so it can be put back verbatim.
///
/// Dropping it would change how Windows decodes any non-ASCII key the user already had.
fn split_bom(contents: &str) -> (&str, &str) {
    match contents.strip_prefix('\u{feff}') {
        Some(body) => ("\u{feff}", body),
        None => ("", contents),
    }
}

/// Joins `lines` with CRLF, including a trailing one, after `bom`.
fn join_lines(bom: &str, lines: &[String]) -> String {
    let mut out = String::from(bom);
    for line in lines {
        out.push_str(line);
        out.push_str("\r\n");
    }
    out
}

#[cfg(windows)]
pub use imp::{apply, has_custom_icon, revert};

#[cfg(windows)]
mod imp {
    use super::{
        desktop_ini_contents, desktop_ini_without_ours, ico_file_name, is_our_ico_name,
        would_revert, INI_NAME,
    };
    use crate::apply::paths::{read_text_if_present, write_atomic};
    use crate::apply::ApplyError;
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Storage::FileSystem::{
        SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_SYSTEM,
    };
    use windows_sys::Win32::UI::Shell::{
        SHChangeNotify, SHCNE_ATTRIBUTES, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_FLUSH,
        SHCNF_PATHW,
    };

    /// Writes the icon file (the [`prepare`](super::prepare)d bytes, under the name its contents
    /// give it) and a `desktop.ini` that points at it, then tells Explorer.
    ///
    /// Any icon file an earlier apply left behind goes once the new one is in place and the ini
    /// names it, so a folder never keeps more than the one it wears and the ini never points at
    /// a file that isn't there.
    pub fn apply(folder: &Path, ico_bytes: &[u8]) -> Result<(), ApplyError> {
        let name = ico_file_name(ico_bytes);
        let ico = folder.join(&name);
        let ini = folder.join(INI_NAME);

        // Explorer only honours desktop.ini in a read-only folder, but that bit — and the
        // hidden+system bits on the files themselves — make replacing our own files from an
        // earlier apply fail, so everything is cleared first and set again at the end.
        set_readonly(folder, false)?;
        clear_attributes(&ico);
        clear_attributes(&ini);

        write_atomic(&ico, ico_bytes)?;
        let existing = read_text_if_present(&ini)?;
        write_atomic(
            &ini,
            desktop_ini_contents(existing.as_deref(), &name).as_bytes(),
        )?;

        for stale in our_ico_files(folder) {
            if stale != ico {
                clear_attributes(&stale);
                let _ = std::fs::remove_file(&stale);
            }
        }

        hide(&ico)?;
        hide(&ini)?;
        set_readonly(folder, true)?;
        notify(folder);
        Ok(())
    }

    /// True when the folder wears FolderSkin's icon, which `revert` would take off.
    pub fn has_custom_icon(folder: &Path) -> bool {
        let ini = read_text_if_present(&folder.join(INI_NAME)).ok().flatten();
        would_revert(ini.as_deref(), !our_ico_files(folder).is_empty())
    }

    /// Removes our ini lines and icon files, leaving anything else in the folder alone.
    pub fn revert(folder: &Path) -> Result<(), ApplyError> {
        let mut touched = false;

        let ini = folder.join(INI_NAME);
        if let Some(existing) = read_text_if_present(&ini)? {
            match desktop_ini_without_ours(&existing) {
                None => {
                    clear_attributes(&ini);
                    std::fs::remove_file(&ini)?;
                    touched = true;
                }
                Some(left) if left != existing => {
                    clear_attributes(&ini);
                    write_atomic(&ini, left.as_bytes())?;
                    hide(&ini)?;
                    touched = true;
                }
                // Nothing of ours in it; leave the file and its attributes untouched.
                Some(_) => {}
            }
        }

        for ico in our_ico_files(folder) {
            clear_attributes(&ico);
            std::fs::remove_file(&ico)?;
            touched = true;
        }

        if !touched {
            // Never skinned by us: change nothing, not even the read-only attribute.
            return Ok(());
        }
        set_readonly(folder, false)?;
        notify(folder);
        Ok(())
    }

    /// Every icon file in `folder` that FolderSkin wrote — the hashed name it writes now and the
    /// fixed one older versions wrote. Empty when the folder can't be read, which is the same
    /// answer as "none of ours", and the caller is about to fail on the folder anyway.
    fn our_ico_files(folder: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|e| is_our_ico_name(&e.file_name().to_string_lossy()))
            .map(|e| e.path())
            .collect()
    }

    /// A NUL-terminated UTF-16 copy of `s`, as every `*W` entry point wants.
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Marks `path` hidden and system, the way Windows marks its own `desktop.ini`.
    fn hide(path: &Path) -> Result<(), ApplyError> {
        set_attributes(path, FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM)
    }

    /// Best-effort reset of `path`'s attributes so it can be replaced or deleted.
    fn clear_attributes(path: &Path) {
        if path.exists() {
            let _ = set_attributes(path, FILE_ATTRIBUTE_NORMAL);
        }
    }

    fn set_attributes(path: &Path, attributes: u32) -> Result<(), ApplyError> {
        let path_w = wide(path.as_os_str());
        // SAFETY: `path_w` is a NUL-terminated UTF-16 buffer that outlives the call.
        let ok = unsafe { SetFileAttributesW(path_w.as_ptr(), attributes) };
        if ok == 0 {
            return Err(ApplyError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    /// Sets or clears the folder's read-only attribute, which is what makes Explorer look at
    /// `desktop.ini` in the first place.
    fn set_readonly(folder: &Path, readonly: bool) -> Result<(), ApplyError> {
        let mut permissions = std::fs::metadata(folder)?.permissions();
        permissions.set_readonly(readonly);
        std::fs::set_permissions(folder, permissions)?;
        Ok(())
    }

    /// Tells the shell the folder's icon changed, so open windows repaint without an F5.
    ///
    /// The one that matters is the last: **the view that draws a folder's icon is the view
    /// listing it, not a window showing what is inside it.** A folder on the Desktop is drawn by
    /// the Desktop; a folder in Documents is drawn by the Documents window. Telling the shell
    /// only about the folder itself, as this used to, left that view holding the icon it had
    /// already drawn — the app wrote everything correctly, `SHGetFileInfo` resolved the new icon,
    /// and the folder on screen did not change until it was refreshed by hand. That is the
    /// "press F5" this app used to tell people about.
    ///
    /// The other two are the rest of what actually changed: applying sets the folder's read-only
    /// attribute, which is the bit that makes Explorer read `desktop.ini` at all, and the folder
    /// as an item now looks different.
    fn notify(folder: &Path) {
        notify_path(folder, SHCNE_ATTRIBUTES);
        notify_path(folder, SHCNE_UPDATEITEM);
        // Also the folder itself: its contents really did change, for a window that has it open.
        notify_path(folder, SHCNE_UPDATEDIR);
        if let Some(parent) = folder.parent() {
            notify_path(parent, SHCNE_UPDATEDIR);
        }
    }

    /// One `SHChangeNotify` about `path`.
    fn notify_path(path: &Path, event: u32) {
        let path_w = wide(path.as_os_str());
        // SAFETY: SHCNF_PATHW promises the first item is a wide path, which `path_w` is and which
        // outlives the call; the second is unused for every event used here.
        unsafe {
            SHChangeNotify(
                event as i32,
                SHCNF_PATHW | SHCNF_FLUSH,
                path_w.as_ptr().cast::<c_void>(),
                std::ptr::null(),
            );
        }
    }
}
