//! Failures, told the same way everywhere: what happened, why, what to try, and a request for
//! help ready to paste into Claude Code. Never a bare "command failed".

use folderskin_local::Event;
use std::fmt::Write as _;

/// Where issues go when FolderSkin itself is at fault.
pub const ISSUES_URL: &str = "https://github.com/prajwal-svm/folderskin/issues/new";

/// The exit code, which says what kind of failure it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Something the person can put right: a path, a picture, a setting.
    Fixable = 1,
    /// The command line itself isn't right.
    Usage = 2,
    /// The computer is missing something: the runtime, the models, a driver, the network.
    Environment = 3,
    /// A bug in FolderSkin.
    Bug = 70,
    /// Stopped with Ctrl+C.
    Cancelled = 130,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliError {
    /// Stable, so scripts and the app can match on it.
    pub code: String,
    pub what: String,
    pub why: String,
    pub fix: Vec<String>,
    pub exit: Exit,
}

impl CliError {
    pub fn new(exit: Exit, code: &str, what: impl Into<String>, why: impl Into<String>) -> Self {
        CliError {
            code: code.to_string(),
            what: what.into(),
            why: why.into(),
            fix: Vec::new(),
            exit,
        }
    }

    pub fn fixable(code: &str, what: impl Into<String>, why: impl Into<String>) -> Self {
        CliError::new(Exit::Fixable, code, what, why)
    }

    pub fn environment(code: &str, what: impl Into<String>, why: impl Into<String>) -> Self {
        CliError::new(Exit::Environment, code, what, why)
    }

    pub fn usage(what: impl Into<String>, why: impl Into<String>) -> Self {
        CliError::new(Exit::Usage, "usage", what, why)
    }

    pub fn bug(what: impl Into<String>, why: impl Into<String>) -> Self {
        CliError::new(Exit::Bug, "bug", what, why)
            .fix(format!(
                "Please report it at {ISSUES_URL} with the command you ran and this message."
            ))
            .fix("Running it again with --verbose shows more of what happened.")
    }

    /// A file or folder that couldn't be read or written.
    pub fn io(doing: &str, path: &std::path::Path, e: &std::io::Error) -> Self {
        folderskin_local::Error::io(doing, path, e).into()
    }

    /// A folder given where a file goes. Asked before the file is opened: Windows reports
    /// opening a folder as a file as a permissions problem, which sent people looking for a
    /// program holding the file open.
    pub fn folder_not_file(doing: &str, path: &std::path::Path) -> Self {
        CliError::fixable(
            "not_a_file",
            format!("Couldn't {doing}."),
            format!("{} is a folder, not a file.", path.display()),
        )
        .fix(format!(
            "Give a file's path instead, such as {}",
            path.join("picture.png").display()
        ))
    }

    /// Adds one thing to try.
    pub fn fix(mut self, step: impl Into<String>) -> Self {
        self.fix.push(step.into());
        self
    }

    pub fn is_cancelled(&self) -> bool {
        self.exit == Exit::Cancelled
    }

    /// The block printed for a person.
    pub fn render(&self, command: &str) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "folderskin: {}", self.what);
        if !self.why.is_empty() {
            let mut lines = self.why.lines();
            if let Some(first) = lines.next() {
                let _ = writeln!(s, "  Why:  {first}");
            }
            for line in lines {
                let _ = writeln!(s, "        {line}");
            }
        }
        for (i, step) in self.fix.iter().enumerate() {
            let label = if i == 0 { "Try:" } else { "" };
            let _ = writeln!(s, "  {label:<5} {step}");
        }
        if !self.is_cancelled() {
            let _ = writeln!(s, "  (error {})", self.code);
            let _ = writeln!(s);
            let _ = write!(s, "Ask Claude: {}", self.ask(command));
        }
        s
    }

    /// A ready-to-paste `claude "…"` asking for help with this failure.
    pub fn ask(&self, command: &str) -> String {
        let why: String = self.why.lines().take(6).collect::<Vec<_>>().join(" ");
        let why = truncate(&why, 500);
        let prompt = format!(
            "On {} {} with folderskin {}, the command {command} failed with {}: {} {} Help me fix it.",
            std::env::consts::OS,
            std::env::consts::ARCH,
            env!("CARGO_PKG_VERSION"),
            self.code,
            self.what,
            why
        );
        format!("claude \"{}\"", shell_safe(&prompt))
    }

    /// The error as the `--json` event.
    pub fn to_event(&self, command: &str) -> Event {
        Event::Error {
            code: self.code.clone(),
            what: self.what.clone(),
            why: self.why.clone(),
            fix: self.fix.clone(),
            ask: if self.is_cancelled() {
                String::new()
            } else {
                self.ask(command)
            },
        }
    }
}

impl From<folderskin_local::Error> for CliError {
    fn from(e: folderskin_local::Error) -> Self {
        let exit = match e.class {
            folderskin_local::Class::Fixable => Exit::Fixable,
            folderskin_local::Class::Environment => Exit::Environment,
            folderskin_local::Class::Cancelled => Exit::Cancelled,
            folderskin_local::Class::Bug => Exit::Bug,
        };
        let error = CliError {
            code: e.code.to_string(),
            what: e.what,
            why: e.why,
            fix: e.fix,
            exit,
        };
        match exit {
            Exit::Cancelled => cancelled(),
            Exit::Bug if error.fix.is_empty() => CliError::bug(error.what, error.why),
            _ => error,
        }
    }
}

/// What Ctrl+C leaves behind.
pub fn cancelled() -> CliError {
    CliError::new(
        Exit::Cancelled,
        "cancelled",
        "Stopped.",
        "It was stopped before it finished.",
    )
    .fix("Run the same command again to carry on: finished downloads and pictures are kept.")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

/// Text that can sit inside double quotes in bash, zsh, PowerShell and cmd without anything in
/// it being run or expanded: no quotes, no `$`, backticks or `!`, no line breaks, and no
/// backslash just before the closing quote. PowerShell also takes typographic quotes (“ ” „ and
/// ‘ ’ ‚ ‛) for plain ones, so a file named `x”;calc;“.png` would end the string there: every
/// quote mark becomes a plain apostrophe.
pub fn shell_safe(text: &str) -> String {
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

/// The command as typed, for an error report: `folderskin` and its arguments, quoted where
/// they need it.
pub fn command_line(args: &[std::ffi::OsString]) -> String {
    std::iter::once("folderskin".to_string())
        .chain(args.iter().skip(1).map(|a| {
            let a = a.to_string_lossy();
            if a.is_empty() || a.contains(char::is_whitespace) {
                format!("'{a}'")
            } else {
                a.into_owned()
            }
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models_missing() -> CliError {
        CliError::environment(
            "models_missing",
            "FLUX.2 [klein] 4B isn't downloaded yet.",
            "flux-2-klein-4b-Q8_0.gguf is missing from C:\\Users\\me\\models.",
        )
        .fix("Download what's missing: folderskin ai setup --backend cuda --tier q8")
    }

    #[test]
    fn an_error_reads_as_what_why_and_what_to_try() {
        let text = models_missing().render("folderskin ai gen 'a koi pond'");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "folderskin: FLUX.2 [klein] 4B isn't downloaded yet."
        );
        assert!(lines[1].starts_with("  Why:  flux-2-klein-4b-Q8_0.gguf is missing"));
        assert_eq!(
            lines[2],
            "  Try:  Download what's missing: folderskin ai setup --backend cuda --tier q8"
        );
        assert_eq!(lines[3], "  (error models_missing)");
        let last = lines.last().unwrap();
        assert!(last.starts_with("Ask Claude: claude \""), "{last}");
        assert!(last.ends_with("Help me fix it.\""), "{last}");
        assert!(!text.contains("command failed"));
    }

    #[test]
    fn several_fixes_line_up_and_long_reasons_keep_their_lines() {
        let e = CliError::environment("generation_failed", "It broke.", "line one\nline two")
            .fix("first")
            .fix("second");
        let text = e.render("folderskin x");
        assert!(
            text.contains("  Why:  line one\n        line two\n"),
            "{text}"
        );
        assert!(text.contains("  Try:  first\n        second\n"), "{text}");
    }

    #[test]
    fn the_ask_line_is_safe_to_paste_into_any_shell() {
        let e = CliError::fixable(
            "x",
            "Couldn't read \"$HOME/a`b`.png\"!",
            "C:\\Users\\me\\out\\\nnext line 100%",
        );
        let ask = e.ask("folderskin image check \"$HOME/a.png\"");
        let inner = ask
            .strip_prefix("claude \"")
            .and_then(|s| s.strip_suffix('"'))
            .unwrap();
        for bad in ['"', '$', '`', '!', '\n', '%'] {
            assert!(!inner.contains(bad), "{bad:?} in {inner}");
        }
        assert!(!inner.ends_with('\\'));
        assert!(inner.contains(std::env::consts::OS), "{inner}");
        assert!(inner.contains(std::env::consts::ARCH), "{inner}");
        assert!(inner.contains("folderskin image check"), "{inner}");
        assert_eq!(
            shell_safe("ends in a path C:\\dir\\"),
            "ends in a path C:\\dir"
        );
    }

    /// Names a reviewer used to break out of the quotes in PowerShell, where “ ” „ close a
    /// double-quoted string as `"` does.
    const HOSTILE: [&str; 4] = [
        "x\u{201D};calc;\u{201C}.png",
        "a\u{201E}|calc|\u{201C}b",
        "c\u{2019};calc;\u{2018}d",
        "e\u{201F}&calc&\u{201B}f",
    ];

    #[test]
    fn typographic_quotes_cant_end_the_string_either() {
        for name in HOSTILE {
            let ask = CliError::io(
                "read the picture",
                std::path::Path::new(name),
                &std::io::Error::from(std::io::ErrorKind::NotFound),
            )
            .ask(&format!("folderskin image check {name}"));
            let inner = &ask["claude \"".len()..ask.len() - 1];
            assert!(
                !inner.contains(|c: char| c == '"' || ('\u{2018}'..='\u{201F}').contains(&c)),
                "{inner}"
            );
            assert!(
                inner.contains("calc"),
                "the text is kept, only defused: {inner}"
            );
        }
    }

    /// Hands the ask line to PowerShell's own parser (nothing is run) and expects exactly one
    /// command, `claude`, with one argument. Skipped where PowerShell isn't installed.
    #[test]
    fn powershell_reads_the_ask_line_as_one_command() {
        let script = "$e = $null; \
            $ast = [System.Management.Automation.Language.Parser]::ParseInput($env:FS_ASK_LINE, [ref]$null, [ref]$e); \
            $cmds = @($ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true)); \
            \"$($e.Count) $($cmds.Count) $($cmds[0].CommandElements.Count)\"";
        for shell in ["pwsh", "powershell"] {
            let mut lines = Vec::new();
            for name in HOSTILE {
                let e = CliError::fixable("io", format!("Couldn't read {name}."), "");
                lines.push(e.ask(&format!("folderskin image check {name}")));
            }
            let mut ran = true;
            for line in &lines {
                let out = std::process::Command::new(shell)
                    .args(["-NoProfile", "-NonInteractive", "-Command", script])
                    .env("FS_ASK_LINE", line)
                    .output();
                let Ok(out) = out else {
                    ran = false;
                    break;
                };
                let said = String::from_utf8_lossy(&out.stdout);
                assert_eq!(said.trim(), "0 1 2", "{shell} split {line}");
            }
            if ran {
                return;
            }
        }
    }

    #[test]
    fn a_stopped_run_asks_nothing() {
        let text = cancelled().render("folderskin ai setup");
        assert!(text.starts_with("folderskin: Stopped."));
        assert!(!text.contains("Ask Claude"));
        assert_eq!(cancelled().exit as i32, 130);
    }

    #[test]
    fn bugs_point_at_the_issue_tracker() {
        let e = CliError::bug("FolderSkin hit a bug.", "It panicked at src/x.rs:1: oops.");
        assert_eq!(e.exit as i32, 70);
        assert!(e.fix[0].contains(ISSUES_URL));
    }

    #[test]
    fn engine_errors_keep_their_code_and_class() {
        let engine = folderskin_local::Error::environment("runtime_missing", "a", "b").fix("c");
        let e: CliError = engine.into();
        assert_eq!(
            (e.code.as_str(), e.exit),
            ("runtime_missing", Exit::Environment)
        );
        let e: CliError = folderskin_local::Error::cancelled().into();
        assert!(e.is_cancelled());
        let e: CliError = folderskin_local::Error::bug("a", "b").into();
        assert!(
            e.fix[0].contains(ISSUES_URL),
            "a bug always says where to report it"
        );
    }

    #[test]
    fn the_json_event_carries_everything() {
        match models_missing().to_event("folderskin ai gen x") {
            Event::Error {
                code,
                what,
                fix,
                ask,
                ..
            } => {
                assert_eq!(code, "models_missing");
                assert!(what.contains("klein"));
                assert_eq!(fix.len(), 1);
                assert!(ask.starts_with("claude \""));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_command_line_is_quoted_where_it_needs_it() {
        let args: Vec<std::ffi::OsString> = [
            "C:\\bin\\folderskin-cli.exe",
            "ai",
            "gen",
            "a koi pond",
            "-n",
            "2",
        ]
        .iter()
        .map(Into::into)
        .collect();
        assert_eq!(command_line(&args), "folderskin ai gen 'a koi pond' -n 2");
    }
}
