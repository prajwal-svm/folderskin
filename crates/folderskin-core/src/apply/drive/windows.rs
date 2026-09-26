//! A drive letter's icon on Windows: a per-user registry key that points Explorer at an icon file
//! FolderSkin keeps in its own data folder.
//!
//! Explorer looks a drive's icon up by its letter, under
//! `HKEY_CURRENT_USER\Software\Classes\Applications\Explorer.exe\Drives\<letter>\DefaultIcon`,
//! before it draws the icon its type would get. The key is the user's own, so no administrator is
//! asked for, and nothing is written to the drive itself: the icon works for a fixed disk, a USB
//! stick, a disc and a mapped network drive alike. It belongs to the letter, not the disk, which is
//! the one thing to know about it (docs/DRIVES.md).
//!
//! The icon file is named after the letter and its own contents, as a folder's icon is
//! ([`super::super::windows::ico_file_name`]): Explorer caches an icon against the path it came
//! from, so a new skin has to be a new path to show at once.
//!
//! What the key held before FolderSkin wrote to it is kept beside FolderSkin's value and put back
//! by a revert, and a revert only ever takes off a value naming FolderSkin's own icon file. That
//! logic is pure, over [`DriveKeys`], so it is tested against a stand-in registry on any computer;
//! only the reading and writing of the real one is Windows'.

use crate::apply::ApplyError;

/// Where under `HKEY_CURRENT_USER` Explorer looks up drive letters' icons.
pub const DRIVES_KEY: &str = r"Software\Classes\Applications\Explorer.exe\Drives";
/// The value, beside FolderSkin's, that keeps what the key held before.
pub const KEPT_VALUE: &str = "FolderSkinBefore";
/// The folder in `%APPDATA%` FolderSkin's data is kept in: the app's identifier, as Tauri names
/// its data folder, so the app and the command line keep drive icons in the same place.
pub const APP_DIR: &str = "app.folderskin.desktop";
/// The folder in it the drive icons are kept in.
pub const ICONS_DIR: &str = "drive-icons";

/// The key whose default value names drive `letter`'s icon.
pub fn icon_key(letter: char) -> String {
    format!(r"{DRIVES_KEY}\{}\DefaultIcon", letter.to_ascii_uppercase())
}

/// The key of drive `letter` itself, which holds the icon's key.
pub fn letter_key(letter: char) -> String {
    format!(r"{DRIVES_KEY}\{}", letter.to_ascii_uppercase())
}

/// The name of drive `letter`'s icon file holding `ico_bytes`: `folderskin-drive-E-<hash>.ico`.
pub fn ico_file_name(letter: char, ico_bytes: &[u8]) -> String {
    let hashed = super::super::windows::ico_file_name(ico_bytes);
    let hash = hashed
        .strip_prefix("folderskin-")
        .unwrap_or(&hashed)
        .trim_end_matches(".ico");
    format!(
        "folderskin-drive-{}-{hash}.ico",
        letter.to_ascii_uppercase()
    )
}

/// True for the name of an icon file FolderSkin keeps for drive `letter`.
pub fn is_our_file_for(letter: char, name: &str) -> bool {
    parse_file_name(name).is_some_and(|l| l == letter.to_ascii_uppercase())
}

/// The drive letter an icon file FolderSkin keeps is for, from its name; `None` for any other
/// name. Strict, since a revert deletes what this matches.
fn parse_file_name(name: &str) -> Option<char> {
    let lower = name.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("folderskin-drive-")?
        .strip_suffix(".ico")?;
    let (letter, hash) = rest.split_once('-')?;
    let mut chars = letter.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    let whole =
        chars.next().is_none() && hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit());
    whole.then(|| letter.to_ascii_uppercase())
}

/// The file an icon value names, without the quotes or the `,index` Explorer allows after it.
pub fn icon_path_of(value: &str) -> &str {
    icon_location(value).0
}

/// The file an icon value names and the icon's index in it: `0` when it says none, as for an
/// `.ico` file.
pub fn icon_location(value: &str) -> (&str, i32) {
    let value = value.trim();
    let (path, index) = match value.rsplit_once(',') {
        Some((path, index)) => match index.trim().parse::<i32>() {
            Ok(index) => (path, index),
            Err(_) => (value, 0),
        },
        None => (value, 0),
    };
    (path.trim().trim_matches('"'), index)
}

/// True when an icon value names an icon file FolderSkin keeps for a drive.
pub fn is_ours(value: &str) -> bool {
    let path = icon_path_of(value);
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    parse_file_name(name).is_some()
}

/// The part of the registry a drive letter's icon lives in, as [`put_on`] and [`take_off`] use it.
pub trait DriveKeys {
    /// The icon key's default value: the icon Explorer draws for the letter.
    fn icon(&self, letter: char) -> Option<String>;
    /// What the icon key held before FolderSkin, kept in [`KEPT_VALUE`].
    fn kept(&self, letter: char) -> Option<String>;
    /// Sets the icon key's default value, making the keys it needs.
    fn set_icon(&mut self, letter: char, value: &str) -> Result<(), ApplyError>;
    fn set_kept(&mut self, letter: char, value: &str) -> Result<(), ApplyError>;
    fn clear_kept(&mut self, letter: char) -> Result<(), ApplyError>;
    /// Deletes the icon key, and the letter's key when nothing else is left in it.
    fn remove_icon_key(&mut self, letter: char) -> Result<(), ApplyError>;
}

/// Points drive `letter`'s icon at the file `ico_path`. An icon the key named before, anyone's
/// but FolderSkin's, is kept aside for [`take_off`] to put back, once: applying again keeps the
/// first one kept.
pub fn put_on(keys: &mut impl DriveKeys, letter: char, ico_path: &str) -> Result<(), ApplyError> {
    if let Some(current) = keys.icon(letter) {
        if !is_ours(&current) && keys.kept(letter).is_none() {
            keys.set_kept(letter, &current)?;
        }
    }
    keys.set_icon(letter, ico_path)
}

/// Takes FolderSkin's icon off drive `letter`: puts back the icon it replaced, or when there was
/// none, deletes the key it made. An icon that isn't FolderSkin's is left as it is. Says whether
/// anything changed.
pub fn take_off(keys: &mut impl DriveKeys, letter: char) -> Result<bool, ApplyError> {
    match keys.icon(letter) {
        Some(current) if is_ours(&current) => {
            match keys.kept(letter) {
                Some(before) => {
                    keys.set_icon(letter, &before)?;
                    keys.clear_kept(letter)?;
                }
                None => keys.remove_icon_key(letter)?,
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// True when drive `letter` wears FolderSkin's icon, which [`take_off`] would take off.
pub fn wears_ours(keys: &impl DriveKeys, letter: char) -> bool {
    keys.icon(letter).is_some_and(|v| is_ours(&v))
}

#[cfg(windows)]
pub use imp::{apply, current_icon, has_custom_icon, revert};

#[cfg(windows)]
mod imp {
    use super::{
        ico_file_name, icon_key, is_our_file_for, letter_key, put_on, take_off, wears_ours,
        DriveKeys, APP_DIR, ICONS_DIR, KEPT_VALUE,
    };
    use crate::apply::paths::write_atomic;
    use crate::apply::ApplyError;
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW, RegOpenKeyExW,
        RegQueryInfoKeyW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_QUERY_VALUE, KEY_READ, KEY_SET_VALUE, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    /// The folder FolderSkin keeps drive icons in: `%APPDATA%\app.folderskin.desktop\drive-icons`.
    fn icons_dir() -> Result<PathBuf, ApplyError> {
        let app_data = std::env::var_os("APPDATA")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                ApplyError::Platform("Windows didn't say where your app data is kept".into())
            })?;
        Ok(PathBuf::from(app_data).join(APP_DIR).join(ICONS_DIR))
    }

    /// Writes the icon file for drive `letter` and points its registry key at it. The letter's
    /// older icon files go once the key names the new one.
    pub fn apply(letter: char, ico_bytes: &[u8]) -> Result<(), ApplyError> {
        let dir = icons_dir()?;
        std::fs::create_dir_all(&dir)?;
        let name = ico_file_name(letter, ico_bytes);
        let path = dir.join(&name);
        write_atomic(&path, ico_bytes)?;
        put_on(&mut Registry, letter, &path.to_string_lossy()).inspect_err(|_| {
            let _ = std::fs::remove_file(&path);
        })?;
        remove_files(letter, Some(&name));
        Ok(())
    }

    /// Takes FolderSkin's icon off drive `letter` and deletes its icon files.
    pub fn revert(letter: char) -> Result<(), ApplyError> {
        if take_off(&mut Registry, letter)? {
            remove_files(letter, None);
        }
        Ok(())
    }

    /// True when drive `letter` wears FolderSkin's icon.
    pub fn has_custom_icon(letter: char) -> bool {
        wears_ours(&Registry, letter)
    }

    /// The icon drive `letter` wears from the registry, FolderSkin's or anyone's, as its value
    /// says it: a file and perhaps an index.
    pub fn current_icon(letter: char) -> Option<String> {
        Registry.icon(letter)
    }

    /// Deletes drive `letter`'s icon files but `keep`.
    fn remove_files(letter: char, keep: Option<&str>) {
        let Ok(dir) = icons_dir() else { return };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_our_file_for(letter, &name) && Some(name.as_str()) != keep {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// The real registry, under `HKEY_CURRENT_USER`.
    struct Registry;

    /// An open key, closed when dropped.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by RegOpenKeyExW or RegCreateKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// What Windows said went wrong, in its own words.
    fn os_error(status: WIN32_ERROR) -> std::io::Error {
        std::io::Error::from_raw_os_error(status as i32)
    }

    fn not_saved(status: WIN32_ERROR) -> ApplyError {
        ApplyError::Platform(format!(
            "Windows couldn't save the drive's icon in the registry ({})",
            os_error(status)
        ))
    }

    fn not_taken_out(status: WIN32_ERROR) -> ApplyError {
        ApplyError::Platform(format!(
            "Windows couldn't take the drive's icon out of the registry ({})",
            os_error(status)
        ))
    }

    fn open(path: &str, access: u32) -> Option<Key> {
        let path = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `path` is NUL-terminated and `key` is written by a successful call.
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) };
        (status == ERROR_SUCCESS).then_some(Key(key))
    }

    fn create(path: &str) -> Result<Key, ApplyError> {
        let wide_path = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `wide_path` is NUL-terminated, and `key` is written by a successful call.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide_path.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_QUERY_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(not_saved(status));
        }
        Ok(Key(key))
    }

    /// A string value of `key`, the default one for `None`.
    fn read(key: &Key, name: Option<&str>) -> Option<String> {
        let name = name.map(wide);
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        let mut kind = 0u32;
        let mut size = 0u32;
        // SAFETY: a null data pointer only asks for the type and the size.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name_ptr,
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) {
            return None;
        }
        let mut data = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut size = (data.len() * 2) as u32;
        // SAFETY: `data` is writable for `size` bytes.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name_ptr,
                std::ptr::null(),
                &mut kind,
                data.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
        Some(String::from_utf16_lossy(&data[..end]))
    }

    fn write(key: &Key, name: Option<&str>, value: &str) -> Result<(), ApplyError> {
        let name = name.map(wide);
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        let data = wide(value);
        // SAFETY: `data` is a NUL-terminated UTF-16 string of the size given, in bytes.
        let status = unsafe {
            RegSetValueExW(
                key.0,
                name_ptr,
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(not_saved(status));
        }
        Ok(())
    }

    /// True when `key` holds no values and no keys.
    fn is_empty(key: &Key) -> bool {
        let (mut keys, mut values) = (0u32, 0u32);
        // SAFETY: only the two counts are asked for; every other pointer is null.
        let status = unsafe {
            RegQueryInfoKeyW(
                key.0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                &mut keys,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut values,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        status == ERROR_SUCCESS && keys == 0 && values == 0
    }

    fn delete_key(path: &str) -> Result<(), ApplyError> {
        let path = wide(path);
        // SAFETY: `path` is NUL-terminated.
        let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, path.as_ptr()) };
        if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
            return Err(not_taken_out(status));
        }
        Ok(())
    }

    impl DriveKeys for Registry {
        fn icon(&self, letter: char) -> Option<String> {
            read(&open(&icon_key(letter), KEY_READ)?, None)
        }

        fn kept(&self, letter: char) -> Option<String> {
            read(&open(&icon_key(letter), KEY_READ)?, Some(KEPT_VALUE))
        }

        fn set_icon(&mut self, letter: char, value: &str) -> Result<(), ApplyError> {
            write(&create(&icon_key(letter))?, None, value)
        }

        fn set_kept(&mut self, letter: char, value: &str) -> Result<(), ApplyError> {
            write(&create(&icon_key(letter))?, Some(KEPT_VALUE), value)
        }

        fn clear_kept(&mut self, letter: char) -> Result<(), ApplyError> {
            let Some(key) = open(&icon_key(letter), KEY_SET_VALUE) else {
                return Ok(());
            };
            let name = wide(KEPT_VALUE);
            // SAFETY: `name` is NUL-terminated and the key is open for setting values.
            let status = unsafe { RegDeleteValueW(key.0, name.as_ptr()) };
            if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
                return Err(not_taken_out(status));
            }
            Ok(())
        }

        fn remove_icon_key(&mut self, letter: char) -> Result<(), ApplyError> {
            delete_key(&icon_key(letter))?;
            if open(&letter_key(letter), KEY_READ).is_some_and(|key| is_empty(&key)) {
                delete_key(&letter_key(letter))?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A stand-in for the registry: each letter's icon key's default value and kept value.
    #[derive(Default)]
    struct Fake {
        icons: HashMap<char, String>,
        kept: HashMap<char, String>,
        /// Letters whose key is there, with or without values.
        keys: Vec<char>,
    }

    impl DriveKeys for Fake {
        fn icon(&self, letter: char) -> Option<String> {
            self.icons.get(&letter).cloned()
        }
        fn kept(&self, letter: char) -> Option<String> {
            self.kept.get(&letter).cloned()
        }
        fn set_icon(&mut self, letter: char, value: &str) -> Result<(), ApplyError> {
            if !self.keys.contains(&letter) {
                self.keys.push(letter);
            }
            self.icons.insert(letter, value.into());
            Ok(())
        }
        fn set_kept(&mut self, letter: char, value: &str) -> Result<(), ApplyError> {
            self.kept.insert(letter, value.into());
            Ok(())
        }
        fn clear_kept(&mut self, letter: char) -> Result<(), ApplyError> {
            self.kept.remove(&letter);
            Ok(())
        }
        fn remove_icon_key(&mut self, letter: char) -> Result<(), ApplyError> {
            self.icons.remove(&letter);
            self.kept.remove(&letter);
            self.keys.retain(|&l| l != letter);
            Ok(())
        }
    }

    const OURS: &str = r"C:\Users\Ana Lima\AppData\Roaming\app.folderskin.desktop\drive-icons\folderskin-drive-E-0123456789abcdef.ico";
    const NEWER: &str = r"C:\Users\Ana Lima\AppData\Roaming\app.folderskin.desktop\drive-icons\folderskin-drive-E-fedcba9876543210.ico";

    #[test]
    fn a_drive_icon_is_named_after_its_letter_and_contents() {
        let name = ico_file_name('e', b"an icon");
        assert!(name.starts_with("folderskin-drive-E-"), "{name}");
        assert!(name.ends_with(".ico"), "{name}");
        assert_eq!(
            name,
            ico_file_name('E', b"an icon"),
            "the same icon, the same file"
        );
        assert_ne!(name, ico_file_name('E', b"another icon"));
        assert!(is_our_file_for('E', &name));
        assert!(
            !is_our_file_for('F', &name),
            "another letter's icon is left alone"
        );
        for other in [
            "folderskin.ico",
            "folderskin-0123456789abcdef.ico",
            "folderskin-drive-E-xyz.ico",
            "folderskin-drive-EE-0123456789abcdef.ico",
            "drive.ico",
        ] {
            assert!(!is_our_file_for('E', other), "{other}");
        }
        assert_eq!(
            icon_key('e'),
            r"Software\Classes\Applications\Explorer.exe\Drives\E\DefaultIcon"
        );
    }

    #[test]
    fn an_icon_value_is_read_as_explorer_reads_it() {
        assert_eq!(icon_path_of(r"C:\Icons\drive.ico"), r"C:\Icons\drive.ico");
        assert_eq!(
            icon_path_of(r#""C:\My Icons\drive.ico",0"#),
            r"C:\My Icons\drive.ico"
        );
        assert_eq!(
            icon_path_of(r"%SystemRoot%\system32\imageres.dll,-30"),
            r"%SystemRoot%\system32\imageres.dll"
        );
        assert_eq!(
            icon_location(r"C:\Icons\drive.ico"),
            (r"C:\Icons\drive.ico", 0)
        );
        assert_eq!(icon_location("imageres.dll,-30"), ("imageres.dll", -30));
        assert_eq!(
            icon_location(r"C:\Odd, Name\drive.ico"),
            (r"C:\Odd, Name\drive.ico", 0)
        );
        assert!(is_ours(OURS));
        assert!(is_ours(&format!("\"{OURS}\",0")));
        assert!(!is_ours(r"C:\Icons\drive.ico"));
        assert!(!is_ours(r"C:\Icons\folderskin-drive-E.ico"));
    }

    #[test]
    fn a_drive_with_no_icon_of_its_own_gets_one_and_loses_it_again() {
        let mut keys = Fake::default();
        put_on(&mut keys, 'E', OURS).unwrap();
        assert_eq!(keys.icon('E').as_deref(), Some(OURS));
        assert!(keys.kept('E').is_none(), "there was nothing to keep");
        assert!(wears_ours(&keys, 'E'));

        // A second skin over the first replaces it, and still keeps nothing.
        put_on(&mut keys, 'E', NEWER).unwrap();
        assert_eq!(keys.icon('E').as_deref(), Some(NEWER));
        assert!(keys.kept('E').is_none());

        assert!(take_off(&mut keys, 'E').unwrap());
        assert!(keys.icon('E').is_none());
        assert!(keys.keys.is_empty(), "the key FolderSkin made is gone");
        assert!(!wears_ours(&keys, 'E'));
        assert!(
            !take_off(&mut keys, 'E').unwrap(),
            "nothing left to take off"
        );
    }

    #[test]
    fn a_drive_that_had_an_icon_gets_it_back() {
        let mut keys = Fake::default();
        let theirs = r"D:\Icons\photos.ico,0";
        keys.set_icon('D', theirs).unwrap();
        assert!(
            !wears_ours(&keys, 'D'),
            "someone else's icon isn't FolderSkin's to take off"
        );
        assert!(!take_off(&mut keys, 'D').unwrap());
        assert_eq!(keys.icon('D').as_deref(), Some(theirs));

        put_on(&mut keys, 'D', OURS).unwrap();
        put_on(&mut keys, 'D', NEWER).unwrap();
        assert_eq!(
            keys.kept('D').as_deref(),
            Some(theirs),
            "kept once, the first time"
        );

        assert!(take_off(&mut keys, 'D').unwrap());
        assert_eq!(keys.icon('D').as_deref(), Some(theirs));
        assert!(keys.kept('D').is_none());
        assert!(keys.keys.contains(&'D'), "their key stays");
    }
}
