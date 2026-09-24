//! Getting a computer ready: the runtime, the models and cwebp, downloaded and checked; and a
//! report of what is there.

use crate::download::{self, Remote};
use crate::event::{Level, Reporter, Stage};
use crate::generate::{Settings, MFLUX_PROBE};
use crate::machine::{Arch, Backend, Machine, Os};
use crate::manifest::{self, MODELS};
use crate::{paths, unzip, CancelToken, Error};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Which stable-diffusion.cpp build to install.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Runtime {
    /// The build everything here was tested with ([`manifest::SDCPP_TAG`]).
    #[default]
    Pinned,
    /// The newest release, found through GitHub's API (`GITHUB_TOKEN` lifts its hourly limit).
    Latest,
}

/// Downloads and installs everything `settings` needs on `machine`: cwebp (for packs), then
/// mflux on Apple Silicon, or stable-diffusion.cpp and both models everywhere else. What is
/// already there and checked is left alone, and an interrupted download carries on. One setup
/// runs at a time on a computer, whichever program started it: another fails with "busy".
pub async fn setup(
    machine: &Machine,
    settings: &Settings,
    runtime: Runtime,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    // No build to install: say so before touching anything.
    if !can_set_up(machine, settings.backend) {
        return Err(no_build(machine.os, machine.arch, settings.backend));
    }
    let _only_one = lock(&paths::home())?;
    let client = download::client()?;
    if let Err(e) = install_webp(&client, machine, reporter, cancel).await {
        if e.is_cancelled() {
            return Err(e);
        }
        // Only a pack needs it; the models matter more.
        reporter.log(
            Level::Warn,
            format!("cwebp wasn't installed: {} {}", e.what, e.why),
        );
    }
    if settings.backend == Backend::Mlx {
        return install_mlx(reporter, cancel).await;
    }
    install_sdcpp(
        &client,
        machine,
        settings.backend,
        runtime,
        reporter,
        cancel,
    )
    .await?;
    for model in &MODELS {
        for file in model.files(settings.tier).all() {
            cancel.check()?;
            let remote = Remote {
                url: file.url(),
                size: file.size,
                sha256: Some(file.sha256.to_string()),
            };
            download::fetch(&client, &remote, &file.local(), reporter, cancel).await?;
        }
    }
    reporter.log(
        Level::Info,
        format!(
            "models ({}) are in {}",
            settings.tier,
            paths::models_dir().display()
        ),
    );
    Ok(())
}

/// Whether [`setup`] has something to install for `backend` on `machine`: mflux, which uv
/// installs, or a stable-diffusion.cpp build published for this computer.
pub fn can_set_up(machine: &Machine, backend: Backend) -> bool {
    backend == Backend::Mlx || manifest::sdcpp_assets(machine.os, machine.arch, backend).is_some()
}

/// Holds `setup.lock` in `home` for as long as the file is kept, so two setups (the app's and
/// `folderskin ai setup` in a terminal, or two terminals) never write into the same download.
/// The system lets go of it when the process ends, however it ends.
fn lock(home: &Path) -> Result<std::fs::File, Error> {
    std::fs::create_dir_all(home).map_err(|e| Error::io("make the models' folder", home, &e))?;
    let path = home.join("setup.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| Error::io("open the setup's lock", &path, &e))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(Error::environment(
            "busy",
            "This computer is already being set up.",
            "Another setup, in FolderSkin or in a terminal, is downloading into the same folder.",
        )
        .fix("Wait for it to finish, then run the command again.")),
        Err(std::fs::TryLockError::Error(e)) => Err(Error::io("lock the setup", &path, &e)),
    }
}

/// Whether a setup holds `setup.lock` in `home` right now: the app's, or `folderskin ai setup`
/// in another terminal. It asks by trying the lock, and lets go of it at once if it gets it.
pub fn setting_up_in(home: &Path) -> bool {
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .open(home.join("setup.lock"))
    else {
        // No lock file: nothing has ever set this computer up, let alone now.
        return false;
    };
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

/// [`setting_up_in`] the models' folder.
pub fn setting_up() -> bool {
    setting_up_in(&paths::home())
}

/// One release asset to install.
struct Download {
    name: String,
    url: String,
    size: u64,
    sha256: Option<String>,
}

async fn install_sdcpp(
    client: &reqwest::Client,
    machine: &Machine,
    backend: Backend,
    runtime: Runtime,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    let exe = paths::sd_cli(backend);
    let Some(pinned) = manifest::sdcpp_assets(machine.os, machine.arch, backend) else {
        return Err(no_build(machine.os, machine.arch, backend));
    };
    let (tag, assets) = match runtime {
        Runtime::Pinned => (
            manifest::SDCPP_TAG.to_string(),
            pinned
                .iter()
                .map(|a| Download {
                    name: a.name.to_string(),
                    url: a.url(),
                    size: a.size,
                    sha256: Some(a.sha256.to_string()),
                })
                .collect::<Vec<_>>(),
        ),
        Runtime::Latest => latest_assets(client, machine.os, machine.arch, backend).await?,
    };
    let stamp = exe.with_file_name(".release");
    // The archives, once unpacked, are only disk taken (the CUDA runtime's alone is 563 MB): they
    // go once the build is in place and recorded, not before, so a setup stopped part-way
    // still has what it already downloaded.
    let zips: Vec<PathBuf> = assets
        .iter()
        .map(|a| paths::downloads_dir().join(&a.name))
        .collect();
    if exe.is_file() && std::fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == tag) {
        zips.iter().for_each(|zip| download::discard(zip));
        reporter.log(
            Level::Info,
            format!("stable-diffusion.cpp {tag} ({backend}) is installed"),
        );
        return Ok(());
    }
    let dir = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| Error::io("make the runtime's folder", &dir, &e))?;
    for asset in assets {
        let zip = paths::downloads_dir().join(&asset.name);
        let remote = Remote {
            url: asset.url,
            size: asset.size,
            sha256: asset.sha256,
        };
        download::fetch(client, &remote, &zip, reporter, cancel).await?;
        cancel.check()?;
        reporter.stage(Stage::Install, format!("Unpacking {}", asset.name));
        let (from, to) = (zip.clone(), dir.clone());
        tokio::task::spawn_blocking(move || unzip::extract(&from, &to, |n| Some(n.to_string())))
            .await
            .map_err(|e| Error::bug("Unpacking stopped unexpectedly.", e.to_string()))?
            .map_err(|e| unpack_error(&zip, &e))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    if !exe.is_file() {
        return Err(Error::environment(
            "runtime_missing",
            "stable-diffusion.cpp unpacked without its program.",
            format!("There is no {} after unpacking.", exe.display()),
        )
        .fix("Run setup again with the pinned build (leave out --runtime latest)."));
    }
    std::fs::write(&stamp, &tag)
        .map_err(|e| Error::io("record the installed build", &stamp, &e))?;
    zips.iter().for_each(|zip| download::discard(zip));
    reporter.log(
        Level::Info,
        format!(
            "installed stable-diffusion.cpp {tag} ({backend}) in {}",
            dir.display()
        ),
    );
    Ok(())
}

/// Why stable-diffusion.cpp can't be set up for `backend` on this computer, and what to do
/// instead: another backend when one has a build, building it, or a provider.
pub fn no_build(os: Os, arch: Arch, backend: Backend) -> Error {
    let others = manifest::sdcpp_backends(os, arch);
    let why = match (others.is_empty(), os) {
        (false, _) => format!(
            "For {} {} it publishes {} builds.",
            os.id(),
            arch.id(),
            others
                .iter()
                .map(|b| b.id())
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        (true, Os::Linux) => {
            "Its Linux builds are for x86_64 processors only, so the local models can't be set \
             up on this one."
                .to_string()
        }
        (true, Os::Macos) => "Its Mac build is for Apple Silicon only.".to_string(),
        (true, Os::Windows) => "It publishes no build for this computer.".to_string(),
    };
    let mut error = Error::environment(
        "no_build_for_platform",
        format!(
            "stable-diffusion.cpp publishes no {backend} build for {} {}.",
            os.id(),
            arch.id()
        ),
        why,
    );
    if let Some(other) = others.first() {
        error = error.fix(format!(
            "Use a backend that has one: folderskin ai setup --backend {other}"
        ));
    }
    error
        .fix(format!(
            "Or build it from source ({}/blob/master/docs/build.md) and put sd-cli in {}",
            manifest::SDCPP_REPO,
            paths::sd_cli(backend)
                .parent()
                .unwrap_or(Path::new("."))
                .display()
        ))
        .fix("Or paint with a provider and your own key instead: folderskin ai models")
}

fn unpack_error(zip: &Path, e: &std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::InvalidData {
        let _ = std::fs::remove_file(zip);
        let _ = std::fs::remove_file(zip.with_extension("zip.ok"));
        Error::fixable(
            "unpack_failed",
            "The runtime's archive couldn't be unpacked.",
            format!("{}: {e}. It was deleted.", zip.display()),
        )
        .fix("Run setup again to download it afresh.")
    } else {
        Error::io("unpack the runtime", zip, e)
    }
}

/// The newest release's tag and the assets of `backend` in it.
async fn latest_assets(
    client: &reqwest::Client,
    os: Os,
    arch: Arch,
    backend: Backend,
) -> Result<(String, Vec<Download>), Error> {
    let failed = |why: String| {
        Error::environment(
            "latest_unavailable",
            "Couldn't find the newest stable-diffusion.cpp release.",
            why,
        )
        .fix("Leave out --runtime latest to install the tested build.")
        .fix("If GitHub is limiting you, set GITHUB_TOKEN to a token of yours and try again.")
    };
    let mut request = client
        .get(manifest::SDCPP_LATEST_API)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|e| failed(format!("GitHub couldn't be reached: {e}.")))?;
    if !response.status().is_success() {
        return Err(failed(format!(
            "GitHub answered HTTP {}.",
            response.status().as_u16()
        )));
    }
    let release: serde_json::Value = response
        .json()
        .await
        .map_err(|e| failed(format!("GitHub's answer couldn't be read: {e}.")))?;
    let tag = release["tag_name"].as_str().unwrap_or("latest").to_string();
    let patterns = manifest::sdcpp_patterns(os, arch, backend).unwrap_or(&[]);
    let assets = release["assets"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for pattern in patterns {
        let found = assets
            .iter()
            .find(|a| a["name"].as_str().is_some_and(|n| pattern.matches(n)))
            .ok_or_else(|| {
                failed(format!(
                    "Release {tag} has no asset like {}*{}*{}.",
                    pattern.prefix, pattern.contains, pattern.suffix
                ))
            })?;
        out.push(Download {
            name: found["name"].as_str().unwrap_or_default().to_string(),
            url: found["browser_download_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            size: found["size"].as_u64().unwrap_or(0),
            sha256: found["digest"]
                .as_str()
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(str::to_string),
        });
    }
    Ok((tag, out))
}

async fn install_webp(
    client: &reqwest::Client,
    machine: &Machine,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    if paths::cwebp().is_some() {
        return Ok(());
    }
    if machine.os != Os::Windows {
        reporter.log(
            Level::Warn,
            "cwebp is missing: install it (`brew install webp`, or the `webp` package) before \
             making a pack",
        );
        return Ok(());
    }
    let zip = webp_zip();
    let remote = Remote {
        url: manifest::WEBP_WINDOWS_URL.to_string(),
        size: manifest::WEBP_WINDOWS_SIZE,
        sha256: Some(manifest::WEBP_WINDOWS_SHA256.to_string()),
    };
    download::fetch(client, &remote, &zip, reporter, cancel).await?;
    let dir = paths::webp_dir();
    let from = zip.clone();
    let to = dir.clone();
    tokio::task::spawn_blocking(move || {
        unzip::extract(&from, &to, |n| {
            (n.ends_with("/bin/cwebp.exe") || n.ends_with("/bin/webpmux.exe"))
                .then(|| n.rsplit('/').next().unwrap_or(n).to_string())
        })
    })
    .await
    .map_err(|e| Error::bug("Unpacking stopped unexpectedly.", e.to_string()))?
    .map_err(|e| unpack_error(&zip, &e))?;
    download::discard(&zip);
    reporter.log(Level::Info, format!("installed cwebp in {}", dir.display()));
    Ok(())
}

/// Where Google's cwebp build is downloaded to on Windows.
fn webp_zip() -> PathBuf {
    let url = manifest::WEBP_WINDOWS_URL;
    paths::downloads_dir().join(url.rsplit('/').next().unwrap_or("libwebp.zip"))
}

/// Whether `settings` can paint here now: the runtime is installed and both models are downloaded
/// (mflux fetches its own weights the first time it runs each model). It only looks at files, so
/// it is quick enough for a list the window shows; [`status`] also asks the runtime whether it
/// starts.
pub fn is_set_up(settings: &Settings) -> bool {
    if settings.backend == Backend::Mlx {
        return paths::find_tool(MFLUX_PROBE).is_some();
    }
    paths::sd_cli(settings.backend).is_file()
        && MODELS.iter().all(|m| {
            m.files(settings.tier)
                .all()
                .iter()
                .all(|f| f.local().is_file())
        })
}

/// The bytes [`setup`] would still download for `settings` on `machine`: the tested runtime
/// build when it isn't installed, cwebp on Windows, and every model file that isn't here yet (a
/// file both models use counted once), less whatever interrupted downloads already brought. 0 for
/// mflux, which downloads each model's weights itself when it first runs it.
pub fn download_size(machine: &Machine, settings: &Settings) -> u64 {
    if settings.backend == Backend::Mlx {
        return 0;
    }
    let mut total = 0;
    if machine.os == Os::Windows && paths::cwebp().is_none() {
        total += download::remaining(&webp_zip(), manifest::WEBP_WINDOWS_SIZE);
    }
    let exe = paths::sd_cli(settings.backend);
    let installed = exe.is_file()
        && std::fs::read_to_string(exe.with_file_name(".release"))
            .is_ok_and(|s| s.trim() == manifest::SDCPP_TAG);
    if !installed {
        for asset in
            manifest::sdcpp_assets(machine.os, machine.arch, settings.backend).unwrap_or(&[])
        {
            total += download::remaining(&paths::downloads_dir().join(asset.name), asset.size);
        }
    }
    let mut seen = std::collections::HashSet::new();
    for model in &MODELS {
        for file in model.files(settings.tier).all() {
            if seen.insert(file.local()) {
                total += download::remaining(&file.local(), file.size);
            }
        }
    }
    total
}

async fn install_mlx(reporter: &Reporter, cancel: &CancelToken) -> Result<(), Error> {
    if paths::find_tool(MFLUX_PROBE).is_some() {
        reporter.log(Level::Info, "mflux is installed");
        return Ok(());
    }
    let Some(uv) = paths::find_tool("uv") else {
        return Err(Error::environment(
            "uv_missing",
            "mflux can't be installed yet.",
            "It is installed with uv, and uv isn't on the PATH, in ~/.local/bin or in Homebrew's \
             folder.",
        )
        .fix("Install uv (https://docs.astral.sh/uv/), then run setup again."));
    };
    reporter.stage(
        Stage::Install,
        format!("Installing mflux {}", manifest::MFLUX_VERSION),
    );
    let mut cmd = std::process::Command::new(uv);
    cmd.args([
        "tool",
        "install",
        &format!("mflux=={}", manifest::MFLUX_VERSION),
    ]);
    let (reporter2, cancel2) = (reporter.clone(), cancel.clone());
    let finished =
        tokio::task::spawn_blocking(move || crate::run::run(cmd, 0, &reporter2, &cancel2))
            .await
            .map_err(|e| Error::bug("Installing mflux stopped unexpectedly.", e.to_string()))?
            .map_err(|e| {
                Error::environment(
                    "mflux_install_failed",
                    "uv couldn't be started.",
                    e.to_string(),
                )
            })?;
    cancel.check()?;
    match finished.status {
        Some(s) if s.success() => {
            reporter.log(
                Level::Info,
                "installed mflux; it downloads each model the first time it runs it",
            );
            Ok(())
        }
        _ => Err(Error::environment(
            "mflux_install_failed",
            "mflux couldn't be installed.",
            format!(
                "uv tool install failed. Its last output:\n{}",
                finished.tail.join("\n")
            ),
        )
        .fix(format!(
            "Run it yourself to see why: uv tool install mflux=={}",
            manifest::MFLUX_VERSION
        ))),
    }
}

/// What is installed, for `doctor`.
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub home: PathBuf,
    pub machine: Machine,
    pub settings: Settings,
    pub runtime: RuntimeStatus,
    /// Empty for mflux, which downloads its own weights.
    pub models: Vec<ModelStatus>,
    pub cwebp: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeStatus {
    /// "stable-diffusion.cpp" or "mflux".
    pub name: String,
    pub path: Option<PathBuf>,
    pub installed: bool,
    /// Whether `setup` can install it here: false when stable-diffusion.cpp publishes no build of
    /// this backend for this computer (ARM64 Linux, an Intel Mac), or mflux off Apple Silicon.
    pub available: bool,
    /// The installed build's tag.
    pub release: Option<String>,
    /// What stable-diffusion.cpp can run on, one "name\tdescription" line each.
    pub devices: Vec<String>,
    /// Why it can't run, when it is installed but won't start.
    pub problem: Option<String>,
    /// The problem's code: "vc_runtime_missing" when Windows can't find the Visual C++ runtime
    /// it needs, otherwise "runtime_failed_to_start".
    pub problem_code: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelStatus {
    pub id: manifest::ModelId,
    pub label: String,
    pub licence: String,
    pub files: Vec<FileStatus>,
    /// Everything it needs, including files it shares with the other model.
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileStatus {
    pub name: String,
    pub size: u64,
    pub present: bool,
    /// Checked against its published hash.
    pub checked: bool,
}

impl ModelStatus {
    pub fn ready(&self) -> bool {
        self.files.iter().all(|f| f.present)
    }
}

/// Looks at what is installed. Asks stable-diffusion.cpp which devices it sees, which takes a
/// second; call it off the UI thread.
pub fn status(machine: &Machine, settings: &Settings) -> Status {
    let runtime = if settings.backend == Backend::Mlx {
        let path = paths::find_tool(MFLUX_PROBE);
        RuntimeStatus {
            name: "mflux".into(),
            installed: path.is_some(),
            available: machine.os == Os::Macos && machine.arch == Arch::Arm64,
            path,
            release: None,
            devices: Vec::new(),
            problem: None,
            problem_code: None,
        }
    } else {
        let exe = paths::sd_cli(settings.backend);
        let installed = exe.is_file();
        let (devices, problem) = if installed {
            probed(&exe, probe_devices)
        } else {
            (Vec::new(), None)
        };
        let (problem_code, problem) = problem.unzip();
        RuntimeStatus {
            name: "stable-diffusion.cpp".into(),
            // One built from source and put in place counts too.
            available: installed
                || manifest::sdcpp_assets(machine.os, machine.arch, settings.backend).is_some(),
            release: std::fs::read_to_string(exe.with_file_name(".release"))
                .ok()
                .map(|s| s.trim().to_string()),
            path: Some(exe),
            installed,
            devices,
            problem,
            problem_code,
        }
    };
    let models = if settings.backend == Backend::Mlx {
        Vec::new()
    } else {
        MODELS
            .iter()
            .map(|m| {
                let files = m.files(settings.tier).all();
                ModelStatus {
                    id: m.id,
                    label: m.label.into(),
                    licence: m.licence.into(),
                    bytes: files.iter().map(|f| f.size).sum(),
                    files: files
                        .iter()
                        .map(|f| FileStatus {
                            name: f.name().into(),
                            size: f.size,
                            present: f.local().is_file(),
                            checked: download::is_done(&f.local(), f.size),
                        })
                        .collect(),
                }
            })
            .collect()
    };
    Status {
        home: paths::home(),
        machine: machine.clone(),
        settings: *settings,
        runtime,
        models,
        cwebp: paths::cwebp(),
    }
}

/// What [`probe_devices`] says of a runtime, and why it wouldn't run if it didn't.
type Probe = (Vec<String>, Option<(&'static str, String)>);

/// A runtime that started: where it is, which build is there (its size and when it was written),
/// and the devices it listed.
type Started = (PathBuf, u64, Option<std::time::SystemTime>, Vec<String>);

/// Every runtime that has started this session.
static STARTED: std::sync::Mutex<Vec<Started>> = std::sync::Mutex::new(Vec::new());

/// [`probe_devices`], asked once a session for a runtime that starts. Starting it takes a second
/// or two (it loads the GPU's libraries), and the settings asked every time they opened, so
/// "Looking at this computer…" showed each time. A build put in place since is asked again, and
/// so is one that didn't start: whatever was wrong (a missing Visual C++ runtime) may have been
/// put right since.
fn probed(exe: &Path, probe: impl FnOnce(&Path) -> Probe) -> Probe {
    let build = std::fs::metadata(exe)
        .ok()
        .map(|m| (m.len(), m.modified().ok()));
    let mut started = STARTED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((len, when)) = build {
        let seen = started
            .iter()
            .find(|(path, l, w, _)| path == exe && *l == len && *w == when);
        if let Some((.., devices)) = seen {
            return (devices.clone(), None);
        }
    }
    let (devices, problem) = probe(exe);
    if let (Some((len, when)), None) = (build, &problem) {
        started.retain(|(path, ..)| path != exe);
        started.push((exe.to_path_buf(), len, when, devices.clone()));
    }
    (devices, problem)
}

/// `sd-cli --list-devices`, and why it wouldn't run if it didn't, with that problem's code.
fn probe_devices(exe: &Path) -> Probe {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--list-devices")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::run::hide_window(&mut cmd);
    match cmd.output() {
        Ok(out) if out.status.success() => (
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect(),
            None,
        ),
        Ok(out) if out.status.code() == Some(0xC000_0135_u32 as i32) => (
            Vec::new(),
            Some((
                "vc_runtime_missing",
                "it needs the Microsoft Visual C++ runtime: install \
                 https://aka.ms/vs/17/release/vc_redist.x64.exe"
                    .into(),
            )),
        ),
        Ok(out) => (
            Vec::new(),
            Some((
                "runtime_failed_to_start",
                format!("it stopped with exit code {:?}", out.status.code()),
            )),
        ),
        Err(e) => (
            Vec::new(),
            Some((
                "runtime_failed_to_start",
                format!("it couldn't be started: {e}"),
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{Gpu, Tier};

    #[test]
    fn a_computer_without_a_build_hears_what_it_can_do_instead() {
        let e = no_build(Os::Linux, Arch::Arm64, Backend::Vulkan);
        assert_eq!(
            (e.code, e.class),
            ("no_build_for_platform", crate::Class::Environment)
        );
        assert!(e.what.contains("vulkan build for linux arm64"), "{e:?}");
        assert!(e.why.contains("x86_64 processors only"), "{e:?}");
        assert!(
            !e.fix.iter().any(|f| f.contains("--backend")),
            "there is no other backend to offer: {e:?}"
        );
        assert!(e.fix.iter().any(|f| f.contains("folderskin ai models")));

        let e = no_build(Os::Linux, Arch::X86_64, Backend::Cuda);
        assert!(e.why.contains("vulkan and cpu builds"), "{e:?}");
        assert!(e.fix[0].ends_with("--backend vulkan"), "{e:?}");
    }

    #[test]
    fn what_is_left_to_download_counts_a_shared_file_once() {
        let machine = |os, arch, gpu| Machine {
            os,
            arch,
            ram_gb: 16.0,
            gpu,
            gpu_name: String::new(),
            vram_gb: 0.0,
        };
        // mflux downloads its own weights.
        let mac = machine(Os::Macos, Arch::Arm64, Gpu::Apple);
        assert_eq!(download_size(&mac, &Settings::for_machine(&mac)), 0);
        // No runtime build and no cwebp for ARM64 Linux, so at most the models, their shared
        // text encoder once: less than the two models' totals added up.
        let arm = machine(Os::Linux, Arch::Arm64, Gpu::None);
        let settings = Settings::for_machine(&arm);
        let each: u64 = MODELS
            .iter()
            .flat_map(|m| m.files(settings.tier).all())
            .map(|f| f.size)
            .sum();
        let shared = MODELS[0].files(settings.tier).llm.size;
        assert!(download_size(&arm, &settings) <= each - shared);
    }

    #[test]
    fn a_runtime_that_starts_is_asked_once_until_another_build_is_put_in() {
        let dir = std::env::temp_dir().join(format!("fs-probed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("sd-cli.exe");
        std::fs::write(&exe, b"build one").unwrap();
        let asked = std::cell::Cell::new(0);
        let starts = |_: &Path| {
            asked.set(asked.get() + 1);
            (vec!["CUDA0	RTX".to_string()], None)
        };
        assert_eq!(probed(&exe, starts).0, ["CUDA0	RTX"]);
        assert_eq!(probed(&exe, starts).0, ["CUDA0	RTX"], "from the first time");
        assert_eq!(asked.get(), 1);
        std::fs::write(&exe, b"build two, longer").unwrap();
        probed(&exe, starts);
        assert_eq!(asked.get(), 2, "another build is asked again");

        // One that won't start is asked every time: it may have been put right.
        let broken = dir.join("broken.exe");
        std::fs::write(&broken, b"x").unwrap();
        let fails = |_: &Path| {
            asked.set(asked.get() + 1);
            (
                Vec::new(),
                Some(("vc_runtime_missing", "no VC++".to_string())),
            )
        };
        assert!(probed(&broken, fails).1.is_some());
        assert!(probed(&broken, fails).1.is_some());
        assert_eq!(asked.get(), 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_second_setup_is_turned_away_while_the_first_holds_the_lock() {
        let home = std::env::temp_dir().join(format!("fs-setup-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        assert!(!setting_up_in(&home), "never set up");
        let first = lock(&home).expect("nothing else is setting up");
        assert!(setting_up_in(&home), "doctor can tell");
        // A second open of the file, as another process's would be.
        let err = lock(&home).unwrap_err();
        assert_eq!(err.code, "busy", "{err:?}");
        assert!(err.fix[0].contains("run the command again"), "{err:?}");
        drop(first);
        assert!(!setting_up_in(&home), "finished");
        let again = lock(&home).expect("the first let go, and so did the question");
        drop(again);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn only_a_computer_with_a_build_or_mflux_can_be_set_up() {
        let machine = |os, arch| Machine {
            os,
            arch,
            ram_gb: 16.0,
            gpu: Gpu::Other,
            gpu_name: String::new(),
            vram_gb: 0.0,
        };
        assert!(can_set_up(
            &machine(Os::Windows, Arch::X86_64),
            Backend::Vulkan
        ));
        assert!(can_set_up(&machine(Os::Macos, Arch::Arm64), Backend::Mlx));
        assert!(!can_set_up(&machine(Os::Macos, Arch::X86_64), Backend::Cpu));
        assert!(!can_set_up(
            &machine(Os::Linux, Arch::Arm64),
            Backend::Vulkan
        ));
    }

    #[test]
    fn setup_on_arm64_linux_stops_before_downloading_a_build_that_cant_run() {
        // pick_backend gives an ARM64 Linux computer with a GPU Vulkan, and without one the CPU;
        // neither has an ARM64 build, so nothing is downloaded or installed.
        for gpu in [Gpu::Other, Gpu::None] {
            let machine = Machine {
                os: Os::Linux,
                arch: Arch::Arm64,
                ram_gb: 16.0,
                gpu,
                gpu_name: String::new(),
                vram_gb: 0.0,
            };
            let settings = Settings::for_machine(&machine);
            assert_eq!(settings.tier, Tier::Q4);
            let err = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(setup(
                    &machine,
                    &settings,
                    Runtime::Pinned,
                    &Reporter::silent(),
                    &CancelToken::new(),
                ))
                .unwrap_err();
            assert_eq!(err.code, "no_build_for_platform", "{gpu:?}: {err:?}");
        }
    }
}
