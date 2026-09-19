//! Library half of folderskin-tools: the procedural skins, the pack checks, making a pack from
//! pictures, and the CLI definition. The binary (`main.rs`) wires them to the compositor and the
//! file system.

pub mod cli;
pub mod gen;
pub mod make;
pub mod packs;
