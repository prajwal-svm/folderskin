//! How an AI command fails: a code the chat offers the right next step for, a sentence, what to
//! try, and a question ready to paste to Claude. src/lib/aiError.ts reads it; its codes are the
//! ones there (`missing_key`, `unauthorized`, `rate_limited`, `network`, `refused`, `timeout`,
//! `stopped`, `no_backdrop`, `no_image`, `local_not_ready`, `out_of_memory`, `failed`), plus the
//! local engine's own (`no_build_for_platform`, `path_not_ascii`, `vc_runtime_missing`,
//! `runtime_failed_to_start`, `busy`, …) passed through as they are.
//!
//! No message ever carries a key: a provider's words are scrubbed of it before they're kept.

use folderskin_ai::AiError;
use folderskin_local::Error as EngineError;
use serde::Serialize;

/// Where to report something that is FolderSkin's own fault.
const ISSUES_URL: &str = "https://github.com/prajwal-svm/folderskin/issues/new";

/// The error every AI command returns.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AiFailure {
    pub code: String,
    /// One or two sentences, ready to show as they are.
    pub message: String,
    /// Things to try, each a sentence.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fix: Vec<String>,
    /// `claude "…"`: a request for help about this failure, safe to paste into any shell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<String>,
}

impl AiFailure {
    pub fn new(code: &str, message: impl AsRef<str>) -> AiFailure {
        AiFailure {
            code: code.to_string(),
            message: sentence(message.as_ref()),
            fix: Vec::new(),
            ask: None,
        }
    }

    /// Something that went wrong with no more to say about it than its sentence.
    pub fn failed(message: impl AsRef<str>) -> AiFailure {
        AiFailure::new("failed", message)
    }

    /// What a stopped run returns; the chat shows it as stopped, not as a failure.
    pub fn stopped() -> AiFailure {
        AiFailure::new("stopped", "Stopped before it finished.")
    }

    /// A step inside FolderSkin that ended unexpectedly (a worker thread that panicked).
    pub fn bug(what: impl AsRef<str>) -> AiFailure {
        AiFailure::new("failed", what).fix(format!(
            "If it happens again, please report it at {ISSUES_URL} with what you were doing."
        ))
    }

    pub fn fix(mut self, step: impl AsRef<str>) -> AiFailure {
        self.fix.push(sentence(step.as_ref()));
        self
    }

    /// Adds the request for help: what was being done, and on what, and how it failed.
    pub fn asking(mut self, doing: &str, why: &str) -> AiFailure {
        let why: String = why.lines().take(6).collect::<Vec<_>>().join(" ");
        let prompt = format!(
            "On {} {} with FolderSkin {}, {doing} failed with {}: {} {} Help me fix it.",
            std::env::consts::OS,
            std::env::consts::ARCH,
            env!("CARGO_PKG_VERSION"),
            self.code,
            self.message,
            folderskin_ai::error::shorten(why.trim(), 500),
        );
        self.ask = Some(format!("claude \"{}\"", shell_safe(&prompt)));
        self
    }

    /// The same failure with `secret` taken out of every word of it.
    pub fn without(mut self, secret: &str) -> AiFailure {
        // A key is long; a short string would take out ordinary words.
        if secret.trim().len() < 8 {
            return self;
        }
        let clean = |s: &str| s.replace(secret.trim(), "[your key]");
        self.message = clean(&self.message);
        self.fix = self.fix.iter().map(|f| clean(f)).collect();
        self.ask = self.ask.as_deref().map(clean);
        self
    }

    pub fn is_stopped(&self) -> bool {
        self.code == "stopped"
    }
}

impl std::fmt::Display for AiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// A provider's failure. `label` is the provider as people call it; `doing` says what was
/// asked, for the request for help.
pub fn from_provider(e: AiError, label: &str, doing: &str) -> AiFailure {
    let text = e.to_string();
    match e {
        AiError::MissingKey(_) => AiFailure::new("missing_key", text),
        AiError::Unauthorized(_) => AiFailure::new("unauthorized", text),
        AiError::RateLimited(_) => AiFailure::new("rate_limited", text),
        AiError::Refused(_) => AiFailure::new("refused", text),
        AiError::Network { .. } => AiFailure::new("network", text)
            .fix("Check the internet connection (and any proxy or firewall), then try again."),
        AiError::Timeout { .. } => AiFailure::new("timeout", text),
        AiError::NoBackdrop => AiFailure::new("no_backdrop", text),
        // Nothing is wrong with the key or the words: the chat offers Try again, and FolderSkin
        // never tries again by itself (docs/AI.md, Cost).
        AiError::NoImage(_) => AiFailure::new("no_image", text),
        AiError::UnknownProvider(_) | AiError::UnknownModel { .. } => {
            AiFailure::failed(text).fix("Choose another model in the provider settings.")
        }
        AiError::Provider { .. }
        | AiError::Decode(_)
        | AiError::Unsupported(_)
        | AiError::NotAnImage => AiFailure::failed(text)
            .fix(format!(
                "Try again in a little while. If it keeps happening, try another model or \
                 another provider than {label}."
            ))
            .asking(doing, ""),
    }
}

/// What the local engine was doing when it failed, for the words around its failure.
pub struct Doing {
    /// "painting a folder picture with the local model", for the request for help.
    pub what: String,
    /// Setting up rather than painting: a missing runtime is then a failed install, not a
    /// computer that isn't set up.
    pub setup: bool,
}

/// The local engine's failure, in the app's words: its fixes name the command line's commands,
/// so those give way to what to do in the window.
pub fn from_engine(e: EngineError, doing: &Doing) -> AiFailure {
    if e.is_cancelled() {
        return AiFailure::stopped();
    }
    let detail = first_line(&e.why);
    let failure = match e.code {
        // Nothing is wrong but that: the card offers the setup, and there's nothing to ask about.
        "runtime_missing" | "models_missing" | "mflux_missing" if !doing.setup => {
            return AiFailure::new(
                "local_not_ready",
                "Skins can't be generated with the local model until it's set up.",
            )
            .fix(format!("{} Set it up in the provider settings.", e.what));
        }
        // Another setup has the computer (the command line's, or another FolderSkin's).
        "busy" => {
            let mut failure = AiFailure::new("busy", join(&e.what, &detail));
            for step in app_fixes(&e.fix) {
                failure = failure.fix(step);
            }
            return failure;
        }
        "generation_failed" if folderskin_local::generate::ran_out_of_memory(&e.why) => {
            AiFailure::new(
                "out_of_memory",
                "The graphics card ran out of memory while painting.",
            )
            .fix("Close apps that use the graphics card, such as games or video editors, then try again.")
            .fix(
                "If it keeps happening, restart the computer: something may still be holding the \
                 graphics card's memory.",
            )
        }
        "generation_failed" => AiFailure::new("failed", join(&e.what, &detail))
            .fix("Try again.")
            .fix("If it keeps happening, open Details to see what the runtime said."),
        "bug" => AiFailure::bug(join("Something went wrong inside FolderSkin.", &e.what)),
        code => {
            let mut failure = AiFailure::new(code, join(&e.what, &detail));
            for step in app_fixes(&e.fix) {
                failure = failure.fix(step);
            }
            match code {
                "no_build_for_platform" | "macos_too_old" => {
                    failure.fix("Or make pictures with a provider and your own key instead.")
                }
                "runtime_failed_to_start" | "runtime_missing" | "mflux_missing" => {
                    failure.fix("Set the local model up again in the provider settings.")
                }
                "blank_picture" => failure.fix("Try again: each picture starts from a new seed."),
                _ => failure,
            }
        }
    };
    failure.asking(&doing.what, &e.why)
}

/// The engine's fixes that make sense in the window. Those that name a `folderskin` command or
/// one of its flags are left out; "run the command again" becomes "try again".
pub fn app_fixes(fixes: &[String]) -> Vec<String> {
    fixes
        .iter()
        .filter(|f| !f.contains("folderskin ") && !f.contains(" --"))
        .map(|f| {
            let mut f = f.clone();
            for (from, to) in [
                ("Run the same command again", "Try again"),
                ("run the same command again", "try again"),
                ("Run the command again", "Try again"),
                ("run the command again", "try again"),
                ("Run setup again", "Set the local model up again"),
                ("run setup again", "set the local model up again"),
            ] {
                f = f.replace(from, to);
            }
            f
        })
        .collect()
}

/// The first line of a why, without the "Its last output:" that introduces the runtime's words
/// (those are in the details already).
fn first_line(why: &str) -> String {
    let line = why.lines().next().unwrap_or("").trim();
    line.strip_suffix("Its last output:")
        .unwrap_or(line)
        .trim()
        .to_string()
}

fn join(a: &str, b: &str) -> String {
    match (a.trim(), b.trim()) {
        (a, "") => a.to_string(),
        ("", b) => b.to_string(),
        (a, b) => format!("{} {b}", sentence(a)),
    }
}

/// A capital first, a full stop last.
pub fn sentence(text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return "Something went wrong.".into();
    }
    let mut chars = t.chars();
    let first: String = chars
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    let s = first + chars.as_str();
    if s.ends_with(['.', '!', '?', '…', ')']) {
        s
    } else {
        s + "."
    }
}

/// Text that can sit inside double quotes in bash, zsh, PowerShell and cmd without anything in
/// it being run or expanded, by the command line's rules (folderskin-cli's `shell_safe`): no
/// quotes of any kind, no `$`, backticks, `!` or `%`, no line breaks, and no backslash just
/// before the closing quote.
fn shell_safe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' | '`' | '\u{2018}'..='\u{201F}' => out.push('\''),
            '$' => out.push_str("USD "),
            '!' => out.push('.'),
            '%' => out.push_str(" percent"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_end_matches('\\').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_local::Class;

    fn painting() -> Doing {
        Doing {
            what: "painting a folder picture with the local model (CUDA, RTX 3050 Ti, 4 GB)".into(),
            setup: false,
        }
    }

    #[test]
    fn a_failure_is_the_object_the_chat_reads() {
        let json = serde_json::to_value(AiFailure::stopped()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"code": "stopped", "message": "Stopped before it finished."})
        );
        let json = serde_json::to_value(AiFailure::new("x", "it broke").fix("try this")).unwrap();
        assert_eq!(json["message"], "It broke.");
        assert_eq!(json["fix"], serde_json::json!(["Try this."]));
    }

    #[test]
    fn provider_failures_get_the_codes_the_chat_knows() {
        let code = |e: AiError| from_provider(e, "OpenAI", "asking OpenAI").code;
        assert_eq!(code(AiError::MissingKey("OpenAI".into())), "missing_key");
        assert_eq!(code(AiError::Unauthorized("OpenAI".into())), "unauthorized");
        assert_eq!(code(AiError::RateLimited("OpenAI".into())), "rate_limited");
        assert_eq!(code(AiError::Refused("no".into())), "refused");
        assert_eq!(
            code(AiError::Network {
                provider: "OpenAI".into(),
                detail: "dns".into()
            }),
            "network"
        );
        assert_eq!(
            code(AiError::Timeout {
                provider: "OpenAI".into()
            }),
            "timeout"
        );
        assert_eq!(code(AiError::NoBackdrop), "no_backdrop");
        let empty = from_provider(
            AiError::NoImage("Google Gemini".into()),
            "Google Gemini",
            "asking Google Gemini for folder artwork",
        );
        assert_eq!(empty.code, "no_image");
        assert_eq!(
            empty.message,
            "Google Gemini finished without painting a picture. Try again, or reword the idea."
        );
        // Trying again is the answer, so there is nothing to ask about.
        assert_eq!(empty.ask, None);
        let odd = from_provider(
            AiError::Provider {
                provider: "OpenAI".into(),
                status: 500,
                message: "oops".into(),
            },
            "OpenAI",
            "asking OpenAI for a folder picture",
        );
        assert_eq!(odd.code, "failed");
        assert!(
            odd.ask.as_deref().unwrap().starts_with("claude \""),
            "{odd:?}"
        );
        let missing = from_provider(AiError::MissingKey("OpenAI".into()), "OpenAI", "x");
        assert_eq!(missing.message, "Add your OpenAI API key first.");
    }

    #[test]
    fn a_key_never_survives_into_a_failure() {
        let key = "sk-proj-abcdefghijklmnop";
        let e = AiFailure::failed(format!("OpenAI said: bad key {key}"))
            .fix(format!("check {key}"))
            .asking("asking OpenAI", key)
            .without(key);
        let all = serde_json::to_string(&e).unwrap();
        assert!(!all.contains(key), "{all}");
        assert!(e.message.contains("[your key]"));
    }

    #[test]
    fn a_stop_is_a_stop_whatever_the_engine_was_doing() {
        let e = from_engine(EngineError::cancelled(), &painting());
        assert!(e.is_stopped());
        assert_eq!(e.ask, None);
    }

    #[test]
    fn a_computer_that_isnt_set_up_is_offered_the_setup() {
        let e = EngineError::environment(
            "models_missing",
            "FLUX.2 [klein] 4B isn't downloaded yet.",
            "z.gguf is missing from C:\\models.",
        )
        .fix("Download what's missing: folderskin ai setup --backend cuda --tier q8");
        let f = from_engine(e.clone(), &painting());
        assert_eq!(f.code, "local_not_ready");
        assert!(f.fix.iter().all(|s| !s.contains("folderskin ai")), "{f:?}");
        // Setting up is the answer, so there's nothing to ask Claude, as in the preview.
        assert_eq!(f.ask, None);
        // While setting up, it is what went wrong with the setup instead.
        let during_setup = Doing {
            what: "setting the local model up".into(),
            setup: true,
        };
        assert_eq!(from_engine(e, &during_setup).code, "models_missing");
    }

    #[test]
    fn running_out_of_memory_says_so_and_what_helps() {
        let e = EngineError::environment(
            "generation_failed",
            "The picture couldn't be painted.",
            "stable-diffusion.cpp stopped with exit code 1. Its last output:\n[ERROR] ggml_cuda: out of memory",
        );
        let f = from_engine(e, &painting());
        assert_eq!(f.code, "out_of_memory");
        assert_eq!(f.fix.len(), 2, "{f:?}");
        assert!(f.fix[0].contains("Close apps"), "{f:?}");
        assert!(f.fix[1].contains("restart the computer"), "{f:?}");
        assert!(f.ask.as_deref().unwrap().contains("RTX 3050 Ti"));
    }

    #[test]
    fn running_out_of_memory_on_vulkan_is_heard_too() {
        // What stable-diffusion.cpp's Vulkan backend prints when an AMD or Intel card is full.
        for said in [
            "ggml_vulkan: Device memory allocation of size 4831838208 failed.",
            "ggml_vulkan: vk::Device::allocateMemory: ErrorOutOfDeviceMemory",
            "ggml_backend_alloc_ctx_tensors_from_buft: failed to allocate Vulkan0 buffer of size 4831838208",
        ] {
            let e = EngineError::environment(
                "generation_failed",
                "The picture couldn't be painted.",
                format!(
                    "stable-diffusion.cpp stopped with exit code 1. Its last output:\n{said}"
                ),
            );
            assert_eq!(
                from_engine(e, &painting()).code,
                "out_of_memory",
                "{said}"
            );
        }
    }

    #[test]
    fn a_setup_from_the_command_line_holding_the_computer_says_so_plainly() {
        let e = EngineError::environment(
            "busy",
            "This computer is already being set up.",
            "Another setup, in FolderSkin or in a terminal, is downloading into the same folder.",
        )
        .fix("Wait for it to finish, then run the command again.");
        let during_setup = Doing {
            what: "setting the local model up".into(),
            setup: true,
        };
        let f = from_engine(e, &during_setup);
        assert_eq!(f.code, "busy");
        assert!(f.message.contains("in a terminal"), "{f:?}");
        assert_eq!(f.fix, ["Wait for it to finish, then try again."]);
        assert_eq!(f.ask, None);
    }

    #[test]
    fn other_engine_failures_keep_their_code_in_the_apps_words() {
        let e = EngineError::environment(
            "generation_failed",
            "The picture couldn't be painted.",
            "stable-diffusion.cpp stopped with exit code 3. Its last output:\nbad things",
        )
        .fix("Run again with --verbose to see everything the runtime printed.");
        let f = from_engine(e, &painting());
        assert_eq!(f.code, "failed");
        assert_eq!(
            f.message,
            "The picture couldn't be painted. stable-diffusion.cpp stopped with exit code 3."
        );

        let f = from_engine(no_build(), &painting());
        assert_eq!(f.code, "no_build_for_platform");
        assert!(f.fix.iter().all(|s| !s.contains("folderskin ai")), "{f:?}");
        assert!(f.fix.last().unwrap().contains("your own key"));

        let e = EngineError::fixable("bad_seed", "The seed 9 is too big.", "A seed goes to 8.")
            .fix("Use a smaller seed, or leave it out for a random one.");
        let f = from_engine(e, &painting());
        assert_eq!(
            (f.code.as_str(), f.message.as_str()),
            ("bad_seed", "The seed 9 is too big. A seed goes to 8.")
        );

        let e = EngineError::fixable(
            "hash_mismatch",
            "a.gguf didn't download correctly.",
            "Its SHA-256 is x.",
        )
        .fix("Run the same command again to download it afresh.");
        assert_eq!(
            from_engine(e, &painting()).fix,
            ["Try again to download it afresh."]
        );
        let e = EngineError::new(Class::Bug, "bug", "Painting stopped unexpectedly.", "panic");
        assert!(from_engine(e, &painting()).fix[0].contains(ISSUES_URL));
    }

    fn no_build() -> EngineError {
        folderskin_local::setup::no_build(
            folderskin_local::machine::Os::Linux,
            folderskin_local::machine::Arch::Arm64,
            folderskin_local::Backend::Vulkan,
        )
    }

    #[test]
    fn the_request_for_help_is_safe_in_any_shell() {
        let f = AiFailure::failed("it said \"$HOME\" 100% !")
            .asking("painting `x`", "line one\nline two\\");
        let ask = f.ask.unwrap();
        assert!(ask.starts_with("claude \"") && ask.ends_with('"'), "{ask}");
        let inner = &ask["claude \"".len()..ask.len() - 1];
        assert!(!inner.contains(['"', '$', '`', '!', '%', '\n']), "{inner}");
    }
}
