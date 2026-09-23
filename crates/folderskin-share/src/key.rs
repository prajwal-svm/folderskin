//! The computer's Ed25519 key.
//!
//! The app makes one the first time someone shares without GitHub and keeps it sealed with the
//! API keys (src-tauri/src/keys.rs), where it only opens on the computer that saved it. The
//! recovery file is how it moves to another computer, or survives a reinstall: the key in plain
//! PKCS#8, so it has to be kept as privately as a password. Nothing here ever prints the private
//! half; `Debug` shows only the public key.

use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde::{Deserialize, Serialize};

/// What a recovery file says it is, so another JSON file isn't taken for one.
pub const RECOVERY_KIND: &str = "folderskin-sharing-key";
const RECOVERY_VERSION: u32 = 1;

/// Why a key couldn't be made or read. Each is a sentence the app can show.
#[derive(Debug, PartialEq, Eq)]
pub enum KeyError {
    /// The system's random numbers or its crypto library failed.
    Unavailable,
    /// What was saved isn't a key FolderSkin made.
    Damaged,
    /// The file isn't a recovery file at all.
    NotRecoveryFile,
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            KeyError::Unavailable => "This computer couldn't make a key for sharing. Try again.",
            KeyError::Damaged => "The saved sharing key is damaged, so it can't be used.",
            KeyError::NotRecoveryFile => "That file isn't a FolderSkin recovery file.",
        })
    }
}

impl std::error::Error for KeyError {}

/// The recovery file, as JSON.
#[derive(Serialize, Deserialize)]
struct RecoveryFile {
    kind: String,
    version: u32,
    /// Said again inside the file, for whoever opens it.
    note: String,
    public: String,
    secret: String,
}

pub struct DeviceKey {
    pair: Ed25519KeyPair,
    pkcs8: Vec<u8>,
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKey")
            .field("public", &self.public())
            .finish_non_exhaustive()
    }
}

impl DeviceKey {
    /// A new key.
    pub fn generate() -> Result<DeviceKey, KeyError> {
        let pair = Ed25519KeyPair::generate().map_err(|_| KeyError::Unavailable)?;
        let pkcs8 = pair
            .to_pkcs8()
            .map_err(|_| KeyError::Unavailable)?
            .as_ref()
            .to_vec();
        Ok(DeviceKey { pair, pkcs8 })
    }

    fn from_pkcs8(pkcs8: Vec<u8>) -> Result<DeviceKey, KeyError> {
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8).map_err(|_| KeyError::Damaged)?;
        Ok(DeviceKey { pair, pkcs8 })
    }

    /// The key as the app keeps it: PKCS#8, base64url.
    pub fn to_secret(&self) -> String {
        B64URL.encode(&self.pkcs8)
    }

    /// A key [`DeviceKey::to_secret`] wrote.
    pub fn from_secret(secret: &str) -> Result<DeviceKey, KeyError> {
        let pkcs8 = B64URL
            .decode(secret.trim())
            .map_err(|_| KeyError::Damaged)?;
        DeviceKey::from_pkcs8(pkcs8)
    }

    /// The public key, base64url: what the service knows this computer by.
    pub fn public(&self) -> String {
        B64URL.encode(self.pair.public_key().as_ref())
    }

    /// Signs `message`; the signature is base64url.
    pub fn sign(&self, message: &[u8]) -> String {
        B64URL.encode(self.pair.sign(message).as_ref())
    }

    /// The recovery file for this key, as the text to save.
    pub fn recovery_file(&self) -> String {
        let file = RecoveryFile {
            kind: RECOVERY_KIND.into(),
            version: RECOVERY_VERSION,
            note: "This is the key FolderSkin shares packs with. Anyone who has this file can \
                   share as you, so keep it as private as a password."
                .into(),
            public: self.public(),
            secret: self.to_secret(),
        };
        serde_json::to_string_pretty(&file).unwrap_or_default() + "\n"
    }

    /// The key in a recovery file. A file whose public key doesn't match its private one is
    /// damaged, whatever else it says.
    pub fn from_recovery_file(text: &str) -> Result<DeviceKey, KeyError> {
        let file: RecoveryFile =
            serde_json::from_str(text).map_err(|_| KeyError::NotRecoveryFile)?;
        if file.kind != RECOVERY_KIND || file.version != RECOVERY_VERSION {
            return Err(KeyError::NotRecoveryFile);
        }
        let key = DeviceKey::from_secret(&file.secret)?;
        if key.public() != file.public {
            return Err(KeyError::Damaged);
        }
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sign;

    #[test]
    fn a_key_signs_what_its_public_half_verifies() {
        let key = DeviceKey::generate().unwrap();
        assert_eq!(key.public().len(), 43, "32 bytes of base64url");
        let signature = key.sign(b"GET|/v1/me|1|e3b0");
        assert_eq!(signature.len(), 86, "64 bytes of base64url");
        assert!(sign::verify(
            &key.public(),
            b"GET|/v1/me|1|e3b0",
            &signature
        ));
        assert!(!sign::verify(
            &key.public(),
            b"GET|/v1/me|2|e3b0",
            &signature
        ));
        let other = DeviceKey::generate().unwrap();
        assert!(!sign::verify(
            &other.public(),
            b"GET|/v1/me|1|e3b0",
            &signature
        ));
    }

    #[test]
    fn a_kept_key_comes_back_the_same() {
        let key = DeviceKey::generate().unwrap();
        let again = DeviceKey::from_secret(&key.to_secret()).unwrap();
        assert_eq!(again.public(), key.public());
        assert_eq!(
            DeviceKey::from_secret("not a key").unwrap_err(),
            KeyError::Damaged
        );
    }

    #[test]
    fn a_recovery_file_carries_the_key_and_is_checked() {
        let key = DeviceKey::generate().unwrap();
        let text = key.recovery_file();
        assert!(text.contains(RECOVERY_KIND));
        assert!(text.contains("as private as a password"));
        assert_eq!(
            DeviceKey::from_recovery_file(&text).unwrap().public(),
            key.public()
        );

        // Another key's public half with this private one: damaged, not a different identity.
        let other = DeviceKey::generate().unwrap();
        let swapped = text.replace(&key.public(), &other.public());
        assert_eq!(
            DeviceKey::from_recovery_file(&swapped).unwrap_err(),
            KeyError::Damaged
        );
        assert_eq!(
            DeviceKey::from_recovery_file("{\"kind\": \"something else\"}").unwrap_err(),
            KeyError::NotRecoveryFile
        );
        assert_eq!(
            DeviceKey::from_recovery_file("not json").unwrap_err(),
            KeyError::NotRecoveryFile
        );
    }

    #[test]
    fn debug_never_shows_the_private_half() {
        let key = DeviceKey::generate().unwrap();
        let shown = format!("{key:?}");
        assert!(shown.contains(&key.public()));
        assert!(!shown.contains(&key.to_secret()));
    }
}
