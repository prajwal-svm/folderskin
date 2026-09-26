//! Painting one picture: the prompt, the runtime, and the clean-up afterwards.
//!
//! What comes out is a picture, not an icon. Artwork is wrapped onto a folder by the app's own
//! compositor, so its geometry is always exact; a whole folder is the base's blank template
//! repainted and cut out along the app's own silhouette; a free icon is cut out of the flat
//! backdrop it was painted on. Beside each picture goes a `.json` with everything needed to paint
//! it again and to say where it came from when it is shared.

use crate::command::{self, HEIGHT, WIDTH};
use crate::event::{Level, Reporter, Stage};
use crate::machine::{pick_backend, pick_tier, Arch, Backend, Machine, Os, Tier};
use crate::manifest::{sdcpp_assets, Model, ModelId, SDCPP_TAG};
use crate::prompts::{self, Shape};
use crate::{paths, CancelToken, Error};
use folderskin_ai::recipe::{self, Key, Lettering, Role, Treatment, RECIPE_VERSION};
use folderskin_core::base::{Base, MAC_FOLDER, TEMPLATE_VERSION};
use folderskin_core::{compositor, matte, painted};
use image::{GrayImage, Luma, RgbaImage};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// How the models run on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Settings {
    pub backend: Backend,
    pub tier: Tier,
    /// Dedicated VRAM in GB, 0 when unknown: below 6 GB references are encoded at half size.
    pub vram_gb: f64,
}

impl Settings {
    /// What this machine should use when nothing is configured.
    pub fn for_machine(machine: &Machine) -> Settings {
        Settings {
            backend: pick_backend(machine),
            tier: pick_tier(machine),
            vram_gb: machine.vram_gb,
        }
    }
}

/// One picture to paint.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// The subject and the scene in plain words. Words it quotes are lettered on the picture.
    pub idea: String,
    /// A style preset's key ([`prompts::styles`]), someone's own words, or "none".
    pub style: String,
    /// The look, when the app chose it: a built-in style's or a saved prompt's. Used in place
    /// of `style`.
    pub treatment: Option<Treatment>,
    pub shape: Shape,
    /// What it is for: FolderSkin's own folder unless another base is asked for. A free icon
    /// always paints [`Shape::Icon`] ([`Job::painted`]).
    pub base: &'static Base,
    /// Reference pictures to paint from.
    pub refs: Vec<PathBuf>,
    /// What each reference picture is for, in the order of `refs`: a picture with none is a
    /// subject.
    pub roles: Vec<Role>,
    pub seed: u64,
    /// The file name without its extension; `None` names it from the idea, style and seed.
    pub name: Option<String>,
    /// The model; `None` is the default, klein.
    pub model: Option<ModelId>,
    /// Send `idea` to the model word for word, without FolderSkin's prompt around it.
    pub raw: bool,
}

impl Job {
    pub fn new(idea: impl Into<String>) -> Job {
        Job {
            idea: idea.into(),
            style: "none".into(),
            treatment: None,
            shape: Shape::Artwork,
            base: &MAC_FOLDER,
            refs: Vec::new(),
            roles: Vec::new(),
            seed: 0,
            name: None,
            model: None,
            raw: false,
        }
    }

    /// The model that paints it: the one asked for, or klein, which paints from words and from
    /// pictures (references, and the blank folder a whole-folder skin repaints) alike.
    pub fn model(&self) -> &'static Model {
        self.model.unwrap_or(ModelId::Klein).info()
    }

    /// What is painted: the shape asked for, on its base ([`Shape::on`]).
    pub fn painted(&self) -> Shape {
        self.shape.on(self.base)
    }

    /// How it looks: the treatment the app chose, or the style named.
    pub fn look(&self) -> Option<Treatment> {
        self.treatment
            .clone()
            .or_else(|| Treatment::named(&self.style))
    }

    /// The look by name, for the file name and the record: a style's or a saved prompt's id,
    /// someone's own words, or "none".
    pub fn style_name(&self) -> String {
        match &self.treatment {
            Some(t) if !t.id.is_empty() => t.id.clone(),
            Some(t) => t.words.clone(),
            None => self.style.trim().to_string(),
        }
    }

    /// Each reference picture's role, in the order of `refs`.
    pub fn ref_roles(&self) -> Vec<Role> {
        (0..self.refs.len())
            .map(|i| match self.roles.get(i) {
                Some(r) if !r.is_made() => *r,
                _ => Role::Subject,
            })
            .collect()
    }

    /// The colour a whole shape's template or a free icon's canvas is drawn on: green for a look
    /// or an idea full of pink or violet, which a magenta backdrop would bleed into.
    pub fn key(&self) -> Key {
        recipe::key_for("local", &self.idea, self.look().as_ref())
    }

    /// The prompt the model gets.
    pub fn prompt(&self) -> String {
        if self.raw {
            self.idea.trim().to_string()
        } else {
            let look = self.look();
            prompts::compose_slots(&prompts::Slots {
                idea: &self.idea,
                treatment: look.as_ref(),
                shape: self.painted(),
                base: self.base,
                refs: &self.ref_roles(),
            })
        }
    }

    /// The words the idea asks to be lettered.
    pub fn lettering(&self) -> Vec<String> {
        let look = self.look();
        Lettering::of(
            &self.idea,
            look.as_ref(),
            self.base,
            self.painted().recipe(),
        )
        .map(|l| l.words)
        .unwrap_or_default()
    }

    /// The file name, without extension, the picture is saved under.
    pub fn file_name(&self) -> String {
        match &self.name {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => format!(
                "{}-{}-{}",
                slug(&self.idea, 40),
                slug(&self.style_name(), 16),
                self.seed
            ),
        }
    }
}

/// Lower-case words joined by dashes, at most `limit` characters: "A Koi Pond!" → "a-koi-pond".
pub fn slug(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    let cut: String = out.chars().take(limit).collect();
    let cut = cut.trim_end_matches('-');
    if cut.is_empty() {
        "skin".into()
    } else {
        cut.to_string()
    }
}

/// Where a picture came from, written beside it as `<name>.json`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Provenance {
    pub idea: String,
    pub style: String,
    pub shape: Shape,
    /// The base it was made for, by id ([`folderskin_core::base`]).
    pub base: String,
    /// The prompt templates' version ([`RECIPE_VERSION`]).
    pub recipe: u32,
    pub prompt: String,
    /// The words it was asked to letter.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lettering: Vec<String>,
    /// The template repainted and the version it was drawn in ("mac-folder/1"), when one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// The colour the template or canvas was drawn on, for a whole shape or a free icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Key>,
    pub model: String,
    pub model_licence: String,
    pub tier: Tier,
    pub seed: u64,
    pub steps: u32,
    pub size: [u32; 2],
    pub backend: Backend,
    pub runtime: String,
    pub references: Vec<Reference>,
    pub border_trimmed: bool,
    pub silhouette_fit: Option<f64>,
    pub seconds: f64,
    pub made: String,
    pub digital_source_type: String,
}

/// A reference picture, by name, content and what it was for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reference {
    pub file: String,
    pub sha256: String,
    pub role: Role,
}

/// IPTC's term for a picture a model made, for anyone reading the provenance.
pub const TRAINED_ALGORITHMIC_MEDIA: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

/// A painted picture.
#[derive(Clone, Debug)]
pub struct Picture {
    /// The PNG, ready for FolderSkin.
    pub path: PathBuf,
    /// Artwork, a whole folder or a free icon; a folder whose shape the model changed, or an icon
    /// with no flat backdrop to cut it out of, stays on its backdrop.
    pub shape: Shape,
    pub provenance: Provenance,
}

/// Says what is missing before a job is started, so a batch fails at once rather than at its
/// first picture.
pub fn check_ready(job: &Job, settings: &Settings) -> Result<(), Error> {
    if job.seed > MAX_SEED {
        return Err(Error::fixable(
            "bad_seed",
            format!("The seed {} is too big.", job.seed),
            format!("A seed goes from 0 to {MAX_SEED}, the most the runtimes take."),
        )
        .fix("Use a smaller seed, or leave it out for a random one."));
    }
    for r in &job.refs {
        if !r.is_file() {
            return Err(Error::fixable(
                "reference_missing",
                "A reference picture isn't there.",
                format!("{} doesn't exist or isn't a file.", r.display()),
            )
            .fix("Check the path given with --ref (it is relative to the current folder)."));
        }
    }
    let setup = format!(
        "Download what's missing: folderskin ai setup --backend {} --tier {}",
        settings.backend, settings.tier
    );
    if settings.backend == Backend::Mlx {
        if paths::mflux(MFLUX_PROBE).is_none() {
            return Err(Error::environment(
                "mflux_missing",
                "mflux isn't installed.",
                "On Apple Silicon the models run in mflux, which setup installs.",
            )
            .fix(setup));
        }
    } else {
        let exe = paths::sd_cli(settings.backend);
        if !exe.is_file() {
            let (os, arch) = (Os::this(), Arch::this());
            if sdcpp_assets(os, arch, settings.backend).is_none() {
                // Setup would only say the same, after the person had gone to run it.
                return Err(crate::setup::no_build(os, arch, settings.backend));
            }
            return Err(Error::environment(
                "runtime_missing",
                format!(
                    "stable-diffusion.cpp isn't installed for {}.",
                    settings.backend
                ),
                format!("There is no {}.", exe.display()),
            )
            .fix(setup));
        }
    }
    let model = job.model();
    let files = model.files_for(settings.backend, settings.tier);
    let missing: Vec<&str> = files
        .iter()
        .filter(|f| !f.local.is_file())
        .map(|f| f.name.as_str())
        .collect();
    if !missing.is_empty() {
        let kept_in = if settings.backend == Backend::Mlx {
            model.mlx(settings.tier).dir()
        } else {
            paths::models_dir()
        };
        return Err(Error::environment(
            "models_missing",
            format!("{} isn't downloaded yet.", model.label),
            format!(
                "{} {} missing from {}.",
                missing.join(", "),
                if missing.len() == 1 { "is" } else { "are" },
                kept_in.display()
            ),
        )
        .fix(setup));
    }
    if settings.backend == Backend::Mlx {
        return Ok(());
    }
    // Paths sd-cli can't open fail here, in words, rather than as "file not found" in its log.
    if files.iter().any(|f| paths::for_sdcpp(&f.local).is_none()) {
        return Err(Error::environment(
            "path_not_ascii",
            "stable-diffusion.cpp can't open the models where they are.",
            format!(
                "On Windows it can only open paths written in plain ASCII, and {} has other \
                 letters in it, with no short name to use instead.",
                paths::models_dir().display()
            ),
        )
        .fix(format!(
            "Move {} to a folder with a plain name, such as C:\\folderskin-localgen, and set \
             FOLDERSKIN_LOCALGEN_HOME to that folder.",
            paths::home().display()
        )));
    }
    for r in &job.refs {
        if paths::for_sdcpp(r).is_none() {
            return Err(not_ascii(r, "open"));
        }
    }
    Ok(())
}

/// A picture or folder whose path stable-diffusion.cpp can't use ([`paths::for_sdcpp`]).
fn not_ascii(path: &Path, doing: &str) -> Error {
    Error::fixable(
        "path_not_ascii",
        format!("stable-diffusion.cpp can't {doing} {}.", path.display()),
        "On Windows it can only use paths written in plain ASCII, and this one has other \
         letters in it, with no short name to use instead.",
    )
    .fix("Use a folder and a file name with plain letters, e.g. C:\\folderskin\\photo.png.")
}

/// The mflux program whose presence means mflux is installed: the one that paints from words.
pub(crate) const MFLUX_PROBE: &str = "mflux-generate-flux2";

/// Paints `job` into `out_dir` (made if needed), reporting as it goes. Takes about half a minute
/// to a minute on a laptop GPU; the runtime is ended if `cancel` is set.
pub async fn generate(
    job: &Job,
    settings: &Settings,
    out_dir: &Path,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<Picture, Error> {
    let (job, settings, out_dir) = (job.clone(), *settings, out_dir.to_path_buf());
    let (reporter, cancel) = (reporter.clone(), cancel.clone());
    tokio::task::spawn_blocking(move || {
        generate_blocking(&job, &settings, &out_dir, &reporter, &cancel)
    })
    .await
    .map_err(|e| Error::bug("Painting stopped unexpectedly.", e.to_string()))?
}

fn generate_blocking(
    job: &Job,
    settings: &Settings,
    out_dir: &Path,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<Picture, Error> {
    if job.idea.trim().is_empty() {
        return Err(Error::fixable(
            "no_idea",
            "There is nothing to paint.",
            "The idea is empty.",
        )
        .fix("Describe the subject and the scene, e.g. \"a lighthouse on a rock at dusk\"."));
    }
    check_ready(job, settings)?;
    cancel.check()?;
    std::fs::create_dir_all(out_dir)
        .map_err(|e| Error::io("make the output folder", out_dir, &e))?;
    let name = job.file_name();
    let out = out_dir.join(format!("{name}.png"));
    let model = job.model();

    let shape = job.painted();
    let pictures = pictures(job, settings.backend, out_dir)?;
    let silhouette = (shape == Shape::Folder)
        .then(|| silhouette_of(job.base, WIDTH, HEIGHT))
        .flatten();
    let prompt = job.prompt();
    let steps = model.steps;

    let (cmd, runtime, painted) = if settings.backend == Backend::Mlx {
        let weights = model.mlx(settings.tier);
        let (program, args) = command::mflux(model, weights, &prompt, job.seed, &pictures, &out);
        // By its full path: setup's folder is on no PATH, and an app opened from the Finder has
        // no ~/.local/bin on its PATH either.
        let mut cmd = Command::new(paths::mflux(program).unwrap_or_else(|| program.into()));
        cmd.args(args);
        // The weights are all in their folder, checked: painting never reaches for the network,
        // and a missing file fails at once in words rather than as a download in the background.
        cmd.env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1");
        let painted = Painting {
            at: out.clone(),
            stand_in: false,
        };
        (cmd, "mflux".to_string(), painted)
    } else {
        // sd-cli can't write to a name that isn't plain ASCII ("фото.png" comes out garbled), so
        // it paints to a plain name in the same folder and the picture is renamed afterwards.
        let dir = paths::for_sdcpp(out_dir).ok_or_else(|| not_ascii(out_dir, "write to"))?;
        let painted = Painting {
            at: dir.join(format!(".painting-{}.png", std::process::id())),
            stand_in: true,
        };
        let pictures = pictures
            .iter()
            .map(|p| paths::for_sdcpp(p).ok_or_else(|| not_ascii(p, "open")))
            .collect::<Result<Vec<_>, _>>()?;
        let exe = paths::sd_cli(settings.backend);
        let mut cmd = Command::new(&exe);
        cmd.args(command::sdcpp(
            &model.files(settings.tier),
            model,
            &prompt,
            job.seed,
            &pictures,
            &painted.at,
            settings.backend,
            settings.vram_gb,
        ));
        let release = std::fs::read_to_string(exe.with_file_name(".release"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| SDCPP_TAG.to_string());
        (cmd, format!("stable-diffusion.cpp {release}"), painted)
    };
    // A picture left from an earlier run with the same name must not pass for this one's.
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&painted.at);

    reporter.stage(
        Stage::Load,
        format!("{name}: {}, seed {}", model.label, job.seed),
    );
    let started = Instant::now();
    let finished = crate::run::run(cmd, steps, reporter, cancel).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound && settings.backend == Backend::Mlx {
            Error::environment(
                "mflux_missing",
                "mflux isn't installed.",
                format!(
                    "mflux's program for {} isn't where setup installs it, on the PATH or where \
                     uv installs its tools.",
                    model.label
                ),
            )
            .fix("Install it: folderskin ai setup --backend mlx")
        } else {
            Error::environment(
                "runtime_failed_to_start",
                "The runtime couldn't be started.",
                format!("{runtime}: {e}."),
            )
            // Two steps, so the app keeps the first and says the second in its own words.
            .fix(format!(
                "Reinstall it: delete {}",
                paths::sd_cli(settings.backend)
                    .parent()
                    .unwrap_or(Path::new("."))
                    .display()
            ))
            .fix("Then set it up again: folderskin ai setup")
        }
    })?;
    let seconds = started.elapsed().as_secs_f64();
    cancel.check()?;
    let Some(status) = finished.status else {
        return Err(Error::cancelled());
    };
    if let Some(e) = runtime_error(status.code(), &finished.tail, &runtime, settings.backend) {
        return Err(e);
    }
    if !painted.at.is_file() {
        return Err(Error::environment(
            "generation_failed",
            "The runtime finished without a picture.",
            format!(
                "{runtime} ended normally but wrote nothing to {}.{}",
                painted.at.display(),
                tail_text(&finished.tail)
            ),
        )
        .fix("Run the same command again with --verbose to see everything it printed.")
        .fix("Check the runtime and the models: folderskin ai doctor"));
    }
    if painted.stand_in {
        std::fs::rename(&painted.at, &out).map_err(|e| Error::io("save the picture", &out, &e))?;
    }

    reporter.stage(Stage::Finish, format!("{name}: done in {seconds:.0} s"));
    let img = image::open(&out)
        .map_err(|e| {
            Error::environment(
                "generation_failed",
                "The runtime wrote a picture that can't be read.",
                format!("{}: {e}.", out.display()),
            )
            .fix("Run the same command again.")
        })?
        .to_rgba8();
    if painted::is_blank(&img) {
        // A backend can fail quietly and write a flat white or black picture (sd.cpp's Metal
        // backend does on some Macs); that is a failure, not a skin.
        let _ = std::fs::remove_file(&out);
        let other = if settings.backend == Backend::Metal {
            Backend::Mlx
        } else {
            Backend::Cpu
        };
        return Err(Error::fixable(
            "blank_picture",
            "The picture came out blank.",
            format!("The {} backend painted one flat colour.", settings.backend),
        )
        .fix(format!("Try another backend, e.g. --backend {other}")));
    }

    let raw_dir = out_dir.join("raw");
    let keep_raw = || -> Result<(), Error> {
        std::fs::create_dir_all(&raw_dir)
            .map_err(|e| Error::io("keep the original", &raw_dir, &e))?;
        let raw = raw_dir.join(format!("{name}.png"));
        std::fs::copy(&out, &raw).map_err(|e| Error::io("keep the original", &raw, &e))?;
        Ok(())
    };
    let mut border_trimmed = false;
    let mut silhouette_fit = None;
    let key = job.key();
    match &silhouette {
        None if shape == Shape::Icon => match cut_icon(&img, key) {
            Some(cutout) => {
                keep_raw()?;
                std::fs::write(&out, folderskin_core::raster::encode_png(&cutout))
                    .map_err(|e| Error::io("save the picture", &out, &e))?;
                reporter.log(Level::Info, format!("{name}: cut out of its backdrop"));
            }
            None => reporter.log(
                Level::Warn,
                format!(
                    "{name}: the model painted no plain backdrop to cut it out of, so it's left as \
                     painted"
                ),
            ),
        },
        None => {
            // A paper margin, or paper down two opposite sides (klein painted a pop-art fox that
            // way): on a folder either becomes blank bands.
            if let Some((border, trimmed)) = painted::trim_paper(&img) {
                keep_raw()?;
                save_rgb(&trimmed, &out)?;
                border_trimmed = true;
                reporter.log(
                    Level::Info,
                    format!("{name}: cut away its paper ({})", border.describe()),
                );
            }
        }
        Some(silhouette) => {
            let cut = painted::cut_along_silhouette(&img, silhouette);
            silhouette_fit = Some((cut.fit * 10_000.0).round() / 10_000.0);
            match cut.image {
                Some(cutout) => {
                    keep_raw()?;
                    std::fs::write(&out, folderskin_core::raster::encode_png(&cutout))
                        .map_err(|e| Error::io("save the picture", &out, &e))?;
                    reporter.log(
                        Level::Info,
                        format!(
                            "{name}: cut out along FolderSkin's silhouette (fit {:.3})",
                            cut.fit
                        ),
                    );
                }
                None => match cut_reshaped_folder(&img) {
                    Some(own) => {
                        keep_raw()?;
                        std::fs::write(&out, folderskin_core::raster::encode_png(&own))
                            .map_err(|e| Error::io("save the picture", &out, &e))?;
                        reporter.log(
                            Level::Warn,
                            format!(
                                "{name}: the model changed the folder's shape (fit {:.3}), so \
                                 it's cut out along its own outline",
                                cut.fit
                            ),
                        );
                    }
                    None => reporter.log(
                        Level::Warn,
                        format!(
                            "{name}: the model changed the folder's shape (fit {:.3}), so it's left \
                             on its backdrop",
                            cut.fit
                        ),
                    ),
                },
            }
        }
    }

    let references = job
        .refs
        .iter()
        .zip(job.ref_roles())
        .map(|(p, role)| {
            Ok(Reference {
                file: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                sha256: crate::download::sha256_file(p, &CancelToken::new())
                    .map_err(|e| Error::io("read a reference picture", p, &e))?,
                role,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let provenance = Provenance {
        idea: job.idea.clone(),
        style: job.style_name(),
        shape,
        base: job.base.id.to_string(),
        recipe: RECIPE_VERSION,
        prompt,
        lettering: job.lettering(),
        template: (shape == Shape::Folder).then(|| format!("{}/{TEMPLATE_VERSION}", job.base.id)),
        key: (shape != Shape::Artwork).then_some(key),
        model: model.label.to_string(),
        model_licence: model.licence.to_string(),
        tier: settings.tier,
        seed: job.seed,
        steps,
        size: [WIDTH, HEIGHT],
        backend: settings.backend,
        runtime,
        references,
        border_trimmed,
        silhouette_fit,
        seconds: (seconds * 10.0).round() / 10.0,
        made: utc_now(),
        digital_source_type: TRAINED_ALGORITHMIC_MEDIA.to_string(),
    };
    let json = out.with_extension("json");
    let text = serde_json::to_string_pretty(&provenance)
        .map_err(|e| Error::bug("The picture's record couldn't be written.", e.to_string()))?;
    std::fs::write(&json, text + "\n")
        .map_err(|e| Error::io("save the picture's record", &json, &e))?;
    Ok(Picture {
        path: out,
        shape,
        provenance,
    })
}

/// The pictures `job` is painted from, in the order the runtime is given them
/// ([`prompts::roles`]): for a whole base, its blank template first, and for a free icon a flat
/// canvas, each in the key colour, written into `out_dir` under their names and the size the
/// runtime takes them at ([`command::template_size`]); then the reference pictures.
pub fn pictures(job: &Job, backend: Backend, out_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut pictures = job.refs.clone();
    let (w, h) = command::template_size(backend);
    let key = job.key();
    let (name, img) = match job.painted() {
        Shape::Artwork => return Ok(pictures),
        Shape::Folder => (
            format!(".template-{}-{}-{w}x{h}.png", job.base.id, key.name()),
            job.base
                .blank(w, h, key.rgb())
                .unwrap_or_else(|| compositor::blank_template(w, h, key.rgb())),
        ),
        Shape::Icon => (
            format!(".canvas-{}-{w}x{h}.png", key.name()),
            canvas(w, h, key),
        ),
    };
    let first = out_dir.join(name);
    if !first.is_file() {
        std::fs::write(&first, folderskin_core::raster::encode_png(&img))
            .map_err(|e| Error::io("write the picture it is painted on", &first, &e))?;
    }
    pictures.insert(0, first);
    Ok(pictures)
}

/// A flat `width` x `height` picture in the key colour: what a free icon is painted in the
/// middle of.
pub fn canvas(width: u32, height: u32, key: Key) -> RgbaImage {
    let [r, g, b] = key.rgb();
    RgbaImage::from_pixel(width, height, image::Rgba([r, g, b, 255]))
}

/// Where the runtime writes its picture: its own name, or a plain stand-in that is renamed to it.
/// A stand-in left behind when painting stops early (a cancel, a failure) is removed.
struct Painting {
    at: PathBuf,
    stand_in: bool,
}

impl Drop for Painting {
    fn drop(&mut self) {
        if self.stand_in {
            let _ = std::fs::remove_file(&self.at);
        }
    }
}

/// FolderSkin's silhouette in a `width` x `height` frame: white inside the folder, as the blank
/// template the model repaints has it.
pub fn folder_silhouette(width: u32, height: u32) -> GrayImage {
    let cut = compositor::blank_template_cutout(width, height);
    GrayImage::from_fn(width, height, |x, y| Luma([cut.get_pixel(x, y).0[3]]))
}

/// `base`'s silhouette in a `width` x `height` frame, as its blank template has it; `None` for a
/// free icon, which has no template.
pub fn silhouette_of(base: &Base, width: u32, height: u32) -> Option<GrayImage> {
    folderskin_ai::finish::silhouette_of(base, width, height)
}

/// A free icon cut out of what it was painted on: the canvas's `key` where the model kept it
/// clean; otherwise the subject lifted off it by the system (on a Mac, Preview's own lifting,
/// which tells a white robot lit lavender by its backdrop from the backdrop); otherwise any flat
/// colour the model drifted to, cut from the edge in. `None` when none of that finds it.
///
/// Magenta almost never belongs to the art, so it goes wherever it is. Green does, in every leaf
/// and stem, so a green backdrop only goes where it reaches the edge of the picture, from the
/// shade actually painted, as a drifted backdrop of any colour does.
pub fn cut_icon(img: &RgbaImage, key: Key) -> Option<RgbaImage> {
    cut_icon_lifting(img, key, matte::lifted)
}

/// [`cut_icon`], lifting the subject with `lift` where the key isn't clean.
pub fn cut_icon_lifting(
    img: &RgbaImage,
    key: Key,
    lift: impl Fn(&RgbaImage) -> Option<RgbaImage>,
) -> Option<RgbaImage> {
    let connected = |backdrop: [u8; 3]| {
        let cut = matte::cutout_connected(img, backdrop);
        // Something substantial is left, or the backdrop took the subject with it.
        let solid = cut.pixels().filter(|p| p.0[3] >= 128).count();
        (solid as f32 >= matte::MIN_SUBJECT_SHARE * (img.width() * img.height()) as f32)
            .then_some(cut)
    };
    let clean = match (key, matte::surround(img, key.rgb())) {
        (Key::Magenta, _) => matte::finished_cutout(img, key.rgb()),
        (Key::Green, matte::Surround::Keyed) => matte::flat_backdrop(img).and_then(connected),
        (Key::Green, _) => None,
    };
    clean
        .or_else(|| lift(img))
        .or_else(|| matte::flat_backdrop(img).and_then(connected))
}

/// A whole folder the model reshaped, cut out along its own outline: lifted off a plain backdrop
/// by the system, the way a free icon is. `None` where it can't be.
pub fn cut_reshaped_folder(img: &RgbaImage) -> Option<RgbaImage> {
    matte::on_plain_backdrop(img)
        .then(|| matte::lifted(img))
        .flatten()
}

fn save_rgb(img: &RgbaImage, path: &Path) -> Result<(), Error> {
    image::DynamicImage::ImageRgba8(img.clone())
        .to_rgb8()
        .save(path)
        .map_err(|e| match e {
            image::ImageError::IoError(io) => Error::io("save the picture", path, &io),
            other => Error::bug("The picture couldn't be encoded.", other.to_string()),
        })
}

/// The runtime's last words, for an error message.
fn tail_text(tail: &[String]) -> String {
    if tail.is_empty() {
        String::new()
    } else {
        format!(" Its last output:\n{}", tail.join("\n"))
    }
}

/// Windows' "a DLL the program needs isn't there".
const STATUS_DLL_NOT_FOUND: i32 = 0xC000_0135_u32 as i32;

/// What a runtime's exit code means, or `None` when it succeeded.
fn runtime_error(
    code: Option<i32>,
    tail: &[String],
    runtime: &str,
    backend: Backend,
) -> Option<Error> {
    match code {
        Some(0) => None,
        Some(STATUS_DLL_NOT_FOUND) => Some(
            Error::environment(
                "vc_runtime_missing",
                "stable-diffusion.cpp couldn't start.",
                "It needs the Microsoft Visual C++ runtime, and that isn't installed.",
            )
            .fix("Install it from https://aka.ms/vs/17/release/vc_redist.x64.exe")
            .fix("Then run the same command again."),
        ),
        code => {
            let said = tail.join("\n").to_lowercase();
            let exit = code.map_or("was stopped by the system".to_string(), |c| {
                format!("stopped with exit code {c}")
            });
            let mut error = Error::environment(
                "generation_failed",
                "The picture couldn't be painted.",
                format!("{runtime} {exit}.{}", tail_text(tail)),
            );
            if ran_out_of_memory(&said) {
                error = error
                    .fix("Close other programs that use the GPU or a lot of memory, then try again.")
                    .fix("Or use the smaller weights: add --tier q4 (download them with folderskin ai setup --tier q4).");
            }
            if backend == Backend::Cuda && (said.contains("cuda") && said.contains("driver")) {
                error = error.fix(
                    "Update the NVIDIA driver, or try --backend vulkan (folderskin ai setup --backend vulkan).",
                );
            }
            Some(
                error
                    .fix("Check the runtime and the models: folderskin ai doctor")
                    .fix("Run again with --verbose to see everything the runtime printed."),
            )
        }
    }
}

/// Whether a runtime's output says the graphics card (or the computer) ran out of memory, in
/// each backend's words: CUDA's "cudaMalloc failed: out of memory", Vulkan's "Device memory
/// allocation of size … failed", "ErrorOutOfDeviceMemory" and "failed to allocate Vulkan0 buffer
/// of size …", Metal's "failed to allocate buffer", and MLX's "…ErrorOutOfMemory".
pub fn ran_out_of_memory(output: &str) -> bool {
    let said = output.to_lowercase();
    said.contains("out of memory")
        || said.contains("cannot allocate")
        || said.contains("outofmemory")
        || said.contains("outofdevicememory")
        || said.contains("memory allocation of size")
        || (said.contains("failed to allocate") && said.contains("buffer"))
}

/// Now, in UTC, as ISO 8601: "2026-09-23T13:32:05Z".
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    iso_utc(secs)
}

/// `secs` since the Unix epoch as ISO 8601 in UTC, by Howard Hinnant's days-to-civil algorithm.
fn iso_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The largest seed there is: sd-cli reads it as a signed 64-bit number, and stops without a
/// word on anything bigger.
pub const MAX_SEED: u64 = i64::MAX as u64;

/// A seed nobody picked: below 2³¹, which every runtime takes.
pub fn random_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.write_u32(std::process::id());
    h.finish() % (1 << 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_the_idea_the_style_and_the_seed() {
        let mut job = Job::new("A retro film camera, with a chrome lens!");
        job.style = "pop-art".into();
        job.seed = 42;
        assert_eq!(
            job.file_name(),
            "a-retro-film-camera-with-a-chrome-lens-pop-art-42"
        );
        job.name = Some("camera".into());
        assert_eq!(job.file_name(), "camera");
        assert_eq!(slug("Ünïcode ☃", 40), "n-code");
        assert_eq!(slug("!!!", 40), "skin");
        assert_eq!(slug("a very long idea about many things", 10), "a-very-lon");
        assert_eq!(
            slug("abcdefghi jkl", 10),
            "abcdefghi",
            "no dash left at the end"
        );
    }

    #[test]
    fn klein_paints_words_folders_and_references() {
        let mut job = Job::new("x");
        assert_eq!(job.model().id, ModelId::Klein);
        job.shape = Shape::Folder;
        assert_eq!(
            job.model().id,
            ModelId::Klein,
            "a folder repaints a picture"
        );
        job.shape = Shape::Artwork;
        job.refs = vec![PathBuf::from("dog.jpg")];
        assert_eq!(job.model().id, ModelId::Klein);
        assert!(job.model().takes_pictures);
    }

    #[test]
    fn a_raw_prompt_goes_to_the_model_word_for_word() {
        let mut job = Job::new("  exactly this, nothing more ");
        job.style = "oil".into();
        assert!(job.prompt().contains("classical oil painting"));
        job.raw = true;
        assert_eq!(job.prompt(), "exactly this, nothing more");
    }

    #[test]
    fn exit_codes_are_explained() {
        assert_eq!(runtime_error(Some(0), &[], "sd", Backend::Cuda), None);
        let dll = runtime_error(Some(STATUS_DLL_NOT_FOUND), &[], "sd", Backend::Cuda).unwrap();
        assert_eq!(dll.code, "vc_runtime_missing");
        assert!(dll.fix[0].contains("vc_redist.x64.exe"));
        let tail = vec!["[ERROR] ggml_cuda: out of memory".to_string()];
        let oom = runtime_error(Some(1), &tail, "stable-diffusion.cpp", Backend::Cuda).unwrap();
        assert_eq!(oom.code, "generation_failed");
        assert!(oom.why.contains("exit code 1"), "{}", oom.why);
        assert!(oom.why.contains("out of memory"), "{}", oom.why);
        assert!(
            oom.fix.iter().any(|f| f.contains("--tier q4")),
            "{:?}",
            oom.fix
        );
        let killed = runtime_error(None, &[], "sd", Backend::Vulkan).unwrap();
        assert!(killed.why.contains("stopped by the system"));
    }

    #[test]
    fn running_out_of_memory_is_heard_in_every_backends_words() {
        for said in [
            "ggml_cuda: cudaMalloc failed: out of memory",
            "ggml_vulkan: Device memory allocation of size 4831838208 failed.",
            "ggml_vulkan: vk::Device::allocateMemory: ErrorOutOfDeviceMemory",
            "ggml_backend_alloc_ctx_tensors_from_buft: failed to allocate Vulkan0 buffer of size 4831838208",
            "ggml_metal: failed to allocate buffer, size = 3072.00 MiB",
            "[METAL] Command buffer execution failed: Insufficient Memory \
             (00000008:kIOGPUCommandBufferCallbackErrorOutOfMemory)",
        ] {
            assert!(ran_out_of_memory(said), "{said}");
        }
        assert!(!ran_out_of_memory("[ERROR] failed to load model"));
        let tail = vec!["ggml_vulkan: Device memory allocation of size 1 failed.".to_string()];
        let oom = runtime_error(Some(1), &tail, "stable-diffusion.cpp", Backend::Vulkan).unwrap();
        assert!(oom.fix.iter().any(|f| f.contains("--tier q4")), "{oom:?}");
    }

    #[test]
    fn a_job_without_its_runtime_or_references_says_what_to_do() {
        let mut job = Job::new("x");
        job.refs = vec![PathBuf::from("/definitely/not/here.png")];
        let settings = Settings {
            backend: Backend::Cuda,
            tier: Tier::Q8,
            vram_gb: 4.0,
        };
        let err = check_ready(&job, &settings).unwrap_err();
        assert_eq!(err.code, "reference_missing");
        assert!(err.why.contains("here.png"));
    }

    #[test]
    fn a_seed_the_runtimes_cant_take_is_refused_before_they_run() {
        let settings = Settings {
            backend: Backend::Cuda,
            tier: Tier::Q8,
            vram_gb: 4.0,
        };
        let mut job = Job::new("x");
        job.seed = MAX_SEED + 1;
        let err = check_ready(&job, &settings).unwrap_err();
        assert_eq!((err.code, err.class), ("bad_seed", crate::Class::Fixable));
        assert!(err.why.contains("9223372036854775807"), "{err:?}");
        job.seed = MAX_SEED;
        assert_ne!(
            check_ready(&job, &settings).err().map(|e| e.code),
            Some("bad_seed")
        );
    }

    #[test]
    fn timestamps_are_iso_utc() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_790_170_325), "2026-09-23T13:32:05Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert!(random_seed() < 1 << 31);
    }

    #[test]
    fn the_silhouette_is_white_inside_and_black_around() {
        let s = folder_silhouette(256, 240);
        assert_eq!(s.dimensions(), (256, 240));
        assert_eq!(s.get_pixel(0, 0).0[0], 0);
        assert_eq!(s.get_pixel(128, 160).0[0], 255);
        // The same as the Mac folder's own, and Windows' is its own shape.
        assert_eq!(silhouette_of(&MAC_FOLDER, 256, 240), Some(s.clone()));
        let windows = silhouette_of(&folderskin_core::base::WINDOWS_FOLDER, 256, 240).unwrap();
        assert_ne!(windows, s);
        assert!(silhouette_of(&folderskin_core::base::FREE, 256, 240).is_none());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-local-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_whole_folder_is_painted_from_its_own_bases_template() {
        use folderskin_core::base::{FREE, WINDOWS_FOLDER};
        let dir = temp_dir("pictures");
        let mut job = Job::new("a koi pond");
        job.shape = Shape::Folder;
        job.refs = vec![PathBuf::from("dog.jpg")];
        for (base, backend) in [
            (&MAC_FOLDER, Backend::Cuda),
            (&WINDOWS_FOLDER, Backend::Cuda),
            (&WINDOWS_FOLDER, Backend::Mlx),
        ] {
            job.base = base;
            let pictures = pictures(&job, backend, &dir).unwrap();
            assert_eq!(pictures.len(), 2, "the template, then the reference");
            assert_eq!(pictures[1], PathBuf::from("dog.jpg"));
            let (w, h) = command::template_size(backend);
            let written = image::open(&pictures[0]).unwrap().to_rgba8();
            assert_eq!(
                written,
                base.blank(w, h, matte::MAGENTA).unwrap(),
                "{}",
                base.id
            );
            assert!(
                pictures[0].to_string_lossy().contains(base.id),
                "each base keeps its own template: {}",
                pictures[0].display()
            );
        }
        // A pink idea is painted on green, which it can't bleed into.
        job.idea = "a pink flamingo".into();
        job.base = &MAC_FOLDER;
        assert_eq!(job.key(), Key::Green);
        let green = pictures(&job, Backend::Cuda, &dir).unwrap();
        let (w, h) = command::template_size(Backend::Cuda);
        assert_eq!(
            image::open(&green[0]).unwrap().to_rgba8(),
            MAC_FOLDER.blank(w, h, Key::Green.rgb()).unwrap()
        );
        // Artwork is painted from the references alone, and a free icon on a flat canvas.
        job.shape = Shape::Artwork;
        assert_eq!(pictures(&job, Backend::Cuda, &dir).unwrap(), job.refs);
        job.idea = "a fox".into();
        job.shape = Shape::Folder;
        job.base = &FREE;
        assert_eq!(job.painted(), Shape::Icon);
        let icon = pictures(&job, Backend::Cuda, &dir).unwrap();
        assert_eq!(icon[1..], job.refs[..]);
        assert_eq!(
            image::open(&icon[0]).unwrap().to_rgba8(),
            canvas(w, h, Key::Magenta)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_prompt_honours_the_base_the_style_and_the_lettering() {
        use folderskin_core::base::{FREE, WINDOWS_FOLDER};
        let mut job = Job::new("a lighthouse that says \"GO\"");
        job.base = &WINDOWS_FOLDER;
        job.shape = Shape::Folder;
        assert!(
            job.prompt()
                .contains("the curved step where the front panel rises to meet it"),
            "{}",
            job.prompt()
        );
        job.base = &FREE;
        job.style = "a woodblock print with bold outlines".into();
        let p = job.prompt();
        assert!(
            p.starts_with("A lighthouse that says \"GO\", as a woodblock print with bold outlines. It is one single, complete object in the middle of image 1"),
            "{p}"
        );
        assert!(
            p.contains("The words \"GO\" are written once in bold, clean letters"),
            "{p}"
        );
        assert_eq!(job.lettering(), ["GO"]);
        // A look the app chose wins over the style named, and letters its own way.
        job.treatment = Treatment::named("pixel");
        let p = job.prompt();
        assert!(
            p.contains(", as detailed 16-bit pixel art:") && p.contains("in a chunky pixel font"),
            "{p}"
        );
        assert_eq!(job.style_name(), "pixel");
        job.refs = vec![PathBuf::from("dog.png"), PathBuf::from("style.png")];
        job.roles = vec![Role::Subject, Role::Style];
        assert_eq!(job.ref_roles(), [Role::Subject, Role::Style]);
        assert!(
            job.prompt().contains("Image 3 is a style reference only"),
            "{}",
            job.prompt()
        );
        job.roles = vec![Role::Template];
        assert_eq!(
            job.ref_roles(),
            [Role::Subject, Role::Subject],
            "only FolderSkin makes templates"
        );
    }

    #[test]
    fn a_free_icon_is_cut_out_of_its_backdrop_whatever_its_colour() {
        let on = |backdrop: [u8; 3]| {
            let mut img = RgbaImage::from_pixel(
                300,
                280,
                image::Rgba([backdrop[0], backdrop[1], backdrop[2], 255]),
            );
            for y in 60..220 {
                for x in 90..210 {
                    img.put_pixel(x, y, image::Rgba([230, 160, 40, 255]));
                }
            }
            img
        };
        // Magenta as asked, the purple klein drifts to, a plain white, and a green canvas.
        for backdrop in [[255, 0, 255], [150, 40, 170], [250, 250, 248], [0, 255, 0]] {
            for key in [Key::Magenta, Key::Green] {
                let cut = cut_icon_lifting(&on(backdrop), key, |_| None)
                    .unwrap_or_else(|| panic!("{backdrop:?}"));
                assert_eq!(cut.dimensions(), (120, 160), "{backdrop:?}");
                assert_eq!(cut.get_pixel(60, 80).0, [230, 160, 40, 255]);
            }
        }
        // A painting that fills the frame has nothing to cut it out of.
        let scene = RgbaImage::from_fn(300, 280, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        assert!(cut_icon_lifting(&scene, Key::Magenta, |_| None).is_none());
    }

    #[test]
    fn a_free_icon_on_a_clean_key_is_keyed_and_on_anything_else_lifted_first() {
        let on = |backdrop: [u8; 3]| {
            let mut img = RgbaImage::from_pixel(
                300,
                280,
                image::Rgba([backdrop[0], backdrop[1], backdrop[2], 255]),
            );
            for y in 60..220 {
                for x in 90..210 {
                    img.put_pixel(x, y, image::Rgba([230, 160, 40, 255]));
                }
            }
            img
        };
        let lifted = RgbaImage::from_pixel(7, 7, image::Rgba([1, 2, 3, 255]));
        let lift = |_: &RgbaImage| Some(lifted.clone());
        let size = |backdrop, key| {
            cut_icon_lifting(&on(backdrop), key, lift)
                .unwrap()
                .dimensions()
        };
        assert_eq!(size([255, 0, 255], Key::Magenta), (120, 160), "keyed");
        assert_eq!(
            size([0, 255, 0], Key::Green),
            (120, 160),
            "cut from the edge"
        );
        // The lavender sweep klein drifts to, flat here, and white.
        assert_eq!(size([181, 154, 198], Key::Magenta), (7, 7), "lifted");
        assert_eq!(size([250, 250, 248], Key::Green), (7, 7), "lifted");
    }

    #[test]
    fn a_green_leaf_on_an_icon_cut_out_of_green_stays() {
        // An orange fruit with a leaf of the backdrop's own green in its middle.
        let mut img = RgbaImage::from_pixel(300, 280, image::Rgba([0, 255, 0, 255]));
        for y in 60..220 {
            for x in 90..210 {
                let leaf = (130..170).contains(&x) && (120..160).contains(&y);
                let p = if leaf {
                    [0, 255, 0, 255]
                } else {
                    [230, 160, 40, 255]
                };
                img.put_pixel(x, y, image::Rgba(p));
            }
        }
        let cut = cut_icon_lifting(&img, Key::Green, |_| None).unwrap();
        assert_eq!(cut.dimensions(), (120, 160));
        assert_eq!(
            cut.get_pixel(60, 80).0,
            [0, 255, 0, 255],
            "the leaf is kept"
        );
        assert_eq!(cut.get_pixel(5, 5).0, [230, 160, 40, 255]);
    }
}
