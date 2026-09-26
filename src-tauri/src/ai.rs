//! The assistant's commands: where pictures can be made, keys, setting this computer up, and one
//! picture made and saved as a skin.
//!
//! A picture is made on this computer by folderskin-local's open-weight models ([`local`]), or by
//! a provider with the user's own key, read from FolderSkin's private key file at the moment of
//! the call. Nothing runs unless the user asks. A run reports how it's going over its channel
//! ([`events`]), can be stopped by its name ([`jobs`]), and fails with a structured
//! [`AiFailure`] the chat reads ([`failure`]). No key is ever returned to the webview or put in a
//! message.

pub mod events;
pub mod failure;
pub mod jobs;
pub mod local;

use crate::commands::SkinDto;
use crate::keys::Keys;
use crate::state::AppState;
use crate::store::{self, NewSkin, SkinImage, SkinSource};
use events::AiEvent;
use failure::{AiFailure, Doing};
use folderskin_ai::prompts::{self, Shape};
use folderskin_ai::recipe::{Record, Treatment};
use folderskin_ai::skill::Skill;
use folderskin_ai::{AiError, Cut, Finished, ProviderInfo, Reference, Role};
use folderskin_core::base::{self, Base};
use folderskin_core::compositor::Artwork;
use jobs::Jobs;
use local::{Local, LocalStatusDto};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::State;

#[derive(Serialize)]
pub struct AiModelDto {
    pub id: String,
    pub label: String,
    pub native_alpha: bool,
    pub accepts_reference: bool,
    /// How many pictures one request can carry, FolderSkin's template included.
    pub max_references: usize,
    pub sizes: Vec<String>,
    pub price_hint: String,
}

#[derive(Serialize)]
pub struct AiProviderDto {
    pub id: String,
    pub label: String,
    /// "local" runs on this computer and needs no key; "key" uses the user's own.
    pub kind: &'static str,
    pub models: Vec<AiModelDto>,
    pub keys_url: String,
    pub docs_url: String,
    pub key_hint: String,
    /// A key is saved; for this computer, it is set up and ready.
    pub has_key: bool,
}

#[derive(Serialize)]
pub struct AiPresetDto {
    pub id: String,
    pub label: String,
    pub idea: String,
}

#[derive(Serialize)]
pub struct AiCatalogueDto {
    pub providers: Vec<AiProviderDto>,
    pub presets: Vec<AiPresetDto>,
}

#[derive(Deserialize)]
pub struct AiGenerateRequest {
    pub provider: String,
    pub model: String,
    pub idea: String,
    /// "skin" (flat artwork for our compositor) or "folder" (the model draws the whole base). A
    /// free icon is painted as one whatever this says.
    pub shape: String,
    /// The shape it is for, by [`folderskin_core::base`] id: "mac-folder", "windows-folder",
    /// "free". FolderSkin's own folder when it is left out, as a window from before shapes does.
    #[serde(default)]
    pub base: Option<String>,
    /// A built-in style to paint it in, by [`folderskin_ai::styles`] id.
    #[serde(default)]
    pub style: Option<String>,
    /// A saved prompt whose look to paint it in, by id ([`crate::prompts`]). Its look is used in
    /// place of `style`'s.
    #[serde(default)]
    pub skill: Option<String>,
    /// The model's first size, as the window sends it. The shape decides the size a picture is
    /// asked for ([`folderskin_ai::plan`]), so this isn't used.
    pub size: Option<String>,
    pub reference_path: Option<String>,
    /// Every reference picture, for the models that take more than one.
    #[serde(default)]
    pub reference_paths: Vec<String>,
    /// What each of `reference_paths` is for: "subject", "style" or "palette". A picture with
    /// none is a subject.
    #[serde(default)]
    pub reference_roles: Vec<String>,
    /// Tags for the result, such as the style the idea asks for. Cleaned before saving.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The run's name, so `ai_cancel` can stop it.
    #[serde(default)]
    pub job: Option<String>,
}

impl AiGenerateRequest {
    /// The base it is for: the one named, or FolderSkin's own folder.
    fn base(&self) -> &'static Base {
        base::of_skin(self.base.as_deref())
    }

    /// The built-in style it asks for, when it names one this build has.
    fn style(&self) -> Option<&'static folderskin_ai::styles::Style> {
        self.style.as_deref().and_then(folderskin_ai::styles::style)
    }

    /// How it looks, for `provider`: the saved prompt's look when it names one that has a look
    /// (`skill`, read from `skills`), otherwise the built-in style's.
    fn treatment(&self, skill: Option<&Skill>, provider: &str) -> Option<Treatment> {
        skill
            .and_then(|s| s.treatment_for(provider))
            .or_else(|| self.style().map(Treatment::of))
    }

    /// Its tags: the ones it came with and its style's, or the style its saved prompt uses.
    fn tags(&self, skill: Option<&Skill>) -> Vec<String> {
        let mut tags = self.tags.clone();
        tags.extend(
            skill
                .and_then(Skill::style)
                .or_else(|| self.style())
                .map(|s| s.tag.clone()),
        );
        tags
    }

    /// Every reference picture given, the one-picture field included, each with its role.
    fn references(&self) -> Vec<(PathBuf, Role)> {
        let mut refs: Vec<(&String, Role)> = self
            .reference_paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let role = self
                    .reference_roles
                    .get(i)
                    .map_or(Role::Subject, |r| Role::of_picture(r));
                (p, role)
            })
            .filter(|(p, _)| !p.trim().is_empty())
            .collect();
        if refs.is_empty() {
            refs.extend(
                self.reference_path
                    .iter()
                    .filter(|p| !p.trim().is_empty())
                    .map(|p| (p, Role::Subject)),
            );
        }
        refs.into_iter()
            .map(|(p, role)| (PathBuf::from(p), role))
            .collect()
    }
}

/// Where pictures can be made: this computer, then the providers, with whether each is ready;
/// and the prompt presets. Looking at the machine takes a second the first time the list is
/// asked for; after that it's a few file checks.
#[tauri::command]
pub async fn ai_catalogue(
    keys: State<'_, Keys>,
    local: State<'_, Local>,
) -> Result<AiCatalogueDto, AiFailure> {
    let here = local.inner().clone();
    let ready = tauri::async_runtime::spawn_blocking(move || here.is_ready())
        .await
        .unwrap_or(false);
    let mut providers = vec![local::provider(ready)];
    providers.extend(
        folderskin_ai::providers()
            .iter()
            .map(|p| key_provider(p, keys.has(p.id))),
    );
    let presets = prompts::PRESETS
        .iter()
        .map(|p| AiPresetDto {
            id: p.id.to_string(),
            label: p.label.to_string(),
            idea: p.idea.to_string(),
        })
        .collect();
    Ok(AiCatalogueDto { providers, presets })
}

/// A shape the chat can paint on, as the prompt's picker shows it (src/lib/shapes.ts).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AiShapeDto {
    pub id: &'static str,
    /// Its name in English; the window names it in the language on show, by `id`.
    pub label: &'static str,
    /// "mac", "windows" or "linux", or "any" for a free icon.
    pub system: &'static str,
    /// "folder", "drive" or "free": where its skins go in the library.
    pub family: &'static str,
    /// Whether it can be painted whole, from its own template, as well as as artwork.
    pub whole: bool,
    /// The shape bare, as its system draws it, as a PNG data URL; none for a free icon.
    pub thumbnail: Option<String>,
}

/// Edge of a shape's picture in the picker, in pixels: sharp at twice the size it's shown at.
const SHAPE_THUMB: u32 = 96;

/// Every shape the chat can paint on, in the order the picker lists them
/// ([`folderskin_core::base::BASES`]), each with its bare picture. The pictures are drawn once.
#[tauri::command(async)]
pub fn ai_shapes() -> Vec<AiShapeDto> {
    static SHAPES: std::sync::OnceLock<Vec<AiShapeDto>> = std::sync::OnceLock::new();
    SHAPES
        .get_or_init(|| base::BASES.iter().map(shape_dto).collect())
        .clone()
}

fn shape_dto(b: &'static Base) -> AiShapeDto {
    AiShapeDto {
        id: b.id,
        label: b.label,
        system: b.system.id(),
        family: b.family.id(),
        whole: !b.is_free(),
        thumbnail: b
            .bare(SHAPE_THUMB)
            .map(|img| crate::commands::data_url(&folderskin_core::raster::encode_png(&img))),
    }
}

fn key_provider(p: &ProviderInfo, has_key: bool) -> AiProviderDto {
    AiProviderDto {
        id: p.id.to_string(),
        label: p.label.to_string(),
        kind: "key",
        models: p
            .models
            .iter()
            .map(|m| AiModelDto {
                id: m.id.to_string(),
                label: m.label.to_string(),
                native_alpha: m.native_alpha,
                accepts_reference: m.accepts_reference,
                max_references: m.max_references,
                sizes: m.sizes.iter().map(|s| s.to_string()).collect(),
                price_hint: m.price_hint.to_string(),
            })
            .collect(),
        keys_url: p.keys_url.to_string(),
        docs_url: p.docs_url.to_string(),
        key_hint: p.key_hint.to_string(),
        has_key,
    }
}

/// Saves a key to the private key file. The key never comes back out to the webview. Off the main
/// thread, like every command that waits for the disk.
#[tauri::command(async)]
pub fn ai_set_key(keys: State<'_, Keys>, provider: String, key: String) -> Result<(), AiFailure> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AiFailure::new("missing_key", "That key is empty."));
    }
    if folderskin_ai::catalogue::provider(&provider).is_none() {
        return Err(AiFailure::failed(format!(
            "FolderSkin doesn't know a provider called {provider:?}."
        )));
    }
    keys.set(&provider, key)
        .map_err(|e| AiFailure::failed(format!("The key couldn't be saved: {e}")).without(key))
}

#[tauri::command(async)]
pub fn ai_clear_key(keys: State<'_, Keys>, provider: String) -> Result<(), AiFailure> {
    keys.clear(&provider)
        .map_err(|e| AiFailure::failed(format!("The key couldn't be removed: {e}")))
}

/// Confirms the stored key is accepted by the provider, with a request that makes nothing.
#[tauri::command]
pub async fn ai_test_key(keys: State<'_, Keys>, provider: String) -> Result<(), AiFailure> {
    let info = known_provider(&provider)?;
    let key = stored_key(&keys, info)?;
    folderskin_ai::test_key(info.id, &key).await.map_err(|e| {
        failure::from_provider(e, info.label, &format!("checking a {} API key", info.label))
            .without(&key)
    })
}

/// Stops the run named `job`: a provider's request is dropped, this computer's painting or
/// setting up ([`jobs::LOCAL_SETUP`]) is ended. A Stop that arrives just before its run starts
/// stops it as it starts; one that arrives just after its run ended does nothing.
#[tauri::command]
pub fn ai_cancel(jobs: State<'_, Jobs>, job: String) {
    jobs.cancel(&job);
}

/// Whether the local model can paint here, what setting it up takes, and whether a setup is under
/// way (which `ai_local_setup` then joins).
#[tauri::command]
pub async fn ai_local_status(local: State<'_, Local>) -> Result<LocalStatusDto, AiFailure> {
    let here = local.inner().clone();
    tauri::async_runtime::spawn_blocking(move || here.status())
        .await
        .map_err(|e| AiFailure::bug(format!("Looking at your machine stopped: {e}.")))
}

/// Downloads and checks what this computer needs to paint, telling `on_event` as it goes, and
/// says how it stands afterwards. Stopped with `ai_cancel("local-setup")`; what was downloaded
/// is kept, and setting up again carries on from there. Asked while a setup is under way (the
/// window was closed and opened again), it joins that one: `on_event` hears where it has got to
/// and what comes next, and the answer is how it ends.
#[tauri::command]
pub async fn ai_local_setup(
    local: State<'_, Local>,
    jobs: State<'_, Jobs>,
    on_event: Channel<AiEvent>,
) -> Result<LocalStatusDto, AiFailure> {
    let lead = match local.join_setup(sender(on_event)) {
        local::SetupTurn::Join(join) => return join.outcome().await,
        local::SetupTurn::Lead(lead) => lead,
    };
    let outcome = set_up(local.inner(), &jobs, lead.listener()).await;
    lead.finish(outcome)
}

/// Removes what setting up downloaded (the model's files, the runtimes and their downloads, and
/// mflux when setup installed it) and says how the local model stands afterwards. Refused while a
/// setup is under way or a picture is being painted, which both use those files; the painting
/// turn is held meanwhile, so none starts half-way through.
#[tauri::command]
pub async fn ai_local_remove(local: State<'_, Local>) -> Result<LocalStatusDto, AiFailure> {
    if local.is_setting_up() {
        return Err(AiFailure::new("busy", "The local model is being set up.")
            .fix("Stop the setup, then remove the model."));
    }
    let Some(_turn) = local.try_turn() else {
        return Err(
            AiFailure::new("busy", "The local model is painting a picture.")
                .fix("Wait for it to finish, or stop it, then remove the model."),
        );
    };
    let here = local.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let doing = Doing {
            what: "removing the local model".into(),
            setup: true,
        };
        folderskin_local::remove(&folderskin_local::Reporter::silent())
            .map_err(|e| failure::from_engine(e, &doing))?;
        Ok(here.status())
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Removing the local model stopped: {e}.")))?
}

/// Removes the model files an earlier setup left that the Local Model doesn't use now, and says
/// how it stands after. Refused while it is being set up or painting, as removing the model is.
#[tauri::command]
pub async fn ai_local_remove_unused(local: State<'_, Local>) -> Result<LocalStatusDto, AiFailure> {
    if local.is_setting_up() {
        return Err(AiFailure::new("busy", "The local model is being set up.")
            .fix("Stop the setup, then remove the files."));
    }
    let Some(_turn) = local.try_turn() else {
        return Err(
            AiFailure::new("busy", "The local model is painting a picture.")
                .fix("Wait for it to finish, or stop it, then remove the files."),
        );
    };
    let here = local.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let doing = Doing {
            what: "removing model files the local model doesn't use".into(),
            setup: true,
        };
        folderskin_local::remove_unused(&here.settings(), &folderskin_local::Reporter::silent())
            .map_err(|e| failure::from_engine(e, &doing))?;
        Ok(here.status())
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Removing the files stopped: {e}.")))?
}

/// Sets the local model up, telling `send` how it goes.
async fn set_up(local: &Local, jobs: &Jobs, send: Sender) -> Result<LocalStatusDto, AiFailure> {
    // Only one setup is let through join_setup, so this is only a guard.
    let Some(running) = jobs.try_start(jobs::LOCAL_SETUP) else {
        return Err(
            AiFailure::new("busy", "The local model is already being set up.")
                .fix("Wait for it to finish, then try again."),
        );
    };
    send(AiEvent::stage("check", "Looking at your machine"));
    let here = local.clone();
    let (machine, settings) = tauri::async_runtime::spawn_blocking(move || {
        let machine = here.machine();
        let settings = here.settings();
        (machine, settings)
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Looking at your machine stopped: {e}.")))?;
    let on = format!(
        "{}, {}",
        local::backend_name(settings.backend),
        local::device(&machine, settings.backend)
    );
    send(AiEvent::info(format!(
        "{} -> {} with {} weights",
        machine.describe(),
        settings.backend,
        settings.tier
    )));
    let doing = Doing {
        what: format!("setting the local model up ({on})"),
        setup: true,
    };
    folderskin_local::setup(
        &machine,
        &settings,
        folderskin_local::Runtime::Pinned,
        &reporter(send.clone()),
        &running.token,
    )
    .await
    .map_err(|e| match failure::from_engine(e, &doing) {
        stopped if stopped.is_stopped() => AiFailure::new(
            "stopped",
            "Stopped. What was downloaded is kept, and setting up again carries on from there.",
        ),
        failure => failure,
    })?;
    send(AiEvent::stage("check", "Checking it runs"));
    let here = local.clone();
    let mut status = tauri::async_runtime::spawn_blocking(move || here.status())
        .await
        .map_err(|e| AiFailure::bug(format!("Checking the setup stopped: {e}.")))?;
    // Done with, as far as the window is concerned.
    status.setting_up = false;
    if let Some(problem) = &status.problem {
        return Err(runtime_wont_start(problem, &settings, &doing));
    }
    if !status.ready {
        let why = "Everything downloaded, but it still isn't ready to paint.";
        return Err(AiFailure::failed(why)
            .fix("Try again. What was downloaded is kept.")
            .asking(&doing.what, why));
    }
    Ok(status)
}

/// A runtime that setup installed but won't start. Setting up again leaves an installed runtime
/// as it is, so the fix is what does change it.
fn runtime_wont_start(
    problem: &local::RuntimeProblem,
    settings: &folderskin_local::Settings,
    doing: &Doing,
) -> AiFailure {
    let failure = AiFailure::new(problem.code, &problem.message);
    match problem.code {
        "vc_runtime_missing" => failure
            .fix("Install it from https://aka.ms/vs/17/release/vc_redist.x64.exe, then try again."),
        _ => failure
            .fix(format!(
                "Delete {} so it is installed afresh, then try again.",
                folderskin_local::paths::sd_cli(settings.backend)
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .display()
            ))
            .asking(&doing.what, &problem.message),
    }
}

/// Makes one picture and saves it as a skin, like an imported picture, telling `on_event` how
/// it's going. Stopped with `ai_cancel(req.job)`, it fails with the code "stopped".
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn ai_generate(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    local: State<'_, Local>,
    jobs: State<'_, Jobs>,
    saved: State<'_, crate::prompts::Prompts>,
    req: AiGenerateRequest,
    on_event: Channel<AiEvent>,
) -> Result<SkinDto, AiFailure> {
    static RUNS: AtomicU64 = AtomicU64::new(0);
    let job = req
        .job
        .clone()
        .filter(|j| !j.trim().is_empty())
        .unwrap_or_else(|| format!("run-{}", RUNS.fetch_add(1, Ordering::Relaxed)));
    let running = jobs.start(&job);
    let cancel = running.token.clone();
    let send = sender(on_event);
    if req.idea.trim().is_empty() {
        return Err(AiFailure::new(
            "no_idea",
            "Describe what the skin should look like first.",
        ));
    }
    // The saved prompt whose look it's painted in, read as it is now. One removed since it was
    // picked leaves the look of the style it used, or none.
    let skill = match req.skill.clone().filter(|s| !s.trim().is_empty()) {
        Some(id) => {
            let saved = saved.inner().clone();
            tauri::async_runtime::spawn_blocking(move || saved.find(&id))
                .await
                .ok()
                .flatten()
        }
        None => None,
    };
    let made = if req.provider == local::PROVIDER_ID {
        paint_here(
            local.inner(),
            &req,
            skill.as_ref(),
            &job,
            send.clone(),
            &cancel,
        )
        .await?
    } else {
        ask_provider(&keys, &req, skill.as_ref(), send.clone(), &cancel).await?
    };
    // Stopped as it arrived: it isn't wanted.
    if cancel.is_cancelled() {
        return Err(AiFailure::stopped());
    }
    send(AiEvent::stage("save", "Saving it to Yours"));
    let new = NewSkin {
        id: store::skin_id(&made.bytes),
        name: short_name(&req.idea),
        source: SkinSource::Ai,
        provider: Some(req.provider.clone()),
        model: Some(made.model),
        idea: Some(req.idea.trim().to_string()),
        tags: req.tags(skill.as_ref()),
        pack: None,
        pack_name: None,
        author: None,
        license: None,
        pack_hash: None,
        // Where it goes in the library: with the folders, or anywhere for a free icon.
        base: Some(req.base().id.to_string()),
        // What it was made from: the prompt as sent, its style and the pictures by role.
        recipe: Some(made.record),
    };
    let state = state.inner().clone();
    let image = made.image;
    tauri::async_runtime::spawn_blocking(move || {
        // A picture that took a paid request or a minute of the graphics card: a failed write
        // keeps it for the session instead of throwing it away.
        let (entry, thumb) = state.save(new.clone(), image.clone()).unwrap_or_else(|e| {
            eprintln!("folderskin: keeping {} for this session only: {e}", new.id);
            state.keep_unsaved(new, image)
        });
        SkinDto::saved(&entry, &thumb)
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Saving the picture stopped: {e}.")))
}

/// A picture, ready for the library.
struct Made {
    image: SkinImage,
    /// The bytes that name the skin: what the provider sent, or the PNG the runtime wrote.
    bytes: Vec<u8>,
    /// The model that made it, by id.
    model: String,
    /// What it was made from.
    record: Record,
}

type Sender = Arc<dyn Fn(AiEvent) + Send + Sync>;

/// Passes events to the window. One it can't take (the window has gone) is dropped: the run
/// carries on and its picture is still saved.
fn sender(channel: Channel<AiEvent>) -> Sender {
    Arc::new(move |event| {
        let _ = channel.send(event);
    })
}

/// The local engine's reporter, turning its events into the window's.
fn reporter(send: Sender) -> folderskin_local::Reporter {
    folderskin_local::Reporter::new(move |event| {
        for e in events::from_engine(event) {
            send(e);
        }
    })
}

async fn paint_here(
    local: &Local,
    req: &AiGenerateRequest,
    skill: Option<&Skill>,
    job: &str,
    send: Sender,
    cancel: &folderskin_local::CancelToken,
) -> Result<Made, AiFailure> {
    let shape = match req.shape.as_str() {
        "folder" => folderskin_local::Shape::Folder,
        "skin" => folderskin_local::Shape::Artwork,
        "icon" => folderskin_local::Shape::Icon,
        other => return Err(AiFailure::failed(format!("Unknown shape {other:?}."))),
    };
    let model = local::model_choice(&req.model)?;
    let here = local.clone();
    let (machine, settings) =
        tauri::async_runtime::spawn_blocking(move || (here.machine(), here.settings()))
            .await
            .map_err(|e| AiFailure::bug(format!("Looking at your machine stopped: {e}.")))?;
    // One painting at a time: two would share a graphics card that has room for one.
    let _turn = local.wait_turn(&*send, cancel).await?;
    let (refs, roles): (Vec<PathBuf>, Vec<Role>) = req.references().into_iter().unzip();
    let painted = local::paint(
        local::Order {
            job,
            idea: &req.idea,
            shape,
            base: req.base(),
            treatment: req.treatment(skill, local::PROVIDER_ID),
            model,
            refs: &refs,
            roles: &roles,
        },
        &machine,
        &settings,
        send,
        cancel,
    )
    .await?;
    Ok(Made {
        image: painted.image,
        bytes: painted.bytes,
        model: painted.model.id().to_string(),
        record: painted.record,
    })
}

async fn ask_provider(
    keys: &Keys,
    req: &AiGenerateRequest,
    skill: Option<&Skill>,
    send: Sender,
    cancel: &folderskin_local::CancelToken,
) -> Result<Made, AiFailure> {
    let base = req.base();
    let shape = Shape::from_id(&req.shape)
        .ok_or_else(|| AiFailure::failed(format!("Unknown shape {:?}.", req.shape)))?
        .on(base);
    let info = known_provider(&req.provider)?;
    let label = info.label;
    let model = folderskin_ai::model(info.id, &req.model).ok_or_else(|| {
        AiFailure::failed(format!(
            "{label} doesn't offer a model called {}.",
            req.model
        ))
        .fix("Choose another model in the provider settings.")
    })?;
    let key = stored_key(keys, info)?;
    let doing = format!(
        "asking {label} ({}) for {}",
        model.label,
        match shape {
            Shape::Folder => format!("a whole picture of the {}", base.label.to_lowercase()),
            Shape::Skin => format!("artwork for the {}", base.label.to_lowercase()),
            Shape::Icon => "a free icon".to_string(),
        }
    );
    let fail = |e: AiError| failure::from_provider(e, label, &doing).without(&key);

    // Every reference picture, with its role, for a model that takes pictures.
    let references: Vec<(PathBuf, Role)> = if model.accepts_reference {
        req.references()
    } else {
        Vec::new()
    };
    let pictures = tauri::async_runtime::spawn_blocking(move || {
        references
            .into_iter()
            .map(|(path, role)| load_reference(path).map(|png| Reference::new(role, png)))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Reading the pictures stopped: {e}.")))??;
    // The prompt, the shape's size, and the base's blank template when a whole base should
    // repaint it; shared with the command line (folderskin_ai::finish).
    let (provider, idea) = (info.id.to_string(), req.idea.clone());
    let treatment = req.treatment(skill, info.id);
    let planned = tauri::async_runtime::spawn_blocking(move || {
        let brief = folderskin_ai::Brief {
            idea: &idea,
            base,
            shape,
            treatment: treatment.as_ref(),
            pictures,
        };
        folderskin_ai::plan(&provider, model, &brief, None)
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Preparing the request stopped: {e}.")))?;
    if cancel.is_cancelled() {
        return Err(AiFailure::stopped());
    }
    let (request, cut, mut record) = (planned.request, planned.cut, planned.record);

    send(AiEvent::stage(
        "send",
        format!("Sending your idea to {label}"),
    ));
    send(AiEvent::info(format!(
        "model: {} ({}), {}",
        model.label,
        model.id,
        request.size.as_deref().unwrap_or(model.sizes[0])
    )));
    let started = Instant::now();
    // On a task of its own, so Stop can drop the request instead of waiting for the provider.
    let mut task = {
        let (request, key) = (request.clone(), key.clone());
        tokio::spawn(async move { folderskin_ai::generate(&request, &key).await })
    };
    let mut painting = false;
    let result = loop {
        if cancel.is_cancelled() {
            task.abort();
            return Err(AiFailure::stopped());
        }
        match tokio::time::timeout(Duration::from_millis(100), &mut task).await {
            Ok(joined) => {
                break joined
                    .map_err(|e| AiFailure::bug(format!("The request stopped: {e}.")))?
                    .map_err(fail)?
            }
            Err(_) if !painting && started.elapsed() > Duration::from_millis(1500) => {
                send(AiEvent::stage("paint", format!("{label} is painting it")));
                painting = true;
            }
            Err(_) => {}
        }
    };
    send(AiEvent::info(format!(
        "{label} answered in {:.0} s",
        started.elapsed().as_secs_f64()
    )));
    if let Some(revised) = &result.revised_prompt {
        send(AiEvent::info(format!(
            "{label} rewrote the prompt: {revised}"
        )));
    }
    // What the provider says it used or charged, so the price can be checked against the hint.
    if let Some(usage) = &result.usage {
        send(AiEvent::info(usage.clone()));
    }
    record.revised_prompt = result.revised_prompt.clone();
    if shape != Shape::Skin {
        send(AiEvent::stage("cut", "Cutting it out of the background"));
    }
    let bytes = result.image.clone();
    let (image, warning) =
        tauri::async_runtime::spawn_blocking(move || finish(&result, base, shape, cut))
            .await
            .map_err(|e| AiFailure::bug(format!("Finishing the picture stopped: {e}.")))?
            .map_err(fail)?;
    if let Some(warning) = warning {
        send(AiEvent::warn(warning));
    }
    Ok(Made {
        image,
        bytes,
        model: model.id.to_string(),
        record,
    })
}

/// A provider's picture as the library keeps it, for `base`, cut as `cut` says. A whole folder
/// that came back as a scene, with no backdrop to cut it out of, is kept as artwork for the
/// folder rather than thrown away: it was paid for. A free icon that did is kept as the square
/// picture it is, which is still an icon. The warning says which.
fn finish(
    result: &folderskin_ai::GenerateResult,
    base: &Base,
    shape: Shape,
    cut: Cut,
) -> Result<(SkinImage, Option<String>), AiError> {
    let shape = shape.on(base);
    let (finished, warning) = match folderskin_ai::finish(result, base, shape, cut) {
        Err(AiError::NoBackdrop) if shape == Shape::Icon => (
            folderskin_ai::finish::uncut(result)?,
            Some(
                "it came back without a plain background to cut it out of, so it's kept as the \
                 square picture it is"
                    .to_string(),
            ),
        ),
        Err(AiError::NoBackdrop) => (
            folderskin_ai::finish(result, base, Shape::Skin, cut)?,
            Some(
                "it came back as a scene rather than a folder on a plain backdrop, so it's kept \
                 as artwork for FolderSkin's folder"
                    .to_string(),
            ),
        ),
        other => (other?, None),
    };
    let image = match finished {
        Finished::Folder(cut) => SkinImage::Folder(Arc::new(cut)),
        Finished::Artwork(rgba) => SkinImage::Artwork(Arc::new(Artwork {
            rgba,
            focus: (0.5, 0.5),
        })),
    };
    Ok((image, warning))
}

// ---------- helpers ----------

fn known_provider(id: &str) -> Result<&'static ProviderInfo, AiFailure> {
    folderskin_ai::catalogue::provider(id).ok_or_else(|| {
        AiFailure::failed(format!("FolderSkin doesn't know a provider called {id:?}."))
            .fix("Choose another in the provider settings.")
    })
}

fn stored_key(keys: &Keys, provider: &ProviderInfo) -> Result<String, AiFailure> {
    keys.get(provider.id).ok_or_else(|| {
        failure::from_provider(
            AiError::MissingKey(provider.label.to_string()),
            provider.label,
            "",
        )
    })
}

/// Reads a reference picture and re-encodes it as a modest PNG for upload.
fn load_reference(path: PathBuf) -> Result<Vec<u8>, AiFailure> {
    let img = image::open(&path)
        .map_err(|_| {
            AiFailure::new(
                "reference_unreadable",
                "Couldn't read that reference picture.",
            )
            .fix("Use a PNG, JPEG or WebP picture.")
        })?
        .to_rgba8();
    Ok(folderskin_ai::finish::reference_png(img))
}

/// Little words a name shouldn't end on: "A lighthouse at" says less than "A lighthouse".
const LITTLE_WORDS: &[&str] = &[
    "a", "an", "the", "at", "of", "in", "on", "with", "and", "for", "to", "by",
];

/// A short, human label for a generated skin: the idea's first few words, never ending on a
/// little one, as the preview names it (src/lib/devMock.ts).
pub fn short_name(idea: &str) -> String {
    const MAX_BYTES: usize = 28;
    const MAX_WORDS: usize = 4;
    let cleaned: String = idea
        .chars()
        .map(|c| if ",.;:!?".contains(c) { ' ' } else { c })
        .collect();
    let mut words: Vec<String> = Vec::new();
    let mut len = 0;
    for word in cleaned.split_whitespace().take(MAX_WORDS) {
        let add = word.len() + usize::from(!words.is_empty());
        if len + add > MAX_BYTES {
            if words.is_empty() {
                // One long word: cut it on a character boundary (`truncate` panics inside one).
                let cut = (0..=MAX_BYTES)
                    .rev()
                    .find(|&i| word.is_char_boundary(i))
                    .unwrap_or(0);
                words.push(word[..cut].to_string());
            }
            break;
        }
        len += add;
        words.push(word.to_string());
    }
    while words.len() > 1
        && words
            .last()
            .is_some_and(|w| LITTLE_WORDS.contains(&w.to_lowercase().as_str()))
    {
        words.pop();
    }
    let name = words.join(" ");
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Generated".into(),
    }
}

/// First 12 hex characters of the image's SHA-256, so the same bytes always map to one id.
pub fn hash12(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_name_uses_the_first_words_and_capitalises() {
        assert_eq!(short_name("a night sky with aurora ribbons"), "A night sky");
        assert_eq!(short_name(""), "Generated");
        assert_eq!(short_name("   "), "Generated");
        assert!(short_name(&"verylongword".repeat(10)).len() <= 28);
    }

    #[test]
    fn short_name_never_ends_on_a_little_word() {
        assert_eq!(
            short_name("a lighthouse at dusk, oil painting"),
            "A lighthouse at dusk"
        );
        assert_eq!(short_name("a lighthouse on the rocks"), "A lighthouse");
        assert_eq!(short_name("Pop art cats of the city"), "Pop art cats");
        assert_eq!(short_name("the"), "The", "one word stays, little or not");
        assert_eq!(short_name("koi, in a pond"), "Koi in a pond");
        assert_eq!(short_name("koi, in a"), "Koi");
    }

    #[test]
    fn short_name_never_cuts_a_character_in_half() {
        // Ten three-byte characters: byte 28 falls inside the tenth.
        let name = short_name("桜桜桜桜桜桜桜桜桜桜 at night");
        assert_eq!(name, "桜".repeat(9));
        assert_eq!(short_name("🦊🦊🦊🦊🦊🦊🦊🦊"), "🦊".repeat(7));
    }

    #[test]
    fn a_runtime_that_wont_start_says_what_changes_that_not_to_set_up_again() {
        let settings = folderskin_local::Settings {
            backend: folderskin_local::Backend::Cuda,
            tier: folderskin_local::Tier::Q8,
            vram_gb: 4.0,
        };
        let doing = Doing {
            what: "setting the local model up (CUDA, RTX 3050 Ti)".into(),
            setup: true,
        };
        let vc = runtime_wont_start(
            &local::RuntimeProblem {
                code: "vc_runtime_missing",
                message: "stable-diffusion.cpp is installed but won't start: it needs the \
                          Microsoft Visual C++ runtime."
                    .into(),
            },
            &settings,
            &doing,
        );
        assert_eq!(vc.code, "vc_runtime_missing");
        assert!(vc.fix[0].contains("vc_redist.x64.exe"), "{vc:?}");
        assert!(!vc.fix.iter().any(|f| f.contains("up again")), "{vc:?}");
        assert_eq!(vc.ask, None, "installing it is the answer");
        let broken = runtime_wont_start(
            &local::RuntimeProblem {
                code: "runtime_failed_to_start",
                message: "stable-diffusion.cpp is installed but won't start: it stopped with \
                          exit code Some(3)."
                    .into(),
            },
            &settings,
            &doing,
        );
        assert_eq!(broken.code, "runtime_failed_to_start");
        let cuda = folderskin_local::paths::sd_cli(settings.backend);
        assert!(
            broken.fix[0].contains(&cuda.parent().unwrap().display().to_string()),
            "{broken:?}"
        );
        assert!(broken.ask.is_some());
    }

    #[test]
    fn hash12_is_stable_and_short() {
        assert_eq!(hash12(b"abc"), hash12(b"abc"));
        assert_ne!(hash12(b"abc"), hash12(b"abd"));
        assert_eq!(hash12(b"abc").len(), 12);
    }

    #[test]
    fn every_reference_is_used_with_its_role_and_the_one_picture_field_is_the_fallback() {
        let req = |one: Option<&str>, many: &[&str], roles: &[&str]| AiGenerateRequest {
            provider: "local".into(),
            model: "auto".into(),
            idea: "x".into(),
            shape: "folder".into(),
            base: None,
            style: None,
            skill: None,
            size: None,
            reference_path: one.map(str::to_string),
            reference_paths: many.iter().map(|s| s.to_string()).collect(),
            reference_roles: roles.iter().map(|s| s.to_string()).collect(),
            tags: Vec::new(),
            job: None,
        };
        assert_eq!(
            req(Some("a.png"), &["a.png", "b.png"], &["subject", "style"]).references(),
            [
                (PathBuf::from("a.png"), Role::Subject),
                (PathBuf::from("b.png"), Role::Style)
            ]
        );
        // A picture with no role, or one only FolderSkin gives, is a subject.
        assert_eq!(
            req(None, &["a.png", "b.png"], &["template"]).references(),
            [
                (PathBuf::from("a.png"), Role::Subject),
                (PathBuf::from("b.png"), Role::Subject)
            ]
        );
        assert_eq!(
            req(Some("a.png"), &[], &[]).references(),
            [(PathBuf::from("a.png"), Role::Subject)]
        );
        assert!(req(Some(" "), &[""], &[]).references().is_empty());
    }

    #[test]
    fn a_request_from_an_older_window_still_reads() {
        let req: AiGenerateRequest = serde_json::from_value(serde_json::json!({
            "provider": "openai", "model": "gpt-image-1", "idea": "x", "shape": "skin",
            "size": null, "reference_path": null
        }))
        .unwrap();
        assert!(req.job.is_none() && req.reference_paths.is_empty() && req.tags.is_empty());
        // Made before there were shapes: for FolderSkin's own folder, in no style.
        assert_eq!(req.base(), &base::MAC_FOLDER);
        assert!(req.style().is_none() && req.skill.is_none());
        assert!(req.treatment(None, "openai").is_none());
    }

    #[test]
    fn a_request_names_its_shape_and_style_and_is_tagged_with_the_style() {
        let req: AiGenerateRequest = serde_json::from_value(serde_json::json!({
            "provider": "local", "model": "klein", "idea": "a fox", "shape": "folder",
            "base": "windows-folder", "style": "ukiyoe", "tags": ["fox"],
            "size": null, "reference_path": null
        }))
        .unwrap();
        assert_eq!(req.base(), &base::WINDOWS_FOLDER);
        assert_eq!(req.style().map(|s| s.id.as_str()), Some("woodblock"));
        assert_eq!(req.tags(None), ["fox", "woodblock"]);
        assert_eq!(
            req.treatment(None, "local").map(|t| t.id),
            Some("woodblock".to_string())
        );
        // A shape or a style this build doesn't know is left out, not refused.
        let unknown: AiGenerateRequest = serde_json::from_value(serde_json::json!({
            "provider": "local", "model": "klein", "idea": "a fox", "shape": "folder",
            "base": "a-drive-from-later", "style": "nope",
            "size": null, "reference_path": null
        }))
        .unwrap();
        assert_eq!(unknown.base(), &base::MAC_FOLDER);
        assert!(unknown.style().is_none() && unknown.tags(None).is_empty());
    }

    #[test]
    fn a_saved_prompts_look_wins_over_the_style_and_tags_with_its_own() {
        let req: AiGenerateRequest = serde_json::from_value(serde_json::json!({
            "provider": "openai", "model": "gpt-image-2.5-flare", "idea": "a fox",
            "shape": "skin", "style": "oil", "skill": "night-prints-7k2q",
            "size": null, "reference_path": null
        }))
        .unwrap();
        let mut skill = Skill::saved("Night prints", "a koi pond", Some("woodblock"), "now");
        skill.light = Some("cool moonlight".into());
        let t = req.treatment(Some(&skill), "openai").unwrap();
        assert_eq!(t.id, skill.id);
        assert!(t.words.ends_with(", cool moonlight"), "{}", t.words);
        assert_eq!(req.tags(Some(&skill)), ["woodblock"]);
        // A saved prompt with words and no look leaves the style to the request.
        let words_only = Skill::saved("Koi", "a koi pond", None, "now");
        assert_eq!(
            req.treatment(Some(&words_only), "openai").map(|t| t.id),
            Some("oil".into())
        );
    }

    #[test]
    fn every_shape_is_listed_with_its_picture_and_where_it_goes() {
        let shapes = ai_shapes();
        let ids: Vec<&str> = shapes.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["mac-folder", "windows-folder", "free"]);
        for s in &shapes {
            assert_eq!(s.whole, s.family != "free", "{}", s.id);
            assert_eq!(s.thumbnail.is_some(), s.family != "free", "{}", s.id);
        }
        let mac = &shapes[0];
        assert_eq!(
            (mac.label, mac.system, mac.family),
            ("Mac folder", "mac", "folder")
        );
        assert!(mac
            .thumbnail
            .as_deref()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert_eq!((shapes[2].system, shapes[2].family), ("any", "free"));
        let json = serde_json::to_value(&shapes[2]).unwrap();
        assert!(json["thumbnail"].is_null(), "{json}");
        // Drawn once: the second list is the same pictures.
        assert_eq!(ai_shapes(), shapes);
    }

    fn keyed(img: &image::RgbaImage) -> folderskin_ai::GenerateResult {
        folderskin_ai::GenerateResult {
            image: folderskin_core::raster::encode_png(img),
            media_type: "image/png".into(),
            native_alpha: false,
            model_used: "m".into(),
            revised_prompt: None,
            key_colour: None,
            usage: None,
        }
    }

    const MAGENTA_CUT: Cut = Cut {
        key: folderskin_ai::recipe::Key::Magenta,
        template: None,
    };

    #[test]
    fn a_folder_without_its_backdrop_is_kept_as_artwork() {
        let scene = image::RgbaImage::from_fn(300, 280, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        let (image, warning) = finish(
            &keyed(&scene),
            &base::MAC_FOLDER,
            Shape::Folder,
            MAGENTA_CUT,
        )
        .unwrap();
        assert!(matches!(image, SkinImage::Artwork(_)));
        assert!(warning.unwrap().contains("kept as artwork"));
        // Artwork asked for is artwork, with nothing to warn about, at its folder's own size.
        let (image, warning) =
            finish(&keyed(&scene), &base::MAC_FOLDER, Shape::Skin, MAGENTA_CUT).unwrap();
        assert!(matches!(image, SkinImage::Artwork(_)) && warning.is_none());
        let (image, _) = finish(
            &keyed(&scene),
            &base::WINDOWS_FOLDER,
            Shape::Skin,
            MAGENTA_CUT,
        )
        .unwrap();
        assert_eq!(image.rgba().dimensions(), (1024, 805));
        // Something that isn't a picture still fails.
        let broken = folderskin_ai::GenerateResult {
            image: b"nope".to_vec(),
            ..keyed(&scene)
        };
        assert!(matches!(
            finish(&broken, &base::MAC_FOLDER, Shape::Folder, MAGENTA_CUT),
            Err(AiError::NotAnImage)
        ));
    }

    #[test]
    fn a_free_icon_is_cut_out_or_kept_whole_but_never_made_artwork() {
        let mut icon = image::RgbaImage::from_pixel(200, 200, image::Rgba([255, 0, 255, 255]));
        for y in 50..150 {
            for x in 60..140 {
                icon.put_pixel(x, y, image::Rgba([240, 170, 30, 255]));
            }
        }
        // Whatever shape a free icon is asked as, it is an icon, cut out and used as it is.
        let (image, warning) =
            finish(&keyed(&icon), &base::FREE, Shape::Skin, MAGENTA_CUT).unwrap();
        assert!(matches!(&image, SkinImage::Folder(cut) if cut.dimensions() == (80, 100)));
        assert!(warning.is_none());
        let scene = image::RgbaImage::from_fn(300, 280, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        let (image, warning) =
            finish(&keyed(&scene), &base::FREE, Shape::Icon, MAGENTA_CUT).unwrap();
        assert!(matches!(&image, SkinImage::Folder(whole) if whole.dimensions() == (300, 280)));
        assert!(warning.unwrap().contains("square picture"));
    }
}
