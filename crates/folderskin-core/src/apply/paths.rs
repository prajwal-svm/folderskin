//! Folder path validation and the atomic write every writer uses.
//!
//! A folder icon is applied by putting files *inside* the user's folder (Windows, Linux) or by
//! asking the OS to attach a resource to it (macOS). Both are destructive enough that the path
//! is checked first: it must be an existing directory, and a handful of paths are refused
//! outright because skinning them is never what the user meant and undoing it is awkward.

use super::ApplyError;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The prefix `std::fs::canonicalize` puts on Windows paths.
const VERBATIM: &str = r"\\?\";
/// The prefix `std::fs::canonicalize` puts on Windows UNC paths.
const VERBATIM_UNC: &str = r"\\?\UNC\";

/// Checks that `path` is a folder FolderSkin may write to and returns its canonical form.
///
/// Refuses filesystem roots (`/`, `C:\`, `\\server\share`) and the user's home directory
/// itself — a skin on either is a mistake the user cannot easily see, let alone undo.
/// Canonicalising resolves symlinks, so the returned path is the folder that will really be
/// written to and is what the refusals are checked against.
pub fn validate_folder(path: &Path) -> Result<PathBuf, ApplyError> {
    if !path.is_dir() {
        return Err(ApplyError::NotADirectory(path.to_path_buf()));
    }
    if is_root(path) {
        return Err(ApplyError::Refused(
            "this is the root of a drive, not a folder FolderSkin can skin".into(),
        ));
    }

    let canonical = strip_verbatim(path.canonicalize()?);
    if is_root(&canonical) {
        return Err(ApplyError::Refused(
            "this is the root of a drive, not a folder FolderSkin can skin".into(),
        ));
    }
    if is_home(&canonical) {
        return Err(ApplyError::Refused(
            "this is your home folder; pick a folder inside it instead".into(),
        ));
    }
    if is_system_location(&canonical) {
        return Err(ApplyError::Refused(
            "that folder belongs to the operating system or an app; pick one of your own folders"
                .into(),
        ));
    }
    Ok(canonical)
}

/// True for folders the OS or an installed app owns: `/System`, `/Library`, `/usr`, `/bin`,
/// `/sbin`, `/private/etc`, `/private/var`, anything inside a macOS `.app` bundle, and on
/// Windows the `Windows` and `Program Files` trees.
pub fn is_system_location(path: &Path) -> bool {
    // The OS temp directory sits under one of these roots on macOS (/private/var/folders/…),
    // and a folder there is scratch space, not the system's.
    if let Ok(tmp) = std::env::temp_dir().canonicalize() {
        if path.starts_with(&tmp) {
            return false;
        }
    }

    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    let unix_roots = [
        "/system/",
        "/library/",
        "/usr/",
        "/bin/",
        "/sbin/",
        "/private/etc/",
        "/private/var/",
        "/etc/",
        "/var/",
        "/proc/",
        "/dev/",
    ];
    let with_slash = format!("{lower}/");
    if unix_roots.iter().any(|r| with_slash.starts_with(r)) {
        return true;
    }
    if with_slash.contains(".app/") {
        return true;
    }
    if let Some(rest) = lower.get(2..) {
        if lower.chars().nth(1) == Some(':') {
            let rest = format!("{rest}/");
            if [
                "/windows/",
                "/program files/",
                "/program files (x86)/",
                "/programdata/",
            ]
            .iter()
            .any(|r| rest.starts_with(r))
            {
                return true;
            }
        }
    }
    false
}

/// True when `path` is a filesystem root: `/`, a Windows drive root, or a UNC share root.
///
/// `Path::parent` is `None` exactly when a path ends in a root or a prefix, which covers all
/// three shapes without special-casing any of them.
fn is_root(path: &Path) -> bool {
    path.parent().is_none()
}

/// True when `path` is the user's home directory itself (not something inside it).
fn is_home(path: &Path) -> bool {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let Some(home) = std::env::var_os(var) else {
        return false;
    };
    let home = PathBuf::from(home);
    if home.as_os_str().is_empty() {
        return false;
    }
    // The home dir may itself be a symlink (/home -> /System/Volumes/Data/home and friends),
    // so compare canonical forms and fall back to the literal path when it cannot be resolved.
    let home = match home.canonicalize() {
        Ok(resolved) => strip_verbatim(resolved),
        Err(_) => home,
    };
    path == home
}

/// Strips the `\\?\` and `\\?\UNC\` prefixes `canonicalize` adds on Windows.
///
/// Pure, so it is unit-tested on every OS even though only the Windows build calls it.
pub fn strip_verbatim_prefix(path: &str) -> std::borrow::Cow<'_, str> {
    if let Some(rest) = path.strip_prefix(VERBATIM_UNC) {
        return std::borrow::Cow::Owned(format!(r"\\{rest}"));
    }
    std::borrow::Cow::Borrowed(path.strip_prefix(VERBATIM).unwrap_or(path))
}

/// `strip_verbatim_prefix` for a whole path. A path that is not valid UTF-8 keeps the verbatim
/// prefix: it is only cosmetic, and re-encoding the rest of the path would risk mangling it.
#[cfg(windows)]
fn strip_verbatim(path: PathBuf) -> PathBuf {
    match path.to_str() {
        Some(s) => PathBuf::from(strip_verbatim_prefix(s).into_owned()),
        None => path,
    }
}

/// No-op off Windows: `canonicalize` adds no prefix there, and a file really called `\\?\x`
/// must keep its name.
#[cfg(not(windows))]
fn strip_verbatim(path: PathBuf) -> PathBuf {
    path
}

/// Distinguishes the temp files of concurrent writes in the same folder.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` atomically: a temp file in the same folder, then a rename.
///
/// The temp file is a sibling so the rename stays within one filesystem, and it is flushed
/// before the rename so a crash leaves either the old file or the whole new one — never a
/// half-written `desktop.ini`, which Explorer would happily act on.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "folderskin".into());
    let tmp = dir.join(format!(
        ".{name}.folderskin-{}-{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    let write = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// Reads a text file that may not exist. `Ok(None)` means "no such file".
///
/// A file that is not UTF-8 is refused rather than guessed at: both `desktop.ini` and
/// `.directory` are read back, edited and written again, and a lossy round-trip through a
/// file we did not write would corrupt the user's own keys.
pub fn read_text_if_present(path: &Path) -> Result<Option<String>, ApplyError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    match String::from_utf8(bytes) {
        Ok(s) => Ok(Some(s)),
        Err(_) => Err(ApplyError::Refused(format!(
            "{} is not UTF-8 text, so FolderSkin will not rewrite it",
            path.display()
        ))),
    }
}
