//! What gets signed. services/community/src/auth.ts rebuilds the same strings and checks them.
//!
//! A request: `METHOD|/path?query|unix seconds|hex(sha256(body))`, sent with the key, the time
//! and the signature in `X-FS-Key`, `X-FS-Ts` and `X-FS-Sig`. The verification link:
//! `verify|key|nonce|unix seconds|handle`. No method is called "verify", so a signature over one
//! can never pass for the other.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use sha2::{Digest, Sha256};

pub const KEY_HEADER: &str = "X-FS-Key";
pub const TIME_HEADER: &str = "X-FS-Ts";
pub const SIGNATURE_HEADER: &str = "X-FS-Sig";
/// The service's own clock, sent back when a request's time is too far from it.
pub const SERVER_TIME_HEADER: &str = "X-FS-Time";

/// Lower-case hex of SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The string a request's signature covers.
pub fn request_message(method: &str, path_and_query: &str, ts: u64, body: &[u8]) -> String {
    format!("{method}|{path_and_query}|{ts}|{}", sha256_hex(body))
}

/// The string a verification link's signature covers.
pub fn verify_message(key: &str, nonce: &str, ts: u64, handle: &str) -> String {
    format!("verify|{key}|{nonce}|{ts}|{handle}")
}

/// Checks a signature from `public` (base64url, 32 bytes) over `message`. The service does this
/// for real; here it is for tests and for tools that want to be sure of a key.
pub fn verify(public: &str, message: &[u8], signature: &str) -> bool {
    let (Ok(public), Ok(signature)) = (B64URL.decode(public), B64URL.decode(signature)) else {
        return false;
    };
    aws_lc_rs::signature::UnparsedPublicKey::new(&aws_lc_rs::signature::ED25519, public)
        .verify(message, &signature)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_message_covers_the_method_path_time_and_body() {
        assert_eq!(
            request_message(
                "PUT",
                "/v1/submissions/sub_x/sheets/0",
                1_790_000_000,
                b"abc"
            ),
            "PUT|/v1/submissions/sub_x/sheets/0|1790000000|\
             ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // An empty body still has a hash: the one every GET signs.
        assert!(request_message("GET", "/v1/me", 1, b"")
            .ends_with("|e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
    }

    /// The same key, message and signature are checked by the Worker's tests
    /// (services/community/test/signing.test.ts), so the two sides can't drift apart.
    #[test]
    fn the_shared_test_vector_signs_the_same_on_both_sides() {
        let key = crate::DeviceKey::from_secret(
            "MC4CAQAwBQYDK2VwBCIEIAcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcH",
        )
        .unwrap();
        assert_eq!(key.public(), "6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw");
        let message = request_message(
            "PUT",
            "/v1/submissions/sub_aaaaaaaaaaaaaaaaaaaa/sheets/0",
            1_790_000_000,
            b"abc",
        );
        assert_eq!(
            key.sign(message.as_bytes()),
            "Xza0u6d4ddc5BZclGh7ne85lgkeu1yHSYmjK1F8BYFHfGu_YHYCBAdtvSYmZdv36Do9UhY0Fwq9PLAsFJf4jCg"
        );
    }

    #[test]
    fn a_verify_message_can_never_be_a_request_message() {
        let link = verify_message("k", "n", 1, "sunny-otter");
        assert_eq!(link, "verify|k|n|1|sunny-otter");
        assert!(!["GET", "PUT", "POST", "DELETE"]
            .iter()
            .any(|m| link.starts_with(&format!("{m}|"))));
    }
}
