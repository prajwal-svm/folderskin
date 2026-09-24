//! Everything the command line says. For a person: results on standard output, progress and
//! notes on standard error, a single self-redrawing line for progress in a terminal. With
//! `--json`: every event as one JSON object per line on standard output, errors included.

use crate::error::CliError;
use folderskin_local::{Event, Level, Reporter, Stage};
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Out {
    json: bool,
    verbose: bool,
    /// Standard error is a terminal, so progress can redraw one line in place.
    live: bool,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// How long the progress line on screen is, so it can be wiped before other output.
    status_len: usize,
    /// When a download's progress was last written as a line of its own (no terminal).
    last_download: Option<Instant>,
}

impl Out {
    pub fn new(json: bool, verbose: bool) -> Arc<Out> {
        Arc::new(Out {
            json,
            verbose,
            live: std::io::stderr().is_terminal(),
            state: Mutex::new(State::default()),
        })
    }

    pub fn json(&self) -> bool {
        self.json
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A reporter for the engine that prints through here.
    pub fn reporter(self: &Arc<Self>) -> Reporter {
        let out = self.clone();
        Reporter::new(move |event| out.event(&event))
    }

    /// Writes `event` the way the mode calls for.
    pub fn event(&self, event: &Event) {
        if self.json {
            if matches!(
                event,
                Event::Log {
                    level: Level::Debug,
                    ..
                }
            ) && !self.verbose
            {
                return;
            }
            let line = serde_json::to_string(event).unwrap_or_default();
            // Held while writing, so lines from two threads never interleave.
            let _state = self.lock();
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "{line}");
            let _ = stdout.flush();
            return;
        }
        match event {
            Event::Stage { stage, message } => {
                let skip = matches!(stage, Stage::Paint | Stage::Decode) && self.live;
                if !skip {
                    self.note(message);
                }
            }
            Event::Progress { step, steps } => {
                if self.live {
                    self.status(&format!("  painting {} {step}/{steps}", bar(*step, *steps)));
                } else {
                    self.note(&format!("  step {step} of {steps}"));
                }
            }
            Event::Download { file, done, total } => {
                let text = format!(
                    "{file}: {:.2} / {:.2} GB",
                    *done as f64 / (1u64 << 30) as f64,
                    *total as f64 / (1u64 << 30) as f64
                );
                if self.live {
                    self.status(&format!("  {} {text}", bar(*done, *total)));
                } else {
                    let mut state = self.lock();
                    let due = state
                        .last_download
                        .is_none_or(|t| t.elapsed() >= Duration::from_secs(2));
                    if due || done == total {
                        state.last_download = Some(Instant::now());
                        drop(state);
                        self.note(&text);
                    }
                }
            }
            Event::Log { level, message } => match level {
                Level::Debug if self.verbose => self.note(&format!("  | {message}")),
                Level::Debug => {}
                Level::Info => self.note(message),
                Level::Warn => self.note(&format!("warning: {message}")),
                Level::Error => self.note(&format!("error: {message}")),
            },
            Event::Result { .. } | Event::Error { .. } => {}
        }
    }

    /// A line on standard error, after wiping any progress line.
    pub fn note(&self, message: &str) {
        if self.json {
            return self.event(&Event::Log {
                level: Level::Info,
                message: message.to_string(),
            });
        }
        let mut state = self.lock();
        let mut stderr = std::io::stderr().lock();
        wipe(&mut stderr, &mut state);
        let prefix = if message.starts_with(' ') {
            ""
        } else {
            "folderskin: "
        };
        let _ = writeln!(stderr, "{prefix}{message}");
    }

    /// A warning, in either mode.
    pub fn warn(&self, message: &str) {
        self.event(&Event::Log {
            level: Level::Warn,
            message: message.to_string(),
        });
    }

    /// The progress line, redrawn in place.
    fn status(&self, text: &str) {
        let mut state = self.lock();
        let mut stderr = std::io::stderr().lock();
        let width = text.chars().count();
        let pad = state.status_len.saturating_sub(width);
        let _ = write!(stderr, "\r{text}{}", " ".repeat(pad));
        let _ = stderr.flush();
        state.status_len = width;
    }

    /// Something a command made or found. `human` goes to standard output for a person (or to
    /// standard error when standard output carries a picture); with `--json` the event does.
    pub fn result(
        &self,
        path: Option<&Path>,
        kind: &str,
        meta: serde_json::Value,
        human: &str,
        stdout_busy: bool,
    ) {
        if self.json {
            let event = Event::Result {
                path: path.map(Path::to_path_buf),
                kind: kind.to_string(),
                meta,
            };
            if stdout_busy {
                // The picture is on standard output and nothing else may be, so the result
                // goes to standard error, still one JSON object on a line of its own.
                let line = serde_json::to_string(&event).unwrap_or_default();
                let _state = self.lock();
                let _ = writeln!(std::io::stderr().lock(), "{line}");
                return;
            }
            return self.event(&event);
        }
        let mut state = self.lock();
        {
            let mut stderr = std::io::stderr().lock();
            wipe(&mut stderr, &mut state);
        }
        if human.is_empty() {
            return;
        }
        if stdout_busy {
            let _ = writeln!(std::io::stderr().lock(), "{human}");
        } else {
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "{human}");
            let _ = stdout.flush();
        }
    }

    /// The failure, as a block for a person or an event for a program.
    pub fn error(&self, error: &CliError, command: &str) {
        if self.json {
            return self.event(&error.to_event(command));
        }
        let mut state = self.lock();
        let mut stderr = std::io::stderr().lock();
        wipe(&mut stderr, &mut state);
        let _ = writeln!(stderr, "{}", error.render(command));
    }

    /// Ends a progress line so the next output starts on a line of its own.
    pub fn finish_line(&self) {
        let mut state = self.lock();
        let mut stderr = std::io::stderr().lock();
        wipe(&mut stderr, &mut state);
    }
}

/// Clears the progress line, if one is showing.
fn wipe(stderr: &mut impl Write, state: &mut State) {
    if state.status_len > 0 {
        let _ = write!(stderr, "\r{}\r", " ".repeat(state.status_len));
        state.status_len = 0;
    }
}

/// `[#####-----]`, `done` of `total`.
fn bar(done: impl Into<u64>, total: impl Into<u64>) -> String {
    let (done, total) = (done.into(), total.into().max(1));
    let filled = ((done.min(total) * 20) / total) as usize;
    format!("[{}{}]", "#".repeat(filled), "-".repeat(20 - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_fill_in_proportion() {
        assert_eq!(bar(0u32, 8u32), "[--------------------]");
        assert_eq!(bar(4u32, 8u32), "[##########----------]");
        assert_eq!(bar(9u64, 8u64), "[####################]");
        assert_eq!(bar(1u64, 0u64), "[####################]");
    }
}
