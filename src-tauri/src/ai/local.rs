//! The Local Model: pictures painted here by folderskin-local's open-weight models, with no key
//! and no account. What the provider list shows for it, what setting it up takes, and one
//! painting turned into what the library keeps.

use super::events::AiEvent;
use super::failure::{self, AiFailure, Doing};
use super::{AiModelDto, AiProviderDto};
use crate::store::SkinImage;
use folderskin_core::compositor::{Artwork, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte;
use folderskin_local::machine::Gpu;
use folderskin_local::{
    Backend, CancelToken, Job, Machine, ModelId, Reporter, Settings, Shape, Tier,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tokio::sync::watch;

/// The provider id the chat sends for the Local Model.
pub const PROVIDER_ID: &str = "local";

/// The model on offer, as the provider list shows it: klein, which paints from words and from
/// pictures alike.
const MODELS: [(&str, &str, bool); 1] = [("klein", "FLUX.2 klein 4B", true)];

/// Z-Image Turbo, which an earlier build also offered: skins it painted keep its name.
const RETIRED_LABELS: [(&str, &str); 1] = [("zimage", "Z-Image Turbo")];

/// The machine, looked at once (it runs `nvidia-smi` or asks the system for its display
/// adapters, a second or two), a turn for the one painting the graphics card has room for, and
/// the setup under way, if one is.
#[derive(Clone, Default)]
pub struct Local(Arc<Inner>);

#[derive(Default)]
struct Inner {
    machine: OnceLock<Machine>,
    turn: tokio::sync::Mutex<()>,
    /// Whether the runtime started the last time it was asked; `None` until it has been.
    runtime_starts: Mutex<Option<bool>>,
    /// The setup under way: a window that asks to set up joins it rather than starting another.
    setup: Mutex<Option<Arc<SetupRun>>>,
}

/// A lock that a panic elsewhere can't leave unusable: every change under these is one step.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Local {
    /// This computer. Slow the first time; call it off the async threads.
    pub fn machine(&self) -> Machine {
        self.0.machine.get_or_init(folderskin_local::detect).clone()
    }

    /// How the models run here.
    pub fn settings(&self) -> Settings {
        Settings::for_machine(&self.machine())
    }

    /// Whether pictures can be painted here now, by the same test as [`Local::status`]'s
    /// `ready`: the files are here, and the runtime starts. Whether it starts is remembered from
    /// the last status, so this is quick enough for the provider list; the first time the files
    /// are all here it is asked, which takes a second.
    pub fn is_ready(&self) -> bool {
        if !folderskin_local::is_set_up(&self.settings()) {
            return false;
        }
        let known = *lock(&self.0.runtime_starts);
        known.unwrap_or_else(|| self.status().ready)
    }

    /// How this computer stands ([`status`]), and whether it is being set up. Asks the runtime
    /// whether it starts, which takes a second: call it off the async threads.
    pub fn status(&self) -> LocalStatusDto {
        let mut status = status(&self.machine(), &self.settings());
        *lock(&self.0.runtime_starts) = Some(status.problem.is_none());
        status.setting_up = self.is_setting_up();
        status
    }

    /// Whether a setup is under way.
    pub fn is_setting_up(&self) -> bool {
        lock(&self.0.setup).is_some()
    }

    /// The turn to paint, when no picture is being painted; while it is held none can start.
    pub fn try_turn(&self) -> Option<tokio::sync::MutexGuard<'_, ()>> {
        self.0.turn.try_lock().ok()
    }

    /// Joins the setup under way, which `listener` then hears from where it has got to; or, when
    /// none is, makes this caller the one that runs it.
    pub fn join_setup(&self, listener: Listener) -> SetupTurn {
        let mut slot = lock(&self.0.setup);
        if let Some(run) = slot.as_ref() {
            run.listen(listener);
            return SetupTurn::Join(SetupJoin(run.done.subscribe()));
        }
        let run = Arc::new(SetupRun {
            heard: Mutex::new(Heard {
                listeners: vec![listener],
                ..Heard::default()
            }),
            done: watch::Sender::new(None),
        });
        *slot = Some(run.clone());
        SetupTurn::Lead(SetupLead {
            local: self.clone(),
            run,
            ended: false,
        })
    }

    /// Waits for the turn to paint, saying so if another picture is painting, and gives up the
    /// moment the run is stopped.
    pub async fn wait_turn(
        &self,
        send: &(dyn Fn(AiEvent) + Send + Sync),
        cancel: &CancelToken,
    ) -> Result<tokio::sync::MutexGuard<'_, ()>, AiFailure> {
        let mut said = false;
        loop {
            if cancel.is_cancelled() {
                return Err(AiFailure::stopped());
            }
            if let Ok(turn) = self.0.turn.try_lock() {
                return Ok(turn);
            }
            if !said {
                send(AiEvent::stage(
                    "wait",
                    "Waiting for the picture before it to finish",
                ));
                said = true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    }
}

/// Something that hears a run's events: a window's channel.
pub type Listener = Arc<dyn Fn(AiEvent) + Send + Sync>;

/// How a setup ended, as `ai_local_setup` answers.
pub type SetupOutcome = Result<LocalStatusDto, AiFailure>;

/// The log lines a window that joins a setup part-way is shown, as many as the window keeps.
const KEPT_LOG: usize = 300;

/// A setup under way. Every window that asked for it hears how it goes and gets how it ended; one
/// that joins part-way first hears where it has got to.
struct SetupRun {
    heard: Mutex<Heard>,
    done: watch::Sender<Option<SetupOutcome>>,
}

#[derive(Default)]
struct Heard {
    listeners: Vec<Listener>,
    stage: Option<AiEvent>,
    download: Option<AiEvent>,
    log: VecDeque<AiEvent>,
}

impl SetupRun {
    fn tell(&self, event: AiEvent) {
        let mut heard = lock(&self.heard);
        match &event {
            // A new stage is about something else ("Checking", the next file): the file before it
            // isn't where the setup has got to any more, and one that joins isn't told it is.
            AiEvent::Stage { .. } => {
                heard.stage = Some(event.clone());
                heard.download = None;
            }
            AiEvent::Download { .. } => heard.download = Some(event.clone()),
            AiEvent::Log { .. } => {
                if heard.log.len() == KEPT_LOG {
                    heard.log.pop_front();
                }
                heard.log.push_back(event.clone());
            }
            AiEvent::Progress { .. } => {}
        }
        // Sent while the list is held, so one that joins can't miss an event or hear it twice.
        for listener in &heard.listeners {
            listener(event.clone());
        }
    }

    fn listen(&self, listener: Listener) {
        let mut heard = lock(&self.heard);
        for event in heard.log.iter().chain(&heard.stage).chain(&heard.download) {
            listener(event.clone());
        }
        heard.listeners.push(listener);
    }
}

/// Whether a caller of [`Local::join_setup`] runs the setup or waits on the one under way.
pub enum SetupTurn {
    Lead(SetupLead),
    Join(SetupJoin),
}

/// Running the setup: its events go to every window that asked ([`SetupLead::listener`]), and
/// [`SetupLead::finish`] tells them all how it ended. Dropped without finishing, it tells them it
/// stopped unexpectedly, so nobody waits for ever.
pub struct SetupLead {
    local: Local,
    run: Arc<SetupRun>,
    ended: bool,
}

impl SetupLead {
    /// Where the setup's events go: to every window that asked for it.
    pub fn listener(&self) -> Listener {
        let run = self.run.clone();
        Arc::new(move |event| run.tell(event))
    }

    /// Ends the setup with `outcome`, which every window that joined it gets too.
    pub fn finish(mut self, outcome: SetupOutcome) -> SetupOutcome {
        self.end(outcome.clone());
        outcome
    }

    fn end(&mut self, outcome: SetupOutcome) {
        if std::mem::replace(&mut self.ended, true) {
            return;
        }
        let mut slot = lock(&self.local.0.setup);
        if slot.as_ref().is_some_and(|run| Arc::ptr_eq(run, &self.run)) {
            *slot = None;
        }
        drop(slot);
        self.run.done.send_replace(Some(outcome));
    }
}

impl Drop for SetupLead {
    fn drop(&mut self) {
        self.end(Err(AiFailure::bug(
            "Setting the local model up stopped unexpectedly.",
        )));
    }
}

/// Waiting on a setup another window started.
pub struct SetupJoin(watch::Receiver<Option<SetupOutcome>>);

impl SetupJoin {
    /// How the setup ended, once it has.
    pub async fn outcome(mut self) -> SetupOutcome {
        match self.0.wait_for(Option::is_some).await {
            Ok(ended) => ended.clone().unwrap_or_else(|| {
                Err(AiFailure::bug(
                    "Setting the local model up stopped unexpectedly.",
                ))
            }),
            Err(_) => Err(AiFailure::bug(
                "Setting the local model up stopped unexpectedly.",
            )),
        }
    }
}

/// The Local Model as the provider list shows it; `ready` is shown as its key being saved.
pub fn provider(ready: bool) -> AiProviderDto {
    AiProviderDto {
        id: PROVIDER_ID.into(),
        label: "Local Model".into(),
        kind: "local",
        models: MODELS
            .iter()
            .map(|(id, label, accepts_reference)| AiModelDto {
                id: id.to_string(),
                label: label.to_string(),
                native_alpha: false,
                accepts_reference: *accepts_reference,
                sizes: vec!["1024x1024".into()],
                price_hint: "Free".into(),
            })
            .collect(),
        keys_url: String::new(),
        docs_url: String::new(),
        key_hint: String::new(),
        has_key: ready,
    }
}

/// A local model by the name the provider list gave it. The names an earlier build offered,
/// `auto` and `zimage`, paint with klein too, so a chat saved then goes on working.
pub fn model_choice(id: &str) -> Result<Option<ModelId>, AiFailure> {
    match id {
        "auto" | "" | "zimage" => Ok(None),
        other => ModelId::parse(other).map(Some).ok_or_else(|| {
            AiFailure::failed(format!("There is no local model called {other:?}."))
                .fix("Choose another model in the provider settings.")
        }),
    }
}

/// The name a local model goes by in the library ("Local Model · FLUX.2 klein 4B"), a
/// retired one's too.
pub fn model_label(id: &str) -> Option<&'static str> {
    MODELS
        .iter()
        .map(|(m, label, _)| (*m, *label))
        .chain(RETIRED_LABELS)
        .find(|(m, _)| *m == id)
        .map(|(_, label)| label)
}

/// How the models run here, as people know it: CUDA, Vulkan, Metal, MLX or CPU.
pub fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Cuda => "CUDA",
        Backend::Vulkan => "Vulkan",
        Backend::Metal => "Metal",
        Backend::Cpu => "CPU",
        Backend::Mlx => "MLX",
    }
}

/// What the models run on, as people know it: "NVIDIA GeForce RTX 3050 Ti Laptop GPU, 4 GB".
pub fn device(machine: &Machine, backend: Backend) -> String {
    let gb = |v: f64| format!("{:.0} GB", v.max(1.0));
    match machine.gpu {
        _ if backend == Backend::Cpu || machine.gpu == Gpu::None => {
            format!("The processor, with {} of memory", gb(machine.ram_gb))
        }
        _ if machine.vram_gb > 0.0 => format!("{}, {}", machine.gpu_name, gb(machine.vram_gb)),
        _ => machine.gpu_name.clone(),
    }
}

/// Whether pictures can be made here, and what it takes (src/lib/tauri.ts `LocalStatus`).
#[derive(Clone, Debug, Serialize)]
pub struct LocalStatusDto {
    pub ready: bool,
    /// Whether setting up has anything to install here: false on a computer the runtime has no
    /// build for (an Intel Mac, ARM64 Linux), where `note` says so and nothing is offered.
    pub can_set_up: bool,
    /// A setup is under way, started by this window or another; `ai_local_setup` joins it.
    pub setting_up: bool,
    pub backend: String,
    pub device: String,
    /// What setting up downloads: the runtime's build (stable-diffusion.cpp) and the model's
    /// files, less what is already here.
    pub download_bytes: u64,
    /// The runtime setting up installs besides that, when it isn't installed yet: "mflux" on
    /// Apple Silicon, which setup installs with a uv and a Python of its own, whose packages
    /// `download_bytes` can't count.
    pub installs: Option<String>,
    /// What setup's files take on disk now, partial downloads included: what removing the model
    /// gives back.
    pub kept_bytes: u64,
    /// Of those, the model files an earlier setup left that the model doesn't use now: the other
    /// tier's, or Z-Image Turbo's ([`folderskin_local::unused`]).
    pub unused_bytes: u64,
    /// The model it paints with ("FLUX.2 [klein] 4B"), how finely ("4-bit") and what its files
    /// come to here.
    pub model: String,
    pub quality: String,
    pub model_bytes: u64,
    /// Free space on the disk the model goes on, when the system says, and what setting up wants
    /// free ([`folderskin_local::space_wanted`]): with less, it refuses and says to clear some.
    pub free_bytes: Option<u64>,
    pub wanted_bytes: u64,
    pub seconds_per_image: Option<f64>,
    pub home: String,
    pub note: Option<String>,
    /// Why the runtime won't start, when it is installed but doesn't; for `ai_local_setup`'s
    /// failure, since setting up again doesn't change it.
    #[serde(skip)]
    pub problem: Option<RuntimeProblem>,
}

/// An installed runtime that won't start.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProblem {
    /// "vc_runtime_missing" or "runtime_failed_to_start".
    pub code: &'static str,
    /// "stable-diffusion.cpp is installed but won't start: …"
    pub message: String,
}

/// Below this much memory a Mac swaps while klein paints: at 1024 x 960 mflux's footprint peaks
/// at 7.8 GB, decoding in tiles with a 2 GB cache (measured on an M3 Pro with mflux 0.20.0, 4-bit),
/// and an 8 GB Mac has little more than that for everything.
const LOW_MEMORY_GB: f64 = 16.0;

/// Looks at what is installed. Asks the runtime whether it starts, which takes a second: call it
/// off the async threads. Says nobody is setting it up; [`Local::status`] knows better.
pub fn status(machine: &Machine, settings: &Settings) -> LocalStatusDto {
    let status = folderskin_local::status(machine, settings);
    let model = ModelId::Klein.info();
    let runtime = &status.runtime;
    let models_here = status.models.iter().all(|m| m.ready());
    let ready = runtime.installed && runtime.problem.is_none() && models_here;
    let can_set_up =
        runtime.available && folderskin_local::setup::can_set_up(machine, settings.backend);
    let problem = runtime.problem.as_ref().map(|p| RuntimeProblem {
        code: runtime.problem_code.unwrap_or("runtime_failed_to_start"),
        message: format!("{} is installed but won't start: {p}.", runtime.name),
    });
    let mut notes = Vec::new();
    if let Some(problem) = &problem {
        notes.push(problem.message.clone());
    }
    if !ready && !can_set_up {
        // Nothing to set up and nothing to paint with: only what to do instead.
        let why = folderskin_local::setup::cannot_set_up(machine, settings.backend).unwrap_or_else(
            || folderskin_local::setup::no_build(machine.os, machine.arch, settings.backend),
        );
        notes.push(cannot_note(&why));
    } else {
        if settings.backend == Backend::Cpu {
            notes.push(
                "No graphics card it can use was found, so pictures are painted on the \
                 processor: expect a few minutes each."
                    .into(),
            );
        }
        if settings.backend == Backend::Mlx && machine.ram_gb < LOW_MEMORY_GB {
            notes.push(format!(
                "With {:.0} GB of memory this Mac paints slowly, and other apps slow down while it \
                 does: the model needs about 8 GB. A provider with your own key is quicker.",
                machine.ram_gb
            ));
        }
    }
    LocalStatusDto {
        ready,
        can_set_up,
        setting_up: false,
        backend: backend_name(settings.backend).into(),
        device: device(machine, settings.backend),
        download_bytes: if ready || !can_set_up {
            0
        } else {
            folderskin_local::download_size(machine, settings)
        },
        installs: (settings.backend == Backend::Mlx && !runtime.installed && can_set_up)
            .then(|| runtime.name.clone()),
        kept_bytes: folderskin_local::kept_bytes(),
        unused_bytes: folderskin_local::unused_bytes(settings),
        model: model.label.into(),
        quality: match settings.tier {
            Tier::Q4 => "4-bit",
            Tier::Q8 => "8-bit",
        }
        .into(),
        model_bytes: model
            .files_for(settings.backend, settings.tier)
            .iter()
            .map(|f| f.size)
            .sum(),
        free_bytes: folderskin_local::paths::free_space(&folderskin_local::home()),
        wanted_bytes: if ready || !can_set_up {
            0
        } else {
            folderskin_local::space_wanted(folderskin_local::space_needed(machine, settings))
        },
        seconds_per_image: Timing::read().seconds_for(settings),
        home: status.home.display().to_string(),
        note: (!notes.is_empty()).then(|| notes.join(" ")),
        problem,
    }
}

/// What the settings say when the local model can't be set up on this computer: why, what can be
/// done about it from the window (updating macOS), and that providers still work.
fn cannot_note(why: &folderskin_local::Error) -> String {
    let mut parts = vec![format!("{} {}", why.what, why.why)];
    if why.code == "macos_too_old" {
        // The command line's other ways, such as another backend, aren't the window's.
        parts.extend(failure::app_fixes(&why.fix));
    }
    parts.push("You can still make pictures with a provider and your own key.".into());
    parts.join(" ")
}

/// How long the last picture took here, so the next status can say roughly how long one takes.
/// Kept beside the models, since it belongs to this computer and how it runs them.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    pub backend: String,
    pub tier: String,
    pub seconds: f64,
}

impl Timing {
    fn file() -> PathBuf {
        folderskin_local::home().join("app-timing.json")
    }

    fn read() -> Timing {
        std::fs::read(Timing::file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// The time, when it was measured with these settings.
    pub fn seconds_for(&self, settings: &Settings) -> Option<f64> {
        (self.backend == settings.backend.id()
            && self.tier == settings.tier.id()
            && self.seconds > 0.0)
            .then_some(self.seconds)
    }

    fn record(settings: &Settings, seconds: f64) {
        let timing = Timing {
            backend: settings.backend.id().into(),
            tier: settings.tier.id().into(),
            seconds: (seconds * 10.0).round() / 10.0,
        };
        if let Ok(text) = serde_json::to_vec(&timing) {
            // Only a hint for next time: a computer that can't keep it just doesn't say.
            let _ = std::fs::write(Timing::file(), text);
        }
    }
}

/// What the chat asked this computer for.
pub struct Order<'a> {
    pub job: &'a str,
    pub idea: &'a str,
    pub shape: Shape,
    pub model: Option<ModelId>,
    pub refs: &'a [PathBuf],
}

/// A painting, ready for the library.
pub struct Painted {
    pub image: SkinImage,
    /// The PNG the runtime wrote, which names the skin.
    pub bytes: Vec<u8>,
    /// The model that painted it.
    pub model: ModelId,
}

/// Paints `order` with `settings` on `machine`, telling `send` how it goes.
pub async fn paint(
    order: Order<'_>,
    machine: &Machine,
    settings: &Settings,
    send: Arc<dyn Fn(AiEvent) + Send + Sync>,
    cancel: &CancelToken,
) -> Result<Painted, AiFailure> {
    let work = WorkDir::new(order.job);
    let job_refs = {
        let (dir, refs) = (work.0.clone(), order.refs.to_vec());
        tokio::task::spawn_blocking(move || copy_refs(&dir, &refs))
            .await
            .map_err(|e| AiFailure::bug(format!("Getting the pictures ready stopped: {e}.")))?
    };
    let job = Job {
        idea: order.idea.trim().to_string(),
        // The idea already says its style, in the chat's own words.
        style: "none".into(),
        shape: order.shape,
        refs: job_refs,
        seed: folderskin_local::random_seed(),
        name: Some("picture".into()),
        model: order.model,
        raw: false,
    };
    let model = job.model().id;
    let doing = Doing {
        what: format!(
            "painting {} with the local model ({}, {})",
            match order.shape {
                Shape::Folder => "a whole folder",
                Shape::Artwork => "folder artwork",
            },
            backend_name(settings.backend),
            device(machine, settings.backend)
        ),
        setup: false,
    };
    send(AiEvent::info(format!(
        "backend: {} ({})",
        backend_name(settings.backend),
        device(machine, settings.backend)
    )));
    send(AiEvent::info(format!(
        "model: {}, {} weights",
        model.info().label,
        settings.tier
    )));
    let reporter = {
        let send = send.clone();
        Reporter::new(move |event| {
            for e in super::events::from_engine(event) {
                send(e);
            }
        })
    };
    let picture = folderskin_local::generate(&job, settings, &work.0, &reporter, cancel)
        .await
        .map_err(|e| failure::from_engine(e, &doing))?;
    Timing::record(settings, picture.provenance.seconds);
    let shape = picture.shape;
    if order.shape == Shape::Folder {
        send(AiEvent::stage("cut", "Cutting it out of the background"));
    }
    let (image, bytes, warning) = tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&picture.path).map_err(|e| {
            AiFailure::failed(format!("The painting couldn't be read back: {e}.")).fix("Try again.")
        })?;
        let rgba = image::load_from_memory(&bytes)
            .map_err(|e| {
                AiFailure::failed(format!("The painting couldn't be read back: {e}."))
                    .fix("Try again.")
            })?
            .to_rgba8();
        let (image, warning) = skin_image(rgba, shape);
        Ok::<_, AiFailure>((image, bytes, warning))
    })
    .await
    .map_err(|e| AiFailure::bug(format!("Finishing the painting stopped: {e}.")))??;
    if let Some(warning) = warning {
        send(AiEvent::warn(warning));
    }
    Ok(Painted {
        image,
        bytes,
        model,
    })
}

/// What the library keeps of a painting: artwork for FolderSkin's folder, cropped to the
/// template's shape; or a whole folder, cut out. A folder that couldn't be cut out (the model
/// changed its shape and no backdrop is left to key) is kept as artwork rather than lost, and
/// the warning says so.
pub fn skin_image(rgba: image::RgbaImage, shape: Shape) -> (SkinImage, Option<String>) {
    let artwork = |rgba: &image::RgbaImage| {
        SkinImage::Artwork(Arc::new(Artwork {
            rgba: matte::crop_to_aspect(rgba, SKIN_WIDTH, SKIN_HEIGHT, (0.5, 0.5)),
            focus: (0.5, 0.5),
        }))
    };
    match shape {
        Shape::Artwork => (artwork(&rgba), None),
        Shape::Folder => match matte::finished_cutout(&rgba, matte::MAGENTA) {
            Some(cut) => (SkinImage::Folder(Arc::new(cut)), None),
            None => (
                artwork(&rgba),
                Some(
                    "the folder couldn't be cut out of its picture, so it's kept as artwork for \
                     FolderSkin's folder"
                        .into(),
                ),
            ),
        },
    }
}

/// Copies the reference pictures in beside the painting under plain names, so a picture kept
/// in a folder whose path isn't plain ASCII still reaches stable-diffusion.cpp. One that isn't
/// there is passed on as it is, for the engine to say so.
fn copy_refs(dir: &Path, refs: &[PathBuf]) -> Vec<PathBuf> {
    refs.iter()
        .enumerate()
        .map(|(i, r)| {
            let ext = r
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or_else(|| "png".into());
            let to = dir.join(format!("reference-{}.{ext}", i + 1));
            match std::fs::copy(r, &to) {
                Ok(_) => to,
                Err(_) => r.clone(),
            }
        })
        .collect()
}

/// A folder of its own for one painting, under the engine's home (whose path the models already
/// need to be usable from), gone again when the painting is done with.
struct WorkDir(PathBuf);

impl WorkDir {
    fn new(job: &str) -> WorkDir {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let n = SERIAL.fetch_add(1, Ordering::Relaxed);
        let dir = folderskin_local::home().join("app").join(format!(
            "{}-{}-{n}",
            folderskin_local::slug(job, 40),
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        WorkDir(dir)
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_local::machine::{Arch, Os};
    use image::{Rgba, RgbaImage};

    fn machine(gpu: Gpu, name: &str, vram: f64) -> Machine {
        Machine {
            os: Os::Windows,
            arch: Arch::X86_64,
            ram_gb: 31.7,
            gpu,
            gpu_name: name.into(),
            vram_gb: vram,
        }
    }

    #[test]
    fn this_computer_is_listed_like_the_preview_lists_it() {
        let p = provider(true);
        assert_eq!(
            (p.id.as_str(), p.label.as_str(), p.kind),
            ("local", "Local Model", "local")
        );
        assert!(p.has_key, "set up and ready shows as ready");
        assert!(!provider(false).has_key);
        let ids: Vec<&str> = p.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["klein"], "one model, which paints everything");
        assert!(p
            .models
            .iter()
            .all(|m| m.price_hint == "Free" && !m.native_alpha && m.accepts_reference));
        assert!(p.keys_url.is_empty() && p.key_hint.is_empty());
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["kind"], "local");
    }

    #[test]
    fn models_are_chosen_by_the_names_the_list_gives_them() {
        assert_eq!(model_choice("klein").unwrap(), Some(ModelId::Klein));
        // What chats saved by an earlier build ask for still paints, with klein.
        assert_eq!(model_choice("auto").unwrap(), None);
        assert_eq!(model_choice("").unwrap(), None);
        assert_eq!(model_choice("zimage").unwrap(), None);
        assert_eq!(model_choice("sdxl").unwrap_err().code, "failed");
        assert_eq!(model_label("klein"), Some("FLUX.2 klein 4B"));
        // A skin Z-Image painted keeps its name in the library.
        assert_eq!(model_label("zimage"), Some("Z-Image Turbo"));
        assert_eq!(model_label("nope"), None);
    }

    #[test]
    fn the_device_is_named_as_people_know_it() {
        let nvidia = machine(Gpu::Nvidia, "NVIDIA GeForce RTX 3050 Ti Laptop GPU", 4.0);
        assert_eq!(
            device(&nvidia, Backend::Cuda),
            "NVIDIA GeForce RTX 3050 Ti Laptop GPU, 4 GB"
        );
        let amd = machine(Gpu::Other, "AMD Radeon(TM) Graphics", 0.0);
        assert_eq!(device(&amd, Backend::Vulkan), "AMD Radeon(TM) Graphics");
        let none = machine(Gpu::None, "", 0.0);
        assert_eq!(
            device(&none, Backend::Cpu),
            "The processor, with 32 GB of memory"
        );
        assert_eq!(backend_name(Backend::Cuda), "CUDA");
        assert_eq!(backend_name(Backend::Mlx), "MLX");
    }

    #[test]
    fn a_computer_with_no_build_is_offered_no_setup() {
        let intel_mac = Machine {
            os: Os::Macos,
            arch: Arch::X86_64,
            ram_gb: 16.0,
            gpu: Gpu::Other,
            gpu_name: "Intel Iris Plus Graphics".into(),
            vram_gb: 0.0,
        };
        let settings = Settings::for_machine(&intel_mac);
        assert_eq!((settings.backend, settings.tier), (Backend::Cpu, Tier::Q4));
        let s = status(&intel_mac, &settings);
        assert!(!s.can_set_up && !s.setting_up);
        if !s.ready {
            assert_eq!(s.download_bytes, 0, "nothing would be downloaded");
            let note = s.note.as_deref().unwrap();
            assert!(note.contains("Apple Silicon only"), "{note}");
            assert!(
                !note.contains("processor") && !note.contains("4-bit"),
                "only what to do instead: {note}"
            );
        }
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("problem").is_none(), "{json}");
        assert_eq!(
            json["installs"],
            serde_json::Value::Null,
            "nothing to install here"
        );
    }

    #[test]
    fn a_mac_too_old_for_the_local_model_is_told_to_update_macos() {
        let note = cannot_note(&folderskin_local::setup::macos_too_old((13, 6)));
        assert!(
            note.starts_with("The local model needs macOS 14 or later"),
            "{note}"
        );
        assert!(
            note.contains("Software Update), then set the local model up again."),
            "{note}"
        );
        assert!(
            !note.contains("folderskin ai"),
            "no commands in the window: {note}"
        );
        assert!(note.ends_with("a provider and your own key."), "{note}");
    }

    /// A listener that keeps what it hears.
    fn keeping() -> (Listener, Arc<std::sync::Mutex<Vec<AiEvent>>>) {
        let kept = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = kept.clone();
        (Arc::new(move |e| sink.lock().unwrap().push(e)), kept)
    }

    fn block_on<T>(fut: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fut)
    }

    #[test]
    fn a_window_opened_again_joins_the_setup_under_way() {
        let here = Local::default();
        assert!(!here.is_setting_up());
        let (first, first_heard) = keeping();
        let SetupTurn::Lead(lead) = here.join_setup(first) else {
            panic!("nothing was being set up");
        };
        assert!(here.is_setting_up());
        let tell = lead.listener();
        tell(AiEvent::info("installed cwebp"));
        tell(AiEvent::stage(
            "download",
            "Downloading z_image_turbo-Q8_0.gguf",
        ));
        let at = |done| AiEvent::Download {
            file: "z_image_turbo-Q8_0.gguf".into(),
            done,
            total: 100,
        };
        tell(at(10));
        tell(at(20));
        let (second, second_heard) = keeping();
        let SetupTurn::Join(join) = here.join_setup(second) else {
            panic!("it joins the one under way");
        };
        // Where it has got to, at once, then what comes next.
        tell(at(50));
        assert_eq!(
            *second_heard.lock().unwrap(),
            [
                AiEvent::info("installed cwebp"),
                AiEvent::stage("download", "Downloading z_image_turbo-Q8_0.gguf"),
                at(20),
                at(50),
            ]
        );
        assert_eq!(
            first_heard.lock().unwrap().len(),
            5,
            "the first hears it all"
        );
        let ended = lead.finish(Err(AiFailure::stopped()));
        assert!(ended.unwrap_err().is_stopped());
        assert!(!here.is_setting_up());
        assert!(block_on(join.outcome()).unwrap_err().is_stopped());
        // A setup asked for now is a new one.
        let (third, _) = keeping();
        assert!(matches!(here.join_setup(third), SetupTurn::Lead(_)));
        assert!(!here.is_setting_up(), "dropped, it lets go");
    }

    #[test]
    fn a_window_that_joins_between_files_is_told_the_stage_alone() {
        let here = Local::default();
        let (first, _) = keeping();
        let SetupTurn::Lead(lead) = here.join_setup(first) else {
            panic!("nothing was being set up");
        };
        let tell = lead.listener();
        tell(AiEvent::stage("download", "Downloading libwebp.zip"));
        tell(AiEvent::Download {
            file: "libwebp.zip".into(),
            done: 100,
            total: 100,
        });
        tell(AiEvent::stage("download", "Downloading sd-master.zip"));
        let (second, second_heard) = keeping();
        let SetupTurn::Join(_) = here.join_setup(second) else {
            panic!("it joins the one under way");
        };
        // Not libwebp's last count under sd-master's stage.
        assert_eq!(
            *second_heard.lock().unwrap(),
            [AiEvent::stage("download", "Downloading sd-master.zip")]
        );
        drop(lead);
    }

    #[test]
    fn a_setup_that_ends_unexpectedly_still_answers_those_waiting() {
        let here = Local::default();
        let (a, _) = keeping();
        let (b, _) = keeping();
        let SetupTurn::Lead(lead) = here.join_setup(a) else {
            panic!("nothing was being set up");
        };
        let SetupTurn::Join(join) = here.join_setup(b) else {
            panic!("it joins");
        };
        drop(lead);
        assert!(!here.is_setting_up());
        let e = block_on(join.outcome()).unwrap_err();
        assert_eq!(e.code, "failed");
        assert!(e.message.contains("unexpectedly"), "{e:?}");
    }

    #[test]
    fn a_runtime_that_wouldnt_start_isnt_listed_as_ready() {
        // As the settings panel last found it: installed, but it wouldn't start.
        let here = Local::default();
        *lock(&here.0.runtime_starts) = Some(false);
        assert!(!here.is_ready(), "the list and the panel agree");
    }

    #[test]
    fn a_mac_says_what_setting_up_takes_and_when_it_is_short_of_memory() {
        let mac = |ram_gb| Machine {
            os: Os::Macos,
            arch: Arch::Arm64,
            ram_gb,
            gpu: Gpu::Apple,
            gpu_name: "Apple M2".into(),
            vram_gb: 0.0,
        };
        let small = mac(8.0);
        let s = status(&small, &Settings::for_machine(&small));
        assert!(s.can_set_up);
        assert_eq!(s.backend, "MLX");
        if !s.ready {
            // The weights are counted now, not left to download the first time it paints.
            assert!(s.download_bytes > 0 || s.installs.is_some(), "{s:?}");
            assert!(s.download_bytes <= 4_619_699_678, "{s:?}");
            let note = s.note.as_deref().unwrap_or_default();
            assert!(
                note.contains("8 GB of memory") && note.contains("12 GB"),
                "{note}"
            );
        }
        assert!(s.installs.is_none() || s.installs.as_deref() == Some("mflux"));
        let roomy = mac(36.0);
        let s = status(&roomy, &Settings::for_machine(&roomy));
        assert!(
            !s.note.as_deref().unwrap_or_default().contains("12 GB"),
            "{s:?}"
        );
    }

    #[test]
    fn a_timing_counts_only_for_the_settings_it_was_measured_with() {
        let settings = Settings {
            backend: Backend::Cuda,
            tier: Tier::Q8,
            vram_gb: 4.0,
        };
        let t = Timing {
            backend: "cuda".into(),
            tier: "q8".into(),
            seconds: 28.4,
        };
        assert_eq!(t.seconds_for(&settings), Some(28.4));
        let vulkan = Settings {
            backend: Backend::Vulkan,
            ..settings
        };
        assert_eq!(t.seconds_for(&vulkan), None);
        assert_eq!(Timing::default().seconds_for(&settings), None);
    }

    #[test]
    fn artwork_is_cropped_to_the_template_and_a_cut_folder_kept_whole() {
        let painting = RgbaImage::from_pixel(1024, 960, Rgba([40, 90, 160, 255]));
        match skin_image(painting.clone(), Shape::Artwork) {
            (SkinImage::Artwork(art), None) => {
                assert_eq!(art.rgba.dimensions(), (SKIN_WIDTH, SKIN_HEIGHT))
            }
            _ => panic!("artwork stays artwork"),
        }
        // Cut out along the silhouette: transparent around the folder.
        let mut cut = RgbaImage::from_pixel(1024, 960, Rgba([0, 0, 0, 0]));
        for y in 200..800 {
            for x in 100..900 {
                cut.put_pixel(x, y, Rgba([200, 120, 40, 255]));
            }
        }
        match skin_image(cut, Shape::Folder) {
            (SkinImage::Folder(f), None) => assert_eq!(f.dimensions(), (800, 600)),
            _ => panic!("a cut-out folder is a folder"),
        }
    }

    #[test]
    fn a_folder_that_couldnt_be_cut_out_is_kept_as_artwork() {
        // The model painted a scene over the whole frame: no backdrop to key.
        let scene = RgbaImage::from_fn(1024, 960, |x, y| {
            Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
        });
        let (image, warning) = skin_image(scene, Shape::Folder);
        assert!(matches!(image, SkinImage::Artwork(_)));
        assert!(warning.unwrap().contains("kept as artwork"));
    }

    #[test]
    fn references_are_copied_in_under_plain_names() {
        let dir = std::env::temp_dir().join(format!("fs-refs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("фото")).unwrap();
        let photo = dir.join("фото").join("кот.JPG");
        std::fs::write(&photo, b"jpeg").unwrap();
        let gone = dir.join("gone.png");
        let out = dir.join("work");
        std::fs::create_dir_all(&out).unwrap();
        let refs = copy_refs(&out, &[photo, gone.clone()]);
        assert_eq!(refs[0], out.join("reference-1.jpg"));
        assert_eq!(std::fs::read(&refs[0]).unwrap(), b"jpeg");
        assert_eq!(
            refs[1], gone,
            "a missing one is left for the engine to report"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Paints `order` here, keeping every event it sends.
    fn paint_here(
        order: Order<'_>,
        cancel: &CancelToken,
    ) -> (Result<Painted, AiFailure>, Vec<AiEvent>) {
        let here = Local::default();
        assert!(
            here.is_ready(),
            "set this computer up first: folderskin ai setup"
        );
        let (machine, settings) = (here.machine(), here.settings());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        let send: Arc<dyn Fn(AiEvent) + Send + Sync> =
            Arc::new(move |e| sink.lock().unwrap().push(e));
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(paint(order, &machine, &settings, send, cancel));
        let events = seen.lock().unwrap().clone();
        (result, events)
    }

    #[test]
    #[ignore = "paints on this computer's graphics card for half a minute, once it is set up"]
    fn a_whole_folder_is_painted_here_with_its_progress() {
        let (result, events) = paint_here(
            Order {
                job: "c-test-folder",
                idea: "a koi pond at night with paper lanterns",
                shape: Shape::Folder,
                model: None,
                refs: &[],
            },
            &CancelToken::new(),
        );
        let painted = result.unwrap();
        assert_eq!(
            painted.model,
            ModelId::Klein,
            "a whole folder goes to klein"
        );
        assert!(image::load_from_memory(&painted.bytes).is_ok());
        let stages: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                AiEvent::Stage { message, .. } => Some(message.as_str()),
                _ => None,
            })
            .collect();
        assert!(stages.contains(&"Loading the model"), "{stages:?}");
        assert!(stages.contains(&"Painting"), "{stages:?}");
        assert!(events
            .iter()
            .any(|e| matches!(e, AiEvent::Progress { steps, .. } if *steps > 0)));
        let work = folderskin_local::home().join("app");
        assert!(
            !std::fs::read_dir(&work)
                .is_ok_and(|mut d| d.any(|e| e
                    .is_ok_and(|e| e.file_name().to_string_lossy().starts_with("c-test-folder")))),
            "the painting's own folder is gone afterwards"
        );
    }

    #[test]
    #[ignore = "starts the runtime on this computer's graphics card, once it is set up"]
    fn a_painting_stops_when_asked() {
        let cancel = CancelToken::new();
        let stop = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(4));
            stop.cancel();
        });
        let started = std::time::Instant::now();
        let (result, _) = paint_here(
            Order {
                job: "c-test-stop",
                idea: "a lighthouse at dusk",
                shape: Shape::Artwork,
                model: None,
                refs: &[],
            },
            &cancel,
        );
        assert!(result.err().is_some_and(|e| e.is_stopped()));
        assert!(started.elapsed() < std::time::Duration::from_secs(15));
    }
}
