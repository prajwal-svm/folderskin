//! API keys for the AI assistant, encrypted in FolderSkin's own folder.
//!
//! Not the system keychain. macOS ties a keychain item to the code signature of the app that
//! saved it, and every rebuild or update of an unsigned or ad-hoc signed build (which is what
//! most people run of an open-source app) has a new one, so reading the key asks for the login
//! password again and again.
//!
//! Instead the keys are sealed with AES-256-GCM before they reach the disk. The encryption key is
//! derived (HKDF-SHA256) from a random secret made the first time a key is saved, kept in a file
//! of its own, and from this computer's hardware id. The key file alone gives nothing away, and
//! the two files copied to another computer don't open there. Both files can only be read by the
//! user's own account (mode 0600). A program running as this user could still read both and the
//! id; only an OS keystore, with its prompts, would stop that.
//!
//! ```text
//! <app config dir>/keys.json     {"version": 2, "bound": true, "nonce": "…", "sealed": "…"}
//! <app config dir>/keys.secret   32 random bytes
//! ```
//!
//! Both are written atomically and created owner-only from the first byte. A version 1 file, which
//! held the keys as plain text, is sealed the first time it is opened. A file that can't be opened
//! (damaged, its secret gone, sealed on another computer) is set aside rather than overwritten,
//! and its keys have to be entered again. Keys never go back to the webview: it only learns
//! whether one is saved.

use aws_lc_rs::aead::{Aad, Nonce, RandomizedNonceKey, AES_256_GCM};
use aws_lc_rs::hkdf::{Salt, HKDF_SHA256};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

const FILE: &str = "keys.json";
const SECRET_FILE: &str = "keys.secret";
/// Version 1 held the keys as plain text; version 2 seals them.
const VERSION: u32 = 2;
const SECRET_LEN: usize = 32;
/// What the encryption key is for, mixed into its derivation.
const INFO: &[u8] = b"folderskin api keys";
/// Authenticated with every sealed file, so one can't be passed off as anything else.
const AAD: &[u8] = b"folderskin api keys v2";

type KeyMap = BTreeMap<String, String>;

/// The file as version 2 writes it.
#[derive(Serialize, Deserialize)]
struct SealedFile {
    version: u32,
    /// Whether this computer's id is part of the encryption key. It isn't when the id couldn't be
    /// read, so such a file still opens later on the same computer.
    bound: bool,
    nonce: String,
    sealed: String,
}

/// The file as version 1 wrote it, keys in plain text.
#[derive(Deserialize)]
struct PlainFile {
    keys: KeyMap,
}

#[derive(Default)]
struct Inner {
    /// Where keys are saved; `None` keeps them in memory for this session only.
    path: Option<PathBuf>,
    /// This computer's id, when it could be read; part of the encryption key.
    machine: Option<String>,
    keys: KeyMap,
}

/// The saved API keys, one per provider id.
#[derive(Default)]
pub struct Keys(Mutex<Inner>);

impl Keys {
    /// Starts saving to `dir/keys.json`, loading whatever is already there. A file that can't be
    /// opened is set aside rather than overwritten, so a key in it is never silently lost.
    pub fn open(&self, dir: &Path) {
        self.open_on(dir, machine_id());
    }

    /// [`Keys::open`] on a computer whose id is `machine`.
    fn open_on(&self, dir: &Path, machine: Option<String>) {
        let path = dir.join(FILE);
        let (keys, plain) = read(&path, machine.as_deref());
        tighten(&path);
        tighten(&dir.join(SECRET_FILE));
        let mut inner = self.lock();
        inner.path = Some(path);
        inner.machine = machine;
        inner.keys = keys;
        if plain {
            // Keys saved before they were encrypted: seal them now, so no plain copy stays.
            if let Err(e) = save(&inner) {
                eprintln!("folderskin: couldn't encrypt the saved API keys: {e}");
            }
        }
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

/// The keys in `path`, and whether they were in plain text. Anything that can't be opened is set
/// aside and comes back empty.
fn read(path: &Path, machine: Option<&str>) -> (KeyMap, bool) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (KeyMap::new(), false),
        Err(e) => {
            eprintln!("folderskin: couldn't read {}: {e}", path.display());
            return (KeyMap::new(), false);
        }
    };
    let opened = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| e.to_string())
        .and_then(
            |value| match value.get("version").and_then(|v| v.as_u64()) {
                Some(1) => serde_json::from_value::<PlainFile>(value)
                    .map(|file| (file.keys, true))
                    .map_err(|e| e.to_string()),
                Some(2) => serde_json::from_value::<SealedFile>(value)
                    .map_err(|e| e.to_string())
                    .and_then(|file| unseal(path, &file, machine))
                    .map(|keys| (keys, false)),
                other => Err(format!("unknown version {other:?}")),
            },
        );
    opened.unwrap_or_else(|why| {
        set_aside(path, &why);
        (KeyMap::new(), false)
    })
}

/// Opens a sealed file, or says why it can't be.
fn unseal(path: &Path, file: &SealedFile, machine: Option<&str>) -> Result<KeyMap, String> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let secret = read_secret(dir)?.ok_or("its secret is missing")?;
    let machine = if file.bound {
        Some(machine.ok_or("this computer's id can't be read")?)
    } else {
        None
    };
    let key = cipher(&secret, machine).map_err(|_| "the key couldn't be derived")?;
    let nonce = B64
        .decode(&file.nonce)
        .ok()
        .and_then(|n| Nonce::try_assume_unique_for_key(&n).ok())
        .ok_or("its nonce is damaged")?;
    let mut data = B64
        .decode(&file.sealed)
        .map_err(|_| "its contents are damaged")?;
    let plain = key
        .open_in_place(nonce, Aad::from(AAD), &mut data)
        .map_err(|_| {
            "it was sealed with another secret or on another computer, or changed since"
        })?;
    serde_json::from_slice(plain).map_err(|e| e.to_string())
}

fn save(inner: &Inner) -> std::io::Result<()> {
    let Some(path) = &inner.path else {
        return Ok(()); // no app folder: keys last for this session only
    };
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let secret = secret(dir)?;
    let failed = |_| std::io::Error::other("the keys couldn't be encrypted");
    let key = cipher(&secret, inner.machine.as_deref()).map_err(failed)?;
    let mut data = serde_json::to_vec(&inner.keys)?;
    let nonce = key
        .seal_in_place_append_tag(Aad::from(AAD), &mut data)
        .map_err(failed)?;
    let text = serde_json::to_vec_pretty(&SealedFile {
        version: VERSION,
        bound: inner.machine.is_some(),
        nonce: B64.encode(nonce.as_ref()),
        sealed: B64.encode(&data),
    })?;
    write_private(path, &text)
}

/// The key the keys are sealed with: HKDF-SHA256 of the secret, salted with this computer's id.
fn cipher(
    secret: &[u8; SECRET_LEN],
    machine: Option<&str>,
) -> Result<RandomizedNonceKey, aws_lc_rs::error::Unspecified> {
    let salt = format!("folderskin:{}", machine.unwrap_or(""));
    let mut key = [0u8; 32];
    Salt::new(HKDF_SHA256, salt.as_bytes())
        .extract(secret)
        .expand(&[INFO], &AES_256_GCM)?
        .fill(&mut key)?;
    RandomizedNonceKey::new(&AES_256_GCM, &key)
}

/// The secret in `dir`: `None` when there is none yet, an error when it is damaged.
fn read_secret(dir: &Path) -> Result<Option<[u8; SECRET_LEN]>, String> {
    match std::fs::read(dir.join(SECRET_FILE)) {
        Ok(bytes) => <[u8; SECRET_LEN]>::try_from(bytes.as_slice())
            .map(Some)
            .map_err(|_| "its secret is damaged".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("its secret can't be read: {e}")),
    }
}

/// The secret in `dir`, made (and saved owner-only) the first time. A damaged one is set aside
/// and replaced; whatever it sealed was already set aside when it failed to open.
fn secret(dir: &Path) -> std::io::Result<[u8; SECRET_LEN]> {
    match read_secret(dir) {
        Ok(Some(secret)) => return Ok(secret),
        Ok(None) => {}
        Err(why) => set_aside(&dir.join(SECRET_FILE), &why),
    }
    let mut secret = [0u8; SECRET_LEN];
    aws_lc_rs::rand::fill(&mut secret)
        .map_err(|_| std::io::Error::other("no random numbers for the secret"))?;
    write_private(&dir.join(SECRET_FILE), &secret)?;
    Ok(secret)
}

/// Moves a file that can't be used out of the way, so the next save doesn't destroy it.
fn set_aside(path: &Path, why: &str) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "keys".into());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let aside = path.with_file_name(format!("{stem}-unreadable-{}{ext}", crate::store::now_ms()));
    eprintln!(
        "folderskin: {} can't be opened ({why}); moved it to {}",
        path.display(),
        aside.display()
    );
    let _ = std::fs::rename(path, &aside);
}

/// Something that identifies this computer and survives restarts: the hardware UUID on macOS,
/// the machine id on Linux, the machine GUID on Windows. `None` when it can't be read.
fn machine_id() -> Option<String> {
    let id = platform_machine_id()?;
    let id = id.trim().to_string();
    (!id.is_empty()).then_some(id)
}

#[cfg(target_os = "macos")]
fn platform_machine_id() -> Option<String> {
    let out = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    // `    "IOPlatformUUID" = "0A1B2C3D-…"`
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|line| line.contains("\"IOPlatformUUID\""))
        .and_then(|line| line.split('"').nth(3))
        .map(str::to_string)
}

#[cfg(target_os = "linux")]
fn platform_machine_id() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

#[cfg(windows)]
fn platform_machine_id() -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
    };
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let key = wide(r"SOFTWARE\Microsoft\Cryptography");
    let value = wide("MachineGuid");
    let mut buf = [0u16; 128];
    let mut bytes = std::mem::size_of_val(&buf) as u32;
    // SAFETY: the names are null-terminated, and `bytes` is the size of `buf` in bytes, which
    // RegGetValueW writes no further than and sets to what it wrote.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 {
        return None;
    }
    let len = (bytes as usize / 2).min(buf.len());
    Some(
        String::from_utf16_lossy(&buf[..len])
            .trim_end_matches('\0')
            .to_string(),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn platform_machine_id() -> Option<String> {
    None
}

/// Distinguishes the temp files of concurrent writes.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` atomically, in a file only the owner can read from the moment it is
/// created (not chmod-ed afterwards, which would leave a readable window).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE.into());
    let tmp = path.with_file_name(format!(
        ".{name}.{}-{}.tmp",
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

/// Makes an existing file owner-only, in case it was copied in with wider permissions.
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

    const HERE: &str = "machine-here";
    const ELSEWHERE: &str = "machine-elsewhere";

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("folderskin-keys-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn opened(dir: &Path, machine: Option<&str>) -> Keys {
        let keys = Keys::default();
        keys.open_on(dir, machine.map(str::to_string));
        keys
    }

    fn set_aside_files(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("-unreadable-"))
            .count()
    }

    #[test]
    fn a_saved_key_comes_back_after_reopening() {
        let dir = temp_dir("roundtrip");
        let keys = opened(&dir, Some(HERE));
        assert!(!keys.has("openai"));
        keys.set("openai", "sk-test-123").unwrap();
        keys.set("xai", "xai-test-456").unwrap();

        let again = opened(&dir, Some(HERE));
        assert_eq!(again.get("openai").as_deref(), Some("sk-test-123"));
        assert!(again.has("xai"));

        again.clear("openai").unwrap();
        let third = opened(&dir, Some(HERE));
        assert!(!third.has("openai"));
        assert!(third.has("xai"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keys_are_encrypted_on_disk() {
        let dir = temp_dir("sealed");
        opened(&dir, Some(HERE))
            .set("openai", "sk-secret-value")
            .unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(!text.contains("sk-secret-value"), "{text}");
        assert!(!text.contains("openai"), "not even which providers");
        let file: SealedFile = serde_json::from_str(&text).unwrap();
        assert_eq!((file.version, file.bound), (2, true));
        assert_eq!(std::fs::read(dir.join(SECRET_FILE)).unwrap().len(), 32);

        // Every save seals with a fresh nonce.
        opened(&dir, Some(HERE)).set("xai", "xai-other").unwrap();
        let again: SealedFile =
            serde_json::from_slice(&std::fs::read(dir.join(FILE)).unwrap()).unwrap();
        assert_ne!(again.nonce, file.nonce);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plain_text_keys_are_encrypted_when_opened() {
        let dir = temp_dir("migrate");
        std::fs::write(
            dir.join(FILE),
            r#"{"version": 1, "keys": {"openai": "sk-old-plain"}}"#,
        )
        .unwrap();
        let keys = opened(&dir, Some(HERE));
        assert_eq!(keys.get("openai").as_deref(), Some("sk-old-plain"));
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(!text.contains("sk-old-plain"), "{text}");
        assert_eq!(
            opened(&dir, Some(HERE)).get("openai").as_deref(),
            Some("sk-old-plain")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn they_dont_open_on_another_computer_and_are_set_aside() {
        let dir = temp_dir("elsewhere");
        opened(&dir, Some(HERE)).set("openai", "sk-here").unwrap();

        let elsewhere = opened(&dir, Some(ELSEWHERE));
        assert!(!elsewhere.has("openai"));
        assert_eq!(set_aside_files(&dir), 1, "kept, not overwritten");
        // Keys saved there from now on work there.
        elsewhere.set("openai", "sk-elsewhere").unwrap();
        assert_eq!(
            opened(&dir, Some(ELSEWHERE)).get("openai").as_deref(),
            Some("sk-elsewhere")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_their_secret_they_dont_open() {
        let dir = temp_dir("secret");
        opened(&dir, Some(HERE)).set("openai", "sk-here").unwrap();
        std::fs::remove_file(dir.join(SECRET_FILE)).unwrap();
        assert!(!opened(&dir, Some(HERE)).has("openai"));
        assert_eq!(set_aside_files(&dir), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_changed_file_doesnt_open() {
        let dir = temp_dir("tampered");
        opened(&dir, Some(HERE)).set("openai", "sk-here").unwrap();
        let mut file: SealedFile =
            serde_json::from_slice(&std::fs::read(dir.join(FILE)).unwrap()).unwrap();
        let mut sealed = B64.decode(&file.sealed).unwrap();
        sealed[0] ^= 1;
        file.sealed = B64.encode(&sealed);
        std::fs::write(dir.join(FILE), serde_json::to_vec(&file).unwrap()).unwrap();
        assert!(!opened(&dir, Some(HERE)).has("openai"));
        assert_eq!(set_aside_files(&dir), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_a_computer_id_they_still_open_on_the_same_computer() {
        let dir = temp_dir("unbound");
        opened(&dir, None).set("openai", "sk-unbound").unwrap();
        let file: SealedFile =
            serde_json::from_slice(&std::fs::read(dir.join(FILE)).unwrap()).unwrap();
        assert!(!file.bound);
        assert_eq!(
            opened(&dir, None).get("openai").as_deref(),
            Some("sk-unbound")
        );
        // An id that becomes readable later doesn't lock them out: the file says it isn't bound.
        assert_eq!(
            opened(&dir, Some(HERE)).get("openai").as_deref(),
            Some("sk-unbound")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_key_files_are_readable_by_their_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        let dir = temp_dir("mode");
        opened(&dir, Some(HERE)).set("openai", "sk-test").unwrap();
        assert_eq!(mode(&dir.join(FILE)), 0o600);
        assert_eq!(mode(&dir.join(SECRET_FILE)), 0o600);

        // Copies with wider permissions are tightened when they are opened.
        for file in [FILE, SECRET_FILE] {
            std::fs::set_permissions(dir.join(file), std::fs::Permissions::from_mode(0o644))
                .unwrap();
        }
        opened(&dir, Some(HERE));
        assert_eq!(mode(&dir.join(FILE)), 0o600);
        assert_eq!(mode(&dir.join(SECRET_FILE)), 0o600);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = temp_dir("damaged");
        std::fs::write(dir.join(FILE), b"{ not json").unwrap();
        let keys = opened(&dir, Some(HERE));
        assert!(!keys.has("openai"));
        assert_eq!(set_aside_files(&dir), 1);
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

    #[test]
    fn this_computer_has_an_id_and_keeps_it() {
        let id = machine_id();
        // macOS and Windows always have one; a Linux container may have no machine-id file.
        if cfg!(any(target_os = "macos", windows)) {
            assert!(id.as_deref().is_some_and(|id| id.len() >= 8), "{id:?}");
        }
        assert_eq!(machine_id(), id, "the same one every time");
    }
}
