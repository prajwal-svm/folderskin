//! The command line's shape (clap derive).

use clap::{Args, Parser, Subcommand, ValueEnum};
use folderskin_tools::cli::{parse_focus, PacksCommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "folderskin",
    version,
    about = "Give your folders a skin, from a terminal: paint folder art with a model on this \
             computer or your own key, clean pictures up, and apply them",
    after_help = "Start with `folderskin ai doctor`, then `folderskin ai setup` and \
                  `folderskin ai gen \"a lighthouse at dusk\"`."
)]
pub struct Cli {
    /// Write every result, progress update and error as one JSON object per line
    #[arg(long, global = true)]
    pub json: bool,
    /// Also show everything the runtime prints
    #[arg(long, short, global = true)]
    pub verbose: bool,
    /// The folder artwork goes on, as in the app: mac or windows (default: the one chosen in the
    /// app)
    #[arg(long, global = true, value_enum)]
    pub look: Option<LookArg>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookArg {
    Mac,
    Windows,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Paint folder art: with open-weight models on this computer, or with your own API key
    #[command(subcommand)]
    Ai(AiCommand),
    /// Look at, clean up and adjust a picture before it goes on a folder
    #[command(subcommand)]
    Image(ImageCommand),
    /// Put a picture on a folder, the way the app does
    Apply(ApplyArgs),
    /// Put a folder's own icon back
    Revert {
        /// The folder whose custom icon should go
        folder: PathBuf,
    },
    /// Draw a picture (or a solid colour) as the folder icon the app makes of it, to a PNG
    Render(RenderArgs),
    /// Write the blank folder a model repaints, and optionally its silhouette
    Template(TemplateArgs),
    /// Make, check and index community skin packs
    #[command(subcommand)]
    Packs(PacksCommand),
}

#[derive(Subcommand, Debug)]
pub enum AiCommand {
    /// What this computer is, what is installed, and what `ai gen` would use
    Doctor(MachineArgs),
    /// Download the runtime and the models (about 15.7 GB at q8, 9.4 GB at q4), resuming and
    /// checking every file
    Setup(SetupArgs),
    /// Paint pictures from one idea
    Gen(GenArgs),
    /// Paint every brief in a JSON file
    Batch(BatchArgs),
    /// Paint every folder under a root from its own name, in one style, and apply them if asked
    Theme(ThemeArgs),
    /// List the style presets
    Styles,
    /// List the local models and the providers you can use with your own key
    Models,
    /// Show or change the defaults: provider, model, tier and backend
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
    /// Save, remove or test an API key for a provider
    #[command(subcommand)]
    Key(KeyCommand),
}

/// Which runtime and which weights: detected from the computer unless set here or in `ai config`.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineArgs {
    /// What runs the models
    #[arg(long, value_enum)]
    pub backend: Option<BackendArg>,
    /// q8: best, wants ~24 GB of RAM; q4: smaller and a little softer
    #[arg(long, value_enum)]
    pub tier: Option<TierArg>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendArg {
    Auto,
    Cuda,
    Vulkan,
    Metal,
    Cpu,
    Mlx,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TierArg {
    Auto,
    Q8,
    Q4,
}

#[derive(Args, Debug)]
pub struct SetupArgs {
    #[command(flatten)]
    pub machine: MachineArgs,
    /// pinned: the stable-diffusion.cpp build this was tested with; latest: the newest release
    #[arg(long, value_enum, default_value_t = RuntimeArg::Pinned)]
    pub runtime: RuntimeArg,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeArg {
    Pinned,
    Latest,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShapeArg {
    /// Wrapped onto FolderSkin's folder by the app (the normal case)
    #[default]
    Artwork,
    /// The whole folder painted as one object, cut out along FolderSkin's silhouette
    Folder,
}

#[derive(Args, Debug)]
pub struct GenArgs {
    /// The subject and the scene in plain words, e.g. "a retro film camera on a desk"; `-` reads
    /// it from standard input
    pub idea: String,
    /// A preset from `ai styles` (pop-art, anime, oil, sketch, woodblock…), or your own words
    #[arg(long, default_value = "none")]
    pub style: String,
    /// artwork: wrapped onto FolderSkin's folder; folder: the whole folder painted
    #[arg(long, value_enum, default_value_t = ShapeArg::Artwork)]
    pub shape: ShapeArg,
    /// A picture to paint from; repeat for several (local models take several, providers one)
    #[arg(long = "ref", value_name = "PICTURE")]
    pub refs: Vec<PathBuf>,
    /// How many to paint; each takes the next seed
    #[arg(short, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub n: u32,
    /// Where the seeds start, so a good picture can be painted again exactly (local models)
    #[arg(long, value_parser = parse_seed)]
    pub seed: Option<u64>,
    /// Local: auto, zimage or klein (pictures always go to klein). With --provider: its model id
    #[arg(long)]
    pub model: Option<String>,
    /// local (the default), or a provider for your own key: openai, xai, google, bfl, recraft,
    /// stability, ideogram
    #[arg(long)]
    pub provider: Option<String>,
    /// The file name, without extension (with -n 1)
    #[arg(long)]
    pub name: Option<String>,
    /// Where the pictures go
    #[arg(long, default_value = "folderskin-out")]
    pub out: PathBuf,
    /// Also put the (first) picture on this folder
    #[arg(long, value_name = "FOLDER")]
    pub apply: Option<PathBuf>,
    /// Send the idea to the model word for word, without FolderSkin's prompt around it
    #[arg(long)]
    pub raw: bool,
    /// Don't draw each picture as the folder the app makes of it
    #[arg(long)]
    pub no_preview: bool,
    /// Say what would be painted, and where it would go, without painting anything
    #[arg(long)]
    pub dry_run: bool,
    #[command(flatten)]
    pub machine: MachineArgs,
}

#[derive(Args, Debug)]
pub struct BatchArgs {
    /// A JSON list of briefs: [{"idea": …, "style": …, "shape": …, "refs": […], "n": 2, "name":
    /// …, "seed": …, "model": …}]; reference paths are relative to the file
    pub briefs: PathBuf,
    /// The style for briefs that don't name one
    #[arg(long, default_value = "none")]
    pub style: String,
    /// local (the default), or a provider for your own key
    #[arg(long)]
    pub provider: Option<String>,
    #[arg(long, default_value = "folderskin-out")]
    pub out: PathBuf,
    /// Say what every brief would paint, without painting anything
    #[arg(long)]
    pub dry_run: bool,
    #[command(flatten)]
    pub machine: MachineArgs,
}

#[derive(Args, Debug)]
pub struct ThemeArgs {
    /// The folder whose folders get painted
    pub root: PathBuf,
    /// A preset from `ai styles`, or your own words: it holds the whole drive together
    #[arg(long, default_value = "none")]
    pub style: String,
    #[arg(long, value_enum, default_value_t = ShapeArg::Artwork)]
    pub shape: ShapeArg,
    /// Local: klein unless told (twice as fast, and a drive has many folders). With --provider:
    /// its model id
    #[arg(long)]
    pub model: Option<String>,
    /// local (the default), or a provider for your own key
    #[arg(long)]
    pub provider: Option<String>,
    /// How many levels of folders below the root
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=5))]
    pub depth: u32,
    /// The first folder's seed; the next folder gets the next one
    #[arg(long, value_parser = parse_seed)]
    pub seed: Option<u64>,
    /// Put each picture on its folder, as the app would
    #[arg(long)]
    pub apply: bool,
    /// Where the pictures go (default folderskin-out/theme-<root>)
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// List the folders it would paint (and, with --apply, change), without painting or
    /// applying anything
    #[arg(long)]
    pub dry_run: bool,
    #[command(flatten)]
    pub machine: MachineArgs,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show one setting, or all of them
    Get { key: Option<ConfigKey> },
    /// Change a setting: `ai config set provider openai`, `ai config set tier q4`
    Set { key: ConfigKey, value: String },
    /// Go back to the default for a setting
    Unset { key: ConfigKey },
    /// Where the settings are kept
    Path,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigKey {
    /// local, or a provider for your own key
    Provider,
    /// The provider's model; for local, auto, zimage or klein
    Model,
    /// auto, q8 or q4
    Tier,
    /// auto, cuda, vulkan, metal, cpu or mlx
    Backend,
}

impl ConfigKey {
    pub fn id(self) -> &'static str {
        match self {
            ConfigKey::Provider => "provider",
            ConfigKey::Model => "model",
            ConfigKey::Tier => "tier",
            ConfigKey::Backend => "backend",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum KeyCommand {
    /// Save a key, typed or piped in (never as an argument, which would land in your shell's
    /// history). The app sees it too.
    Set { provider: String },
    /// Remove a saved key
    Clear { provider: String },
    /// Check a key with the provider, without generating anything
    Test { provider: String },
}

#[derive(Subcommand, Debug)]
pub enum ImageCommand {
    /// Crop to the folder's artwork shape (or any other) around a focus point, or to a box
    Crop(CropArgs),
    /// Cut away a paper margin or frame a model painted around its art
    Trim(OneImage),
    /// Cut a whole-folder picture out along FolderSkin's own silhouette
    Clip(OneImage),
    /// Key out a flat magenta backdrop (or any flat colour) and trim to what is left
    Cutout(CutoutArgs),
    /// Check a picture will land cleanly on a folder, and say what to fix
    Check {
        /// The picture; `-` reads standard input
        input: PathBuf,
    },
    /// More or less colour: -100 is grey, 100 twice as vivid
    Saturate(AmountArgs),
    /// Swap every colour for its opposite
    Invert(InvertArgs),
    /// Lighter or darker: -100 to 100
    Brightness(AmountArgs),
    /// More or less contrast: -100 to 100
    Contrast(AmountArgs),
    /// Several adjustments at once, as the composer applies them
    Adjust(AdjustArgs),
    /// What a picture is and what FolderSkin will make of it
    Info {
        /// The picture; `-` reads standard input
        input: PathBuf,
    },
    /// Draw a picture as the folder icon the app makes of it
    Render(RenderArgs),
    /// Write the blank folder a model repaints, and optionally its silhouette
    Template(TemplateArgs),
}

#[derive(Args, Debug)]
pub struct OneImage {
    /// The picture; `-` reads standard input
    pub input: PathBuf,
    /// Where the result goes; `-` writes a PNG to standard output (default: beside the input, or
    /// standard output for a picture from standard input)
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct CropArgs {
    #[command(flatten)]
    pub image: OneImage,
    /// artwork (1024 × 958, what the folder takes), square, or W:H such as 16:9
    #[arg(long, default_value = "artwork")]
    pub aspect: String,
    /// What stays in the middle, e.g. 0.5,0.4
    #[arg(long, value_parser = parse_focus)]
    pub focus: Option<(f32, f32)>,
    /// An exact box instead, in pixels: X,Y,WIDTH,HEIGHT
    #[arg(long = "box", value_name = "X,Y,W,H", value_parser = parse_box, conflicts_with = "focus")]
    pub crop_box: Option<(u32, u32, u32, u32)>,
}

#[derive(Args, Debug)]
pub struct CutoutArgs {
    #[command(flatten)]
    pub image: OneImage,
    /// Cut away a flat backdrop of any colour, not only magenta; only what reaches the edge goes
    #[arg(long)]
    pub flat_backdrop: bool,
}

#[derive(Args, Debug)]
pub struct AmountArgs {
    /// The picture; `-` reads standard input
    pub input: PathBuf,
    /// How much, from -100 to 100
    #[arg(allow_negative_numbers = true, value_parser = parse_amount)]
    pub amount: f64,
    /// Where the result goes; `-` writes a PNG to standard output (default: beside the input, or
    /// standard output for a picture from standard input)
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct InvertArgs {
    #[command(flatten)]
    pub image: OneImage,
    /// How far, from 0 to 100
    #[arg(long, default_value_t = 100.0, allow_negative_numbers = true, value_parser = parse_share)]
    pub amount: f64,
}

#[derive(Args, Debug)]
pub struct AdjustArgs {
    #[command(flatten)]
    pub image: OneImage,
    /// -100 to 100
    #[arg(long, allow_negative_numbers = true, value_parser = parse_amount)]
    pub brightness: Option<f64>,
    /// -100 to 100
    #[arg(long, allow_negative_numbers = true, value_parser = parse_amount)]
    pub contrast: Option<f64>,
    /// -100 to 100
    #[arg(long, allow_negative_numbers = true, value_parser = parse_amount)]
    pub saturation: Option<f64>,
    /// Degrees round the colour wheel, -180 to 180
    #[arg(long, allow_negative_numbers = true, value_parser = parse_hue)]
    pub hue: Option<f64>,
    /// 0 to 100
    #[arg(long, value_parser = parse_share)]
    pub grayscale: Option<f64>,
    /// 0 to 100
    #[arg(long, value_parser = parse_share)]
    pub sepia: Option<f64>,
    /// 0 to 100
    #[arg(long, value_parser = parse_share)]
    pub invert: Option<f64>,
}

#[derive(Args, Debug)]
pub struct RenderArgs {
    /// The picture (PNG, JPEG, WebP). A finished folder, cut out or on magenta, is used as it
    /// is; anything else is wrapped onto FolderSkin's folder, as the app does
    #[arg(conflicts_with = "solid")]
    pub image: Option<PathBuf>,
    /// A solid colour instead of a picture, e.g. 2A9D8F
    #[arg(long, value_name = "RRGGBB")]
    pub solid: Option<String>,
    #[arg(long, default_value = "preview.png")]
    pub out: PathBuf,
    /// Its size in pixels (square)
    #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(16..=2048))]
    pub size: u32,
    /// What stays in the middle of the crop, e.g. 0.5,0.4 (artwork only)
    #[arg(long, value_parser = parse_focus)]
    pub focus: Option<(f32, f32)>,
}

#[derive(Args, Debug)]
pub struct TemplateArgs {
    #[arg(long, default_value = "template.png")]
    pub out: PathBuf,
    #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(64..=4096))]
    pub width: u32,
    #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u32).range(64..=4096))]
    pub height: u32,
    /// The colour around the folder
    #[arg(long, default_value = "FF00FF", value_name = "RRGGBB")]
    pub backdrop: String,
    /// Also write the folder's silhouette here, white on black, lined up with the template
    #[arg(long, value_name = "PNG")]
    pub mask: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ApplyArgs {
    /// The folder to give the icon
    pub folder: PathBuf,
    /// Any picture: a finished folder is used as it is, anything else goes on FolderSkin's
    /// folder; `-` reads standard input
    #[arg(long)]
    pub image: PathBuf,
    /// What stays in the middle of the crop, e.g. 0.5,0.4 (artwork only)
    #[arg(long, value_parser = parse_focus)]
    pub focus: Option<(f32, f32)>,
}

fn parse_number(s: &str, lo: f64, hi: f64, what: &str) -> Result<f64, String> {
    let v: f64 = s
        .trim()
        .parse()
        .map_err(|_| format!("{what} must be a number, like {}", (lo + hi) / 4.0))?;
    if !(lo..=hi).contains(&v) {
        return Err(format!("{what} goes from {lo} to {hi}"));
    }
    Ok(v)
}

fn parse_amount(s: &str) -> Result<f64, String> {
    parse_number(s, -100.0, 100.0, "the amount")
}

fn parse_share(s: &str) -> Result<f64, String> {
    parse_number(s, 0.0, 100.0, "the amount")
}

fn parse_hue(s: &str) -> Result<f64, String> {
    parse_number(s, -180.0, 180.0, "the hue")
}

/// A whole number from 0 to [`folderskin_local::MAX_SEED`], the most the runtimes take.
pub fn parse_seed(s: &str) -> Result<u64, String> {
    s.trim()
        .parse::<u64>()
        .ok()
        .filter(|seed| *seed <= folderskin_local::MAX_SEED)
        .ok_or_else(|| {
            format!(
                "a seed is a whole number from 0 to {}, e.g. 42",
                folderskin_local::MAX_SEED
            )
        })
}

/// `X,Y,W,H` in pixels.
pub fn parse_box(s: &str) -> Result<(u32, u32, u32, u32), String> {
    let parts: Vec<u32> = s
        .split(',')
        .map(|p| p.trim().parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| "a box looks like X,Y,WIDTH,HEIGHT in whole pixels, e.g. 0,40,1024,958")?;
    match parts[..] {
        [x, y, w, h] if w > 0 && h > 0 => Ok((x, y, w, h)),
        [_, _, _, _] => Err("a box needs a width and a height above 0".into()),
        _ => Err("a box has four numbers: X,Y,WIDTH,HEIGHT".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("folderskin").chain(args.iter().copied()))
    }

    #[test]
    fn the_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn gen_takes_every_option() {
        let cli = parse(&[
            "ai",
            "gen",
            "a koi pond",
            "--style",
            "woodblock",
            "--shape",
            "folder",
            "--ref",
            "a.jpg",
            "--ref",
            "b.png",
            "-n",
            "3",
            "--seed",
            "7",
            "--model",
            "klein",
            "--out",
            "renders",
            "--apply",
            "D:/Photos",
            "--json",
        ])
        .unwrap();
        assert!(cli.json);
        let Command::Ai(AiCommand::Gen(g)) = cli.command else {
            panic!("not gen");
        };
        assert_eq!(g.idea, "a koi pond");
        assert_eq!(g.style, "woodblock");
        assert_eq!(g.shape, ShapeArg::Folder);
        assert_eq!(g.refs, [PathBuf::from("a.jpg"), PathBuf::from("b.png")]);
        assert_eq!((g.n, g.seed), (3, Some(7)));
        assert_eq!(g.model.as_deref(), Some("klein"));
        assert_eq!(g.out, PathBuf::from("renders"));
        assert_eq!(g.apply, Some(PathBuf::from("D:/Photos")));
        assert_eq!(g.machine, MachineArgs::default());
    }

    #[test]
    fn gen_defaults_to_local_artwork_in_folderskin_out() {
        let Command::Ai(AiCommand::Gen(g)) = parse(&["ai", "gen", "x"]).unwrap().command else {
            panic!("not gen");
        };
        assert_eq!(
            (g.style.as_str(), g.shape, g.n),
            ("none", ShapeArg::Artwork, 1)
        );
        assert_eq!(g.out, PathBuf::from("folderskin-out"));
        assert!(g.provider.is_none() && g.model.is_none() && !g.raw && !g.no_preview);
        assert!(parse(&["ai", "gen", "x", "-n", "0"]).is_err());
        assert!(parse(&["ai", "gen", "x", "--shape", "skin"]).is_err());
        assert!(parse(&["ai", "gen"]).is_err(), "an idea is needed");
    }

    #[test]
    fn seeds_stop_where_the_runtimes_do() {
        // sd-cli reads a seed as a signed 64-bit number and stops without a word beyond it.
        let seed = |s: &str| match parse(&["ai", "gen", "x", "--seed", s]).map(|c| c.command) {
            Ok(Command::Ai(AiCommand::Gen(g))) => Ok(g.seed),
            Ok(_) => panic!("not gen"),
            Err(e) => Err(e.kind()),
        };
        assert_eq!(seed("9223372036854775807"), Ok(Some(i64::MAX as u64)));
        for bad in ["9223372036854775808", "18446744073709551615", "x"] {
            assert_eq!(
                seed(bad),
                Err(clap::error::ErrorKind::ValueValidation),
                "{bad}"
            );
        }
        assert!(parse(&["ai", "theme", "d", "--seed", "18446744073709551615"]).is_err());
        assert!(parse_seed("99999999999999999999")
            .unwrap_err()
            .contains("from 0 to 9223372036854775807"));
    }

    #[test]
    fn backends_and_tiers_are_checked_by_name() {
        let Command::Ai(AiCommand::Setup(s)) = parse(&[
            "ai",
            "setup",
            "--backend",
            "vulkan",
            "--tier",
            "q4",
            "--runtime",
            "latest",
        ])
        .unwrap()
        .command
        else {
            panic!("not setup");
        };
        assert_eq!(s.machine.backend, Some(BackendArg::Vulkan));
        assert_eq!(s.machine.tier, Some(TierArg::Q4));
        assert_eq!(s.runtime, RuntimeArg::Latest);
        assert!(parse(&["ai", "doctor", "--backend", "rocm"]).is_err());
        assert!(parse(&["ai", "doctor", "--tier", "q5"]).is_err());
    }

    #[test]
    fn gen_batch_and_theme_can_be_tried_dry() {
        for args in [
            &["ai", "gen", "x", "--dry-run"][..],
            &["ai", "batch", "b.json", "--dry-run"],
            &["ai", "theme", "D:/Projects", "--apply", "--dry-run"],
        ] {
            let dry = match parse(args).unwrap().command {
                Command::Ai(AiCommand::Gen(g)) => g.dry_run,
                Command::Ai(AiCommand::Batch(b)) => b.dry_run,
                Command::Ai(AiCommand::Theme(t)) => t.dry_run,
                other => panic!("{other:?}"),
            };
            assert!(dry, "{args:?}");
        }
    }

    #[test]
    fn theme_uses_klein_one_level_down_unless_told() {
        let Command::Ai(AiCommand::Theme(t)) = parse(&[
            "ai",
            "theme",
            "D:/Projects",
            "--style",
            "risograph",
            "--apply",
        ])
        .unwrap()
        .command
        else {
            panic!("not theme");
        };
        assert_eq!((t.model, t.depth, t.apply), (None, 1, true));
        assert!(parse(&["ai", "theme", "x", "--depth", "9"]).is_err());
    }

    #[test]
    fn config_and_keys() {
        assert!(matches!(
            parse(&["ai", "config"]).unwrap().command,
            Command::Ai(AiCommand::Config { command: None })
        ));
        let Command::Ai(AiCommand::Config {
            command: Some(ConfigCommand::Set { key, value }),
        }) = parse(&["ai", "config", "set", "provider", "openai"])
            .unwrap()
            .command
        else {
            panic!("not config set");
        };
        assert_eq!((key, value.as_str()), (ConfigKey::Provider, "openai"));
        assert!(parse(&["ai", "config", "set", "colour", "red"]).is_err());
        assert!(matches!(
            parse(&["ai", "key", "test", "xai"]).unwrap().command,
            Command::Ai(AiCommand::Key(KeyCommand::Test { .. }))
        ));
        assert!(
            parse(&["ai", "key", "set", "openai", "sk-123"]).is_err(),
            "a key never goes on the command line"
        );
    }

    #[test]
    fn image_amounts_may_be_negative_and_stay_in_range() {
        let Command::Image(ImageCommand::Saturate(a)) =
            parse(&["image", "saturate", "in.png", "-40", "--out", "-"])
                .unwrap()
                .command
        else {
            panic!("not saturate");
        };
        assert_eq!(a.amount, -40.0);
        assert_eq!(a.out, Some(PathBuf::from("-")));
        assert!(parse(&["image", "brightness", "in.png", "150"]).is_err());
        let Command::Image(ImageCommand::Adjust(a)) = parse(&[
            "image",
            "adjust",
            "in.png",
            "--hue",
            "-30",
            "--sepia",
            "40",
            "--contrast",
            "10",
        ])
        .unwrap()
        .command
        else {
            panic!("not adjust");
        };
        assert_eq!(
            (a.hue, a.sepia, a.contrast),
            (Some(-30.0), Some(40.0), Some(10.0))
        );
        assert!(parse(&["image", "adjust", "in.png", "--hue", "200"]).is_err());
    }

    #[test]
    fn crops_take_an_aspect_a_focus_or_a_box() {
        let Command::Image(ImageCommand::Crop(c)) = parse(&[
            "image", "crop", "in.png", "--aspect", "16:9", "--focus", "0.5,0.3",
        ])
        .unwrap()
        .command
        else {
            panic!("not crop");
        };
        assert_eq!(c.aspect, "16:9");
        assert_eq!(c.focus, Some((0.5, 0.3)));
        let Command::Image(ImageCommand::Crop(c)) =
            parse(&["image", "crop", "in.png", "--box", "0,40,1024,958"])
                .unwrap()
                .command
        else {
            panic!("not crop");
        };
        assert_eq!(c.crop_box, Some((0, 40, 1024, 958)));
        assert_eq!(
            parse_box("1,2,3"),
            Err("a box has four numbers: X,Y,WIDTH,HEIGHT".into())
        );
        assert!(parse_box("0,0,0,10").is_err());
    }

    #[test]
    fn the_tools_are_at_the_top_level_too() {
        assert!(matches!(
            parse(&["template", "--out", "t.png"]).unwrap().command,
            Command::Template(_)
        ));
        assert!(matches!(
            parse(&["packs", "check"]).unwrap().command,
            Command::Packs(PacksCommand::Check { .. })
        ));
        assert!(matches!(
            parse(&["apply", "D:/x", "--image", "a.png"])
                .unwrap()
                .command,
            Command::Apply(_)
        ));
        assert!(matches!(
            parse(&["revert", "D:/x"]).unwrap().command,
            Command::Revert { .. }
        ));
        assert!(matches!(
            parse(&["image", "render", "a.png", "--size", "256"])
                .unwrap()
                .command,
            Command::Image(ImageCommand::Render(_))
        ));
    }

    #[test]
    fn options_that_would_be_ignored_are_refused_instead() {
        use clap::error::ErrorKind;
        for args in [
            &["render", "a.png", "--solid", "2A9D8F"][..],
            &[
                "image",
                "crop",
                "a.png",
                "--focus",
                "0.5,0.5",
                "--box",
                "0,0,10,10",
            ],
        ] {
            assert_eq!(
                parse(args).unwrap_err().kind(),
                ErrorKind::ArgumentConflict,
                "{args:?}"
            );
        }
        // A negative amount is out of range, said as it is for the other amounts.
        let e = parse(&["image", "invert", "a.png", "--amount", "-5"]).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::ValueValidation);
        assert!(
            e.to_string().contains("the amount goes from 0 to 100"),
            "{e}"
        );
    }

    #[test]
    fn the_folder_look_can_be_given_anywhere() {
        assert_eq!(parse(&["render", "a.png"]).unwrap().look, None);
        for args in [
            &["render", "a.png", "--look", "windows"][..],
            &["--look", "windows", "apply", "D:/x", "--image", "a.png"],
            &["ai", "gen", "x", "--look", "windows"],
        ] {
            assert_eq!(
                parse(args).unwrap().look,
                Some(LookArg::Windows),
                "{args:?}"
            );
        }
        assert!(parse(&["render", "a.png", "--look", "linux"]).is_err());
    }

    #[test]
    fn version_and_help_are_not_errors() {
        let e = parse(&["--version"]).unwrap_err();
        assert_eq!(e.kind(), clap::error::ErrorKind::DisplayVersion);
        assert!(e.to_string().starts_with("folderskin 0.1.0"), "{e}");
        let e = parse(&["ai", "--help"]).unwrap_err();
        assert_eq!(e.kind(), clap::error::ErrorKind::DisplayHelp);
    }
}
