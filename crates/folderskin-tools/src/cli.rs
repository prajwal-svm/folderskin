//! Command-line surface of folderskin-tools (clap derive).

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "folderskin-tools",
    version,
    about = "Make and check FolderSkin skin packs, preview skins, and apply them from a terminal"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render a picture (or a solid colour) as the folder icon the app makes of it, to a PNG
    Render {
        /// Picture to render (PNG, JPEG, WebP). A finished folder, cut out or on magenta, is used
        /// as it is; anything else is wrapped onto FolderSkin's folder, as the app does
        image: Option<PathBuf>,
        /// Use a solid colour instead of a picture, e.g. 2A9D8F
        #[arg(long, value_name = "RRGGBB")]
        solid: Option<String>,
        #[arg(long, default_value = "preview.png")]
        out: PathBuf,
        /// Output size in pixels (square)
        #[arg(long, default_value_t = 1024)]
        size: u32,
        /// Focus point that stays centred in the crop, e.g. 0.5,0.4 (artwork only)
        #[arg(long, value_parser = parse_focus)]
        focus: Option<(f32, f32)>,
    },
    /// Write a 1024×958 safe-area template showing where the tab, paper and front panel land
    Guide {
        #[arg(long, default_value = "guide.png")]
        out: PathBuf,
    },
    /// Write the layers the composer draws a design between: back.png, front.png, middle.png,
    /// top.png and outline.png
    ComposerLayers {
        /// Folder to write them into; made if it isn't there
        #[arg(long)]
        out: PathBuf,
        /// Edge of each layer in pixels (square)
        #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(16..=4096))]
        size: u32,
    },
    /// Validate the app icon source (1024×1024 PNG with alpha) and print the command that builds the icon set
    AppIcon {
        #[arg(long = "in", default_value = "assets/logo/app-icon-1024.png")]
        input: PathBuf,
        #[arg(long, default_value = "src-tauri/icons")]
        out: PathBuf,
    },
    /// Apply a picture to a folder from the terminal, the way the app would
    Apply {
        folder: PathBuf,
        /// Any picture file
        #[arg(long)]
        image: PathBuf,
        /// Focus point that stays centred in the crop, e.g. 0.5,0.4 (artwork only)
        #[arg(long, value_parser = parse_focus)]
        focus: Option<(f32, f32)>,
    },
    /// Put the default icon back
    Revert { folder: PathBuf },
    /// Make, check and index community skin packs
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
        /// The largest a picture may be, in KB, if less than the pack limit of 2048
        #[arg(long, value_name = "KB")]
        max_kb: Option<usize>,
    },
    /// Make a pack from pictures: finished folders (on magenta or transparency) are cut out,
    /// everything is shrunk and compressed to fit, and pack.json is written
    Make {
        /// Pictures (PNG, JPEG or WebP), or folders of them, taken in name order
        #[arg(required = true)]
        pictures: Vec<PathBuf>,
        /// The pack's id, which is also its folder's name: lower-case words joined by dashes
        #[arg(long)]
        id: String,
        /// The pack's name as the app shows it
        #[arg(long)]
        name: String,
        /// Tags for every skin, comma-separated; the first one names the pack
        #[arg(long, value_delimiter = ',', required = true)]
        tags: Vec<String>,
        /// The GitHub name of whoever made the pictures
        #[arg(long)]
        author: String,
        /// CC0-1.0, CC-BY-4.0 or MIT
        #[arg(long, default_value = "CC0-1.0")]
        license: String,
        /// The community folder, holding packs/
        #[arg(long, default_value = "community")]
        dir: PathBuf,
        /// The largest a picture may be, in KB. The pack limit is 2048; smaller pictures make a
        /// pack quicker to add
        #[arg(long, value_name = "KB", default_value_t = 400)]
        max_kb: usize,
        /// Also write a PNG showing every skin as the folder it makes
        #[arg(long, value_name = "PNG")]
        preview: Option<PathBuf>,
        /// Cut away a flat backdrop of any colour, not only magenta: for renders whose #FF00FF
        /// drifted to pink or raspberry. Only the backdrop reaching the edge goes
        #[arg(long)]
        flat_backdrop: bool,
    },
    /// Check every pack, then write <dir>/index.json and <dir>/previews/<id>.png (deterministic)
    Index {
        /// The community folder, holding packs/
        #[arg(long, default_value = "community")]
        dir: PathBuf,
    },
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
    fn parses_guide_and_apply_with_a_picture() {
        match Cli::parse_from(["folderskin-tools", "guide"]).command {
            Command::Guide { out } => assert_eq!(out, PathBuf::from("guide.png")),
            other => panic!("{other:?}"),
        }
        let cli = Cli::parse_from([
            "folderskin-tools",
            "apply",
            "/tmp/folder",
            "--image",
            "koi.webp",
            "--focus",
            "0.3,0.6",
        ]);
        match cli.command {
            Command::Apply {
                folder,
                image,
                focus,
            } => {
                assert_eq!(folder, PathBuf::from("/tmp/folder"));
                assert_eq!(image, PathBuf::from("koi.webp"));
                assert_eq!(focus, Some((0.3, 0.6)));
            }
            other => panic!("{other:?}"),
        }
        assert!(
            Cli::try_parse_from(["folderskin-tools", "apply", "/tmp/folder"]).is_err(),
            "apply needs a picture"
        );
        for gone in [
            vec!["folderskin-tools", "apply", "/tmp/f", "--skin", "aurora"],
            vec!["folderskin-tools", "skin", "gen"],
        ] {
            assert!(Cli::try_parse_from(&gone).is_err(), "{gone:?}");
        }
    }

    #[test]
    fn parses_composer_layers_at_1024_unless_told_otherwise() {
        let parse = |args: &[&str]| {
            Cli::try_parse_from([&["folderskin-tools", "composer-layers"], args].concat())
                .map(|cli| cli.command)
        };
        match parse(&["--out", "docs/images/composer"]).unwrap() {
            Command::ComposerLayers { out, size } => {
                assert_eq!(out, PathBuf::from("docs/images/composer"));
                assert_eq!(size, 1024);
            }
            other => panic!("{other:?}"),
        }
        match parse(&["--out", "/tmp/layers", "--size", "2048"]).unwrap() {
            Command::ComposerLayers { size, .. } => assert_eq!(size, 2048),
            other => panic!("{other:?}"),
        }
        assert!(parse(&[]).is_err(), "it needs a folder to write to");
        for size in ["0", "8", "8192", "big"] {
            assert!(parse(&["--out", "x", "--size", size]).is_err(), "{size}");
        }
    }

    #[test]
    fn parses_packs_commands_with_community_as_the_default_folder() {
        match Cli::parse_from(["folderskin-tools", "packs", "check"]).command {
            Command::Packs {
                command: PacksCommand::Check { dir, max_kb },
            } => {
                assert_eq!(dir, PathBuf::from("community"));
                assert_eq!(max_kb, None);
            }
            other => panic!("{other:?}"),
        }
        match Cli::parse_from(["folderskin-tools", "packs", "index", "--dir", "/tmp/c"]).command {
            Command::Packs {
                command: PacksCommand::Index { dir },
            } => assert_eq!(dir, PathBuf::from("/tmp/c")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_packs_make_into_the_community_folder_by_default() {
        let cli = Cli::parse_from([
            "folderskin-tools",
            "packs",
            "make",
            "renders/",
            "extra.png",
            "--id",
            "3d",
            "--name",
            "3D",
            "--tags",
            "3d,glossy",
            "--author",
            "prajwal-svm",
        ]);
        match cli.command {
            Command::Packs {
                command:
                    PacksCommand::Make {
                        pictures,
                        tags,
                        dir,
                        max_kb,
                        license,
                        flat_backdrop,
                        ..
                    },
            } => {
                assert_eq!(
                    pictures,
                    [PathBuf::from("renders/"), PathBuf::from("extra.png")]
                );
                assert_eq!(tags, ["3d", "glossy"]);
                assert_eq!(dir, PathBuf::from("community"));
                assert_eq!(max_kb, 400);
                assert_eq!(license, "CC0-1.0");
                assert!(!flat_backdrop);
            }
            other => panic!("{other:?}"),
        }
    }
}
