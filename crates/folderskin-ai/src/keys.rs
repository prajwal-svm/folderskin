//! API keys in the operating system's keychain.
//!
//! macOS Keychain, Windows Credential Manager and the Linux Secret Service all sit behind the
//! `keyring` crate. FolderSkin stores one entry per provider and never writes a key anywhere
//! else: not to its config, not to a log, not into an error message.

use crate::AiError;

/// Keychain service name; the account is the provider id.
const SERVICE: &str = "app.folderskin.desktop";

fn entry(provider_id: &str) -> Result<keyring::Entry, AiError> {
    keyring::Entry::new(SERVICE, provider_id).map_err(|e| AiError::Keyring(e.to_string()))
}

/// Saves (or replaces) the key for one provider.
pub fn set(provider_id: &str, key: &str) -> Result<(), AiError> {
    entry(provider_id)?
        .set_password(key)
        .map_err(|e| AiError::Keyring(e.to_string()))
}

/// The stored key, or `None` when the user has not added one. A missing entry is not an error.
pub fn get(provider_id: &str) -> Result<Option<String>, AiError> {
    match entry(provider_id)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AiError::Keyring(e.to_string())),
    }
}

/// Removes the key. Removing one that is not there succeeds.
pub fn clear(provider_id: &str) -> Result<(), AiError> {
    match entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AiError::Keyring(e.to_string())),
    }
}

/// True when a key is saved. A broken keychain reads as "no key" rather than failing the UI.
pub fn has(provider_id: &str) -> bool {
    matches!(get(provider_id), Ok(Some(_)))
}

/// `sk-…9f2a`: safe to show in the window and to put in a log line.
pub fn redact(key: &str) -> String {
    let key = key.trim();
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "…".into();
    }
    let head: String = chars.iter().take(3).collect();
    let tail: String = chars.iter().skip(chars.len() - 4).collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_keeps_only_the_ends() {
        assert_eq!(redact("sk-proj-abcdefgh9f2a"), "sk-…9f2a");
        assert_eq!(redact("short"), "…");
        assert!(!redact("sk-proj-abcdefgh9f2a").contains("abcdefgh"));
    }

    #[test]
    fn redact_handles_whitespace_and_unicode() {
        assert_eq!(redact("  sk-abcdefghij  "), "sk-…ghij");
        assert_eq!(redact("🔑🔑🔑"), "…");
    }
}
