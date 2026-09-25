//! Command-line surface of folderskin-tools (clap derive).

use clap::{ArgGroup, Args, Parser, Subcommand};
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
    /// Write the blank folder an image model repaints (FolderSkin's folder in flat grey, centred
    /// on a flat backdrop), and optionally its silhouette as a mask: white inside the folder
    Template {
        #[arg(long, default_value = "template.png")]
        out: PathBuf,
        #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(64..=4096))]
        width: u32,
        #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(64..=4096))]
        height: u32,
        /// The colour around the folder
        #[arg(long, default_value = "FF00FF", value_name = "RRGGBB")]
        backdrop: String,
        /// Also write the folder's silhouette here, white on black, lined up with the template
        #[arg(long, value_name = "PNG")]
        mask: Option<PathBuf>,
    },
    /// Write the layers the composer draws a design between: back.png, front.png, middle.png,
    /// top.png and outline.png, and the same for Windows' folder in windows/
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
    /// Look after the community service: packs shared from the app, their review, and pulling
    /// approved ones into the packs repository (github.com/prajwal-svm/folderskin-community)
    Community {
        #[command(subcommand)]
        command: CommunityCommand,
    },
}

/// The community service and the maintainer's key, for every command that talks to it.
#[derive(Args, Debug)]
pub struct Service {
    /// The service's address, e.g. https://community.example.org
    #[arg(long, env = "FOLDERSKIN_COMMUNITY_API")]
    pub api: String,
    /// The maintainer's key, from `community keygen`
    #[arg(long, env = "FOLDERSKIN_ADMIN_KEY", value_name = "FILE")]
    pub key: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum CommunityCommand {
    /// Make a maintainer's signing key; its public half goes in ADMIN_KEYS in wrangler.toml
    Keygen {
        /// Where to save it. Whoever has this file can approve packs, so keep it private
        #[arg(long)]
        out: PathBuf,
    },
    /// List packs waiting for review, flagged ones first
    Queue {
        /// waiting, flagged, pending, approved or open
        #[arg(long, default_value = "waiting")]
        status: String,
        #[command(flatten)]
        service: Service,
    },
    /// Approve a pack, or turn it down with reasons from docs/PACK-TERMS.md
    Decide {
        /// The submission, sub_…
        id: String,
        #[arg(value_parser = ["approve", "reject"])]
        decision: String,
        /// A reason code such as quality, brand or sexual; repeat for more. Needed to reject
        #[arg(long = "reason")]
        reasons: Vec<String>,
        /// A note the author reads
        #[arg(long, default_value = "")]
        note: String,
        #[command(flatten)]
        service: Service,
    },
    /// Take a pack down at once, published or still waiting
    Takedown {
        id: String,
        /// A reason code from docs/PACK-TERMS.md; repeat for more
        #[arg(long = "reason", required = true)]
        reasons: Vec<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[command(flatten)]
        service: Service,
    },
    /// List the reports people sent, newest first, with how to reach whoever sent each one
    Reports {
        /// How many days back to look, up to 180
        #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u32).range(1..=180))]
        days: u32,
        #[command(flatten)]
        service: Service,
    },
    /// Stop taking new packs and verifications: the kill switch
    Pause {
        /// Said to anyone who tries to share while it's paused
        #[arg(long, default_value = "")]
        message: String,
        #[command(flatten)]
        service: Service,
    },
    /// Take new packs and verifications again
    Resume {
        #[command(flatten)]
        service: Service,
    },
    /// Write approved packs into a folderskin-community checkout's packs/ as ordinary pack folders,
    /// checked like any other, under the ids the service gave them. The service hears about each
    /// pack as soon as it is written, unless --no-done leaves that to `community done`
    Pull {
        /// The packs folder
        #[arg(long, default_value = "packs")]
        out: PathBuf,
        /// Don't tell the service yet: `community done --from <FILE>` does, once the packs are
        /// pushed, so a pack that never gets there comes back next time. Needs --pulled
        #[arg(long, requires = "pulled")]
        no_done: bool,
        /// Save the packs that were pulled here, as a JSON list for `community done --from`
        #[arg(long, value_name = "FILE")]
        pulled: Option<PathBuf>,
        #[command(flatten)]
        service: Service,
    },
    /// Tell the service that the packs `community pull --no-done` pulled are in the repository
    /// now. Telling it twice does no harm
    Done {
        /// The list `community pull --pulled` saved
        #[arg(long, value_name = "FILE")]
        from: PathBuf,
        #[command(flatten)]
        service: Service,
    },
    /// Copy the published tree (v2/) to its public mirror through the service, which holds the
    /// bucket's keys: every file the mirror lacks, then head.json, once all of them are there
    Mirror {
        /// The folder holding v2/, such as a folderskin-community checkout
        #[arg(long, default_value = ".")]
        tree: PathBuf,
        /// Where the mirror serves the tree from, such as https://packs.folderskin.app
        #[arg(long, value_name = "URL")]
        public: String,
        #[command(flatten)]
        service: Service,
    },
}

#[derive(Subcommand, Debug)]
pub enum PacksCommand {
    /// Check every pack in <dir>/packs the way the app will; exit 1 on problems
    Check {
        /// The folderskin-community checkout, holding packs/
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// The largest a picture may be, in KB, if less than the pack limit of 2048
        #[arg(long, value_name = "KB")]
        max_kb: Option<usize>,
        /// Also turn down a pack whose id isn't a generated one, a name and six random characters
        /// such as classic-art-k7q2mx
        #[arg(long)]
        require_generated_ids: bool,
    },
    /// Make a pack from pictures: finished folders (on magenta or transparency) are cut out,
    /// everything is shrunk and compressed to fit, and pack.json is written. A new pack gets an
    /// id of its own, its name and six random characters, which is its folder's name
    Make {
        /// Pictures (PNG, JPEG or WebP), or folders of them, taken in name order
        #[arg(required = true)]
        pictures: Vec<PathBuf>,
        /// Make the pack with this id again, which has to be in packs/ already: its folder is
        /// replaced and it keeps its id. Leave it out for a new pack
        #[arg(long)]
        id: Option<String>,
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
        /// The folderskin-community checkout, holding packs/
        #[arg(long, default_value = ".")]
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
    /// Give packs generated ids, a name and six random characters such as classic-art-k7q2mx:
    /// each folder moves with git mv, featured.json and official.json follow, and moved.json
    /// records every move. The rename is staged, ready to commit
    #[command(group(ArgGroup::new("which").required(true).args(["all", "id"])))]
    Rename {
        /// The folderskin-community checkout, holding packs/
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Every pack whose id isn't a generated one; packs that have one are left alone
        #[arg(long)]
        all: bool,
        /// One pack, by its id
        id: Option<String>,
        /// The generated id it gets, instead of a new one; no pack can have it or have had it
        #[arg(long, requires = "id", conflicts_with = "all", value_name = "NEW_ID")]
        to: Option<String>,
    },
    /// Check every pack, then write <dir>/index.json and <dir>/previews/<id>.png (deterministic).
    /// Each pack is dated by the commit that added it, under its first id when moved.json says it
    /// moved, and marked official when official.json lists it
    Index {
        /// The folderskin-community checkout, holding packs/ and, if there is one, official.json
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Check every pack, then write the tree the app searches: a SQLite catalog, thumbnails,
    /// preview strips, pictures and manifests named after their contents, and head.json last
    /// (deterministic; only what changed is written)
    Catalog {
        /// The folderskin-community checkout, holding packs/ and, if there are, featured.json and
        /// official.json
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Where the tree goes (default: v2 in that folder, beside packs/)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Another https:// folder serving the same tree, which the app tries first; repeatable
        #[arg(long = "mirror", value_name = "URL")]
        mirrors: Vec<String>,
    },
}

/// Where `packs catalog` writes its tree: `--out`, or `v2` in the packs folder it reads, as
/// `packs index` writes into that folder too.
pub fn catalog_out(dir: &std::path::Path, out: Option<PathBuf>) -> PathBuf {
    out.unwrap_or_else(|| dir.join("v2"))
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
    fn parses_template_on_magenta_at_1024_unless_told_otherwise() {
        match Cli::parse_from(["folderskin-tools", "template"]).command {
            Command::Template {
                out,
                width,
                height,
                backdrop,
                mask,
            } => {
                assert_eq!(out, PathBuf::from("template.png"));
                assert_eq!((width, height), (1024, 1024));
                assert_eq!(backdrop, "FF00FF");
                assert_eq!(mask, None);
            }
            other => panic!("{other:?}"),
        }
        let cli = Cli::parse_from([
            "folderskin-tools",
            "template",
            "--width",
            "1024",
            "--height",
            "960",
            "--mask",
            "m.png",
        ]);
        match cli.command {
            Command::Template {
                width,
                height,
                mask,
                ..
            } => {
                assert_eq!((width, height), (1024, 960));
                assert_eq!(mask, Some(PathBuf::from("m.png")));
            }
            other => panic!("{other:?}"),
        }
        assert!(Cli::try_parse_from(["folderskin-tools", "template", "--width", "8"]).is_err());
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
    fn parses_packs_commands_run_inside_a_skins_checkout_by_default() {
        match Cli::parse_from(["folderskin-tools", "packs", "check"]).command {
            Command::Packs {
                command:
                    PacksCommand::Check {
                        dir,
                        max_kb,
                        require_generated_ids,
                    },
            } => {
                assert_eq!(dir, PathBuf::from("."));
                assert_eq!(max_kb, None);
                assert!(!require_generated_ids, "off unless asked");
            }
            other => panic!("{other:?}"),
        }
        match Cli::parse_from([
            "folderskin-tools",
            "packs",
            "check",
            "--require-generated-ids",
        ])
        .command
        {
            Command::Packs {
                command:
                    PacksCommand::Check {
                        require_generated_ids,
                        ..
                    },
            } => assert!(require_generated_ids),
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
    fn parses_packs_catalog_into_v2_beside_packs_by_default() {
        match Cli::parse_from(["folderskin-tools", "packs", "catalog"]).command {
            Command::Packs {
                command: PacksCommand::Catalog { dir, out, mirrors },
            } => {
                assert_eq!(dir, PathBuf::from("."));
                assert_eq!(catalog_out(&dir, out), PathBuf::from(".").join("v2"));
                assert!(mirrors.is_empty());
            }
            other => panic!("{other:?}"),
        }
        let cli = Cli::parse_from([
            "folderskin-tools",
            "packs",
            "catalog",
            "--out",
            "/tmp/v2",
            "--mirror",
            "https://a.example/v2",
            "--mirror",
            "https://b.example/v2",
        ]);
        match cli.command {
            Command::Packs {
                command: PacksCommand::Catalog { out, mirrors, .. },
            } => {
                assert_eq!(out, Some(PathBuf::from("/tmp/v2")));
                assert_eq!(mirrors, ["https://a.example/v2", "https://b.example/v2"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_catalog_goes_into_the_packs_folder_it_was_given() {
        let cli = Cli::parse_from([
            "folderskin-tools",
            "packs",
            "catalog",
            "--dir",
            "elsewhere/mine",
        ]);
        match cli.command {
            Command::Packs {
                command: PacksCommand::Catalog { dir, out, .. },
            } => assert_eq!(
                catalog_out(&dir, out),
                PathBuf::from("elsewhere/mine").join("v2")
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_community_commands_with_the_service_and_key() {
        let service = [
            "--api",
            "https://community.example.org",
            "--key",
            "admin.key",
        ];
        let parse = |args: &[&str]| {
            Cli::try_parse_from([&["folderskin-tools", "community"], args, &service].concat())
                .map(|cli| cli.command)
        };
        match parse(&["pull"]).unwrap() {
            Command::Community {
                command:
                    CommunityCommand::Pull {
                        out,
                        no_done,
                        pulled,
                        service,
                    },
            } => {
                assert_eq!(out, PathBuf::from("packs"));
                assert!(
                    !no_done,
                    "a person pulling by hand tells the service at once"
                );
                assert_eq!(pulled, None);
                assert_eq!(service.api, "https://community.example.org");
                assert_eq!(service.key, PathBuf::from("admin.key"));
            }
            other => panic!("{other:?}"),
        }
        match parse(&["pull", "--no-done", "--pulled", "/tmp/pulled.json"]).unwrap() {
            Command::Community {
                command:
                    CommunityCommand::Pull {
                        no_done, pulled, ..
                    },
            } => {
                assert!(no_done);
                assert_eq!(pulled, Some(PathBuf::from("/tmp/pulled.json")));
            }
            other => panic!("{other:?}"),
        }
        assert!(
            parse(&["pull", "--no-done"]).is_err(),
            "the packs left untold have to be written down somewhere"
        );
        match parse(&["done", "--from", "/tmp/pulled.json"]).unwrap() {
            Command::Community {
                command: CommunityCommand::Done { from, .. },
            } => assert_eq!(from, PathBuf::from("/tmp/pulled.json")),
            other => panic!("{other:?}"),
        }
        assert!(parse(&["done"]).is_err(), "done needs the list");
        match parse(&["mirror", "--public", "https://packs.example.org"]).unwrap() {
            Command::Community {
                command: CommunityCommand::Mirror { tree, public, .. },
            } => {
                assert_eq!(tree, PathBuf::from("."), "a checkout, holding v2/");
                assert_eq!(public, "https://packs.example.org");
            }
            other => panic!("{other:?}"),
        }
        assert!(
            parse(&["mirror"]).is_err(),
            "the mirror's address is needed"
        );
        match parse(&[
            "decide",
            "sub_aaaaaaaaaaaaaaaaaaaa",
            "reject",
            "--reason",
            "quality",
            "--reason",
            "brand",
        ])
        .unwrap()
        {
            Command::Community {
                command:
                    CommunityCommand::Decide {
                        decision, reasons, ..
                    },
            } => {
                assert_eq!(decision, "reject");
                assert_eq!(reasons, ["quality", "brand"]);
            }
            other => panic!("{other:?}"),
        }
        assert!(parse(&["decide", "sub_x", "maybe"]).is_err());
        assert!(
            parse(&["takedown", "sub_x"]).is_err(),
            "a takedown says why"
        );
        match parse(&["reports"]).unwrap() {
            Command::Community {
                command: CommunityCommand::Reports { days, .. },
            } => assert_eq!(days, 7),
            other => panic!("{other:?}"),
        }
        assert!(parse(&["reports", "--days", "0"]).is_err());
        assert!(Cli::try_parse_from(["folderskin-tools", "community", "keygen"]).is_err());
    }

    #[test]
    fn parses_packs_make_into_the_community_folder_by_default() {
        let make = [
            "folderskin-tools",
            "packs",
            "make",
            "renders/",
            "extra.png",
            "--name",
            "3D",
            "--tags",
            "3d,glossy",
            "--author",
            "prajwal-svm",
        ];
        match Cli::parse_from(make).command {
            Command::Packs {
                command:
                    PacksCommand::Make {
                        pictures,
                        id,
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
                assert_eq!(id, None, "a new pack gets an id of its own");
                assert_eq!(tags, ["3d", "glossy"]);
                assert_eq!(dir, PathBuf::from("."));
                assert_eq!(max_kb, 400);
                assert_eq!(license, "CC0-1.0");
                assert!(!flat_backdrop);
            }
            other => panic!("{other:?}"),
        }
        let again = Cli::parse_from([&make[..], &["--id", "3d-k7q2mx"]].concat());
        match again.command {
            Command::Packs {
                command: PacksCommand::Make { id, .. },
            } => assert_eq!(id.as_deref(), Some("3d-k7q2mx")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_packs_rename_for_every_pack_or_one() {
        let parse = |args: &[&str]| {
            Cli::try_parse_from([&["folderskin-tools", "packs", "rename"], args].concat())
                .map(|cli| cli.command)
        };
        match parse(&["--all", "--dir", "../folderskin-community"]).unwrap() {
            Command::Packs {
                command: PacksCommand::Rename { dir, all, id, to },
            } => {
                assert_eq!(dir, PathBuf::from("../folderskin-community"));
                assert!(all);
                assert_eq!((id, to), (None, None));
            }
            other => panic!("{other:?}"),
        }
        match parse(&["classic-art", "--to", "classic-art-k7q2mx"]).unwrap() {
            Command::Packs {
                command: PacksCommand::Rename { dir, all, id, to },
            } => {
                assert_eq!(dir, PathBuf::from("."));
                assert!(!all);
                assert_eq!(id.as_deref(), Some("classic-art"));
                assert_eq!(to.as_deref(), Some("classic-art-k7q2mx"));
            }
            other => panic!("{other:?}"),
        }
        for wrong in [
            vec![],
            vec!["--all", "classic-art"],
            vec!["--all", "--to", "x-k7q2mx"],
        ] {
            assert!(parse(&wrong).is_err(), "{wrong:?}");
        }
    }
}
