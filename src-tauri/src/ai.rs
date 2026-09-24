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
use folderskin_ai::{AiError, Finished, ProviderInfo};
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
    /// "skin" (flat artwork for our compositor) or "folder" (the model draws the whole folder).
    pub shape: String,
    pub size: Option<String>,
    pub reference_path: Option<String>,
    /// Every reference picture, for the models that take more than one.
    #[serde(default)]
    pub reference_paths: Vec<String>,
    /// Tags for the result, such as the style the idea asks for. Cleaned before saving.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The run's name, so `ai_cancel` can stop it.
    #[serde(default)]
    pub job: Option<String>,
}

impl AiGenerateRequest {
    /// Every reference picture given, the one-picture field included.
    fn references(&self) -> Vec<PathBuf> {
        let mut refs: Vec<&String> = self
            .reference_paths
            .iter()
            .filter(|p| !p.trim().is_empty())
            .collect();
        if refs.is_empty() {
            refs.extend(self.reference_path.iter().filter(|p| !p.trim().is_empty()));
        }
        refs.into_iter().map(PathBuf::from).collect()
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

/// Saves a key to the private key file. The key never comes back out to the webview.
#[tauri::command]
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

#[tauri::command]
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
            .fix("Try again; what was downloaded is kept.")
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
pub async fn ai_generate(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    local: State<'_, Local>,
    jobs: State<'_, Jobs>,
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
    let made = if req.provider == local::PROVIDER_ID {
        paint_here(local.inner(), &req, &job, send.clone(), &cancel).await?
    } else {
        ask_provider(&keys, &req, send.clone(), &cancel).await?
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
        tags: req.tags.clone(),
        pack: None,
        pack_name: None,
        author: None,
        license: None,
        pack_hash: None,
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
    job: &str,
    send: Sender,
    cancel: &folderskin_local::CancelToken,
) -> Result<Made, AiFailure> {
    let shape = match req.shape.as_str() {
        "folder" => folderskin_local::Shape::Folder,
        "skin" => folderskin_local::Shape::Artwork,
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
    let refs = req.references();
    let painted = local::paint(
        local::Order {
            job,
            idea: &req.idea,
            shape,
            model,
            refs: &refs,
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
    })
}

async fn ask_provider(
    keys: &Keys,
    req: &AiGenerateRequest,
    send: Sender,
    cancel: &folderskin_local::CancelToken,
) -> Result<Made, AiFailure> {
    let shape = Shape::from_id(&req.shape)
        .ok_or_else(|| AiFailure::failed(format!("Unknown shape {:?}.", req.shape)))?;
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
            Shape::Folder => "a whole folder picture",
            Shape::Skin => "folder artwork",
        }
    );
    let fail = |e: AiError| failure::from_provider(e, label, &doing).without(&key);

    // The first reference picture, for a model that takes one.
    let reference = req
        .references()
        .into_iter()
        .next()
        .filter(|_| model.accepts_reference);
    let reference_png = match reference {
        Some(path) => Some(
            tauri::async_runtime::spawn_blocking(move || load_reference(path))
                .await
                .map_err(|e| AiFailure::bug(format!("Reading the picture stopped: {e}.")))??,
        ),
        None => None,
    };
    // The prompt, the size, and our blank template when a whole folder should repaint it; shared
    // with the command line (folderskin_ai::finish).
    let (provider, idea, size) = (info.id.to_string(), req.idea.clone(), req.size.clone());
    let request = tauri::async_runtime::spawn_blocking(move || {
        folderskin_ai::plan(&provider, model, shape, &idea, size, reference_png)
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Preparing the request stopped: {e}.")))?;
    if cancel.is_cancelled() {
        return Err(AiFailure::stopped());
    }

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
    if shape == Shape::Folder {
        send(AiEvent::stage("cut", "Cutting it out of the background"));
    }
    let bytes = result.image.clone();
    let (image, warning) = tauri::async_runtime::spawn_blocking(move || finish(&result, shape))
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
    })
}

/// A provider's picture as the library keeps it. A whole folder that came back as a scene, with
/// no backdrop to cut it out of, is kept as artwork for FolderSkin's folder rather than thrown
/// away: it was paid for. The warning says so.
fn finish(
    result: &folderskin_ai::GenerateResult,
    shape: Shape,
) -> Result<(SkinImage, Option<String>), AiError> {
    let (finished, warning) = match folderskin_ai::finish(result, shape) {
        Err(AiError::NoBackdrop) => (
            folderskin_ai::finish(result, Shape::Skin)?,
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
    fn every_reference_is_used_and_the_one_picture_field_is_the_fallback() {
        let req = |one: Option<&str>, many: &[&str]| AiGenerateRequest {
            provider: "local".into(),
            model: "auto".into(),
            idea: "x".into(),
            shape: "folder".into(),
            size: None,
            reference_path: one.map(str::to_string),
            reference_paths: many.iter().map(|s| s.to_string()).collect(),
            tags: Vec::new(),
            job: None,
        };
        assert_eq!(
            req(Some("a.png"), &["a.png", "b.png"]).references(),
            [PathBuf::from("a.png"), PathBuf::from("b.png")]
        );
        assert_eq!(
            req(Some("a.png"), &[]).references(),
            [PathBuf::from("a.png")]
        );
        assert!(req(Some(" "), &[""]).references().is_empty());
    }

    #[test]
    fn a_request_from_an_older_window_still_reads() {
        let req: AiGenerateRequest = serde_json::from_value(serde_json::json!({
            "provider": "openai", "model": "gpt-image-1", "idea": "x", "shape": "skin",
            "size": null, "reference_path": null
        }))
        .unwrap();
        assert!(req.job.is_none() && req.reference_paths.is_empty() && req.tags.is_empty());
    }

    fn keyed(img: &image::RgbaImage) -> folderskin_ai::GenerateResult {
        folderskin_ai::GenerateResult {
            image: folderskin_core::raster::encode_png(img),
            media_type: "image/png".into(),
            native_alpha: false,
            model_used: "m".into(),
            revised_prompt: None,
        }
    }

    #[test]
    fn a_folder_without_its_backdrop_is_kept_as_artwork() {
        let scene = image::RgbaImage::from_fn(300, 280, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        let (image, warning) = finish(&keyed(&scene), Shape::Folder).unwrap();
        assert!(matches!(image, SkinImage::Artwork(_)));
        assert!(warning.unwrap().contains("kept as artwork"));
        // Artwork asked for is artwork, with nothing to warn about.
        let (image, warning) = finish(&keyed(&scene), Shape::Skin).unwrap();
        assert!(matches!(image, SkinImage::Artwork(_)) && warning.is_none());
        // Something that isn't a picture still fails.
        let broken = folderskin_ai::GenerateResult {
            image: b"nope".to_vec(),
            ..keyed(&scene)
        };
        assert!(matches!(
            finish(&broken, Shape::Folder),
            Err(AiError::NotAnImage)
        ));
    }
}
