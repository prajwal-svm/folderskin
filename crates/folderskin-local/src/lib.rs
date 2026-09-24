//! Local image generation for FolderSkin: open-weight models that run on this computer, with no
//! API key and no account.
//!
//! Two models, the same on every platform, both Apache-2.0:
//!
//! * Z-Image-Turbo (6B, 8 steps): text to picture, the workhorse for artwork.
//! * FLUX.2 [klein] 4B (4 steps): works from pictures. Reference photos, and FolderSkin's own
//!   blank folder repainted for a whole-folder skin.
//!
//! Two runtimes run them: stable-diffusion.cpp on Windows and Linux (CUDA for NVIDIA on Windows,
//! Vulkan for every other GPU) and on a Mac's GPU through Metal, whose `--auto-fit` streams
//! weights from RAM when they don't fit in VRAM (which is what lets a 4 GB laptop GPU run the
//! 8-bit models); and mflux on Apple Silicon, the MLX port of the same two models.
//!
//! The command line (`folderskin ai …`) is the first front end; the app can call the same
//! functions in-process. Everything slow is an `async fn` that takes a [`Reporter`] for its
//! [`Event`]s and a [`CancelToken`]:
//!
//! ```no_run
//! # async fn example() -> Result<(), folderskin_local::Error> {
//! use folderskin_local::{detect, generate, setup, CancelToken, Job, Reporter, Runtime, Settings};
//! let machine = detect();
//! let settings = Settings::for_machine(&machine);
//! let (reporter, cancel) = (Reporter::new(|event| println!("{event:?}")), CancelToken::new());
//! setup(&machine, &settings, Runtime::Pinned, &reporter, &cancel).await?;
//! let picture = generate(&Job::new("a lighthouse at dusk"), &settings, "out".as_ref(), &reporter, &cancel).await?;
//! println!("{}", picture.path.display());
//! # Ok(()) }
//! ```

pub mod cancel;
pub mod command;
pub mod download;
pub mod error;
pub mod event;
pub mod generate;
pub mod machine;
pub mod manifest;
pub mod paths;
pub mod progress;
pub mod prompts;
pub mod run;
pub mod setup;
pub mod unzip;

pub use cancel::CancelToken;
pub use error::{Class, Error};
pub use event::{Event, Level, Reporter, Stage};
pub use generate::{
    check_ready, generate, random_seed, slug, Job, Picture, Provenance, Settings, MAX_SEED,
};
pub use machine::{detect, pick_backend, pick_tier, Backend, Machine, Tier};
pub use manifest::{Model, ModelId, MODELS};
pub use paths::home;
pub use prompts::{compose, Shape, Style, STYLES};
pub use setup::{download_size, is_set_up, setup, status, Runtime, Status};
