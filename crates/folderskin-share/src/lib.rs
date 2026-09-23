//! FolderSkin's community service, from the outside: sharing a pack without a GitHub account.
//!
//! The service (services/community, a Cloudflare Worker) takes packs from computers it has
//! verified once, holds each for the maintainer to review, and hands approved ones to
//! `folderskin-tools community pull`. This crate is what the app and the maintainer's tools share
//! to talk to it:
//!
//! - [`DeviceKey`]: the computer's Ed25519 key, which signs every request, and the recovery file
//!   that carries it to another computer.
//! - [`sign`]: the exact bytes that get signed, which the Worker checks the same way.
//! - [`Client`]: signed requests to the service, with its errors as sentences.
//! - [`api`]: what goes over the wire.

pub mod api;
mod client;
mod key;
pub mod sign;

pub use client::{Client, Error};
pub use key::{DeviceKey, KeyError, RECOVERY_KIND};

/// A handle: the name packs shared from a computer are credited to. It has the shape of a GitHub
/// user name (letters, digits and single dashes, not at either end), because pack.json v1 needs
/// that of an author, and is 3 to 39 characters. The service checks the rest (taken, reserved,
/// words it won't show).
pub fn is_handle(name: &str) -> bool {
    (3..=39).contains(&name.len())
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_is_shaped_like_a_github_user_name() {
        for good in ["sunny-otter", "abc", "A1-b2-C3", &"x".repeat(39)] {
            assert!(is_handle(good), "{good}");
        }
        for bad in [
            "ab",
            "-otter",
            "otter-",
            "two--dashes",
            "with space",
            "émile",
            &"x".repeat(40),
        ] {
            assert!(!is_handle(bad), "{bad}");
        }
    }
}
