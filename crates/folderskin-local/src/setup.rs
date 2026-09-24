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

/// Downloads and installs everything `settings` needs on `machine`: cwebp (for packs), the
/// runtime (mflux on Apple Silicon, stable-diffusion.cpp everywhere else), and the model's
/// weights. What is already there and checked is left alone, and an interrupted download carries
/// on. One setup runs at a time on a computer, whichever program started it: another fails with
/// "busy".
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
        // mflux first: it is quick, and a Mac without uv hears so before a long download.
        install_mlx(reporter, cancel).await?;
    } else {
        install_sdcpp(
            &client,
            machine,
            settings.backend,
            runtime,
            reporter,
            cancel,
        )
        .await?;
    }
    for model in &MODELS {
        for file in model.files_for(settings.backend, settings.tier) {
            cancel.check()?;
            let remote = Remote {
                url: file.url.clone(),
                size: file.size,
                sha256: Some(file.sha256.to_string()),
                label: Some(file.name.clone()),
            };
            download::fetch(&client, &remote, &file.local, reporter, cancel).await?;
        }
        let kept_in = if settings.backend == Backend::Mlx {
            model.mlx(settings.tier).dir()
        } else {
            paths::models_dir()
        };
        reporter.log(
            Level::Info,
            format!(
                "{} ({}) is in {}",
                model.label,
                settings.tier,
                kept_in.display()
            ),
        );
    }
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
    if exe.is_file() && std::fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == tag) {
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
            label: None,
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
        label: None,
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
    reporter.log(Level::Info, format!("installed cwebp in {}", dir.display()));
    Ok(())
}

/// Where Google's cwebp build is downloaded to on Windows.
fn webp_zip() -> PathBuf {
    let url = manifest::WEBP_WINDOWS_URL;
    paths::downloads_dir().join(url.rsplit('/').next().unwrap_or("libwebp.zip"))
}

/// Whether `settings` can paint here now: the runtime is installed and the model's files are all
/// here. It only looks at files, so it is quick enough for a list the window shows; [`status`]
/// also asks the runtime whether it starts.
pub fn is_set_up(settings: &Settings) -> bool {
    let runtime = if settings.backend == Backend::Mlx {
        paths::find_tool(MFLUX_PROBE).is_some()
    } else {
        paths::sd_cli(settings.backend).is_file()
    };
    runtime
        && MODELS.iter().all(|m| {
            m.files_for(settings.backend, settings.tier)
                .iter()
                .all(|f| f.local.is_file())
        })
}

/// The bytes [`setup`] would still download for `settings` on `machine`: stable-diffusion.cpp's
/// tested build when it isn't installed, cwebp on Windows, and every model file that isn't here
/// yet (one two models share counted once), less whatever interrupted downloads already brought.
/// mflux itself, a few hundred MB of Python packages that uv fetches, isn't counted.
pub fn download_size(machine: &Machine, settings: &Settings) -> u64 {
    let mut total = 0;
    if machine.os == Os::Windows && paths::cwebp().is_none() {
        total += download::remaining(&webp_zip(), manifest::WEBP_WINDOWS_SIZE);
    }
    if settings.backend != Backend::Mlx {
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
    }
    let mut seen = std::collections::HashSet::new();
    for model in &MODELS {
        for file in model.files_for(settings.backend, settings.tier) {
            if seen.insert(file.local.clone()) {
                total += download::remaining(&file.local, file.size);
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
                format!("installed mflux {}", manifest::MFLUX_VERSION),
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
    /// Everything it needs.
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
            probe_devices(&exe)
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
    let models = MODELS
        .iter()
        .map(|m| {
            let files = m.files_for(settings.backend, settings.tier);
            ModelStatus {
                id: m.id,
                label: m.label.into(),
                licence: m.licence.into(),
                bytes: files.iter().map(|f| f.size).sum(),
                files: files
                    .iter()
                    .map(|f| FileStatus {
                        name: f.name.clone(),
                        size: f.size,
                        present: f.local.is_file(),
                        checked: download::is_done(&f.local, f.size),
                    })
                    .collect(),
            }
        })
        .collect();
    Status {
        home: paths::home(),
        machine: machine.clone(),
        settings: *settings,
        runtime,
        models,
        cwebp: paths::cwebp(),
    }
}

/// `sd-cli --list-devices`, and why it wouldn't run if it didn't, with that problem's code.
fn probe_devices(exe: &Path) -> (Vec<String>, Option<(&'static str, String)>) {
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
    fn what_is_left_to_download_is_the_model_less_what_is_here() {
        let machine = |os, arch, gpu| Machine {
            os,
            arch,
            ram_gb: 16.0,
            gpu,
            gpu_name: String::new(),
            vram_gb: 0.0,
        };
        let left = |settings: &Settings| -> u64 {
            MODELS
                .iter()
                .flat_map(|m| m.files_for(settings.backend, settings.tier))
                .map(|f| download::remaining(&f.local, f.size))
                .sum()
        };
        // A Mac downloads klein's MLX weights and nothing else: mflux is uv's to fetch, and cwebp
        // comes from Homebrew or not at all.
        let mac = machine(Os::Macos, Arch::Arm64, Gpu::Apple);
        let settings = Settings::for_machine(&mac);
        assert_eq!((settings.backend, settings.tier), (Backend::Mlx, Tier::Q4));
        assert_eq!(download_size(&mac, &settings), left(&settings));
        assert!(left(&settings) <= 4_619_699_678);
        // No runtime build and no cwebp for ARM64 Linux, so at most the model's own files.
        let arm = machine(Os::Linux, Arch::Arm64, Gpu::None);
        let settings = Settings::for_machine(&arm);
        assert_eq!(download_size(&arm, &settings), left(&settings));
    }

    #[test]
    fn a_mac_reports_its_weights_file_by_file() {
        let mac = Machine {
            os: Os::Macos,
            arch: Arch::Arm64,
            ram_gb: 36.0,
            gpu: Gpu::Apple,
            gpu_name: "Apple M3 Pro".into(),
            vram_gb: 0.0,
        };
        let s = status(&mac, &Settings::for_machine(&mac));
        assert_eq!(s.runtime.name, "mflux");
        assert!(s.runtime.available);
        let [klein] = s.models.as_slice() else {
            panic!("one model: {:?}", s.models);
        };
        assert_eq!(klein.label, "FLUX.2 [klein] 4B");
        assert_eq!(klein.bytes, 4_619_699_678);
        assert_eq!(klein.files.len(), 11);
        assert!(klein
            .files
            .iter()
            .any(|f| f.name == "transformer/0.safetensors"));
        assert!(klein.files.iter().any(|f| f.name == "vae/0.safetensors"));
    }

    #[test]
    fn a_second_setup_is_turned_away_while_the_first_holds_the_lock() {
        let home = std::env::temp_dir().join(format!("fs-setup-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let first = lock(&home).expect("nothing else is setting up");
        // A second open of the file, as another process's would be.
        let err = lock(&home).unwrap_err();
        assert_eq!(err.code, "busy", "{err:?}");
        assert!(err.fix[0].contains("run the command again"), "{err:?}");
        drop(first);
        let again = lock(&home).expect("the first let go");
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
