//! Library half of folderskin-tools: how the app takes a picture, the pack checks, making and
//! renaming packs, pulling approved ones from the community service, the published catalog and
//! its mirror, the composer's template layers, and the CLI definition. The binary (`main.rs`)
//! wires them to the compositor and the file system.

pub mod catalog;
pub mod cli;
pub mod composer;
mod git;
pub mod make;
pub mod mirror;
pub mod packs;
pub mod pull;
pub mod rename;
pub mod skin;
