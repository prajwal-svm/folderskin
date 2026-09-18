//! API keys for the AI assistant, kept in a private file in FolderSkin's own folder.
//!
//! Not the system keychain. macOS ties a keychain item to the code signature of the app that
//! saved it, and every rebuild or update of an unsigned or ad-hoc signed build (which is what
//! most people run of an open-source app) has a new one, so reading the key asks for the login
//! password again and again. A file only the user's own account can read (mode 0600) never
//! prompts; it is what `gh`, the AWS CLI and npm use for their tokens, with the same trade-off:
//! any program running as this user could read it too.
//!
//! ```text
//! <app config dir>/keys.json   {"version": 1, "keys": {"openai": "sk-…"}}
//! ```
//!
//! The file is written atomically and created with owner-only permissions from the first byte.
//! Keys never go back to the webview: it only learns whether one is saved.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

const FILE: &str = "keys.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct KeyFile {
    version: u32,
    keys: BTreeMap<String, String>,
}

#[derive(Default)]
struct Inner {
    /// Where keys are saved; `None` keeps them in memory for this session only.
    path: Option<PathBuf>,
    keys: BTreeMap<String, String>,
}

/// The saved API keys, one per provider id.
#[derive(Default)]
pub struct Keys(Mutex<Inner>);

impl Keys {
    /// Starts saving to `dir/keys.json`, loading whatever is already there. A file that can't be
    /// read is set aside rather than overwritten, so a key in it is never silently lost.
    pub fn open(&self, dir: &Path) {
        let path = dir.join(FILE);
        let keys = read(&path);
        tighten(&path);
        let mut inner = self.lock();
        inner.path = Some(path);
        inner.keys = keys;
    }

    pub fn get(&self, provider: &str) -> Option<String> {
        self.lock().keys.get(provider).cloned()
    }

    pub fn has(&self, provider: &str) -> bool {
        self.lock().keys.contains_key(provider)
    }

    pub fn set(&self, provider: &str, key: &str) -> Result<(), String> {
        let mut inner = self.lock();
        let before = inner.keys.insert(provider.to_string(), key.to_string());
        if let Err(e) = save(&inner) {
            match before {
                Some(old) => inner.keys.insert(provider.to_string(), old),
                None => inner.keys.remove(provider),
            };
            return Err(format!("couldn't save the key: {e}"));
        }
        Ok(())
    }

    pub fn clear(&self, provider: &str) -> Result<(), String> {
        let mut inner = self.lock();
        let Some(old) = inner.keys.remove(provider) else {
            return Ok(());
        };
        if let Err(e) = save(&inner) {
            inner.keys.insert(provider.to_string(), old);
            return Err(format!("couldn't remove the key: {e}"));
        }
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn read(path: &Path) -> BTreeMap<String, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return BTreeMap::new(),
        Err(e) => {
            eprintln!("folderskin: couldn't read {}: {e}", path.display());
            return BTreeMap::new();
        }
    };
    match serde_json::from_slice::<KeyFile>(&bytes) {
        Ok(file) => file.keys,
        Err(e) => {
            let aside =
                path.with_file_name(format!("keys-unreadable-{}.json", crate::store::now_ms()));
            eprintln!(
                "folderskin: {} is damaged ({e}); moved it to {}",
                path.display(),
                aside.display()
            );
            let _ = std::fs::rename(path, &aside);
            BTreeMap::new()
        }
    }
}

fn save(inner: &Inner) -> std::io::Result<()> {
    let Some(path) = &inner.path else {
        return Ok(()); // no app folder: keys last for this session only
    };
    let text = serde_json::to_vec_pretty(&KeyFile {
        version: VERSION,
        keys: inner.keys.clone(),
    })?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_private(path, &text)
}

/// Distinguishes the temp files of concurrent writes.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` atomically, in a file only the owner can read from the moment it is
/// created (not chmod-ed afterwards, which would leave a readable window).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let tmp = path.with_file_name(format!(
        ".keys.json.{}-{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let write = || -> std::io::Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    };
    if let Err(e) = write().and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// Makes an existing key file owner-only, in case it was copied in with wider permissions.
fn tighten(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.permissions().mode() & 0o077 != 0 {
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("folderskin-keys-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_saved_key_comes_back_after_reopening() {
        let dir = temp_dir("roundtrip");
        let keys = Keys::default();
        keys.open(&dir);
        assert!(!keys.has("openai"));
        keys.set("openai", "sk-test-123").unwrap();
        keys.set("xai", "xai-test-456").unwrap();

        let again = Keys::default();
        again.open(&dir);
        assert_eq!(again.get("openai").as_deref(), Some("sk-test-123"));
        assert!(again.has("xai"));

        again.clear("openai").unwrap();
        let third = Keys::default();
        third.open(&dir);
        assert!(!third.has("openai"));
        assert!(third.has("xai"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("mode");
        let keys = Keys::default();
        keys.open(&dir);
        keys.set("openai", "sk-test").unwrap();
        let mode = std::fs::metadata(dir.join(FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        // A copy with wider permissions is tightened when it is opened.
        std::fs::set_permissions(dir.join(FILE), std::fs::Permissions::from_mode(0o644)).unwrap();
        Keys::default().open(&dir);
        let mode = std::fs::metadata(dir.join(FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = temp_dir("damaged");
        std::fs::write(dir.join(FILE), b"{ not json").unwrap();
        let keys = Keys::default();
        keys.open(&dir);
        assert!(!keys.has("openai"));
        let aside = std::fs::read_dir(&dir).unwrap().flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("keys-unreadable-")
        });
        assert!(aside, "the damaged file should be kept under another name");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_a_folder_keys_last_for_the_session() {
        let keys = Keys::default();
        keys.set("openai", "sk-memory").unwrap();
        assert_eq!(keys.get("openai").as_deref(), Some("sk-memory"));
        keys.clear("openai").unwrap();
        assert!(!keys.has("openai"));
    }
}
