//! `folderskin`, FolderSkin's command line: paint folder art with open-weight models on this
//! computer (or a provider, with your own key), clean pictures up, check them, and put them on
//! folders, with everything the app does to a picture done the same way here.
//!
//! Every failure is told as what happened, why and what to try, ending with a request for help
//! ready to paste into Claude Code; `--json` makes every command write NDJSON events instead.
//! Exit codes: 0 done, 1 something to put right, 2 the command itself is wrong, 3 the computer
//! is missing something (the runtime, the models, a driver, the network), 70 a bug in FolderSkin,
//! 130 stopped with Ctrl+C.

pub mod ai;
pub mod check;
pub mod cli;
pub mod config;
pub mod error;
pub mod images;
pub mod out;
pub mod paint;
pub mod preview;
pub mod terminal;
pub mod tools;

use clap::Parser;
use cli::{Cli, Command};
use error::{command_line, CliError, Exit};
use out::Out;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

/// Runs the command line with the process's own arguments.
pub fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let command = command_line(&args);
    let json = args.iter().any(|a| a == "--json");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => return usage(e, json, &command),
    };
    let out = Out::new(cli.json, cli.verbose);
    catch_panics();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(cli.command, &out)))
        .unwrap_or_else(|_| Err(panicked()));
    match result {
        Ok(()) => {
            out.finish_line();
            ExitCode::SUCCESS
        }
        Err(e) => {
            out.error(&e, &command);
            ExitCode::from(e.exit as u8)
        }
    }
}

/// Runs one command.
pub fn run(command: Command, out: &Arc<Out>) -> Result<(), CliError> {
    match command {
        Command::Ai(command) => ai::run(command, out),
        Command::Image(command) => images::run(command, out),
        Command::Apply(args) => tools::apply(&args, out),
        Command::Revert { folder } => tools::revert(&folder, out),
        Command::Render(args) => tools::render(&args, out),
        Command::Template(args) => tools::template(&args, out),
        Command::Packs(command) => tools::packs(command, out),
    }
}

/// The async runtime the downloads and requests run on: one thread is plenty for one job.
pub(crate) fn runtime() -> Result<tokio::runtime::Runtime, CliError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::bug("FolderSkin couldn't start its worker.", e.to_string()))
}

/// A command line clap couldn't read. Help and the version are printed as asked; a mistake is
/// clap's own message (which says what was wrong and how the command goes), then the same
/// request for help every error ends with.
fn usage(e: clap::Error, json: bool, command: &str) -> ExitCode {
    use clap::error::ErrorKind;
    let code = e.exit_code();
    let informational = matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion);
    if informational || (!json && e.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand) {
        let _ = e.print();
        return ExitCode::from(code as u8);
    }
    let error = usage_error(&e);
    if json {
        let out = Out::new(true, false);
        out.error(&error, command);
    } else {
        let _ = e.print();
        eprintln!("\nAsk Claude: {}", error.ask(command));
    }
    ExitCode::from(Exit::Usage as u8)
}

/// clap's complaint as an error: its message, up to the usage line after it, as one sentence.
fn usage_error(e: &clap::Error) -> CliError {
    let text = e.render().to_string();
    let message = text
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let message = message
        .trim_start_matches("error: ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = message.chars();
    let why = match chars.next() {
        Some(first) => format!(
            "{}{}.",
            first.to_uppercase(),
            chars.as_str().trim_end_matches('.')
        ),
        None => "The command line couldn't be read.".to_string(),
    };
    CliError::usage("That command isn't quite right.", why)
        .fix("See what it takes with --help, e.g. folderskin ai gen --help")
}

/// The last panic's message and place, kept by the hook for the bug report.
static PANIC: Mutex<Option<String>> = Mutex::new(None);

/// Replaces the default panic message (a stack of Rust internals) with a note of it for the
/// friendly bug report `main` prints.
fn catch_panics() {
    std::panic::set_hook(Box::new(|info| {
        let place = info
            .location()
            .map_or_else(|| "an unknown place".into(), |l| l.to_string());
        let message = info.payload_as_str().unwrap_or("no message");
        if let Ok(mut slot) = PANIC.lock() {
            *slot = Some(format!("It stopped at {place}: {message}."));
        }
    }));
}

fn panicked() -> CliError {
    let why = PANIC
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
        .unwrap_or_else(|| "It stopped unexpectedly.".into());
    CliError::bug("FolderSkin hit a bug.", why)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_command_line_says_what_was_wrong_in_a_sentence() {
        let e = Cli::try_parse_from(["folderskin", "ai", "gen"]).unwrap_err();
        let error = usage_error(&e);
        assert_eq!(error.exit, Exit::Usage);
        assert_eq!(
            error.why,
            "The following required arguments were not provided: <IDEA>."
        );
        let e = Cli::try_parse_from(["folderskin", "ai", "gen", "x", "--tier", "q5"]).unwrap_err();
        assert!(usage_error(&e)
            .why
            .starts_with("Invalid value 'q5' for '--tier <TIER>'"));
    }

    #[test]
    fn a_panic_becomes_a_bug_report() {
        catch_panics();
        let result = std::panic::catch_unwind(|| panic!("the sky fell"));
        assert!(result.is_err());
        let e = panicked();
        assert_eq!((e.code.as_str(), e.exit), ("bug", Exit::Bug));
        assert!(e.why.contains("the sky fell"), "{e:?}");
        assert!(e.why.contains("lib.rs"), "{e:?}");
        assert!(e.fix[0].contains(error::ISSUES_URL));
        let _ = std::panic::take_hook();
    }
}
