//! Failures the assistant can hit, worded for the person looking at the window.

/// Every message is a complete sentence the UI shows verbatim. Keys and raw response bodies
/// never appear in one.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("FolderSkin does not know a provider called {0}")]
    UnknownProvider(String),

    #[error("{provider} does not offer a model called {model}")]
    UnknownModel { provider: String, model: String },

    #[error("add your {0} API key first")]
    MissingKey(String),

    #[error("that key was rejected by {0}. Check it was copied whole and is still active.")]
    Unauthorized(String),

    #[error("{0} is rate limiting you right now. Wait a moment and try again.")]
    RateLimited(String),

    #[error("{0}")]
    Refused(String),

    #[error("couldn't reach {provider}: {detail}")]
    Network { provider: String, detail: String },

    #[error("{provider} returned an error ({status}): {message}")]
    Provider {
        provider: String,
        status: u16,
        message: String,
    },

    #[error("{0}")]
    Decode(String),

    #[error("{0}")]
    Unsupported(String),

    #[error("{provider} took too long to finish the image. Try again, or pick a faster model.")]
    Timeout { provider: String },

    #[error("the provider returned something that is not an image")]
    NotAnImage,

    /// A keyed whole-folder render came back without its flat backdrop.
    #[error(
        "the model drew a scene instead of a folder on a plain backdrop. Try again, or switch to \
         Artwork, which does not need one."
    )]
    NoBackdrop,
}

impl AiError {
    /// Maps an HTTP status plus the provider's own message onto a friendly error.
    pub fn from_status(provider: &str, status: u16, message: String) -> Self {
        let message = trim_message(&message);
        match status {
            401 | 403 => AiError::Unauthorized(provider.to_string()),
            // Black Forest Labs' "Invalid API key format": the key, not the request.
            422 if message.to_lowercase().contains("api key") => {
                AiError::Unauthorized(provider.to_string())
            }
            429 => AiError::RateLimited(provider.to_string()),
            400 if looks_like_refusal(&message) => {
                AiError::Refused(format!("{provider} declined that prompt: {message}"))
            }
            _ => AiError::Provider {
                provider: provider.to_string(),
                status,
                message,
            },
        }
    }
}

/// Keeps a provider message short enough to read in a toast and free of stray whitespace.
pub fn trim_message(message: &str) -> String {
    let flat = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 200 {
        return flat;
    }
    let cut: String = flat.chars().take(200).collect();
    format!("{cut}…")
}

fn looks_like_refusal(message: &str) -> bool {
    let m = message.to_lowercase();
    [
        "safety",
        "content policy",
        "moderation",
        "not allowed",
        "rejected as",
        "violat",
    ]
    .iter()
    .any(|needle| m.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_failures_name_the_provider_and_never_echo_the_body() {
        let e = AiError::from_status(
            "OpenAI",
            401,
            "Incorrect API key provided: sk-abc123".into(),
        );
        let text = e.to_string();
        assert!(text.contains("OpenAI"));
        assert!(
            !text.contains("sk-abc123"),
            "the key must not survive into the message: {text}"
        );
    }

    #[test]
    fn a_key_in_the_wrong_shape_is_a_rejected_key() {
        let e = AiError::from_status("Black Forest Labs", 422, "Invalid API key format".into());
        assert!(matches!(e, AiError::Unauthorized(_)), "{e:?}");
        let other = AiError::from_status("Black Forest Labs", 422, "width must be even".into());
        assert!(matches!(other, AiError::Provider { status: 422, .. }));
    }

    #[test]
    fn rate_limits_tell_the_user_what_to_do() {
        assert!(AiError::from_status("xAI", 429, "slow down".into())
            .to_string()
            .contains("Wait a moment"));
    }

    #[test]
    fn refusals_are_distinguished_from_ordinary_bad_requests() {
        let refusal = AiError::from_status(
            "OpenAI",
            400,
            "Your request was rejected as a result of our safety system".into(),
        );
        assert!(matches!(refusal, AiError::Refused(_)));
        let plain = AiError::from_status("OpenAI", 400, "unknown parameter: foo".into());
        assert!(matches!(plain, AiError::Provider { status: 400, .. }));
    }

    #[test]
    fn long_provider_messages_are_trimmed() {
        let long = "x".repeat(500);
        let out = trim_message(&long);
        assert!(out.chars().count() <= 201, "{}", out.chars().count());
        assert_eq!(trim_message("  a   b \n c "), "a b c");
    }
}
