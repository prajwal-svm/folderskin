//! Reading a runtime's output as it runs.
//!
//! stable-diffusion.cpp writes log lines (`[INFO   ] image.cpp:862  - generating image: 1/1 -
//! seed 42`) ended by `\n`, and redraws progress bars in place with `\r`:
//!
//! ```text
//! \r  |============>                                     | 1/4 - 6.46s/it\x1b[K
//! ```
//!
//! Loading weights draws the same bar in bytes a second (`11/11 - 480.22MB/s`), and decoding
//! draws one per tile of the picture (`1/9 - 1.32it/s`); only the bars between "sampling using"
//! and "sampling completed" are painting steps. mflux (tqdm) draws `50%|█████     | 2/4 [00:05<…,
//! 2.50s/it]`, with no log lines around it, so there a bar counting to the job's own number of
//! steps counts as painting.

use crate::event::{Event, Level, Stage};

/// The painting stage's words when the model doesn't fit in the memory that is free, so the
/// runtime reads its weights from the disk as it paints: several times slower, and nothing else
/// would say why.
pub const SLOW_PAINT: &str =
    "Painting slowly: too little memory is free to hold the model, so it's read from the disk";

/// Where the runtime is, going by what it has said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Loading,
    Painting,
    Decoding,
}

/// Turns a runtime's output into events, one piece (between `\r` or `\n`) at a time.
#[derive(Debug)]
pub struct OutputParser {
    steps: u32,
    phase: Phase,
    last_step: u32,
    /// The runtime said it would keep the model's weights on the disk.
    from_disk: bool,
}

/// One piece of a runtime's output, understood.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// Something to pass on.
    Event(Event),
    /// A log line, for the error report if the run fails.
    Log(String),
    /// A progress bar that isn't a painting step, or an empty piece.
    Nothing,
}

impl OutputParser {
    /// A parser for a run of `steps` painting steps.
    pub fn new(steps: u32) -> OutputParser {
        OutputParser {
            steps,
            phase: Phase::Loading,
            last_step: 0,
            from_disk: false,
        }
    }

    /// Reads one piece of output.
    pub fn line(&mut self, raw: &str) -> Line {
        let text = strip_ansi(raw);
        let text = text.trim();
        if text.is_empty() {
            return Line::Nothing;
        }
        if let Some((step, total, per_step)) = progress_bar(text) {
            if !per_step {
                return Line::Nothing; // bytes a second: weights loading
            }
            let painting = match self.phase {
                Phase::Painting => true,
                Phase::Loading => total == self.steps,
                Phase::Decoding => false,
            };
            // Redrawn bars repeat a step; each is reported once.
            if !painting || step <= self.last_step {
                return Line::Nothing;
            }
            self.phase = Phase::Painting;
            self.last_step = step;
            return Line::Event(Event::Progress { step, steps: total });
        }
        let lower = text.to_lowercase();
        // stable-diffusion.cpp's auto-fit, when the free memory can't hold the weights:
        // "DiT params 6272 MiB ... -> compute CUDA0, params disk" and
        // `--params-backend "diffusion=disk,te=cpu,vae=cpu"`.
        if !self.from_disk
            && (lower.contains("params disk")
                || (lower.contains("params-backend") && lower.contains("=disk")))
        {
            self.from_disk = true;
            return Line::Event(Event::Log {
                level: Level::Warn,
                message: format!(
                    "too little memory is free to hold the model, so it's read from the disk as \
                     it paints, which is several times slower; closing other programs helps ({text})"
                ),
            });
        }
        if lower.contains("sampling using") || lower.contains("generating image") {
            if self.phase != Phase::Painting {
                self.phase = Phase::Painting;
                self.last_step = 0;
                return Line::Event(Event::Stage {
                    stage: Stage::Paint,
                    message: if self.from_disk {
                        SLOW_PAINT
                    } else {
                        "Painting"
                    }
                    .into(),
                });
            }
        } else if (lower.contains("sampling completed")
            // Not "using VAE for encoding / decoding", which it says while loading.
            || (lower.contains("decoding") && lower.contains("latent")))
            && self.phase != Phase::Decoding
        {
            self.phase = Phase::Decoding;
            return Line::Event(Event::Stage {
                stage: Stage::Decode,
                message: "Turning the painting into pixels".into(),
            });
        }
        Line::Log(text.to_string())
    }

    /// Whether any painting step has been seen: a run that fails before one never got going.
    pub fn painted(&self) -> bool {
        self.last_step > 0
    }
}

/// `(step, total, per_step)` of a progress bar: `per_step` when its rate is in steps (`s/it`,
/// `it/s`), not bytes.
fn progress_bar(text: &str) -> Option<(u32, u32, bool)> {
    let first = text.find('|')?;
    let last = text.rfind('|')?;
    if last <= first {
        return None;
    }
    let after = text[last + 1..].trim_start();
    let counts = after.split_whitespace().next()?;
    let (step, total) = counts.split_once('/')?;
    let (step, total) = (step.parse().ok()?, total.parse().ok()?);
    let per_step = after.contains("s/it") || after.contains("it/s");
    (total > 0).then_some((step, total, per_step))
}

/// `text` without terminal escape sequences (`\x1b[K` and colours).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // Parameters and intermediates, then one final byte in @..~.
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Splits a stream of bytes into the pieces a terminal would show: at `\n` and at `\r`.
#[derive(Debug, Default)]
pub struct Splitter {
    pending: Vec<u8>,
}

impl Splitter {
    /// Adds `bytes` and returns every piece they finished.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for &b in bytes {
            if b == b'\n' || b == b'\r' {
                if !self.pending.is_empty() {
                    out.push(String::from_utf8_lossy(&self.pending).into_owned());
                    self.pending.clear();
                }
            } else {
                self.pending.push(b);
            }
        }
        out
    }

    /// Whatever was left without an ending.
    pub fn finish(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| {
            let s = String::from_utf8_lossy(&self.pending).into_owned();
            self.pending.clear();
            s
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// stable-diffusion.cpp master-899's output for a klein picture, abridged, as it came through
    /// a pipe on Windows (RTX 3050 Ti, 4 GB).
    const SDCPP: &[u8] = b"[INFO   ] diffusion_engine.cpp:709  - loading diffusion model from 'C:/m/flux-2-klein-4b-Q8_0.gguf'\n\
[INFO   ] backend_fit.cpp:323  - auto-fit plan:\n\
[INFO   ] model_builders.cpp:586  - using VAE for encoding / decoding\n\
[INFO   ] ggml_graph_cut.cpp:1034 - qwen3 build cached graph cut plan done (taking 2 ms)\n\
\r  |##################################################| 1/1 - 1.33GB/s\x1b[K\r\n\
[INFO   ] model_loader.cpp:1309 - loading tensors completed, taking 0.29s\n\
\r  |################################                  | 7/11 - 5.37MB/s\x1b[K\r  |##################################################| 11/11 - 501.40MB/s\x1b[K\r\n\
[INFO   ] image.cpp:525  - get_learned_condition completed, taking 7.05s\n\
[INFO   ] image.cpp:862  - generating image: 1/1 - seed 42\n\
\r  |##################################################| 4/4 - 1.99GB/s\x1b[K\r\n\
[INFO   ] request.cpp:420  - sampling using Euler method\n\
\r  |============>                                     | 1/4 - 6.46s/it\x1b[K\r  |=========================>                        | 2/4 - 2.74s/it\x1b[K\r  |=====================================>            | 3/4 - 2.72s/it\x1b[K\r  |==================================================| 4/4 - 2.71s/it\x1b[K\r\n\
[INFO   ] image.cpp:895  - sampling completed, taking 14.64s\n\
[INFO   ] image.cpp:550  - decoding 1 latents\n\
[WARN   ] backend_fit.cpp:506  - VAE decode ran out of memory; retrying with spatial tiling\n\
\r  |=====>                                            | 1/9 - 1.32it/s\x1b[K\r  |===========>                                      | 2/9 - 2.23it/s\x1b[K\r\n\
[INFO   ] main.cpp:497  - save result image 0 to 'C:/out/probe.png' (success)\n";

    fn events(steps: u32, output: &[u8]) -> (Vec<Event>, Vec<String>) {
        let mut parser = OutputParser::new(steps);
        let mut splitter = Splitter::default();
        let (mut events, mut logs) = (Vec::new(), Vec::new());
        let mut pieces = splitter.push(output);
        pieces.extend(splitter.finish());
        for piece in pieces {
            match parser.line(&piece) {
                Line::Event(e) => events.push(e),
                Line::Log(l) => logs.push(l),
                Line::Nothing => {}
            }
        }
        (events, logs)
    }

    #[test]
    fn stable_diffusion_cpp_steps_become_progress_and_nothing_else_does() {
        let (events, logs) = events(4, SDCPP);
        let progress: Vec<(u32, u32)> = events
            .iter()
            .filter_map(|e| match e {
                Event::Progress { step, steps } => Some((*step, *steps)),
                _ => None,
            })
            .collect();
        assert_eq!(progress, [(1, 4), (2, 4), (3, 4), (4, 4)]);
        let stages: Vec<Stage> = events
            .iter()
            .filter_map(|e| match e {
                Event::Stage { stage, .. } => Some(*stage),
                _ => None,
            })
            .collect();
        assert_eq!(stages, [Stage::Paint, Stage::Decode]);
        assert!(logs
            .iter()
            .any(|l| l.contains("VAE decode ran out of memory")));
        assert!(
            logs.iter().all(|l| !l.contains('|')),
            "no bars in the log: {logs:?}"
        );
    }

    #[test]
    fn weights_read_from_the_disk_are_said_to_be_slow() {
        let low = b"[INFO   ] backend_fit.cpp:210  - RAM free 6809 MiB, params budget 4761 MiB\n\
[INFO   ] backend_fit.cpp:298  - DiT params 6272 MiB, compute 1180 MiB -> compute CUDA0, params disk\n\
[INFO   ] backend_fit.cpp:330  - auto-fit: --params-backend \"diffusion=disk,te=cpu,vae=cpu\"\n\
[INFO   ] request.cpp:420  - sampling using Euler method\n";
        let (seen, _) = events(8, low);
        let warned = seen
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Log {
                        level: Level::Warn,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(warned, 1, "said once: {seen:?}");
        assert!(seen.contains(&Event::Stage {
            stage: Stage::Paint,
            message: SLOW_PAINT.into()
        }));
        // With room for the weights, it is only painting.
        let (seen, _) = events(4, SDCPP);
        assert!(seen.contains(&Event::Stage {
            stage: Stage::Paint,
            message: "Painting".into()
        }));
        assert!(!seen.iter().any(|e| matches!(e, Event::Log { .. })));
    }

    #[test]
    fn a_tqdm_bar_counting_to_the_steps_is_painting() {
        let mflux = "\r  0%|          | 0/4 [00:00<?, ?it/s]\r 25%|##5       | 1/4 [00:05<00:15,  5.00s/it]\r 50%|#####     | 2/4 [00:10<00:10,  5.00s/it]\r100%|##########| 4/4 [00:20<00:00,  5.00s/it]\n";
        let (seen, _) = events(4, mflux.as_bytes());
        assert_eq!(
            seen,
            [
                Event::Progress { step: 1, steps: 4 },
                Event::Progress { step: 2, steps: 4 },
                Event::Progress { step: 4, steps: 4 },
            ]
        );
        // A bar counting something else before painting starts is not a step.
        let (seen, _) = events(4, b"\r 50%|#####     | 5/10 [00:01<00:01, 5.0it/s]\n");
        assert!(seen.is_empty(), "{seen:?}");
    }

    #[test]
    fn pieces_split_at_both_line_endings_and_escapes_go() {
        let mut s = Splitter::default();
        assert_eq!(s.push(b"one\r\ntw"), ["one"]);
        assert_eq!(s.push(b"o\rthree"), ["two"]);
        assert_eq!(s.finish().as_deref(), Some("three"));
        assert_eq!(s.finish(), None);
        assert_eq!(
            strip_ansi("\x1b[31m[ERROR]\x1b[0m bad\x1b[K"),
            "[ERROR] bad"
        );
        assert_eq!(
            progress_bar("| 3/8 - 2.1s/it"),
            None,
            "one bar edge is not a bar"
        );
        assert_eq!(
            progress_bar("  |===>   | 3/8 - 2.1s/it"),
            Some((3, 8, true))
        );
    }
}
