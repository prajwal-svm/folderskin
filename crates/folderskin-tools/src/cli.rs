//! Command-line surface of folderskin-tools (clap derive).

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "folderskin-tools",
    version,
    about = "Generate, import and check FolderSkin skins"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Work with the skins in assets/skins
    Skin {
        #[command(subcommand)]
        command: SkinCommand,
    },
    /// Render a folder icon preview from a picture (or a solid colour) to a PNG
    Render {
        /// Picture to composite (PNG, JPEG, WebP)
        image: Option<PathBuf>,
        /// Use a solid colour instead of a picture, e.g. 2A9D8F
        #[arg(long, value_name = "RRGGBB")]
        solid: Option<String>,
        #[arg(long, default_value = "preview.png")]
        out: PathBuf,
        /// Output size in pixels (square)
        #[arg(long, default_value_t = 1024)]
        size: u32,
        /// Focus point that stays centred in the crop, e.g. 0.5,0.4
        #[arg(long, value_parser = parse_focus)]
        focus: Option<(f32, f32)>,
    },
    /// Validate the app icon source (1024×1024 PNG with alpha) and print the command that builds the icon set
    AppIcon {
        #[arg(long = "in", default_value = "assets/logo/app-icon-1024.png")]
        input: PathBuf,
        #[arg(long, default_value = "src-tauri/icons")]
        out: PathBuf,
    },
    /// Apply a skin (or a picture) to a folder from the terminal
    Apply {
        folder: PathBuf,
        /// Id of a skin in assets/skins/manifest.json
        #[arg(long, conflicts_with = "image")]
        skin: Option<String>,
        /// Any picture file
        #[arg(long, conflicts_with = "skin")]
        image: Option<PathBuf>,
        #[arg(long, default_value = "assets/skins")]
        dir: PathBuf,
        #[arg(long, value_parser = parse_focus)]
        focus: Option<(f32, f32)>,
    },
    /// Put the default icon back
    Revert { folder: PathBuf },
    /// Check the community skin packs, and write their index and previews
    Packs {
        #[command(subcommand)]
        command: PacksCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum PacksCommand {
    /// Check every pack in <dir>/packs the way the app will; exit 1 on problems
    Check {
        /// The community folder, holding packs/
        #[arg(long, default_value = "community")]
        dir: PathBuf,
    },
    /// Check every pack, then write <dir>/index.json and <dir>/previews/<id>.png (deterministic)
    Index {
        /// The community folder, holding packs/
        #[arg(long, default_value = "community")]
        dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum SkinCommand {
    /// Regenerate the ten built-in skins, the manifest and the previews (deterministic)
    Gen {
        #[arg(long, default_value = "assets/skins")]
        out: PathBuf,
        #[arg(long, default_value = "assets/previews")]
        previews: PathBuf,
    },
    /// Import a picture as a skin: crop to 1024×958, encode, preview, register in the manifest
    Add {
        image: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        collection: String,
        /// Focus point in 0..1 that stays centred in the crop (default 0.5,0.5)
        #[arg(long, value_parser = parse_focus)]
        focus: Option<(f32, f32)>,
        /// Keep PNG instead of JPEG (bigger, exact colours)
        #[arg(long)]
        lossless: bool,
        #[arg(long, default_value = "assets/skins")]
        dir: PathBuf,
        #[arg(long, default_value = "assets/previews")]
        previews: PathBuf,
        #[arg(long, default_value = "")]
        author: String,
        #[arg(long, default_value = "CC0-1.0")]
        license: String,
    },
    /// Validate the manifest and every skin file; exit 1 on problems
    Check {
        #[arg(long, default_value = "assets/skins")]
        dir: PathBuf,
    },
    /// Write a 1024×958 safe-area template showing where the tab, paper and front panel land
    Guide {
        #[arg(long, default_value = "guide.png")]
        out: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Encoding {
    Jpeg,
    Png,
}

pub fn parse_focus(s: &str) -> Result<(f32, f32), String> {
    let (a, b) = s
        .split_once(',')
        .ok_or_else(|| "focus must look like 0.5,0.4".to_string())?;
    let x: f32 = a
        .trim()
        .parse()
        .map_err(|_| "focus x must be a number".to_string())?;
    let y: f32 = b
        .trim()
        .parse()
        .map_err(|_| "focus y must be a number".to_string())?;
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return Err("focus values must be within 0..1".into());
    }
    Ok((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn focus_parsing() {
        assert_eq!(parse_focus("0.5,0.4").unwrap(), (0.5, 0.4));
        assert!(parse_focus("1.5,0").is_err());
        assert!(parse_focus("nope").is_err());
    }

    #[test]
    fn parses_skin_add() {
        let cli = Cli::parse_from([
            "folderskin-tools",
            "skin",
            "add",
            "in.jpg",
            "--id",
            "x",
            "--name",
            "X",
            "--collection",
            "pop",
            "--focus",
            "0.3,0.6",
        ]);
        match cli.command {
            Command::Skin {
                command: SkinCommand::Add { id, focus, .. },
            } => {
                assert_eq!(id, "x");
                assert_eq!(focus, Some((0.3, 0.6)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_packs_commands_with_community_as_the_default_folder() {
        match Cli::parse_from(["folderskin-tools", "packs", "check"]).command {
            Command::Packs {
                command: PacksCommand::Check { dir },
            } => assert_eq!(dir, PathBuf::from("community")),
            other => panic!("{other:?}"),
        }
        match Cli::parse_from(["folderskin-tools", "packs", "index", "--dir", "/tmp/c"]).command {
            Command::Packs {
                command: PacksCommand::Index { dir },
            } => assert_eq!(dir, PathBuf::from("/tmp/c")),
            other => panic!("{other:?}"),
        }
    }
}
