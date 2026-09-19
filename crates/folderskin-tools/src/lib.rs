//! Library half of folderskin-tools: how the app takes a picture, the pack checks, making a pack
//! from pictures, and the CLI definition. The binary (`main.rs`) wires them to the compositor and
//! the file system.

pub mod cli;
pub mod make;
pub mod packs;
pub mod skin;
