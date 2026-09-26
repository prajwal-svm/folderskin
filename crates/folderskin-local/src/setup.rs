//! Getting a computer ready: the runtime and the models, downloaded and checked; and a report of
//! what is there.

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

/// Downloads and installs everything `settings` needs on `machine`: the runtime (mflux on Apple
/// Silicon, with the uv and the Python it needs; stable-diffusion.cpp everywhere else), and the
/// model's weights. Nothing has to be installed first. What is already
/// there and checked is left alone, and an interrupted download carries on. One setup runs at a
/// time on a computer, whichever program started it: another fails with "busy".
pub async fn setup(
    machine: &Machine,
    settings: &Settings,
    runtime: Runtime,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    // Nothing to install, or nothing that would run here: say so before touching anything.
    if let Some(why) = cannot_set_up(machine, settings.backend) {
        return Err(why);
    }
    let _only_one = lock(&paths::home())?;
    check_space(machine, settings)?;
    let client = download::client()?;
    if settings.backend == Backend::Mlx {
        // mflux first: it takes a minute, and whatever stops it is heard before the long
        // download of the weights.
        install_mflux(&client, &paths::mflux_dir(), reporter, cancel).await?;
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

/// Setting up wants half again as much free space as it will put on the disk: for what the
/// system needs meanwhile, and so a nearly full disk is never filled to the last byte.
pub const SPACE_MARGIN: f64 = 1.5;

/// What installing mflux puts on the disk: uv (37 MB), Python 3.13 (65 MB), and mflux 0.20.0
/// with its packages (1.1 GB), measured on macOS 26; a little over, for the next.
pub const MFLUX_INSTALL_BYTES: u64 = 1_300_000_000;

/// The bytes [`setup`] would still put on the disk: [`download_size`], and mflux when it has to
/// be installed.
pub fn space_needed(machine: &Machine, settings: &Settings) -> u64 {
    let mflux = settings.backend == Backend::Mlx && paths::mflux(MFLUX_PROBE).is_none();
    download_size(machine, settings) + if mflux { MFLUX_INSTALL_BYTES } else { 0 }
}

/// The free space setting up wants for `needed` bytes ([`SPACE_MARGIN`]).
pub fn space_wanted(needed: u64) -> u64 {
    (needed as f64 * SPACE_MARGIN).ceil() as u64
}

/// Refuses to start a setup that would leave the disk all but full.
fn check_space(machine: &Machine, settings: &Settings) -> Result<(), Error> {
    let needed = space_needed(machine, settings);
    let Some(free) = paths::free_space(&paths::home()) else {
        return Ok(()); // the system doesn't say: the downloads will say if it fills up
    };
    let wanted = space_wanted(needed);
    if needed == 0 || free >= wanted {
        return Ok(());
    }
    let gb = |b: u64| format!("{:.1} GB", b as f64 / 1e9);
    Err(Error::environment(
        "low_disk_space",
        "There isn't enough free space to set up the model.",
        format!(
            "It needs {}, and setting up wants {} free to be safe, but this disk has {} free.",
            gb(needed),
            gb(wanted),
            gb(free)
        ),
    )
    .fix(format!(
        "Clear some space (about {} more), then set it up again.",
        gb(wanted - free)
    )))
}

/// Whether [`setup`] has something to install for `backend` on `machine` that runs there: mflux
/// on an Apple Silicon Mac new enough for it, or a stable-diffusion.cpp build published for this
/// computer.
pub fn can_set_up(machine: &Machine, backend: Backend) -> bool {
    cannot_set_up(machine, backend).is_none()
}

/// The oldest macOS mflux runs on: MLX and PyTorch publish their Mac packages for macOS 14 and
/// later only.
pub const MFLUX_OLDEST_MACOS: (u32, u32) = (14, 0);

/// Why [`setup`] can't set `backend` up on `machine`, when it can't: no stable-diffusion.cpp build
/// for this computer, or mflux on a computer that isn't an Apple Silicon Mac, or on one whose
/// macOS is older than [`MFLUX_OLDEST_MACOS`].
pub fn cannot_set_up(machine: &Machine, backend: Backend) -> Option<Error> {
    if backend != Backend::Mlx {
        return manifest::sdcpp_assets(machine.os, machine.arch, backend)
            .is_none()
            .then(|| no_build(machine.os, machine.arch, backend));
    }
    if machine.os != Os::Macos || machine.arch != Arch::Arm64 {
        let mut error = Error::environment(
            "no_build_for_platform",
            "mflux runs on Apple Silicon Macs only.",
            format!(
                "It paints with MLX, which is for Apple's own chips, and this is {} {}.",
                machine.os.id(),
                machine.arch.id()
            ),
        );
        if let Some(other) = manifest::sdcpp_backends(machine.os, machine.arch).first() {
            error = error.fix(format!(
                "Use a backend that has a build here: folderskin ai setup --backend {other}"
            ));
        }
        return Some(
            error.fix("Or paint with a provider and your own key instead: folderskin ai models"),
        );
    }
    // The Mac this runs on, which is the one `machine` describes.
    crate::machine::macos_version()
        .filter(|version| *version < MFLUX_OLDEST_MACOS)
        .map(macos_too_old)
}

/// Why mflux can't be installed on a Mac with macOS `major.minor`.
pub fn macos_too_old((major, minor): (u32, u32)) -> Error {
    let (oldest, _) = MFLUX_OLDEST_MACOS;
    Error::environment(
        "macos_too_old",
        format!("The local model needs macOS {oldest} or later, and this Mac has macOS {major}.{minor}."),
        format!(
            "MLX and PyTorch, which it paints with, publish their Mac packages for macOS {oldest} \
             and later only."
        ),
    )
    .fix("Update macOS (System Settings > General > Software Update), then run setup again.")
    .fix("Or paint with a provider and your own key instead: folderskin ai models")
}

/// Holds `setup.lock` in `home` for as long as the file is kept, so two setups (the app's and
/// `folderskin ai setup` in a terminal, or two terminals) never write into the same download.
/// The system lets go of it when the process ends, however it ends.
pub(crate) fn lock(home: &Path) -> Result<std::fs::File, Error> {
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
            label: None,
        };
        download::fetch(client, &remote, &zip, reporter, cancel).await?;
        cancel.check()?;
        reporter.stage(Stage::Install, format!("Unpacking {}", asset.name));
        let (from, to) = (zip.clone(), dir.clone());
        tokio::task::spawn_blocking(move || unzip::extract(&from, &to, |n| Some(n.to_string())))
            .await
            .map_err(|e| Error::bug("Unpacking stopped unexpectedly.", e.to_string()))?
            .map_err(|e| unpack_error(&zip, "The runtime", &e))?;
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

/// Why `archive` (the runtime's, or uv's) couldn't be unpacked. A damaged one is deleted, with
/// its check mark, so the next setup downloads it afresh.
fn unpack_error(archive: &Path, name: &str, e: &std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::InvalidData {
        download::discard(archive);
        Error::fixable(
            "unpack_failed",
            format!("{name}'s archive couldn't be unpacked."),
            format!("{}: {e}. It was deleted.", archive.display()),
        )
        .fix("Run setup again to download it afresh.")
    } else {
        Error::io(&format!("unpack {name}"), archive, e)
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

/// Whether `settings` can paint here now: the runtime is installed and the model's files are all
/// here. It only looks at files, so it is quick enough for a list the window shows; [`status`]
/// also asks the runtime whether it starts.
pub fn is_set_up(settings: &Settings) -> bool {
    let runtime = if settings.backend == Backend::Mlx {
        paths::mflux(MFLUX_PROBE).is_some()
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
/// tested build when it isn't installed, and every model file that isn't here
/// yet (one two models share counted once), less whatever interrupted downloads already brought;
/// and on a Mac, uv when mflux has to be installed. mflux itself isn't counted: Python and a few
/// hundred MB of packages, which uv fetches.
pub fn download_size(machine: &Machine, settings: &Settings) -> u64 {
    let mut total = 0;
    if settings.backend == Backend::Mlx
        && paths::mflux(MFLUX_PROBE).is_none()
        && !uv_installed(&paths::mflux_dir())
    {
        total += download::remaining(&uv_archive(), manifest::UV_SIZE);
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

/// What setup installs as mflux, written in its folder once it is: another FolderSkin that pins
/// another mflux, Python or set of packages installs it again.
fn mflux_release() -> String {
    format!(
        "mflux {} on Python {}, packages as of {}",
        manifest::MFLUX_VERSION,
        manifest::MFLUX_PYTHON,
        manifest::MFLUX_PACKAGES_AS_OF
    )
}

/// The mflux that is already here and will do, if there is one: the one setup installed in `dir`
/// when it is `release`, or else, when setup never installed one there, one of the person's own
/// (`theirs`: installed by hand, or by an earlier FolderSkin with their own uv).
fn mflux_here(
    dir: &Path,
    release: &str,
    theirs: impl FnOnce() -> Option<PathBuf>,
) -> Option<PathBuf> {
    let ours = dir.join("bin").join(MFLUX_PROBE);
    match std::fs::read_to_string(dir.join(".release")) {
        Ok(stamp) => (stamp.trim() == release && ours.is_file()).then_some(ours),
        Err(_) => theirs(),
    }
}

/// Installs mflux in `dir` ([`paths::mflux_dir`]) with a uv of its own, and the Python mflux runs
/// on with it, unless an mflux that will do is here already. Nothing has to be on the Mac first:
/// one out of the box has neither uv nor a Python mflux can use, and whatever uv or Python the
/// person has is left alone.
async fn install_mflux(
    client: &reqwest::Client,
    dir: &Path,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    let release = mflux_release();
    if let Some(found) = mflux_here(dir, &release, || paths::find_tool(MFLUX_PROBE)) {
        reporter.log(
            Level::Info,
            format!("mflux is installed: {}", found.display()),
        );
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| Error::io("make mflux's folder", dir, &e))?;
    let uv = install_uv(client, dir, reporter, cancel).await?;
    // An install stopped part-way, or of another release, is started again: uv puts a tool in a
    // folder of its own. The Python and the packages already downloaded are kept.
    let stamp = dir.join(".release");
    let _ = std::fs::remove_file(&stamp);
    for leftover in [dir.join("tools"), dir.join("bin")] {
        match std::fs::remove_dir_all(&leftover) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::io(
                    "clear an earlier install of mflux",
                    &leftover,
                    &e,
                ))
            }
        }
    }
    let python = manifest::MFLUX_PYTHON;
    reporter.stage(Stage::Install, format!("Installing Python {python}"));
    run_uv(
        &uv,
        dir,
        &["python", "install", python, "--managed-python", "--no-bin"],
        reporter,
        cancel,
        |output| {
            Error::environment(
                "python_install_failed",
                "Python couldn't be installed for mflux.",
                format!("uv python install {python} failed. Its last output:\n{output}"),
            )
        },
    )
    .await?;
    let mflux = format!("mflux=={}", manifest::MFLUX_VERSION);
    reporter.stage(
        Stage::Install,
        format!("Installing mflux {}", manifest::MFLUX_VERSION),
    );
    run_uv(
        &uv,
        dir,
        &[
            "tool",
            "install",
            "--python",
            python,
            "--managed-python",
            // Every package as a published build: never compiled here, which would need Apple's
            // developer tools and would ask for them in a window of its own.
            "--no-build",
            "--exclude-newer",
            manifest::MFLUX_PACKAGES_AS_OF,
            &mflux,
        ],
        reporter,
        cancel,
        |output| {
            Error::environment(
                "mflux_install_failed",
                "mflux couldn't be installed.",
                format!("uv tool install {mflux} failed. Its last output:\n{output}"),
            )
        },
    )
    .await?;
    // The packages as downloaded, which the install has copied out: the space comes back.
    let _ = std::fs::remove_dir_all(dir.join("cache"));
    std::fs::write(&stamp, &release)
        .map_err(|e| Error::io("record mflux's install", &stamp, &e))?;
    reporter.log(
        Level::Info,
        format!("installed {release} in {}", dir.display()),
    );
    Ok(())
}

/// The uv mflux is installed with: the one in `dir` when it is the pinned build, or else that
/// build, downloaded, checked against its hash and unpacked there.
async fn install_uv(
    client: &reqwest::Client,
    dir: &Path,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<PathBuf, Error> {
    let uv = dir.join("uv");
    if uv_installed(dir) {
        return Ok(uv);
    }
    let archive = uv_archive();
    let remote = Remote {
        url: manifest::UV_URL.to_string(),
        size: manifest::UV_SIZE,
        sha256: Some(manifest::UV_SHA256.to_string()),
        label: Some(format!("uv {}", manifest::UV_VERSION)),
    };
    download::fetch(client, &remote, &archive, reporter, cancel).await?;
    cancel.check()?;
    reporter.stage(
        Stage::Install,
        format!("Unpacking uv {}", manifest::UV_VERSION),
    );
    let (from, to) = (archive.clone(), uv.clone());
    tokio::task::spawn_blocking(move || extract_uv(&from, &to))
        .await
        .map_err(|e| Error::bug("Unpacking stopped unexpectedly.", e.to_string()))?
        .map_err(|e| unpack_error(&archive, "uv", &e))?;
    let stamp = dir.join(".uv-release");
    std::fs::write(&stamp, manifest::UV_VERSION)
        .map_err(|e| Error::io("record the installed uv", &stamp, &e))?;
    download::discard(&archive);
    Ok(uv)
}

/// Whether the pinned uv is in `dir`, mflux's folder.
fn uv_installed(dir: &Path) -> bool {
    dir.join("uv").is_file()
        && std::fs::read_to_string(dir.join(".uv-release"))
            .is_ok_and(|s| s.trim() == manifest::UV_VERSION)
}

/// Where uv's release archive is downloaded to.
fn uv_archive() -> PathBuf {
    paths::downloads_dir().join(format!(
        "uv-{}-aarch64-apple-darwin.tar.gz",
        manifest::UV_VERSION
    ))
}

/// Takes the uv program out of Astral's release archive (`uv-aarch64-apple-darwin/uv`, beside
/// uvx, which isn't needed) and puts it at `to`, ready to run, by way of a file beside it, so a
/// program half written is never taken for a whole one.
fn extract_uv(archive: &Path, to: &Path) -> std::io::Result<()> {
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let is_uv = entry.header().entry_type().is_file()
            && entry.path()?.file_name() == Some(std::ffi::OsStr::new("uv"));
        if !is_uv {
            continue;
        }
        let part = to.with_file_name("uv.part");
        let mut file = std::fs::File::create(&part)?;
        std::io::copy(&mut entry, &mut file)?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))?;
        }
        return std::fs::rename(&part, to);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "there is no uv program in it",
    ))
}

/// `uv` with `args`, told to keep everything in `dir` (mflux's folder): the Python it installs,
/// the tool and its programs, and its downloads. The person's own uv folders, the Python on the
/// PATH and the settings of whatever project a terminal is in are left out of it.
fn uv_command(uv: &Path, dir: &Path, args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new(uv);
    cmd.args(args)
        .current_dir(dir)
        .env("UV_PYTHON_INSTALL_DIR", dir.join("python"))
        .env("UV_TOOL_DIR", dir.join("tools"))
        .env("UV_TOOL_BIN_DIR", dir.join("bin"))
        .env("UV_CACHE_DIR", dir.join("cache"));
    // What a terminal may have set that would undo the flags: a Python of its own, or none
    // downloaded.
    for name in [
        "UV_PYTHON",
        "UV_NO_MANAGED_PYTHON",
        "UV_PYTHON_PREFERENCE",
        "UV_PYTHON_DOWNLOADS",
    ] {
        cmd.env_remove(name);
    }
    cmd
}

/// Runs `uv` with `args` in `dir` ([`uv_command`]), its output reported as it comes; `failed`
/// says what went wrong from its last lines when it doesn't succeed.
async fn run_uv(
    uv: &Path,
    dir: &Path,
    args: &[&str],
    reporter: &Reporter,
    cancel: &CancelToken,
    failed: impl FnOnce(String) -> Error,
) -> Result<(), Error> {
    let cmd = uv_command(uv, dir, args);
    let (reporter2, cancel2) = (reporter.clone(), cancel.clone());
    let finished =
        tokio::task::spawn_blocking(move || crate::run::run(cmd, 0, &reporter2, &cancel2))
            .await
            .map_err(|e| Error::bug("Installing mflux stopped unexpectedly.", e.to_string()))?
            .map_err(|e| {
                // Downloaded afresh next time, in case it is the program that is at fault.
                let _ = std::fs::remove_file(dir.join(".uv-release"));
                Error::environment(
                    "mflux_install_failed",
                    "uv couldn't be started.",
                    format!("{}: {e}.", uv.display()),
                )
                .fix("Run setup again.")
            })?;
    cancel.check()?;
    match finished.status {
        Some(status) if status.success() => Ok(()),
        _ => Err(failed(finished.tail.join("\n"))
            .fix("Check that this Mac is online, then run setup again.")),
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
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeStatus {
    /// "stable-diffusion.cpp" or "mflux".
    pub name: String,
    pub path: Option<PathBuf>,
    pub installed: bool,
    /// Whether `setup` can install it here: false when stable-diffusion.cpp publishes no build of
    /// this backend for this computer (ARM64 Linux, an Intel Mac), or for mflux off Apple Silicon
    /// or on a macOS older than it needs ([`MFLUX_OLDEST_MACOS`]).
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
        let path = paths::mflux(MFLUX_PROBE);
        RuntimeStatus {
            name: "mflux".into(),
            installed: path.is_some(),
            available: path.is_some() || can_set_up(machine, Backend::Mlx),
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
        // A Mac downloads klein's MLX weights, and uv when mflux isn't installed: mflux's own
        // packages are uv's to fetch.
        let mac = machine(Os::Macos, Arch::Arm64, Gpu::Apple);
        let settings = Settings::for_machine(&mac);
        assert_eq!((settings.backend, settings.tier), (Backend::Mlx, Tier::Q4));
        let uv = if paths::mflux(MFLUX_PROBE).is_none() && !uv_installed(&paths::mflux_dir()) {
            download::remaining(&uv_archive(), manifest::UV_SIZE)
        } else {
            0
        };
        assert!(uv <= manifest::UV_SIZE);
        assert_eq!(download_size(&mac, &settings), left(&settings) + uv);
        assert!(left(&settings) <= 4_619_699_678);
        // No runtime build for ARM64 Linux, so at most the model's own files.
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
        // Other tests start programs meanwhile, and a program forked in the instant the lock was
        // held shares its file until it execs: give the lock a moment to be free everywhere.
        let started = std::time::Instant::now();
        while setting_up_in(&home) {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "the first never let go"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let again = loop {
            match lock(&home) {
                Ok(file) => break file,
                Err(e) if started.elapsed() < std::time::Duration::from_secs(2) => {
                    assert_eq!(e.code, "busy", "{e:?}");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => panic!("the first never let go: {e:?}"),
            }
        };
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
        // mflux is for Apple's chips: elsewhere it says so, and what has a build instead.
        let e = cannot_set_up(&machine(Os::Linux, Arch::X86_64), Backend::Mlx).unwrap();
        assert_eq!(e.code, "no_build_for_platform", "{e:?}");
        assert!(e.what.contains("Apple Silicon"), "{e:?}");
        assert!(e.fix[0].ends_with("--backend vulkan"), "{e:?}");
        assert!(!can_set_up(&machine(Os::Macos, Arch::X86_64), Backend::Mlx));
    }

    #[test]
    fn a_mac_too_old_for_mflux_hears_it_needs_a_newer_macos() {
        let e = macos_too_old((13, 6));
        assert_eq!(
            (e.code, e.class),
            ("macos_too_old", crate::Class::Environment)
        );
        assert!(e.what.contains("macOS 14 or later"), "{e:?}");
        assert!(e.what.contains("macOS 13.6"), "{e:?}");
        assert!(e.fix[0].contains("Software Update"), "{e:?}");
        // The Mac running this is new enough, or isn't a Mac.
        assert!(crate::machine::macos_version().is_none_or(|v| v >= MFLUX_OLDEST_MACOS));
    }

    #[test]
    fn mflux_is_installed_again_only_when_setups_own_is_missing_or_another_release() {
        let dir = std::env::temp_dir().join(format!("fs-mflux-here-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let theirs = || {
            Some(PathBuf::from(
                "/Users/someone/.local/bin/mflux-generate-flux2",
            ))
        };
        let nobodys = || None;
        let release = mflux_release();
        // Setup never installed one: the person's own will do, and without one it is installed.
        assert_eq!(mflux_here(&dir, &release, theirs), theirs());
        assert_eq!(mflux_here(&dir, &release, nobodys), None);
        // Setup's own, of this release, is used; the person's isn't even looked for.
        let ours = dir.join("bin").join(MFLUX_PROBE);
        std::fs::write(&ours, b"#!/bin/sh\n").unwrap();
        std::fs::write(dir.join(".release"), &release).unwrap();
        assert_eq!(
            mflux_here(&dir, &release, || panic!("not asked")),
            Some(ours.clone())
        );
        // Another release of setup's own is installed again, whoever else has one.
        std::fs::write(dir.join(".release"), "mflux 0.19.0 on Python 3.12").unwrap();
        assert_eq!(mflux_here(&dir, &release, theirs), None);
        // And so is one whose programs have gone.
        std::fs::write(dir.join(".release"), &release).unwrap();
        std::fs::remove_file(&ours).unwrap();
        assert_eq!(mflux_here(&dir, &release, theirs), None);
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(release.contains(manifest::MFLUX_VERSION), "{release}");
        assert!(release.contains(manifest::MFLUX_PYTHON), "{release}");
    }

    #[test]
    fn uv_is_told_to_keep_everything_in_mfluxs_folder() {
        let dir = Path::new("/somewhere/folderskin-localgen/bin/mlx");
        let cmd = uv_command(&dir.join("uv"), dir, &["python", "install", "3.13"]);
        assert_eq!(cmd.get_program(), dir.join("uv").as_os_str());
        assert_eq!(cmd.get_current_dir(), Some(dir), "no project's settings");
        let envs: std::collections::HashMap<_, _> = cmd.get_envs().collect();
        let set = |name: &str| envs.get(std::ffi::OsStr::new(name)).copied().flatten();
        for (name, folder) in [
            ("UV_PYTHON_INSTALL_DIR", "python"),
            ("UV_TOOL_DIR", "tools"),
            ("UV_TOOL_BIN_DIR", "bin"),
            ("UV_CACHE_DIR", "cache"),
        ] {
            assert_eq!(set(name), Some(dir.join(folder).as_os_str()), "{name}");
        }
        // Taken away, not set: a terminal's own choice of Python can't undo --managed-python.
        for name in ["UV_PYTHON", "UV_NO_MANAGED_PYTHON", "UV_PYTHON_DOWNLOADS"] {
            assert!(envs.contains_key(std::ffi::OsStr::new(name)), "{name}");
            assert_eq!(set(name), None, "{name}");
        }
        // mflux's programs are where painting looks first.
        assert_eq!(
            paths::mflux_dir().join("bin"),
            paths::home().join("bin").join("mlx").join("bin")
        );
    }

    /// A .tar.gz like Astral's: a folder, uvx, then uv.
    fn uv_release(dir: &Path, files: &[(&str, &[u8])]) -> PathBuf {
        let archive = dir.join("uv.tar.gz");
        let gz = flate2::write::GzEncoder::new(
            std::fs::File::create(&archive).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(gz);
        let mut folder = tar::Header::new_gnu();
        folder.set_entry_type(tar::EntryType::Directory);
        folder.set_size(0);
        folder.set_mode(0o755);
        tar.append_data(&mut folder, "uv-aarch64-apple-darwin/", std::io::empty())
            .unwrap();
        for (name, bytes) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            tar.append_data(&mut header, name, *bytes).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        archive
    }

    #[test]
    fn uv_is_taken_out_of_its_release_archive_ready_to_run() {
        let dir = std::env::temp_dir().join(format!("fs-uv-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let archive = uv_release(
            &dir,
            &[
                ("uv-aarch64-apple-darwin/uvx", b"uvx"),
                ("uv-aarch64-apple-darwin/uv", b"the uv program"),
            ],
        );
        let uv = dir.join("mlx").join("uv");
        std::fs::create_dir_all(uv.parent().unwrap()).unwrap();
        extract_uv(&archive, &uv).unwrap();
        assert_eq!(std::fs::read(&uv).unwrap(), b"the uv program");
        assert!(!uv.with_file_name("uv.part").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&uv).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }

        // One without uv in it is damaged: setup deletes it and downloads it again.
        let archive = uv_release(&dir, &[("uv-aarch64-apple-darwin/uvx", b"uvx")]);
        let e = extract_uv(&archive, &dir.join("elsewhere")).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData, "{e}");
        let e = unpack_error(&archive, "uv", &e);
        assert_eq!(e.code, "unpack_failed");
        assert_eq!(e.what, "uv's archive couldn't be unpacked.");
        assert!(!archive.exists(), "deleted");
        std::fs::remove_dir_all(&dir).unwrap();
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
