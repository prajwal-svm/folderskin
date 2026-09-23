//! What a long job says while it runs: the events the command line prints (or writes as NDJSON
//! with `--json`) and the app will show.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// One thing that happened. Serialised with a `type` tag, one JSON object per line:
///
/// ```json
/// {"type":"progress","step":3,"steps":8}
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A new part of the job has started.
    Stage { stage: Stage, message: String },
    /// A painting step finished: `step` of `steps`.
    Progress { step: u32, steps: u32 },
    /// Bytes of `file` downloaded so far, out of `total`.
    Download { file: String, done: u64, total: u64 },
    /// Something worth reading, and how much it matters.
    Log { level: Level, message: String },
    /// Something the job made. `path` is `None` for a result that is only a report.
    Result {
        path: Option<PathBuf>,
        kind: String,
        meta: serde_json::Value,
    },
    /// The job failed: what happened, why, what to try, and a ready-to-paste request for help.
    Error {
        code: String,
        what: String,
        why: String,
        fix: Vec<String>,
        ask: String,
    },
}

/// The parts of a job, in the order they usually come.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Looking at the computer and what is installed.
    Check,
    /// Fetching the runtime, the models or a tool.
    Download,
    /// Checking a download against its published hash.
    Verify,
    /// Unpacking or installing what was downloaded.
    Install,
    /// The runtime is loading the model.
    Load,
    /// The model is painting.
    Paint,
    /// The painting is turned into pixels.
    Decode,
    /// The picture is cleaned up: a paper margin cut, a folder cut out.
    Finish,
    /// Sending the request to an image provider.
    Request,
    /// Drawing the picture as the folder the app makes of it.
    Preview,
    /// Putting the picture on a folder.
    Apply,
}

/// How much a [`Event::Log`] matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// The runtime's own output, for when something needs looking into.
    Debug,
    Info,
    Warn,
    Error,
}

/// Where a job's events go. Cheap to clone, and callable from any thread, because the runtime's
/// output is read on threads of its own.
#[derive(Clone)]
pub struct Reporter(Arc<dyn Fn(Event) + Send + Sync>);

impl Reporter {
    pub fn new(f: impl Fn(Event) + Send + Sync + 'static) -> Reporter {
        Reporter(Arc::new(f))
    }

    /// A reporter that drops everything.
    pub fn silent() -> Reporter {
        Reporter::new(|_| {})
    }

    pub fn emit(&self, event: Event) {
        (self.0)(event)
    }

    pub fn stage(&self, stage: Stage, message: impl Into<String>) {
        self.emit(Event::Stage {
            stage,
            message: message.into(),
        })
    }

    pub fn log(&self, level: Level, message: impl Into<String>) {
        self.emit(Event::Log {
            level,
            message: message.into(),
        })
    }
}

impl std::fmt::Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reporter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn events_are_tagged_snake_case_json() {
        let json = |e: &Event| serde_json::to_string(e).unwrap();
        assert_eq!(
            json(&Event::Progress { step: 3, steps: 8 }),
            r#"{"type":"progress","step":3,"steps":8}"#
        );
        assert_eq!(
            json(&Event::Stage {
                stage: Stage::Paint,
                message: "Painting".into()
            }),
            r#"{"type":"stage","stage":"paint","message":"Painting"}"#
        );
        let error = Event::Error {
            code: "models_missing".into(),
            what: "a".into(),
            why: "b".into(),
            fix: vec!["c".into()],
            ask: "d".into(),
        };
        assert!(json(&error).starts_with(r#"{"type":"error","code":"models_missing""#));
        let result = Event::Result {
            path: None,
            kind: "report".into(),
            meta: serde_json::json!({"ok": true}),
        };
        assert_eq!(
            json(&result),
            r#"{"type":"result","path":null,"kind":"report","meta":{"ok":true}}"#
        );
        for e in [error, result] {
            assert_eq!(serde_json::from_str::<Event>(&json(&e)).unwrap(), e);
        }
    }

    #[test]
    fn a_reporter_hands_every_event_on() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let reporter = Reporter::new(move |e| sink.lock().unwrap().push(e));
        reporter.stage(Stage::Load, "Loading");
        reporter.clone().log(Level::Warn, "careful");
        Reporter::silent().log(Level::Error, "dropped");
        assert_eq!(seen.lock().unwrap().len(), 2);
    }
}
