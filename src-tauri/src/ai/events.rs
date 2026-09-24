//! What a run tells the window while it works: the `AiEvent`s of src/state/chats.ts, sent over
//! the command's channel. The local engine's own events are turned into these here.

use folderskin_local::{Event, Level, Stage};
use serde::Serialize;

/// One report, tagged the way the chat reads it: `{"type":"progress","step":3,"steps":8}`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiEvent {
    /// A new part of the run; `message` is what the chat shows while it lasts.
    Stage { stage: String, message: String },
    /// A painting step finished.
    Progress { step: u32, steps: u32 },
    /// Bytes of `file` downloaded so far.
    Download { file: String, done: u64, total: u64 },
    /// A line for the details: "info", "warn" or "error".
    Log {
        level: &'static str,
        message: String,
    },
}

impl AiEvent {
    pub fn stage(stage: &str, message: impl Into<String>) -> AiEvent {
        AiEvent::Stage {
            stage: stage.to_string(),
            message: message.into(),
        }
    }

    pub fn info(message: impl Into<String>) -> AiEvent {
        AiEvent::Log {
            level: "info",
            message: message.into(),
        }
    }

    pub fn warn(message: impl Into<String>) -> AiEvent {
        AiEvent::Log {
            level: "warn",
            message: message.into(),
        }
    }
}

/// What the engine's `event` becomes for the chat. A stage keeps the engine's words when they
/// read well on their own ("Downloading z_image_turbo-Q8_0.gguf"); the ones that name the file
/// and seed ("lighthouse-none-42: FLUX.2 [klein] 4B, seed 42") show a plain stage instead, with
/// the engine's words in the details. The runtime's own output is detail too. A result or an
/// error isn't an event here: the command returns those.
pub fn from_engine(event: Event) -> Vec<AiEvent> {
    match event {
        Event::Stage { stage, message } => {
            let id = stage_id(stage);
            // Why this picture is slow is worth the headline, not the details.
            if message == folderskin_local::progress::SLOW_PAINT {
                return vec![AiEvent::stage(id, message)];
            }
            let plain = match stage {
                Stage::Check => "Looking at this computer",
                Stage::Load => "Loading the model",
                Stage::Paint => "Painting",
                Stage::Decode => "Turning the painting into pixels",
                Stage::Finish => "Finishing it",
                _ => return vec![AiEvent::stage(id, message)],
            };
            if message == plain {
                vec![AiEvent::stage(id, plain)]
            } else {
                vec![AiEvent::stage(id, plain), AiEvent::info(message)]
            }
        }
        Event::Progress { step, steps } => vec![AiEvent::Progress { step, steps }],
        Event::Download { file, done, total } => vec![AiEvent::Download { file, done, total }],
        Event::Log { level, message } => vec![AiEvent::Log {
            level: match level {
                // The runtime's own lines: what the details are for.
                Level::Debug | Level::Info => "info",
                Level::Warn => "warn",
                Level::Error => "error",
            },
            message,
        }],
        Event::Result { .. } | Event::Error { .. } => Vec::new(),
    }
}

/// The engine's name for a stage, as the chat's `stage` field carries it.
fn stage_id(stage: Stage) -> &'static str {
    match stage {
        Stage::Check => "check",
        Stage::Download => "download",
        Stage::Verify => "verify",
        Stage::Install => "install",
        Stage::Load => "load",
        Stage::Paint => "paint",
        Stage::Decode => "decode",
        Stage::Finish => "finish",
        Stage::Request => "request",
        Stage::Preview => "preview",
        Stage::Apply => "apply",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(e: &AiEvent) -> String {
        serde_json::to_string(e).unwrap()
    }

    #[test]
    fn events_are_tagged_the_way_the_chat_reads_them() {
        assert_eq!(
            json(&AiEvent::Progress { step: 3, steps: 8 }),
            r#"{"type":"progress","step":3,"steps":8}"#
        );
        assert_eq!(
            json(&AiEvent::stage("paint", "Painting")),
            r#"{"type":"stage","stage":"paint","message":"Painting"}"#
        );
        assert_eq!(
            json(&AiEvent::Download {
                file: "a.gguf".into(),
                done: 1,
                total: 2
            }),
            r#"{"type":"download","file":"a.gguf","done":1,"total":2}"#
        );
        assert_eq!(
            json(&AiEvent::warn("careful")),
            r#"{"type":"log","level":"warn","message":"careful"}"#
        );
    }

    #[test]
    fn a_stage_that_names_the_file_and_seed_shows_plainly_with_the_detail_in_the_log() {
        let events = from_engine(Event::Stage {
            stage: Stage::Load,
            message: "a-paper-boat-none-42: FLUX.2 [klein] 4B, seed 42".into(),
        });
        assert_eq!(
            events,
            vec![
                AiEvent::stage("load", "Loading the model"),
                AiEvent::info("a-paper-boat-none-42: FLUX.2 [klein] 4B, seed 42"),
            ]
        );
        // Already plain: said once.
        assert_eq!(
            from_engine(Event::Stage {
                stage: Stage::Paint,
                message: "Painting".into()
            }),
            vec![AiEvent::stage("paint", "Painting")]
        );
        // A download's own words read well as they are.
        assert_eq!(
            from_engine(Event::Stage {
                stage: Stage::Download,
                message: "Resuming klein.gguf".into()
            }),
            vec![AiEvent::stage("download", "Resuming klein.gguf")]
        );
    }

    #[test]
    fn progress_downloads_and_logs_pass_through() {
        assert_eq!(
            from_engine(Event::Progress { step: 2, steps: 4 }),
            vec![AiEvent::Progress { step: 2, steps: 4 }]
        );
        assert_eq!(
            from_engine(Event::Download {
                file: "vae.safetensors".into(),
                done: 10,
                total: 20
            }),
            vec![AiEvent::Download {
                file: "vae.safetensors".into(),
                done: 10,
                total: 20
            }]
        );
        let log = |level| {
            from_engine(Event::Log {
                level,
                message: "x".into(),
            })
        };
        // The runtime's own output is what the details are for, so it is shown, not dropped.
        assert_eq!(log(Level::Debug), vec![AiEvent::info("x")]);
        assert_eq!(log(Level::Warn), vec![AiEvent::warn("x")]);
        assert!(json(&log(Level::Error)[0]).contains(r#""level":"error""#));
    }

    #[test]
    fn results_and_errors_are_the_commands_to_return() {
        assert!(from_engine(Event::Result {
            path: None,
            kind: "report".into(),
            meta: serde_json::Value::Null
        })
        .is_empty());
        assert!(from_engine(Event::Error {
            code: "x".into(),
            what: "a".into(),
            why: "b".into(),
            fix: Vec::new(),
            ask: String::new()
        })
        .is_empty());
    }
}
