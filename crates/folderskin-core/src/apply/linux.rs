//! Linux folder icons: a `.directory` desktop entry plus a GIO metadata attribute.
//!
//! There is no single mechanism. KDE's Dolphin reads `Icon=` from a `.directory` file in the
//! folder; Nautilus, Nemo and Caja read the `metadata::custom-icon` GIO attribute instead. So
//! both are written, pointing at the same PNG we drop in the folder.
//!
//! `.directory` is a shared file — it can carry `Name`, `Comment`, sort order and more — so its
//! contents come from pure functions that edit only our own lines. They compile on every OS and
//! are unit-tested there, which is the only way this logic gets covered on a macOS dev machine.

use std::path::Path;

/// The PNG FolderSkin writes into the folder for both mechanisms to point at.
pub const PNG_NAME: &str = ".folderskin.png";
/// The desktop entry Dolphin reads folder appearance from.
pub const DIRECTORY_NAME: &str = ".directory";
/// Comment line that identifies the lines FolderSkin owns.
pub const MARKER: &str = "# managed by FolderSkin";
/// The desktop entry group that holds `Icon=`.
pub const SECTION: &str = "[Desktop Entry]";
/// Size of the PNG written to the folder: big enough for any file manager's largest view.
pub const PNG_SIZE: u32 = 512;

/// The desktop entry key that names the folder's icon. Desktop entry keys are case-sensitive.
const ICON_KEY: &str = "Icon";

/// A whole `.directory` that points at `icon_abs`.
///
/// This is the file written when the folder has no `.directory` of its own, or only one we
/// wrote earlier.
pub fn directory_file_contents(icon_abs: &Path) -> String {
    directory_file_with_icon("", icon_abs)
}

/// `existing` with `Icon=` pointed at `icon_abs` and our marker appended.
///
/// Every other key, group and comment is preserved: a `.directory` often carries the folder's
/// display name or sort order, and losing that to an icon change would be indefensible.
pub fn directory_file_with_icon(existing: &str, icon_abs: &Path) -> String {
    let icon_line = format!("{ICON_KEY}={}", icon_abs.display());
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut seen_section = false;
    let mut inserted = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        if is_header(trimmed) {
            // Our lines belong in [Desktop Entry], so they go in as the next group starts.
            if in_section && !inserted {
                out.push(icon_line.clone());
                out.push(MARKER.to_string());
                inserted = true;
            }
            in_section = is_our_section(trimmed);
            seen_section |= in_section;
            out.push(line.to_string());
            continue;
        }

        // Drop our own marker wherever it sits, and any Icon= in the group we own — that key
        // is exactly what we are replacing.
        if trimmed == MARKER {
            continue;
        }
        if in_section && key_of(trimmed) == Some(ICON_KEY) {
            continue;
        }
        out.push(line.to_string());
    }

    if in_section && !inserted {
        out.push(icon_line.clone());
        out.push(MARKER.to_string());
        inserted = true;
    }
    if !seen_section {
        // No [Desktop Entry] at all: the spec wants that group first in the file.
        let mut head = vec![SECTION.to_string(), icon_line, MARKER.to_string()];
        head.append(&mut out);
        out = head;
        inserted = true;
    }
    debug_assert!(inserted, "our lines must end up somewhere");

    join_lines(&out)
}

/// True when `contents` is a file FolderSkin wrote and nothing else has added to.
///
/// Stricter than "carries our marker": applying to someone else's `.directory` leaves the
/// marker in a file full of their keys, and that file must survive a revert. Only a file whose
/// every line is our marker, an `Icon=` or the group header can safely be deleted outright.
pub fn is_ours(contents: &str) -> bool {
    let mut marked = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == MARKER {
            marked = true;
            continue;
        }
        if is_our_section(trimmed) {
            continue;
        }
        if key_of(trimmed) == Some(ICON_KEY) {
            continue;
        }
        return false;
    }
    marked
}

/// `existing` with FolderSkin's lines removed, or `None` when the file should be deleted.
///
/// Only our marker and an `Icon=` that names our own PNG are removed, so a user who later
/// pointed the key at their own artwork keeps it. `None` is returned when nothing but group
/// headers would be left.
pub fn directory_file_without_ours(existing: &str) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        if is_header(trimmed) {
            in_section = is_our_section(trimmed);
            out.push(line.to_string());
            continue;
        }
        if trimmed == MARKER {
            continue;
        }
        if in_section && is_our_icon(trimmed) {
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
    Some(join_lines(&out))
}

/// True for a `[Group]` header line.
fn is_header(trimmed: &str) -> bool {
    trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2
}

/// True for the `[Desktop Entry]` header.
fn is_our_section(trimmed: &str) -> bool {
    trimmed == SECTION
}

/// The key of a `key=value` line, trimmed; `None` for anything else.
fn key_of(trimmed: &str) -> Option<&str> {
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

/// True for an `Icon=` line whose value names the PNG FolderSkin writes.
fn is_our_icon(trimmed: &str) -> bool {
    let Some((key, value)) = trimmed.split_once('=') else {
        return false;
    };
    key.trim() == ICON_KEY && Path::new(value.trim()).file_name() == Some(PNG_NAME.as_ref())
}

/// Joins `lines` with LF, including a trailing one, as the desktop entry spec wants.
fn join_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(target_os = "linux")]
pub use imp::{apply, revert};

#[cfg(target_os = "linux")]
mod imp {
    use super::{
        directory_file_contents, directory_file_with_icon, directory_file_without_ours, is_ours,
        DIRECTORY_NAME, PNG_NAME, PNG_SIZE,
    };
    use crate::apply::paths::{read_text_if_present, write_atomic};
    use crate::apply::ApplyError;
    use crate::compositor::IconSet;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    /// GIO attribute Nautilus, Nemo and Caja read a per-folder icon from.
    const GIO_KEY: &str = "metadata::custom-icon";

    /// Writes the PNG and `.directory`, then sets the GIO attribute if `gio` is installed.
    pub fn apply(folder: &Path, icons: &IconSet) -> Result<(), ApplyError> {
        let png = icons.png(PNG_SIZE).ok_or_else(|| {
            ApplyError::Platform(format!("the rendered icon has no {PNG_SIZE} px size"))
        })?;
        let png_path = folder.join(PNG_NAME);
        write_atomic(&png_path, &png)?;

        let entry_path = folder.join(DIRECTORY_NAME);
        let contents = match read_text_if_present(&entry_path)? {
            // Somebody else's file: edit their Icon= in place instead of replacing the file.
            Some(existing) if !is_ours(&existing) => directory_file_with_icon(&existing, &png_path),
            _ => directory_file_contents(&png_path),
        };
        write_atomic(&entry_path, contents.as_bytes())?;

        set_gio_icon(folder, &png_path);
        Ok(())
    }

    /// Removes our `.directory` lines, the PNG and the GIO attribute.
    pub fn revert(folder: &Path) -> Result<(), ApplyError> {
        let entry_path = folder.join(DIRECTORY_NAME);
        if let Some(existing) = read_text_if_present(&entry_path)? {
            if is_ours(&existing) {
                std::fs::remove_file(&entry_path)?;
            } else {
                match directory_file_without_ours(&existing) {
                    None => std::fs::remove_file(&entry_path)?,
                    Some(left) if left != existing => {
                        write_atomic(&entry_path, left.as_bytes())?;
                    }
                    Some(_) => {}
                }
            }
        }

        let png_path = folder.join(PNG_NAME);
        match std::fs::remove_file(&png_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        unset_gio_icon(folder);
        Ok(())
    }

    /// `file://` URI for a local path, percent-encoding everything GIO would misread
    /// (spaces, `#`, `%`, `?` and non-ASCII bytes).
    pub fn file_uri(path: &Path) -> String {
        use std::os::unix::ffi::OsStrExt;
        let mut out = String::from("file://");
        for &b in path.as_os_str().as_bytes() {
            let keep = b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~');
            if keep {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    /// `gio set <folder> metadata::custom-icon file://<png>`; ignored when it fails.
    ///
    /// The `.directory` file already covers Dolphin, and a GNOME-less box has no `gio` at all,
    /// so this is an extra rather than a step the apply depends on.
    fn set_gio_icon(folder: &Path, png: &Path) {
        let Some(gio) = gio_on_path() else {
            return;
        };
        let uri = file_uri(png);
        let _ = quiet(&mut Command::new(gio))
            .arg("set")
            .arg(folder)
            .arg(GIO_KEY)
            .arg(uri)
            .status();
    }

    /// `gio set -t unset <folder> metadata::custom-icon`; ignored when it fails.
    fn unset_gio_icon(folder: &Path) {
        let Some(gio) = gio_on_path() else {
            return;
        };
        let _ = quiet(&mut Command::new(gio))
            .arg("set")
            .arg("-t")
            .arg("unset")
            .arg(folder)
            .arg(GIO_KEY)
            .status();
    }

    /// Keeps `gio`'s chatter out of the app's stdio.
    fn quiet(command: &mut Command) -> &mut Command {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
    }

    /// The `gio` executable on `PATH`, if there is one.
    fn gio_on_path() -> Option<PathBuf> {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join("gio"))
            .find(|candidate| {
                std::fs::metadata(candidate)
                    .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            })
    }
}
