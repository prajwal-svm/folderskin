//! Failures, each worded for the person who hit it: what happened, why, and what to do next.
//!
//! `code` is stable, so a front end can match on it and word its own advice; `what`, `why` and
//! `fix` are complete sentences ready to show as they are. The fixes name `folderskin` commands,
//! because the command line is what runs this first.

use std::fmt;
use std::path::Path;

/// What kind of failure it was, which decides the command line's exit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Something the person can put right: a path, a setting, a download to resume.
    Fixable,
    /// The computer is missing something: the runtime, the models, a driver, a network.
    Environment,
    /// The person stopped it.
    Cancelled,
    /// A bug in FolderSkin.
    Bug,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub what: String,
    pub why: String,
    pub fix: Vec<String>,
    pub class: Class,
}

impl Error {
    pub fn new(
        class: Class,
        code: &'static str,
        what: impl Into<String>,
        why: impl Into<String>,
    ) -> Error {
        Error {
            code,
            what: what.into(),
            why: why.into(),
            fix: Vec::new(),
            class,
        }
    }

    pub fn fixable(code: &'static str, what: impl Into<String>, why: impl Into<String>) -> Error {
        Error::new(Class::Fixable, code, what, why)
    }

    pub fn environment(
        code: &'static str,
        what: impl Into<String>,
        why: impl Into<String>,
    ) -> Error {
        Error::new(Class::Environment, code, what, why)
    }

    pub fn bug(what: impl Into<String>, why: impl Into<String>) -> Error {
        Error::new(Class::Bug, "bug", what, why)
    }

    pub fn cancelled() -> Error {
        Error::new(
            Class::Cancelled,
            "cancelled",
            "Stopped.",
            "It was cancelled before it finished.",
        )
    }

    /// Adds one thing to try.
    pub fn fix(mut self, step: impl Into<String>) -> Error {
        self.fix.push(step.into());
        self
    }

    /// A file or folder that couldn't be read or written.
    pub fn io(doing: &str, path: &Path, e: &std::io::Error) -> Error {
        let why = match e.kind() {
            std::io::ErrorKind::NotFound => format!("{} isn't there.", path.display()),
            std::io::ErrorKind::PermissionDenied => {
                format!("This account isn't allowed to use {}.", path.display())
            }
            std::io::ErrorKind::StorageFull => format!(
                "The disk that holds {} is full.",
                path.parent().unwrap_or(path).display()
            ),
            _ => format!("{}: {e}.", path.display()),
        };
        let error = Error::fixable("io", format!("Couldn't {doing}."), why);
        match e.kind() {
            std::io::ErrorKind::StorageFull => {
                error.fix("Free some space on that disk, then run the command again.")
            }
            std::io::ErrorKind::PermissionDenied => error.fix(
                "Check that the file isn't open in another program and that you can write there.",
            ),
            _ => error.fix("Check the path, then run the command again."),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.class == Class::Cancelled
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.what, self.why)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_say_which_file_and_what_to_do() {
        let e = Error::io(
            "save the picture",
            Path::new("/tmp/out/a.png"),
            &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert_eq!(e.what, "Couldn't save the picture.");
        assert!(e.why.contains("a.png"), "{}", e.why);
        assert_eq!(e.fix.len(), 1);
        assert_eq!(e.class, Class::Fixable);
        let full = Error::io(
            "save the picture",
            Path::new("/tmp/out/a.png"),
            &std::io::Error::from(std::io::ErrorKind::StorageFull),
        );
        assert!(full.why.contains("full"), "{}", full.why);
    }

    #[test]
    fn display_reads_as_two_sentences() {
        let e = Error::fixable("x", "It broke.", "Because.").fix("Try again.");
        assert_eq!(e.to_string(), "It broke. Because.");
        assert!(Error::cancelled().is_cancelled());
    }
}
