//! A drive's icon on Linux: the folder's own files at the drive's root, and `.xdg-volume-info`.
//!
//! Dolphin, Nautilus, Nemo and Caja draw a drive's root the way they draw any folder, from the
//! `.directory` file and the GIO attribute a folder's icon is made of ([`super::super::linux`]).
//! GNOME's list of drives reads `.xdg-volume-info` at the root of a drive as it mounts it, and
//! takes the drive's icon from its `IconFile=`, a path inside the drive, so the icon goes with the
//! drive wherever it's mounted next.
//!
//! `.xdg-volume-info` can carry the drive's own name (`Name=`), so it's edited, never replaced:
//! FolderSkin writes its `IconFile=` with a marker beside it, and a revert takes out exactly those
//! lines, deleting the file only when nothing else is left in it. That editing is pure, and tested
//! on any computer.

/// The file GNOME reads a drive's name and icon from, at the drive's root.
pub const VOLUME_INFO: &str = ".xdg-volume-info";
/// Its one group.
pub const SECTION: &str = "[Volume Info]";
/// The comment line beside FolderSkin's own line.
pub const MARKER: &str = "# managed by FolderSkin";

/// The `IconFile=` line FolderSkin writes: the folder icon's PNG, relative to the drive's root.
fn our_line() -> String {
    format!("IconFile={}", super::super::linux::PNG_NAME)
}

/// The key of a `key=value` line.
fn key_of(trimmed: &str) -> Option<&str> {
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

/// `existing` with `IconFile=` pointing at FolderSkin's PNG, and everything else in it kept: a
/// whole new file when there was none. Another `IconFile=` or `Icon=` in the group is put aside
/// as a comment, for [`without_ours`] to put back.
pub fn with_icon(existing: Option<&str>) -> String {
    let existing = existing.map(without_ours_text).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut placed = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_section && !placed {
                out.push(our_line());
                out.push(MARKER.into());
                placed = true;
            }
            in_section = trimmed == SECTION;
            out.push(line.into());
            continue;
        }
        if in_section && matches!(key_of(trimmed), Some("IconFile" | "Icon")) {
            out.push(format!("{PUT_ASIDE}{line}"));
            continue;
        }
        out.push(line.into());
    }
    if in_section && !placed {
        out.push(our_line());
        out.push(MARKER.into());
        placed = true;
    }
    if !placed {
        let mut head = vec![SECTION.to_string(), our_line(), MARKER.to_string()];
        if !out.is_empty() {
            head.push(String::new());
        }
        head.append(&mut out);
        out = head;
    }
    join(&out)
}

/// The start of a line of the drive's own icon, put aside while FolderSkin's is on.
const PUT_ASIDE: &str = "# put aside by FolderSkin: ";

/// `existing` with FolderSkin's lines taken out and anything it put aside put back; `None` when
/// nothing would be left, so the file can go.
pub fn without_ours(existing: &str) -> Option<String> {
    let left = without_ours_text(existing);
    (!left.trim().is_empty()).then_some(left)
}

/// [`without_ours`] as text. A `[Volume Info]` group left with nothing in it goes too, with the
/// blank line FolderSkin put after it: it says nothing, and FolderSkin is what added it.
fn without_ours_text(existing: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed == MARKER || trimmed == our_line() {
            continue;
        }
        match line.trim_start().strip_prefix(PUT_ASIDE) {
            Some(own) => lines.push(own.to_string()),
            None => lines.push(line.into()),
        }
    }
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == SECTION {
            let end = lines[i + 1..]
                .iter()
                .position(|l| l.trim().starts_with('['))
                .map_or(lines.len(), |p| i + 1 + p);
            if lines[i + 1..end].iter().all(|l| l.trim().is_empty()) {
                i = end;
                continue;
            }
        }
        out.push(lines[i].clone());
        i += 1;
    }
    join(&out)
}

/// True when `existing` carries FolderSkin's icon line.
pub fn is_ours(existing: &str) -> bool {
    existing.lines().any(|l| l.trim() == MARKER)
}

fn join(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(target_os = "linux")]
pub use imp::{apply, has_custom_icon, revert};

#[cfg(target_os = "linux")]
mod imp {
    use super::{is_ours, with_icon, without_ours, VOLUME_INFO};
    use crate::apply::paths::{read_text_if_present, write_atomic};
    use crate::apply::{linux, ApplyError};
    use std::path::Path;

    /// The folder icon at the drive's root, and `.xdg-volume-info` pointing at it.
    pub fn apply(root: &Path, png: &[u8]) -> Result<(), ApplyError> {
        linux::apply(root, png)?;
        let info = root.join(VOLUME_INFO);
        let existing = read_text_if_present(&info)?;
        write_atomic(&info, with_icon(existing.as_deref()).as_bytes())?;
        Ok(())
    }

    /// FolderSkin's lines out of `.xdg-volume-info`, and the folder icon off the root.
    pub fn revert(root: &Path) -> Result<(), ApplyError> {
        let info = root.join(VOLUME_INFO);
        if let Some(existing) = read_text_if_present(&info)? {
            if is_ours(&existing) {
                match without_ours(&existing) {
                    Some(left) => write_atomic(&info, left.as_bytes())?,
                    None => std::fs::remove_file(&info)?,
                }
            }
        }
        linux::revert(root)
    }

    /// True when the drive wears an icon a revert would take off.
    pub fn has_custom_icon(root: &Path) -> bool {
        let info = read_text_if_present(&root.join(VOLUME_INFO)).ok().flatten();
        info.as_deref().is_some_and(is_ours) || linux::has_custom_icon(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_with_no_volume_info_gets_one_of_its_own() {
        let written = with_icon(None);
        assert_eq!(
            written,
            "[Volume Info]\nIconFile=.folderskin.png\n# managed by FolderSkin\n"
        );
        assert!(is_ours(&written));
        assert_eq!(without_ours(&written), None, "all FolderSkin's, so it goes");
    }

    #[test]
    fn a_drives_own_name_and_icon_are_kept() {
        let theirs = "[Volume Info]\nName=Holiday photos\nIconFile=.icon.png\n";
        let written = with_icon(Some(theirs));
        assert_eq!(
            written,
            "[Volume Info]\nName=Holiday photos\n# put aside by FolderSkin: IconFile=.icon.png\nIconFile=.folderskin.png\n# managed by FolderSkin\n"
        );
        // A second apply changes nothing, and a revert gives back what was there.
        assert_eq!(with_icon(Some(&written)), written);
        assert_eq!(without_ours(&written).as_deref(), Some(theirs));
    }

    #[test]
    fn a_file_with_other_groups_keeps_them() {
        let theirs = "[Other]\nKey=value\n";
        let written = with_icon(Some(theirs));
        assert!(
            written.starts_with("[Volume Info]\nIconFile=.folderskin.png\n"),
            "{written}"
        );
        assert!(written.ends_with("[Other]\nKey=value\n"), "{written}");
        assert_eq!(
            without_ours(&written).as_deref(),
            Some(theirs),
            "exactly as it was"
        );
    }
}
