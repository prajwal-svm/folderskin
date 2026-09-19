//! Windows folder icons: a `folderskin.ico` beside a `desktop.ini` that points at it.
//!
//! Explorer reads `[.ShellClassInfo] IconResource=` out of a folder's `desktop.ini`. The file
//! is shared — other tools and Windows itself keep `InfoTip`, `LocalizedResourceName`,
//! `ConfirmFileOp` and whole `[ViewState]` sections in it — so the contents are produced by
//! [`desktop_ini_contents`] and [`desktop_ini_without_ours`], which edit only our own two
//! lines and leave every other key where it was. Those two functions are pure and compile on
//! every OS, so the logic that could eat a user's file is unit-tested everywhere rather than
//! only on Windows.

/// The icon file FolderSkin writes into the folder.
pub const ICO_NAME: &str = "folderskin.ico";
/// The Explorer-read ini FolderSkin writes into the folder.
pub const INI_NAME: &str = "desktop.ini";
/// Comment line that identifies the lines FolderSkin owns.
pub const MARKER: &str = "; managed by FolderSkin";
/// The ini section Explorer reads folder appearance from.
pub const SECTION: &str = "[.ShellClassInfo]";
/// Sizes packed into `folderskin.ico`; Explorer picks per view, so all of them are shipped.
pub const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// The key inside [`SECTION`] that names the folder's icon.
const ICON_RESOURCE: &str = "IconResource";

/// The `IconResource` line FolderSkin writes.
fn our_icon_line() -> String {
    format!("{ICON_RESOURCE}={ICO_NAME},0")
}

/// `desktop.ini` contents that point Explorer at `folderskin.ico`.
///
/// `existing` is the file already in the folder, if any; its keys and sections are preserved
/// and our marker plus `IconResource` are (re)written at the end of `[.ShellClassInfo]`.
/// Re-applying is idempotent: our old lines are dropped before the new ones go in. Line
/// endings are CRLF, which is what every other writer of this file uses.
pub fn desktop_ini_contents(existing: Option<&str>) -> String {
    let icon_line = our_icon_line();

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
                    || k.eq_ignore_ascii_case("IconFile")
                    || k.eq_ignore_ascii_case("IconIndex")
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

/// True for an `IconResource` line whose value names our own icon file.
fn is_our_icon_resource(trimmed: &str) -> bool {
    let Some((key, value)) = trimmed.split_once('=') else {
        return false;
    };
    if !key.trim().eq_ignore_ascii_case(ICON_RESOURCE) {
        return false;
    }
    let file = value.split(',').next().unwrap_or("").trim();
    file.eq_ignore_ascii_case(ICO_NAME)
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
        desktop_ini_contents, desktop_ini_without_ours, would_revert, ICO_NAME, ICO_SIZES, INI_NAME,
    };
    use crate::apply::paths::{read_text_if_present, write_atomic};
    use crate::apply::ApplyError;
    use crate::compositor::IconSet;
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Storage::FileSystem::{
        SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_SYSTEM,
    };
    use windows_sys::Win32::UI::Shell::{
        SHChangeNotify, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_FLUSH, SHCNF_PATHW,
    };

    /// Writes `folderskin.ico` and a `desktop.ini` that points at it, then tells Explorer.
    pub fn apply(folder: &Path, icons: &IconSet) -> Result<(), ApplyError> {
        let entries: Vec<(u32, Vec<u8>)> = ICO_SIZES
            .iter()
            .filter_map(|&size| icons.png(size).map(|png| (size, png)))
            .collect();
        if entries.is_empty() {
            return Err(ApplyError::Platform(
                "the rendered icon has no size Windows can use".into(),
            ));
        }

        let ico = folder.join(ICO_NAME);
        let ini = folder.join(INI_NAME);

        // Explorer only honours desktop.ini in a read-only folder, but that bit — and the
        // hidden+system bits on the files themselves — make replacing our own files from an
        // earlier apply fail, so everything is cleared first and set again at the end.
        set_readonly(folder, false)?;
        clear_attributes(&ico);
        clear_attributes(&ini);

        write_atomic(&ico, &crate::ico::write_ico(&entries))?;
        let existing = read_text_if_present(&ini)?;
        write_atomic(&ini, desktop_ini_contents(existing.as_deref()).as_bytes())?;

        hide(&ico)?;
        hide(&ini)?;
        set_readonly(folder, true)?;
        notify(folder);
        Ok(())
    }

    /// True when the folder wears FolderSkin's icon, which `revert` would take off.
    pub fn has_custom_icon(folder: &Path) -> bool {
        let ini = read_text_if_present(&folder.join(INI_NAME)).ok().flatten();
        would_revert(ini.as_deref(), folder.join(ICO_NAME).exists())
    }

    /// Removes our ini lines and icon file, leaving anything else in the folder alone.
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

        let ico = folder.join(ICO_NAME);
        if ico.exists() {
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

    /// Tells the shell the folder changed, so open windows repaint without an F5.
    fn notify(folder: &Path) {
        let path_w = wide(folder.as_os_str());
        let path_ptr = path_w.as_ptr().cast::<c_void>();
        for event in [SHCNE_UPDATEDIR, SHCNE_UPDATEITEM] {
            // SAFETY: SHCNF_PATHW promises `path_ptr` is a wide path, which it is, and the
            // second item is unused for these events.
            unsafe {
                SHChangeNotify(
                    event as i32,
                    SHCNF_PATHW | SHCNF_FLUSH,
                    path_ptr,
                    std::ptr::null(),
                );
            }
        }
    }
}
