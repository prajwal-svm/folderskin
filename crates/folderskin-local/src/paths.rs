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

/// Where setup put Google's cwebp on Windows, before packs were made without it.
pub fn webp_dir() -> PathBuf {
    home().join("bin").join("webp")
}

/// Where setup installs mflux on a Mac: the uv it installs with, the Python mflux runs on, mflux
/// with its packages and its programs (in `bin`), all in one folder among the runtimes, so none
/// of it touches the person's own uv or Python, and removing the model takes it all away.
pub fn mflux_dir() -> PathBuf {
    home().join("bin").join(Backend::Mlx.id())
}

/// An mflux program: the one setup installed, or else one on the PATH or where uv puts its tools
/// (mflux installed by hand, or by an earlier FolderSkin with the person's own uv).
pub fn mflux(program: &str) -> Option<PathBuf> {
    ours_first(&mflux_dir().join("bin"), program)
}

/// `program` in `ours`, or else [`find_tool`]'s.
fn ours_first(ours: &Path, program: &str) -> Option<PathBuf> {
    let here = ours.join(program);
    here.is_file()
        .then_some(here)
        .or_else(|| find_tool(program))
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

/// `program` on the PATH, or where uv, Homebrew and cargo put programs. An app opened from the
/// Finder or the Dock has only launchd's PATH (/usr/bin:/bin:/usr/sbin:/sbin), which has none of
/// them: uv itself lives in ~/.local/bin or /opt/homebrew/bin, and `uv tool install` puts mflux
/// in ~/.local/bin. Windows gives every program the PATH the user set, so there it is [`which`].
pub fn find_tool(program: &str) -> Option<PathBuf> {
    which(program).or_else(|| find_in(program, &tool_dirs()))
}

/// The folders [`find_tool`] looks in after the PATH, uv's own for its tools first.
fn tool_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    let mut dirs: Vec<PathBuf> = ["UV_TOOL_BIN_DIR", "XDG_BIN_HOME"]
        .into_iter()
        .filter_map(env_path)
        .collect();
    dirs.extend(env_path("XDG_DATA_HOME").map(|data| data.join("..").join("bin")));
    if let Some(user) = dirs::home_dir() {
        dirs.push(user.join(".local").join("bin"));
        dirs.push(user.join(".cargo").join("bin"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

/// `program` in the first of `dirs` that has it.
fn find_in(program: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(program))
        .find(|p| p.is_file())
}

/// The bytes free for this user on the disk `path` is on (the nearest folder of it that exists,
/// since the engine's home may not be made yet), when the system says.
pub fn free_space(path: &Path) -> Option<u64> {
    let mut at = path;
    while !at.exists() {
        at = at.parent()?;
    }
    free_space_at(at)
}

#[cfg(unix)]
fn free_space_at(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs fills the zeroed struct for a NUL-terminated path, and only then is it read.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    // Blocks available to an unprivileged user, in fragments: what `df` calls Available.
    #[allow(clippy::unnecessary_cast)] // the field types differ between systems
    Some(stats.f_bavail as u64 * stats.f_frsize as u64)
}

#[cfg(windows)]
fn free_space_at(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0u64;
    // SAFETY: `wide` is NUL-terminated; the two totals it doesn't need are asked for as null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
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

/// cwebp, from the PATH (or Homebrew's folder) or where setup put it: `packs catalog` makes
/// smaller thumbnails with it. Nothing else needs it.
pub fn cwebp() -> Option<PathBuf> {
    find_tool("cwebp").or_else(|| {
        let ours = webp_dir().join(if cfg!(windows) { "cwebp.exe" } else { "cwebp" });
        ours.is_file().then_some(ours)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_space_is_read_from_the_nearest_folder_there_is() {
        let here = free_space(&std::env::temp_dir()).expect("the temp folder's disk says");
        assert!(here > 0);
        // A home not made yet is on the same disk as the folder it will be made in.
        let later = std::env::temp_dir()
            .join("fs-free-space-not-made")
            .join("models");
        assert!(!later.exists());
        let there = free_space(&later).unwrap();
        // The disk is in use meanwhile: the same within a few hundred MB.
        assert!(here.abs_diff(there) < 500_000_000, "{here} {there}");
    }

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
        assert_eq!(find_tool("folderskin-no-such-program-anywhere"), None);
    }

    #[test]
    fn a_tool_is_found_where_uv_and_homebrew_put_it_without_the_path() {
        let dir = std::env::temp_dir().join(format!("fs-tools-{}", std::process::id()));
        let (empty, bin) = (dir.join("empty"), dir.join("bin"));
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let tool = bin.join("mflux-generate-flux2");
        std::fs::write(&tool, b"#!/bin/sh\n").unwrap();
        let found = find_in("mflux-generate-flux2", &[empty.clone(), bin.clone()]);
        assert_eq!(found.as_deref(), Some(tool.as_path()));
        assert_eq!(find_in("uv", &[empty, bin]), None);
        std::fs::remove_dir_all(&dir).unwrap();
        if cfg!(windows) {
            assert!(
                tool_dirs().is_empty(),
                "Windows programs get the user's PATH"
            );
        } else {
            let user = dirs::home_dir().unwrap();
            let dirs = tool_dirs();
            assert!(dirs.contains(&user.join(".local").join("bin")), "{dirs:?}");
            assert!(
                dirs.contains(&PathBuf::from("/opt/homebrew/bin")),
                "{dirs:?}"
            );
        }
    }

    #[test]
    fn the_mflux_setup_installed_comes_before_any_other() {
        let dir = std::env::temp_dir().join(format!("fs-mflux-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = "folderskin-no-such-mflux-program";
        assert_eq!(ours_first(&dir, program), None, "nowhere at all");
        std::fs::write(dir.join(program), b"#!/bin/sh\n").unwrap();
        assert_eq!(ours_first(&dir, program), Some(dir.join(program)));
        std::fs::remove_dir_all(&dir).unwrap();
        // Among the runtimes, so removing the model takes it away with them.
        assert_eq!(mflux_dir(), home().join("bin").join("mlx"));
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
