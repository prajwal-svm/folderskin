//! One way to paint whichever model does it: a local model through folderskin-local, or a
//! provider with the person's own key through folderskin-ai, processed the way the app does.

use crate::cli::{BackendArg, MachineArgs, ShapeArg, TierArg};
use crate::config::{self, Config, KeySource, LOCAL};
use crate::error::CliError;
use crate::out::Out;
use folderskin_ai::prompts::Shape as AiShape;
use folderskin_ai::{AiError, Finished, ModelInfo, ProviderInfo};
use folderskin_local::{
    generate, slug, Backend, CancelToken, Job, Machine, ModelId, Settings, Shape, Stage, Tier,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where a setting came from, for `doctor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Flag,
    Config,
    Detected,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Flag => "from the command line",
            Source::Config => "from ai config",
            Source::Detected => "detected",
        }
    }
}

/// The backend and tier to use: a flag first, then `ai config`, then what suits the computer.
pub fn settings(
    machine: &Machine,
    args: &MachineArgs,
    config: &Config,
) -> Result<(Settings, Source, Source), CliError> {
    let detected = Settings::for_machine(machine);
    let backend_flag = args
        .backend
        .filter(|b| *b != BackendArg::Auto)
        .map(|b| match b {
            BackendArg::Cuda => Backend::Cuda,
            BackendArg::Vulkan => Backend::Vulkan,
            BackendArg::Metal => Backend::Metal,
            BackendArg::Cpu => Backend::Cpu,
            BackendArg::Mlx | BackendArg::Auto => Backend::Mlx,
        });
    let backend_config = config
        .backend
        .as_deref()
        .filter(|b| *b != "auto")
        .map(|b| {
            Backend::parse(b).ok_or_else(|| {
                CliError::fixable(
                    "config_value",
                    format!("The configured backend {b:?} isn't one FolderSkin knows."),
                    "It can be auto, cuda, vulkan, metal, cpu or mlx.",
                )
                .fix("Change it: folderskin ai config set backend auto")
            })
        })
        .transpose()?;
    let (backend, backend_source) = match (backend_flag, backend_config) {
        (Some(b), _) => (b, Source::Flag),
        (None, Some(b)) => (b, Source::Config),
        (None, None) => (detected.backend, Source::Detected),
    };
    let tier_flag = args.tier.filter(|t| *t != TierArg::Auto).map(|t| match t {
        TierArg::Q4 => Tier::Q4,
        TierArg::Q8 | TierArg::Auto => Tier::Q8,
    });
    let tier_config = config
        .tier
        .as_deref()
        .filter(|t| *t != "auto")
        .and_then(Tier::parse);
    let (tier, tier_source) = match (tier_flag, tier_config) {
        (Some(t), _) => (t, Source::Flag),
        (None, Some(t)) => (t, Source::Config),
        (None, None) => (detected.tier, Source::Detected),
    };
    if backend == Backend::Mlx && !(cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")) {
        return Err(CliError::fixable(
            "backend_unavailable",
            "mlx only runs on Apple Silicon.",
            format!(
                "This is {} {}.",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        )
        .fix(format!("Leave --backend out to use {}.", detected.backend)));
    }
    Ok((
        Settings {
            backend,
            tier,
            vram_gb: machine.vram_gb,
        },
        backend_source,
        tier_source,
    ))
}

/// What paints.
#[derive(Clone)]
pub enum Painter {
    Local {
        settings: Settings,
        model: Option<ModelId>,
    },
    Byok {
        provider: &'static ProviderInfo,
        model: &'static ModelInfo,
        key: String,
    },
}

/// One picture to paint.
#[derive(Clone, Debug)]
pub struct Order {
    pub idea: String,
    pub style: String,
    pub shape: Shape,
    pub refs: Vec<PathBuf>,
    pub seed: u64,
    pub name: Option<String>,
    pub raw: bool,
}

/// A painted picture, and what to tell about it.
#[derive(Clone, Debug)]
pub struct Painted {
    pub path: PathBuf,
    pub shape: Shape,
    pub meta: serde_json::Value,
}

pub fn shape(arg: ShapeArg) -> Shape {
    match arg {
        ShapeArg::Artwork => Shape::Artwork,
        ShapeArg::Folder => Shape::Folder,
    }
}

impl Painter {
    /// The painter the flags and `ai config` ask for: local unless a provider is named.
    pub fn choose(
        provider: Option<&str>,
        model: Option<&str>,
        machine_args: &MachineArgs,
        config: &Config,
    ) -> Result<Painter, CliError> {
        let provider = provider
            .map(str::to_lowercase)
            .or_else(|| config.provider.clone())
            .unwrap_or_else(|| LOCAL.into());
        let model = model
            .map(str::to_string)
            .or_else(|| config.model_for(&provider).map(str::to_string));
        if provider == LOCAL {
            let model = match model.as_deref() {
                None => None,
                Some(m) => local_model(m)?,
            };
            let machine = folderskin_local::detect();
            let (settings, _, _) = settings(&machine, machine_args, config)?;
            return Ok(Painter::Local { settings, model });
        }
        let Some(info) = folderskin_ai::provider(&provider) else {
            return Err(CliError::fixable(
                "unknown_provider",
                format!("FolderSkin doesn't know a provider called {provider:?}."),
                format!(
                    "It can paint with {LOCAL} models, or with your key at {}.",
                    config::provider_ids().join(", ")
                ),
            )
            .fix("See them all: folderskin ai models"));
        };
        let model_info = match model.as_deref() {
            None => &info.models[0],
            Some(m) => provider_model(info, m)?,
        };
        let keys = config::keys();
        let Some((key, _)) = config::key_for(info.id, &keys) else {
            return Err(CliError::fixable(
                "key_missing",
                format!("There is no {} API key yet.", info.label),
                format!(
                    "Painting with {} uses your own key, and none is saved or set in {}.",
                    info.label,
                    config::key_variable(info.id)
                ),
            )
            .fix(format!(
                "Get one at {}, then save it: folderskin ai key set {}",
                info.keys_url, info.id
            ))
            .fix(format!(
                "Or set {} for this session.",
                config::key_variable(info.id)
            )));
        };
        Ok(Painter::Byok {
            provider: info,
            model: model_info,
            key,
        })
    }

    /// The same painter with another model: a brief's own, or `theme`'s klein.
    pub fn with_model(&self, model: &str) -> Result<Painter, CliError> {
        Ok(match self {
            Painter::Local { settings, .. } => Painter::Local {
                settings: *settings,
                model: local_model(model)?,
            },
            Painter::Byok { provider, key, .. } => Painter::Byok {
                provider,
                model: provider_model(provider, model)?,
                key: key.clone(),
            },
        })
    }

    /// Says now what is missing, rather than at the first picture of a batch.
    pub fn check_ready(&self, order: &Order) -> Result<(), CliError> {
        match self {
            Painter::Local { settings, model } => {
                folderskin_local::check_ready(&self.job(order, *model), settings)
                    .map_err(CliError::from)
            }
            Painter::Byok { .. } => {
                for r in &order.refs {
                    if !r.is_file() {
                        return Err(CliError::fixable(
                            "reference_missing",
                            "A reference picture isn't there.",
                            format!("{} doesn't exist or isn't a file.", r.display()),
                        ));
                    }
                }
                Ok(())
            }
        }
    }

    fn job(&self, order: &Order, model: Option<ModelId>) -> Job {
        Job {
            idea: order.idea.clone(),
            style: order.style.clone(),
            shape: order.shape,
            refs: order.refs.clone(),
            seed: order.seed,
            name: order.name.clone(),
            model,
            raw: order.raw,
        }
    }

    /// Whether this painter uses seeds, so a picture can be painted again exactly.
    pub fn seeded(&self) -> bool {
        matches!(self, Painter::Local { .. })
    }

    pub fn describe(&self) -> String {
        match self {
            Painter::Local { settings, .. } => {
                format!("local models ({}, {})", settings.backend, settings.tier)
            }
            Painter::Byok {
                provider, model, ..
            } => format!("{} {}", provider.label, model.label),
        }
    }

    /// Paints one picture into `out_dir`.
    pub async fn paint(
        &self,
        order: &Order,
        out_dir: &Path,
        out: &Arc<Out>,
        cancel: &CancelToken,
    ) -> Result<Painted, CliError> {
        match self {
            Painter::Local { settings, model } => {
                let job = self.job(order, *model);
                let picture = generate(&job, settings, out_dir, &out.reporter(), cancel).await?;
                let meta = serde_json::to_value(&picture.provenance).unwrap_or_default();
                Ok(Painted {
                    path: picture.path,
                    shape: picture.shape,
                    meta,
                })
            }
            Painter::Byok {
                provider,
                model,
                key,
            } => byok(provider, model, key, order, out_dir, out, cancel).await,
        }
    }
}

/// A local model by name; `auto` leaves the choice to the job.
fn local_model(name: &str) -> Result<Option<ModelId>, CliError> {
    match name.to_lowercase().as_str() {
        "auto" => Ok(None),
        m => ModelId::parse(m).map(Some).ok_or_else(|| {
            CliError::fixable(
                "unknown_model",
                format!("There is no local model called {m:?}."),
                "The local models are zimage (Z-Image-Turbo) and klein (FLUX.2 [klein] 4B).",
            )
            .fix("Use --model zimage, --model klein, or leave it out.")
        }),
    }
}

/// One of `provider`'s models by id.
fn provider_model(provider: &ProviderInfo, id: &str) -> Result<&'static ModelInfo, CliError> {
    folderskin_ai::model(provider.id, id).ok_or_else(|| {
        CliError::fixable(
            "unknown_model",
            format!("{} doesn't offer a model called {id:?}.", provider.label),
            format!(
                "Its models are {}.",
                provider
                    .models
                    .iter()
                    .map(|m| m.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .fix("See them all: folderskin ai models")
    })
}

async fn byok(
    provider: &'static ProviderInfo,
    model: &'static ModelInfo,
    key: &str,
    order: &Order,
    out_dir: &Path,
    out: &Arc<Out>,
    cancel: &CancelToken,
) -> Result<Painted, CliError> {
    if order.shape == Shape::Folder && !model.accepts_reference && !model.native_alpha {
        out.warn(&format!(
            "{} can't see FolderSkin's folder, so it draws a folder of its own on magenta",
            model.label
        ));
    }
    let reference = match order.refs.as_slice() {
        [] => None,
        [first, rest @ ..] => {
            if !model.accepts_reference {
                out.warn(&format!(
                    "{} can't work from a picture, so the reference is left out",
                    model.label
                ));
                None
            } else {
                if !rest.is_empty() {
                    out.warn(&format!(
                        "{} takes one picture; only {} goes",
                        provider.label,
                        first.display()
                    ));
                }
                let img = image::open(first)
                    .map_err(|e| {
                        CliError::fixable(
                            "reference_unreadable",
                            "The reference picture can't be read.",
                            format!("{}: {e}.", first.display()),
                        )
                        .fix("Use a PNG, JPEG or WebP picture.")
                    })?
                    .to_rgba8();
                Some(folderskin_ai::finish::reference_png(img))
            }
        }
    };
    let style = folderskin_local::prompts::style_text(&order.style).to_string();
    let idea = if style.is_empty() {
        order.idea.trim().to_string()
    } else {
        format!("{}, as {style}", order.idea.trim().trim_end_matches('.'))
    };
    let ai_shape = match order.shape {
        Shape::Artwork => AiShape::Skin,
        Shape::Folder => AiShape::Folder,
    };
    let request = if order.raw {
        folderskin_ai::GenerateRequest {
            provider: provider.id.to_string(),
            model: model.id.to_string(),
            prompt: order.idea.trim().to_string(),
            reference_png: reference,
            size: None,
            want_alpha: false,
        }
    } else {
        let (p, i) = (provider.id.to_string(), idea.clone());
        tokio::task::spawn_blocking(move || {
            folderskin_ai::plan(&p, model, ai_shape, &i, None, reference)
        })
        .await
        .map_err(|e| CliError::bug("Planning the request stopped unexpectedly.", e.to_string()))?
    };
    cancel.check()?;
    out.event(&folderskin_local::Event::Stage {
        stage: Stage::Request,
        message: format!("Asking {} ({})", provider.label, model.label),
    });
    let started = std::time::Instant::now();
    // On a task of its own, so Ctrl+C can drop the request instead of waiting for the provider.
    let task = {
        let (request, key) = (request.clone(), key.to_string());
        tokio::spawn(async move { folderskin_ai::generate(&request, &key).await })
    };
    while !task.is_finished() {
        if cancel.is_cancelled() {
            task.abort();
            return Err(crate::error::cancelled());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let result = task
        .await
        .map_err(|e| CliError::bug("The request stopped unexpectedly.", e.to_string()))?
        .map_err(|e| ai_error(provider, e))?;
    let seconds = started.elapsed().as_secs_f64();
    let finished = {
        let result = result.clone();
        tokio::task::spawn_blocking(move || folderskin_ai::finish(&result, ai_shape))
            .await
            .map_err(|e| {
                CliError::bug("Finishing the picture stopped unexpectedly.", e.to_string())
            })?
            .map_err(|e| ai_error(provider, e))?
    };
    std::fs::create_dir_all(out_dir)
        .map_err(|e| CliError::io("make the output folder", out_dir, &e))?;
    let name = match &order.name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => format!(
            "{}-{}-{}",
            slug(&order.idea, 40),
            slug(&order.style, 16),
            &hash_hex(&result.image)[..8]
        ),
    };
    let path = out_dir.join(format!("{name}.png"));
    let (png, shape) = match finished {
        Finished::Folder(cut) => (folderskin_core::raster::encode_png(&cut), Shape::Folder),
        Finished::Artwork(art) => (folderskin_core::raster::encode_png(&art), Shape::Artwork),
    };
    std::fs::write(&path, png).map_err(|e| CliError::io("save the picture", &path, &e))?;
    let meta = json!({
        "idea": order.idea,
        "style": order.style,
        "shape": shape,
        "prompt": request.prompt,
        "revised_prompt": result.revised_prompt,
        "provider": provider.label,
        "model": model.label,
        "model_id": result.model_used,
        "references": order.refs.iter().take(1).map(|r| r.file_name().map(|n| n.to_string_lossy().into_owned())).collect::<Vec<_>>(),
        "seconds": (seconds * 10.0).round() / 10.0,
        "made": folderskin_local::generate::utc_now(),
        "digital_source_type": folderskin_local::generate::TRAINED_ALGORITHMIC_MEDIA,
    });
    let json_path = path.with_extension("json");
    let text = serde_json::to_string_pretty(&meta).unwrap_or_default() + "\n";
    std::fs::write(&json_path, text)
        .map_err(|e| CliError::io("save the picture's record", &json_path, &e))?;
    Ok(Painted { path, shape, meta })
}

fn hash_hex(bytes: &[u8]) -> String {
    use std::hash::{DefaultHasher, Hasher};
    let mut h = DefaultHasher::new();
    h.write(bytes);
    format!("{:016x}", h.finish())
}

/// A provider's failure, with what to do about it.
pub fn ai_error(provider: &ProviderInfo, e: AiError) -> CliError {
    let sentence = |s: String| {
        let mut chars = s.chars();
        let first: String = chars
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let s = first + chars.as_str();
        if s.ends_with(['.', '!', '?']) {
            s
        } else {
            s + "."
        }
    };
    let text = sentence(e.to_string());
    let key_fix = format!("Save it again: folderskin ai key set {}", provider.id);
    match e {
        AiError::UnknownProvider(_) | AiError::UnknownModel { .. } => {
            CliError::fixable("unknown_model", text, "")
                .fix("See what's on offer: folderskin ai models")
        }
        AiError::MissingKey(_) => CliError::fixable("key_missing", text, "")
            .fix(format!("Save one: folderskin ai key set {}", provider.id)),
        AiError::Unauthorized(_) => CliError::fixable(
            "key_rejected",
            format!("{} didn't accept the key.", provider.label),
            text,
        )
        .fix(format!("Check it at {}", provider.keys_url))
        .fix(key_fix),
        AiError::RateLimited(_) => CliError::environment("rate_limited", text, "")
            .fix("Wait a minute, then run the command again."),
        AiError::Refused(why) => CliError::fixable(
            "refused",
            format!("{} wouldn't paint that.", provider.label),
            sentence(why),
        )
        .fix("Word the idea differently and try again."),
        AiError::Network { .. } => CliError::environment(
            "network",
            format!("{} couldn't be reached.", provider.label),
            text,
        )
        .fix("Check the internet connection (and any proxy or firewall), then try again."),
        AiError::Timeout { .. } => CliError::environment("timeout", text, "")
            .fix("Try again, or pick a faster model with --model."),
        AiError::NoBackdrop => CliError::fixable(
            "no_backdrop",
            "The picture came back as a scene, not a folder on a plain backdrop.",
            text,
        )
        .fix("Try again: another run usually keeps the backdrop.")
        .fix("Or paint artwork instead, which doesn't need one: --shape artwork"),
        AiError::Provider { .. }
        | AiError::Decode(_)
        | AiError::Unsupported(_)
        | AiError::NotAnImage => CliError::environment(
            "provider_error",
            format!("{} couldn't paint it.", provider.label),
            text,
        )
        .fix("Try again in a little while; if it keeps happening, try another model or provider."),
    }
}

/// Where the key for `provider` comes from, for `models` and `doctor`.
pub fn key_state(provider: &ProviderInfo, keys: &folderskin_keys::Keys) -> String {
    match config::key_for(provider.id, keys) {
        Some((_, KeySource::Environment)) => {
            format!("key from {}", config::key_variable(provider.id))
        }
        Some((_, KeySource::Saved)) => "key saved".into(),
        None => "no key".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> Machine {
        Machine {
            os: folderskin_local::machine::Os::Windows,
            arch: folderskin_local::machine::Arch::X86_64,
            ram_gb: 32.0,
            gpu: folderskin_local::machine::Gpu::Nvidia,
            gpu_name: "GPU".into(),
            vram_gb: 4.0,
        }
    }

    #[test]
    fn a_flag_beats_the_config_which_beats_the_computer() {
        let mut config = Config::default();
        let none = MachineArgs::default();
        let (s, b, t) = settings(&machine(), &none, &config).unwrap();
        assert_eq!(
            (s.backend, s.tier, b, t),
            (Backend::Cuda, Tier::Q8, Source::Detected, Source::Detected)
        );
        config.backend = Some("vulkan".into());
        config.tier = Some("q4".into());
        let (s, b, t) = settings(&machine(), &none, &config).unwrap();
        assert_eq!(
            (s.backend, s.tier, b, t),
            (Backend::Vulkan, Tier::Q4, Source::Config, Source::Config)
        );
        let flags = MachineArgs {
            backend: Some(BackendArg::Cpu),
            tier: Some(TierArg::Q8),
        };
        let (s, b, _) = settings(&machine(), &flags, &config).unwrap();
        assert_eq!(
            (s.backend, s.tier, b),
            (Backend::Cpu, Tier::Q8, Source::Flag)
        );
        let auto = MachineArgs {
            backend: Some(BackendArg::Auto),
            tier: Some(TierArg::Auto),
        };
        assert_eq!(
            settings(&machine(), &auto, &Config::default())
                .unwrap()
                .0
                .backend,
            Backend::Cuda
        );
    }

    #[test]
    fn mlx_is_only_for_apple_silicon() {
        let flags = MachineArgs {
            backend: Some(BackendArg::Mlx),
            tier: None,
        };
        let result = settings(&machine(), &flags, &Config::default());
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().code, "backend_unavailable");
        }
    }

    #[test]
    fn provider_failures_say_what_to_do() {
        let openai = folderskin_ai::provider("openai").unwrap();
        let e = ai_error(openai, AiError::Unauthorized("OpenAI".into()));
        assert_eq!(e.code, "key_rejected");
        assert!(e
            .fix
            .iter()
            .any(|f| f.contains("folderskin ai key set openai")));
        let e = ai_error(openai, AiError::NoBackdrop);
        assert!(e.fix.iter().any(|f| f.contains("--shape artwork")));
        let e = ai_error(openai, AiError::MissingKey("OpenAI".into()));
        assert_eq!(e.what, "Add your OpenAI API key first.");
    }

    #[test]
    fn unknown_providers_and_models_are_caught_before_anything_runs() {
        let none = MachineArgs::default();
        let config = Config::default();
        let e = Painter::choose(Some("midjourney"), None, &none, &config)
            .err()
            .unwrap();
        assert_eq!(e.code, "unknown_provider");
        let e = Painter::choose(Some("openai"), Some("dall-e-9"), &none, &config)
            .err()
            .unwrap();
        assert_eq!(e.code, "unknown_model");
        let e = Painter::choose(None, Some("sdxl"), &none, &config)
            .err()
            .unwrap();
        assert_eq!(e.code, "unknown_model");
    }
}
