//! Where the runtimes and the models live: gigabytes that belong in a cache, never in a
//! repository or in the app's own folders. The same folder the local-generation skill has always
//! used, so what it downloaded is used as it is.

use crate::machine::Backend;
use std::path::PathBuf;

/// `FOLDERSKIN_LOCALGEN_HOME` if set, otherwise `%LOCALAPPDATA%\folderskin-localgen` on Windows,
/// `~/Library/Caches/folderskin-localgen` on macOS and `$XDG_CACHE_HOME/folderskin-localgen`
/// (`~/.cache/…`) elsewhere.
pub fn home() -> PathBuf {
    if let Some(dir) = env_path("FOLDERSKIN_LOCALGEN_HOME") {
        return dir;
    }
    let user = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base = if cfg!(windows) {
        env_path("LOCALAPPDATA").unwrap_or_else(|| user.join("AppData").join("Local"))
    } else if cfg!(target_os = "macos") {
        user.join("Library").join("Caches")
    } else {
        env_path("XDG_CACHE_HOME").unwrap_or_else(|| user.join(".cache"))
    };
    base.join("folderskin-localgen")
}

/// An environment variable as a path, when it is set to something.
fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// stable-diffusion.cpp's command-line program for `backend`.
pub fn sd_cli(backend: Backend) -> PathBuf {
    home()
        .join("bin")
        .join(backend.id())
        .join(if cfg!(windows) {
            "sd-cli.exe"
        } else {
            "sd-cli"
        })
}

pub fn models_dir() -> PathBuf {
    home().join("models")
}

pub fn downloads_dir() -> PathBuf {
    home().join("downloads")
}

/// Where setup puts Google's cwebp on Windows, for `packs make`.
pub fn webp_dir() -> PathBuf {
    home().join("bin").join("webp")
}

/// `program` on the PATH (with Windows' executable extensions), if it is there.
pub fn which(program: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let names: Vec<String> = if cfg!(windows) {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        std::iter::once(program.to_string())
            .chain(
                exts.split(';')
                    .map(|e| format!("{program}{}", e.to_lowercase())),
            )
            .collect()
    } else {
        vec![program.to_string()]
    };
    std::env::split_paths(&paths)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

/// cwebp, from the PATH or where setup put it.
pub fn cwebp() -> Option<PathBuf> {
    which("cwebp").or_else(|| {
        let ours = webp_dir().join(if cfg!(windows) { "cwebp.exe" } else { "cwebp" });
        ours.is_file().then_some(ours)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runtime_sits_under_its_backend() {
        let exe = sd_cli(Backend::Vulkan);
        assert!(
            exe.starts_with(home().join("bin").join("vulkan")),
            "{exe:?}"
        );
        assert!(models_dir().starts_with(home()));
    }

    #[test]
    fn which_finds_nothing_that_isnt_there() {
        assert_eq!(which("folderskin-no-such-program-anywhere"), None);
    }
}
