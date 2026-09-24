//! Where the runtimes and the models live: gigabytes that belong in a cache, never in a
//! repository or in the app's own folders. The same folder the local-generation skill has always
//! used, so what it downloaded is used as it is.

use crate::machine::Backend;
use std::path::{Path, PathBuf};

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

/// `path` in a form stable-diffusion.cpp can open, or `None` when there is none.
///
/// On Windows sd-cli opens files through the ANSI file API, so a path with any letter outside
/// ASCII (a Cyrillic user name, a folder called "Фото", an "é") is "not found", and a picture it
/// writes there lands under a garbled name. The 8.3 short name Windows keeps for a file or folder
/// (on the system drive, unless it was turned off) is plain ASCII and names the same thing.
/// Anywhere else, and for a plain path, this is `path` itself.
pub fn for_sdcpp(path: &Path) -> Option<PathBuf> {
    if !cfg!(windows) || is_ascii(path) {
        return Some(path.to_path_buf());
    }
    let full = std::path::absolute(path).ok()?;
    short_name(&full).filter(|short| is_ascii(short))
}

fn is_ascii(path: &Path) -> bool {
    path.to_str().is_some_and(|s| s.is_ascii())
}

/// The 8.3 short form of an existing file or folder.
#[cfg(windows)]
fn short_name(path: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated, and a null buffer of length 0 asks for the length needed.
    let needed = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buf = vec![0u16; needed as usize];
    // SAFETY: `buf` holds `needed` u16s, which is what the call is told.
    let got = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
    if got == 0 || got >= needed {
        return None;
    }
    buf.truncate(got as usize);
    Some(PathBuf::from(std::ffi::OsString::from_wide(&buf)))
}

#[cfg(not(windows))]
fn short_name(_: &Path) -> Option<PathBuf> {
    None
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

    #[test]
    fn stable_diffusion_cpp_gets_paths_it_can_open() {
        let plain = Path::new("C:/models/a.gguf");
        assert_eq!(for_sdcpp(plain).as_deref(), Some(plain));
        let dir = std::env::temp_dir().join(format!("fs-фото-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("снимок.png");
        std::fs::write(&file, b"x").unwrap();
        let usable = for_sdcpp(&file);
        if cfg!(windows) {
            // The system drive keeps short names unless they were turned off; either way nothing
            // that isn't ASCII is handed over.
            if let Some(short) = &usable {
                assert!(is_ascii(short), "{short:?}");
                assert_eq!(std::fs::read(short).unwrap(), b"x", "the same file");
            }
        } else {
            assert_eq!(usable.as_deref(), Some(file.as_path()));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
