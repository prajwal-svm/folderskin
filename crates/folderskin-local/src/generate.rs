//! Painting one picture: the prompt, the runtime, and the clean-up afterwards.
//!
//! What comes out is a picture, not an icon. Artwork is wrapped onto FolderSkin's folder by the
//! app's own compositor, so its geometry is always exact; a whole folder is FolderSkin's blank
//! folder repainted and cut out along the app's own silhouette. Beside each picture goes a `.json`
//! with everything needed to paint it again and to say where it came from when it is shared.

use crate::command::{self, HEIGHT, WIDTH};
use crate::event::{Level, Reporter, Stage};
use crate::machine::{pick_backend, pick_tier, Arch, Backend, Machine, Os, Tier};
use crate::manifest::{sdcpp_assets, Model, ModelId, SDCPP_TAG};
use crate::prompts::{self, Shape};
use crate::{paths, CancelToken, Error};
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
    /// The subject and the scene in plain words.
    pub idea: String,
    /// A style preset's key ([`prompts::STYLES`]), someone's own words, or "none".
    pub style: String,
    pub shape: Shape,
    /// Reference pictures to paint from.
    pub refs: Vec<PathBuf>,
    pub seed: u64,
    /// The file name without its extension; `None` names it from the idea, style and seed.
    pub name: Option<String>,
    /// The model for plain artwork; `None` is Z-Image-Turbo. Pictures always go to klein.
    pub model: Option<ModelId>,
    /// Send `idea` to the model word for word, without FolderSkin's prompt around it.
    pub raw: bool,
}

impl Job {
    pub fn new(idea: impl Into<String>) -> Job {
        Job {
            idea: idea.into(),
            style: "none".into(),
            shape: Shape::Artwork,
            refs: Vec::new(),
            seed: 0,
            name: None,
            model: None,
            raw: false,
        }
    }

    /// The model that paints it: klein whenever there are pictures to work from (references, or
    /// the blank folder a whole-folder skin repaints), otherwise the one asked for.
    pub fn model(&self) -> &'static Model {
        if !self.refs.is_empty() || self.shape == Shape::Folder {
            ModelId::Klein.info()
        } else {
            self.model.unwrap_or(ModelId::Zimage).info()
        }
    }

    /// The prompt the model gets.
    pub fn prompt(&self) -> String {
        if self.raw {
            self.idea.trim().to_string()
        } else {
            prompts::compose(&self.idea, &self.style, self.shape, self.refs.len())
        }
    }

    /// The file name, without extension, the picture is saved under.
    pub fn file_name(&self) -> String {
        match &self.name {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => format!(
                "{}-{}-{}",
                slug(&self.idea, 40),
                slug(&self.style, 16),
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
    pub prompt: String,
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

/// A reference picture, by name and content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reference {
    pub file: String,
    pub sha256: String,
}

/// IPTC's term for a picture a model made, for anyone reading the provenance.
pub const TRAINED_ALGORITHMIC_MEDIA: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

/// A painted picture.
#[derive(Clone, Debug)]
pub struct Picture {
    /// The PNG, ready for FolderSkin.
    pub path: PathBuf,
    /// Artwork, or a whole folder; a folder whose shape the model changed stays on its backdrop.
    pub shape: Shape,
    pub provenance: Provenance,
}

/// Says what is missing before a job is started, so a batch fails at once rather than at its
/// first picture.
pub fn check_ready(job: &Job, settings: &Settings) -> Result<(), Error> {
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
        if paths::which(MFLUX_PROBE).is_none() {
            return Err(Error::environment(
                "mflux_missing",
                "mflux isn't installed.",
                "On Apple Silicon the models run in mflux, and it isn't on the PATH.",
            )
            .fix(setup));
        }
        return Ok(());
    }
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
    let model = job.model();
    let missing: Vec<&str> = model
        .files(settings.tier)
        .all()
        .iter()
        .filter(|f| !f.local().is_file())
        .map(|f| f.name())
        .collect();
    if !missing.is_empty() {
        return Err(Error::environment(
            "models_missing",
            format!("{} isn't downloaded yet.", model.label),
            format!(
                "{} {} missing from {}.",
                missing.join(", "),
                if missing.len() == 1 { "is" } else { "are" },
                paths::models_dir().display()
            ),
        )
        .fix(setup));
    }
    Ok(())
}

/// The mflux program whose presence means mflux is installed.
pub(crate) const MFLUX_PROBE: &str = "mflux-generate-z-image-turbo";

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

    // A whole folder repaints FolderSkin's blank folder, handed in as the first picture.
    let mut pictures = job.refs.clone();
    let silhouette = (job.shape == Shape::Folder).then(|| folder_silhouette(WIDTH, HEIGHT));
    if job.shape == Shape::Folder {
        let template = out_dir.join(".template.png");
        if !template.is_file() {
            let img = compositor::blank_template(WIDTH, HEIGHT, matte::MAGENTA);
            std::fs::write(&template, folderskin_core::raster::encode_png(&img))
                .map_err(|e| Error::io("write the blank folder", &template, &e))?;
        }
        pictures.insert(0, template);
    }
    let prompt = job.prompt();
    let steps = if settings.backend == Backend::Mlx {
        model.mlx_steps
    } else {
        model.steps
    };

    let (cmd, runtime) = if settings.backend == Backend::Mlx {
        let (program, args) =
            command::mflux(model, settings.tier, &prompt, job.seed, &pictures, &out);
        let mut cmd = Command::new(program);
        cmd.args(args);
        (cmd, "mflux".to_string())
    } else {
        let exe = paths::sd_cli(settings.backend);
        let mut cmd = Command::new(&exe);
        cmd.args(command::sdcpp(
            &model.files(settings.tier),
            model,
            &prompt,
            job.seed,
            &pictures,
            &out,
            settings.backend,
            settings.vram_gb,
        ));
        let release = std::fs::read_to_string(exe.with_file_name(".release"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| SDCPP_TAG.to_string());
        (cmd, format!("stable-diffusion.cpp {release}"))
    };
    // A picture left from an earlier run with the same name must not pass for this one's.
    let _ = std::fs::remove_file(&out);

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
                format!("mflux's program for {} isn't on the PATH.", model.label),
            )
            .fix("Install it: folderskin ai setup --backend mlx")
        } else {
            Error::environment(
                "runtime_failed_to_start",
                "The runtime couldn't be started.",
                format!("{runtime}: {e}."),
            )
            .fix(format!(
                "Reinstall it: delete {} and run folderskin ai setup",
                paths::sd_cli(settings.backend)
                    .parent()
                    .unwrap_or(Path::new("."))
                    .display()
            ))
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
    if !out.is_file() {
        return Err(Error::environment(
            "generation_failed",
            "The runtime finished without a picture.",
            format!(
                "{runtime} ended normally but wrote nothing to {}.{}",
                out.display(),
                tail_text(&finished.tail)
            ),
        )
        .fix("Run the same command again with --verbose to see everything it printed.")
        .fix("Check the runtime and the models: folderskin ai doctor"));
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
    match &silhouette {
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
                None => {
                    reporter.log(
                        Level::Warn,
                        format!(
                            "{name}: the model changed the folder's shape (fit {:.3}); left on \
                             its backdrop",
                            cut.fit
                        ),
                    );
                }
            }
        }
    }

    let references = job
        .refs
        .iter()
        .map(|p| {
            Ok(Reference {
                file: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                sha256: crate::download::sha256_file(p, &CancelToken::new())
                    .map_err(|e| Error::io("read a reference picture", p, &e))?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let provenance = Provenance {
        idea: job.idea.clone(),
        style: job.style.clone(),
        shape: job.shape,
        prompt,
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
        shape: job.shape,
        provenance,
    })
}

/// FolderSkin's silhouette in a `width` x `height` frame: white inside the folder, as the blank
/// template the model repaints has it.
pub fn folder_silhouette(width: u32, height: u32) -> GrayImage {
    let cut = compositor::blank_template_cutout(width, height);
    GrayImage::from_fn(width, height, |x, y| Luma([cut.get_pixel(x, y).0[3]]))
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
            if said.contains("out of memory") || said.contains("cannot allocate") {
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
    fn pictures_always_go_to_klein() {
        let mut job = Job::new("x");
        assert_eq!(job.model().id, ModelId::Zimage);
        job.model = Some(ModelId::Klein);
        assert_eq!(job.model().id, ModelId::Klein);
        job.model = Some(ModelId::Zimage);
        job.shape = Shape::Folder;
        assert_eq!(
            job.model().id,
            ModelId::Klein,
            "a folder repaints a picture"
        );
        job.shape = Shape::Artwork;
        job.refs = vec![PathBuf::from("dog.jpg")];
        assert_eq!(job.model().id, ModelId::Klein);
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
    }
}
