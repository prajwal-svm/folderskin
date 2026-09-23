//! Getting a computer ready: the runtime, the models and cwebp, downloaded and checked; and a
//! report of what is there.

use crate::download::{self, Remote};
use crate::event::{Level, Reporter, Stage};
use crate::generate::{Settings, MFLUX_PROBE};
use crate::machine::{Backend, Machine, Os};
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
/// already there and checked is left alone, and an interrupted download carries on.
pub async fn setup(
    machine: &Machine,
    settings: &Settings,
    runtime: Runtime,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
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
    let Some(pinned) = manifest::sdcpp_assets(machine.os, backend) else {
        return Err(Error::environment(
            "no_build_for_platform",
            format!(
                "stable-diffusion.cpp publishes no {backend} build for {} {}.",
                machine.os.id(),
                machine.arch.id()
            ),
            "There is nothing to download for this combination.",
        )
        .fix(format!(
            "Build it from source ({}/blob/master/docs/build.md) and put sd-cli in {}",
            manifest::SDCPP_REPO,
            exe.parent().unwrap_or(Path::new(".")).display()
        ))
        .fix("Or pick a backend that has a build, e.g. --backend vulkan or --backend cpu."));
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
        Runtime::Latest => latest_assets(client, machine.os, backend).await?,
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
    let patterns = manifest::sdcpp_patterns(os, backend).unwrap_or(&[]);
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
    let url = manifest::WEBP_WINDOWS_URL;
    let zip = paths::downloads_dir().join(url.rsplit('/').next().unwrap_or("libwebp.zip"));
    let remote = Remote {
        url: url.to_string(),
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
    reporter.log(Level::Info, format!("installed cwebp in {}", dir.display()));
    Ok(())
}

async fn install_mlx(reporter: &Reporter, cancel: &CancelToken) -> Result<(), Error> {
    if paths::which(MFLUX_PROBE).is_some() {
        reporter.log(Level::Info, "mflux is installed");
        return Ok(());
    }
    let Some(uv) = paths::which("uv") else {
        return Err(Error::environment(
            "uv_missing",
            "mflux can't be installed yet.",
            "It is installed with uv, and uv isn't on the PATH.",
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
    /// The installed build's tag.
    pub release: Option<String>,
    /// What stable-diffusion.cpp can run on, one "name\tdescription" line each.
    pub devices: Vec<String>,
    /// Why it can't run, when it is installed but won't start.
    pub problem: Option<String>,
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
        let path = paths::which(MFLUX_PROBE);
        RuntimeStatus {
            name: "mflux".into(),
            installed: path.is_some(),
            path,
            release: None,
            devices: Vec::new(),
            problem: None,
        }
    } else {
        let exe = paths::sd_cli(settings.backend);
        let installed = exe.is_file();
        let (devices, problem) = if installed {
            probe_devices(&exe)
        } else {
            (Vec::new(), None)
        };
        RuntimeStatus {
            name: "stable-diffusion.cpp".into(),
            release: std::fs::read_to_string(exe.with_file_name(".release"))
                .ok()
                .map(|s| s.trim().to_string()),
            path: Some(exe),
            installed,
            devices,
            problem,
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

/// `sd-cli --list-devices`, and why it wouldn't run if it didn't.
fn probe_devices(exe: &Path) -> (Vec<String>, Option<String>) {
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
            Some(
                "it needs the Microsoft Visual C++ runtime: install \
                 https://aka.ms/vs/17/release/vc_redist.x64.exe"
                    .into(),
            ),
        ),
        Ok(out) => (
            Vec::new(),
            Some(format!("it stopped with exit code {:?}", out.status.code())),
        ),
        Err(e) => (Vec::new(), Some(format!("it couldn't be started: {e}"))),
    }
}
