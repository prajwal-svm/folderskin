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
/// Start of the comment line recording what the folder was before FolderSkin marked it.
pub const WAS_PREFIX: &str = "; folder was: ";
/// Start of a line of the folder's own icon, put aside while FolderSkin's is on: the whole line
/// follows it, as it was, so a revert can put it back exactly.
pub const PUT_ASIDE_PREFIX: &str = "; put aside by FolderSkin: ";

/// Which of the two attributes FolderSkin marks a folder with the folder already had of its own.
///
/// Marking a folder read-only and system is how Explorer is told to read `desktop.ini`, but a
/// folder may have carried either bit before FolderSkin ever saw it — a read-only folder someone
/// set deliberately is not unusual. Clearing both on revert would quietly take that away, so the
/// apply writes down what it found and the revert puts it back. Recorded in the ini FolderSkin
/// already owns rather than anywhere new, and read back by [`was_before`].
///
/// An ini with no such line was written by a version that did not record it; the revert then
/// clears both, which is what that version would have done.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Before {
    pub readonly: bool,
    pub system: bool,
}

impl Before {
    /// The comment line recording it.
    fn line(self) -> String {
        let what = match (self.readonly, self.system) {
            (true, true) => "readonly,system",
            (true, false) => "readonly",
            (false, true) => "system",
            (false, false) => "nothing",
        };
        format!("{WAS_PREFIX}{what}")
    }
}

/// What a `desktop.ini` FolderSkin wrote records the folder as having been, if it says.
pub fn was_before(desktop_ini: &str) -> Option<Before> {
    let (_, body) = split_bom(desktop_ini);
    let value = body
        .lines()
        .find_map(|line| line.trim().strip_prefix(WAS_PREFIX))?
        .trim()
        .to_ascii_lowercase();
    if value == "nothing" {
        return Some(Before::default());
    }
    let mut before = Before::default();
    for part in value.split(',') {
        match part.trim() {
            "readonly" => before.readonly = true,
            "system" => before.system = true,
            // A word we don't know: written by a later version, so say nothing rather than guess.
            _ => return None,
        }
    }
    Some(before)
}

/// True for a line of ours recording what the folder was.
fn is_was_line(trimmed: &str) -> bool {
    trimmed.starts_with(WAS_PREFIX)
}
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
///
/// An icon the folder already had of its own is put aside rather than dropped: its lines stay
/// where they were, commented out behind [`PUT_ASIDE_PREFIX`], and [`desktop_ini_without_ours`]
/// takes the comment off again. Only an `IconResource` naming our own icon file, in an ini that
/// carries our marker, counts as ours.
pub fn desktop_ini_contents(existing: Option<&str>, ico_name: &str, before: Before) -> String {
    let icon_line = our_icon_line(ico_name);
    let was_line = before.line();

    let Some(existing) = existing else {
        return join_lines(
            "",
            &[SECTION.to_string(), MARKER.to_string(), was_line, icon_line],
        );
    };

    let (bom, body) = split_bom(existing);
    let marked = is_ours(existing);
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
                out.push(was_line.clone());
                out.push(icon_line.clone());
                inserted = true;
            }
            in_section = is_our_section(trimmed);
            seen_section |= in_section;
            out.push(line.to_string());
            continue;
        }

        // Drop our own marker wherever it sits, and our own IconResource inside the section we
        // own — that key is exactly what we are replacing.
        if trimmed == MARKER || is_was_line(trimmed) {
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
            // pair would silently win over ours. Someone else's is put aside where it stands,
            // for the revert to put back.
            if !(marked && is_our_icon_resource(trimmed)) {
                out.push(format!("{PUT_ASIDE_PREFIX}{line}"));
            }
            continue;
        }
        out.push(line.to_string());
    }

    if in_section && !inserted {
        out.push(MARKER.to_string());
        out.push(was_line.clone());
        out.push(icon_line.clone());
        inserted = true;
    }
    if !seen_section {
        // No [.ShellClassInfo] at all: ours leads the file, where Explorer expects it.
        let mut head = vec![SECTION.to_string(), MARKER.to_string(), was_line, icon_line];
        head.append(&mut out);
        out = head;
        inserted = true;
    }
    debug_assert!(inserted, "our lines must end up somewhere");

    join_lines(bom, &out)
}

/// `existing` with FolderSkin's lines removed, or `None` when the file should be deleted.
///
/// Only our marker and an `IconResource` that names our own icon file in a marked ini are
/// removed, so a user who later pointed the key at their own icon keeps it, and an icon the
/// folder had before is put back where it was. `None` is returned when nothing but section
/// headers would be left — there is no reason to keep an ini with no keys in it.
pub fn desktop_ini_without_ours(existing: &str) -> Option<String> {
    let (bom, body) = split_bom(existing);
    let marked = is_ours(existing);
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if is_header(trimmed) {
            in_section = is_our_section(trimmed);
            out.push(line.to_string());
            continue;
        }
        if trimmed == MARKER || is_was_line(trimmed) {
            continue;
        }
        if let Some(theirs) = line.strip_prefix(PUT_ASIDE_PREFIX) {
            out.push(theirs.to_string());
            continue;
        }
        if in_section && marked && is_our_icon_resource(trimmed) {
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
/// if the folder has one). Someone else's `IconResource` doesn't count, since a revert leaves it
/// alone; nor does an icon file of ours the ini doesn't name, which a revert leaves alone too.
pub fn would_revert(desktop_ini: Option<&str>) -> bool {
    desktop_ini.is_some_and(|ini| desktop_ini_without_ours(ini).as_deref() != Some(ini))
}

/// The icon files a marked `desktop.ini` names as FolderSkin's: what a revert deletes, and what
/// a new apply replaces.
///
/// Deleting goes by this and not by the names in the folder, so a file the user called
/// `folderskin.ico` in a folder FolderSkin never touched is not ours to take. Each name matched
/// [`is_our_ico_name`], so it is a bare file name in the folder itself, never a path.
pub fn our_icon_files(desktop_ini: &str) -> Vec<String> {
    if !is_ours(desktop_ini) {
        return Vec::new();
    }
    let (_, body) = split_bom(desktop_ini);
    let mut in_section = false;
    let mut names = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if is_header(trimmed) {
            in_section = is_our_section(trimmed);
            continue;
        }
        if in_section && is_our_icon_resource(trimmed) {
            if let Some((_, value)) = trimmed.split_once('=') {
                let name = value.split(',').next().unwrap_or("").trim().to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

/// The icon file a folder's `desktop.ini` points Explorer at, and which icon in it, as the
/// values were written: `("folderskin-1a2b….ico", 0)`, `("%SystemRoot%\\system32\\imageres.dll",
/// 3)`, `("..\\shared\\theirs.ico", 0)`.
///
/// Explorer takes the older `IconFile`/`IconIndex` pair over `IconResource` when a folder carries
/// both, which is why [`desktop_ini_contents`] puts the pair aside when it writes ours — so this reads
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

/// How a `desktop.ini` was written, so it is written back the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IniEncoding {
    /// UTF-8, or plain ASCII, with or without its byte-order mark.
    Utf8,
    /// UTF-16, little-endian, after its byte-order mark: what Windows writes once a folder has a
    /// name of its own (`LocalizedResourceName`), as Music and Videos have.
    Utf16,
}

/// A `desktop.ini`'s text and how it was written. The byte-order mark stays at the front of the
/// text (as U+FEFF), so an edit keeps it and [`encode_ini`] puts it back. `None` for anything
/// that isn't UTF-8 or UTF-16 with its mark, which a lossy read and write would corrupt.
pub fn decode_ini(bytes: &[u8]) -> Option<(String, IniEncoding)> {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let (pairs, odd) = rest.as_chunks::<2>();
        if !odd.is_empty() {
            return None;
        }
        let units: Vec<u16> = pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
        let text = String::from_utf16(&units).ok()?;
        return Some((format!("\u{feff}{text}"), IniEncoding::Utf16));
    }
    String::from_utf8(bytes.to_vec())
        .ok()
        .map(|text| (text, IniEncoding::Utf8))
}

/// `text` as the bytes of a `desktop.ini` written the way `encoding` says: UTF-16 always starts
/// with its byte-order mark.
pub fn encode_ini(text: &str, encoding: IniEncoding) -> Vec<u8> {
    match encoding {
        IniEncoding::Utf8 => text.as_bytes().to_vec(),
        IniEncoding::Utf16 => {
            let marked = if text.starts_with('\u{feff}') {
                std::borrow::Cow::Borrowed(text)
            } else {
                std::borrow::Cow::Owned(format!("\u{feff}{text}"))
            };
            marked.encode_utf16().flat_map(u16::to_le_bytes).collect()
        }
    }
}

#[cfg(windows)]
pub use imp::{apply, has_custom_icon, refresh_shell_icons, revert};

#[cfg(windows)]
mod imp {
    use super::{
        decode_ini, desktop_ini_contents, desktop_ini_without_ours, encode_ini, ico_file_name,
        our_icon_files, was_before, would_revert, Before, IniEncoding, INI_NAME,
    };
    use crate::apply::paths::write_atomic;
    use crate::apply::ApplyError;
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN,
        FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_NOT_CONTENT_INDEXED, FILE_ATTRIBUTE_READONLY,
        FILE_ATTRIBUTE_SYSTEM, INVALID_FILE_ATTRIBUTES,
    };
    use windows_sys::Win32::UI::Shell::{
        ILCreateFromPathW, ILFree, SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNE_ATTRIBUTES,
        SHCNE_RENAMEFOLDER, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_FLUSH, SHCNF_IDLIST,
        SHCNF_PATHW,
    };

    /// How long [`refresh_shell_icons`] waits before asking the shell to redraw, so the change it
    /// is meant to show has been taken in by the time it arrives. Found by measurement: sent with
    /// no pause at all the refresh is always one operation behind.
    const SETTLE: std::time::Duration = std::time::Duration::from_millis(600);

    /// Writes the icon file (the [`prepare`](super::prepare)d bytes, under the name its contents
    /// give it) and a `desktop.ini` that points at it, then tells Explorer.
    ///
    /// Any icon file an earlier apply left behind goes once the new one is in place and the ini
    /// names it, so a folder never keeps more than the one it wears and the ini never points at
    /// a file that isn't there. Only the file the old ini named goes: one the user happened to
    /// call `folderskin.ico` stays.
    ///
    /// A `desktop.ini` that was there before keeps its own read-only, archive and indexing
    /// attributes, so a revert can hand them back, and is written back as it was written: UTF-16
    /// stays UTF-16. One that can't be read as either is refused before anything in the folder
    /// changes, and a write that fails part-way takes back what it did.
    pub fn apply(folder: &Path, ico_bytes: &[u8]) -> Result<(), ApplyError> {
        let name = ico_file_name(ico_bytes);
        let ico = folder.join(&name);
        let ini = folder.join(INI_NAME);
        let existing = read_ini(&ini)?;
        let ini_attributes = attributes(&ini);
        let had_ico = ico.exists();

        // The hidden+system bits on our own files from an earlier apply stop them being
        // replaced, so those are cleared first and set again at the end.
        //
        // The folder's read-only bit is deliberately left alone. It is what makes Explorer read
        // `desktop.ini` at all, and taking it off for the length of the write was a race we lost
        // about half the time: writing these files makes the shell look at the folder again, and
        // when it looked while the bit was off it saw a folder with no custom icon and cached
        // that — so the apply finished correctly and the folder still showed the plain default.
        // A directory's read-only attribute never blocked writing files inside it on Windows; it
        // is only the marker, which is why clearing it bought nothing.
        clear_attributes(&ico);
        clear_attributes(&ini);

        let text = existing.as_ref().map(|(text, _)| text.as_str());
        let encoding = existing.as_ref().map_or(IniEncoding::Utf8, |(_, e)| *e);
        // What the folder was before FolderSkin marked it. On a re-apply the marks are already
        // on, so what an earlier apply wrote down beats what the folder looks like now.
        let before = text
            .and_then(was_before)
            .unwrap_or_else(|| marks_now(folder));
        let written = write_atomic(&ico, ico_bytes).and_then(|()| {
            write_atomic(
                &ini,
                &encode_ini(&desktop_ini_contents(text, &name, before), encoding),
            )
        });
        if let Err(e) = written {
            // Nothing half-done is left: the icon file goes unless it was there already, and the
            // ini gets its attributes back.
            if had_ico {
                let _ = hide(&ico);
            } else {
                let _ = std::fs::remove_file(&ico);
            }
            if let Some(was) = ini_attributes {
                let _ = set_attributes(&ini, was);
            }
            return Err(e.into());
        }

        for stale in text.map(our_icon_files).unwrap_or_default() {
            if stale != name {
                let stale = folder.join(stale);
                clear_attributes(&stale);
                let _ = std::fs::remove_file(&stale);
            }
        }

        hide(&ico)?;
        hide_keeping(&ini, ini_attributes)?;
        set_customized(folder, true, before)?;
        notify(folder);
        Ok(())
    }

    /// True when the folder wears FolderSkin's icon, which `revert` would take off.
    pub fn has_custom_icon(folder: &Path) -> bool {
        let ini = read_ini(&folder.join(INI_NAME)).ok().flatten();
        would_revert(ini.as_ref().map(|(text, _)| text.as_str()))
    }

    /// `desktop.ini`'s text and how it was written, or `None` when the folder has none. One that
    /// is neither UTF-8 nor UTF-16 is refused rather than guessed at: it is read back, edited and
    /// written again, and a lossy round trip would corrupt the keys already in it.
    fn read_ini(path: &Path) -> Result<Option<(String, IniEncoding)>, ApplyError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        decode_ini(&bytes).map(Some).ok_or_else(|| {
            ApplyError::Refused(format!(
                "{} is neither UTF-8 nor UTF-16 text, so FolderSkin will not rewrite it",
                path.display()
            ))
        })
    }

    /// Removes our ini lines and the icon files they name, leaving anything else in the folder
    /// alone, and puts back an icon the folder had of its own.
    pub fn revert(folder: &Path) -> Result<(), ApplyError> {
        let mut touched = false;
        // Read while our lines are still in the file: they are what says whether the folder had
        // either mark of its own. No record means a version that never wrote one, so both come
        // off, which is what that version did.
        let mut before = Before::default();
        let mut icons = Vec::new();

        let ini = folder.join(INI_NAME);
        if let Some((existing, encoding)) = read_ini(&ini)? {
            before = was_before(&existing).unwrap_or_default();
            icons = our_icon_files(&existing);
            match desktop_ini_without_ours(&existing) {
                None => {
                    clear_attributes(&ini);
                    std::fs::remove_file(&ini)?;
                    touched = true;
                }
                Some(left) if left != existing => {
                    let kept = attributes(&ini);
                    clear_attributes(&ini);
                    write_atomic(&ini, &encode_ini(&left, encoding))?;
                    hide_keeping(&ini, kept)?;
                    touched = true;
                }
                // Nothing of ours in it; leave the file and its attributes untouched.
                Some(_) => {}
            }
        }

        for name in icons {
            let ico = folder.join(name);
            if ico.is_file() {
                clear_attributes(&ico);
                std::fs::remove_file(&ico)?;
                touched = true;
            }
        }

        if !touched {
            // Never skinned by us: change nothing, not even the read-only attribute.
            return Ok(());
        }
        set_customized(folder, false, before)?;
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

    /// Marks `path` hidden and system as [`hide`] does, keeping the read-only, archive and
    /// indexing attributes it had in `before` (read before it was cleared to be written). A file
    /// that wasn't there before gets hidden and system alone.
    fn hide_keeping(path: &Path, before: Option<u32>) -> Result<(), ApplyError> {
        const KEPT: u32 =
            FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_ARCHIVE | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;
        let kept = before.unwrap_or(0) & KEPT;
        set_attributes(path, kept | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM)
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

    /// Marks `folder` as one Explorer reads `desktop.ini` from, or takes the marks off again.
    ///
    /// Both bits go on. Windows' own guidance for customising a folder is to mark it **system**,
    /// and that is the one that counts: with read-only alone a folder on the OneDrive-synced
    /// Desktop kept drawing the plain folder however correctly the skin had been written, and no
    /// amount of telling the shell about it helped. Read-only stays as well because it is what
    /// FolderSkin has always set, so a folder skinned by an older version still reverts cleanly.
    ///
    /// The other attributes are kept as they were: this reads what is there and changes only
    /// these two bits, so a folder does not lose its archive bit or anything else to an apply.
    fn set_customized(folder: &Path, on: bool, before: Before) -> Result<(), ApplyError> {
        let Some(current) = attributes(folder) else {
            return Err(ApplyError::Io(std::io::Error::last_os_error()));
        };
        let next = if on {
            current | MARKS
        } else {
            // Off again, except for whatever the folder had of its own: taking a read-only bit
            // off a folder someone set read-only themselves is not this program's business.
            let mut next = current & !MARKS;
            if before.readonly {
                next |= FILE_ATTRIBUTE_READONLY;
            }
            if before.system {
                next |= FILE_ATTRIBUTE_SYSTEM;
            }
            next
        };
        if next == current {
            return Ok(());
        }
        set_attributes(folder, next)
    }

    /// The two marks the folder carries right now, for an apply with nothing written down yet.
    fn marks_now(folder: &Path) -> Before {
        let current = attributes(folder).unwrap_or(0);
        Before {
            readonly: current & FILE_ATTRIBUTE_READONLY != 0,
            system: current & FILE_ATTRIBUTE_SYSTEM != 0,
        }
    }

    /// `folder`'s attributes, or `None` when they can't be read.
    fn attributes(folder: &Path) -> Option<u32> {
        let path_w = wide(folder.as_os_str());
        // SAFETY: `path_w` is a NUL-terminated UTF-16 path that outlives the call.
        let current = unsafe { GetFileAttributesW(path_w.as_ptr()) };
        (current != INVALID_FILE_ATTRIBUTES).then_some(current)
    }

    /// The two attributes that tell Explorer to read a folder's `desktop.ini`.
    const MARKS: u32 = FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_SYSTEM;

    /// Tells the shell the folder's icon changed, so open windows repaint without an F5.
    ///
    /// Two things had to be got right, and each was found by applying a skin to a folder that was
    /// already on screen and watching whether the icon changed on its own.
    ///
    /// **The view that draws a folder's icon is the view listing it**, not a window showing what
    /// is inside it. A folder on the Desktop is drawn by the Desktop; a folder in Documents by the
    /// Documents window. Naming only the folder, as this used to, left that view holding the icon
    /// it had already drawn.
    ///
    /// **And the Desktop does not listen by path.** It watches the shell namespace, so a
    /// `SHCNF_PATHW` notification never reaches it — which is why a folder on the Desktop kept its
    /// old icon even once the parent was being told. The same notifications therefore go out a
    /// second time as item id lists ([`notify_pidl`]). Ordinary Explorer windows take the path
    /// form, the Desktop takes the id-list form, and sending both is what covers every view; a
    /// view that hears about a change twice simply redraws once.
    ///
    /// The events are the three things that actually changed: the folder's read-only attribute
    /// (the bit that makes Explorer read `desktop.ini` at all), the folder as an item, and the
    /// listing it appears in.
    fn notify(folder: &Path) {
        notify_path(folder, SHCNE_ATTRIBUTES);
        notify_path(folder, SHCNE_UPDATEITEM);
        // Also the folder itself: its contents really did change, for a window that has it open.
        notify_path(folder, SHCNE_UPDATEDIR);
        notify_pidl(folder, SHCNE_ATTRIBUTES);
        notify_pidl(folder, SHCNE_UPDATEITEM);
        if let Some(parent) = folder.parent() {
            notify_path(parent, SHCNE_UPDATEDIR);
            notify_pidl(parent, SHCNE_UPDATEDIR);
        }
        notify_renamed_in_place(folder);
    }

    /// Tells the shell the folder was renamed — to the name it already has.
    ///
    /// This is what makes the icon on the *desktop* change. The desktop's icon view keeps the
    /// image it first drew for an item and re-reads it for nothing: not `SHCNE_UPDATEITEM`,
    /// `SHCNE_ATTRIBUTES`, `SHCNE_UPDATEDIR` on its parent, nor `SHCNE_UPDATEIMAGE` for the image
    /// the folder resolves to, by path or by id list. A rename is different in kind — the item is
    /// dropped and made again, and the new item asks for its icon — so naming the folder as both
    /// the old and the new path is a rename that moves nothing and refreshes everything.
    ///
    /// Ordinary Explorer windows are already following the notifications above by this point;
    /// this is for the desktop, which is not an ordinary window.
    fn notify_renamed_in_place(folder: &Path) {
        let path_w = wide(folder.as_os_str());
        let path_ptr = path_w.as_ptr().cast::<c_void>();
        // SAFETY: SHCNF_PATHW promises both items are wide paths. They are the same buffer, which
        // outlives the call: a rename from the folder's name to the folder's name.
        unsafe {
            SHChangeNotify(
                SHCNE_RENAMEFOLDER as i32,
                SHCNF_PATHW | SHCNF_FLUSH,
                path_ptr,
                path_ptr,
            );
        }
    }

    /// One `SHChangeNotify` naming `path` as a path.
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

    /// Asks the shell to draw folder icons again, everywhere.
    ///
    /// The Desktop is the reason this exists. It does not repaint for any notification about the
    /// folder that changed: not by path, not by item id list, not `SHCNE_UPDATEITEM`,
    /// `SHCNE_ATTRIBUTES`, `SHCNE_UPDATEDIR` on its parent, nor `SHCNE_UPDATEIMAGE` for the image
    /// index the folder resolves to — each was tried against a folder sitting on the Desktop while
    /// its skin changed, and the icon stayed as it was. `SHCNE_ASSOCCHANGED` is the one that
    /// works, and it is what folder-icon tools have always used.
    ///
    /// It is blunt: the shell treats it as "file associations changed" and refreshes icons across
    /// every view, which can show as a brief flicker. So it is *not* part of [`apply`] — the
    /// caller sends it once when a whole operation has finished, rather than once per folder, so a
    /// run over a thousand folders costs one refresh and not a thousand.
    ///
    /// And it waits first. Sent the instant the last file is written, the refresh is processed
    /// before the shell has taken in the change, and the folder keeps its old icon until the
    /// *next* refresh — applying to one folder would repaint the folder skinned before it, one
    /// operation behind for ever. [`SETTLE`] is the pause that stops that.
    ///
    /// The pause is spent on a thread of its own and this returns at once, so the apply the user
    /// is waiting on finishes when the folder is written rather than [`SETTLE`] later. Asking
    /// again while one is waiting replaces it instead of adding to it: a run of applies ends in
    /// one refresh, [`SETTLE`] after the last of them, not a flicker for each.
    pub fn refresh_shell_icons() {
        let mine = REFRESHES.fetch_add(1, Ordering::SeqCst) + 1;
        std::thread::spawn(move || {
            std::thread::sleep(SETTLE);
            // Someone asked again while this one was waiting. Their settle has not run out yet,
            // and it covers this change as well as theirs, so leave the single refresh to them.
            if REFRESHES.load(Ordering::SeqCst) != mine {
                return;
            }
            // SAFETY: SHCNE_ASSOCCHANGED takes no items, so both are null, which is what the
            // documentation asks for.
            unsafe {
                SHChangeNotify(
                    SHCNE_ASSOCCHANGED as i32,
                    SHCNF_IDLIST | SHCNF_FLUSH,
                    std::ptr::null(),
                    std::ptr::null(),
                );
            }
        });
    }

    /// Counts calls to [`refresh_shell_icons`], so each waiting refresh can tell whether it is
    /// still the last one asked for.
    static REFRESHES: AtomicU64 = AtomicU64::new(0);

    /// One `SHChangeNotify` naming `path` as an item id list, which is how the Desktop hears it.
    ///
    /// `ILCreateFromPath` rather than `SHParseDisplayName` because it needs no COM: this runs on
    /// whichever blocking thread the apply landed on, and the folder is a plain filesystem path.
    /// A path the shell cannot make an id list of is skipped — the path notification above has
    /// already gone out, and failing to repaint is not worth failing an apply over.
    fn notify_pidl(path: &Path, event: u32) {
        let path_w = wide(path.as_os_str());
        // SAFETY: `path_w` is a NUL-terminated wide path. The returned id list is owned by this
        // function, handed to SHChangeNotify while it is alive, and freed exactly once.
        unsafe {
            let pidl = ILCreateFromPathW(path_w.as_ptr());
            if pidl.is_null() {
                return;
            }
            SHChangeNotify(
                event as i32,
                SHCNF_IDLIST | SHCNF_FLUSH,
                pidl.cast::<c_void>(),
                std::ptr::null(),
            );
            ILFree(pidl);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::apply::tempfile_dir;

        /// A picture's worth of icon bytes; `apply` only names and writes them.
        const ICO: &[u8] = b"not really an icon, but apply only writes it";

        #[test]
        fn revert_leaves_icon_files_it_did_not_write() {
            let folder = tempfile_dir();
            for name in ["folderskin.ico", "folderskin-0123456789abcdef.ico"] {
                std::fs::write(folder.join(name), b"mine").unwrap();
            }
            assert!(!has_custom_icon(&folder));
            revert(&folder).unwrap();
            for name in ["folderskin.ico", "folderskin-0123456789abcdef.ico"] {
                assert_eq!(std::fs::read(folder.join(name)).unwrap(), b"mine", "{name}");
            }

            // Nor does an apply take them as leftovers of its own.
            apply(&folder, ICO).unwrap();
            revert(&folder).unwrap();
            assert!(!folder.join(ico_file_name(ICO)).exists());
            for name in ["folderskin.ico", "folderskin-0123456789abcdef.ico"] {
                assert_eq!(std::fs::read(folder.join(name)).unwrap(), b"mine", "{name}");
            }
        }

        #[test]
        fn a_folders_own_icon_and_its_ini_come_back_as_they_were() {
            let folder = tempfile_dir();
            let ini = folder.join(INI_NAME);
            let theirs = "[.ShellClassInfo]\r\nIconResource=mine.ico,0\r\nInfoTip=hi\r\n";
            std::fs::write(folder.join("mine.ico"), b"mine").unwrap();
            std::fs::write(&ini, theirs).unwrap();
            let was = FILE_ATTRIBUTE_READONLY
                | FILE_ATTRIBUTE_HIDDEN
                | FILE_ATTRIBUTE_SYSTEM
                | FILE_ATTRIBUTE_ARCHIVE;
            set_attributes(&ini, was).unwrap();
            set_attributes(&folder, FILE_ATTRIBUTE_SYSTEM).unwrap();

            apply(&folder, ICO).unwrap();
            assert!(has_custom_icon(&folder));
            revert(&folder).unwrap();

            assert_eq!(std::fs::read_to_string(&ini).unwrap(), theirs);
            assert_eq!(attributes(&ini), Some(was));
            assert_eq!(std::fs::read(folder.join("mine.ico")).unwrap(), b"mine");
            assert!(!folder.join(ico_file_name(ICO)).exists());
            let marks = attributes(&folder).unwrap() & MARKS;
            assert_eq!(marks, FILE_ATTRIBUTE_SYSTEM, "the folder's own mark stays");

            // The scratch folder's cleanup can't take a read-only file away.
            set_attributes(&ini, FILE_ATTRIBUTE_NORMAL).unwrap();
        }

        #[test]
        fn a_utf16_ini_as_windows_writes_it_is_skinned_and_comes_back_byte_for_byte() {
            let folder = tempfile_dir();
            let ini = folder.join(INI_NAME);
            let theirs = encode_ini(
                "[.ShellClassInfo]\r\nLocalizedResourceName=Мои фото\r\nIconResource=mine.ico,0\r\n",
                IniEncoding::Utf16,
            );
            assert_eq!(theirs[..2], [0xFF, 0xFE]);
            std::fs::write(folder.join("mine.ico"), b"mine").unwrap();
            std::fs::write(&ini, &theirs).unwrap();
            let was = FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;
            set_attributes(&ini, was).unwrap();
            set_attributes(&folder, FILE_ATTRIBUTE_SYSTEM).unwrap();

            apply(&folder, ICO).unwrap();
            assert!(has_custom_icon(&folder));
            let (skinned, encoding) = decode_ini(&std::fs::read(&ini).unwrap()).unwrap();
            assert_eq!(encoding, IniEncoding::Utf16, "still UTF-16");
            assert!(
                skinned.contains("LocalizedResourceName=Мои фото"),
                "{skinned}"
            );
            assert!(skinned.contains(&ico_file_name(ICO)), "{skinned}");
            assert_eq!(
                attributes(&ini).unwrap() & was,
                was,
                "hidden and system still"
            );

            revert(&folder).unwrap();
            assert_eq!(std::fs::read(&ini).unwrap(), theirs);
            assert_eq!(attributes(&ini), Some(was));
            assert!(!folder.join(ico_file_name(ICO)).exists());
            set_attributes(&ini, FILE_ATTRIBUTE_NORMAL).unwrap();
        }

        #[test]
        fn an_ini_it_wont_rewrite_is_refused_before_anything_changes() {
            let folder = tempfile_dir();
            let ini = folder.join(INI_NAME);
            // Neither UTF-8 nor UTF-16 with its mark: Latin-1, say.
            let theirs = b"[.ShellClassInfo]\r\nInfoTip=caf\xe9\r\n".to_vec();
            std::fs::write(&ini, &theirs).unwrap();
            let was = FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;
            set_attributes(&ini, was).unwrap();

            let e = apply(&folder, ICO).unwrap_err();
            assert!(matches!(e, ApplyError::Refused(_)), "{e:?}");
            assert_eq!(std::fs::read(&ini).unwrap(), theirs);
            assert_eq!(
                attributes(&ini),
                Some(was),
                "its attributes are as they were"
            );
            assert!(
                !folder.join(ico_file_name(ICO)).exists(),
                "no icon left behind"
            );
            // And revert leaves it be too, having nothing of its own to take off.
            assert!(!has_custom_icon(&folder));
            set_attributes(&ini, FILE_ATTRIBUTE_NORMAL).unwrap();
        }
    }
}
