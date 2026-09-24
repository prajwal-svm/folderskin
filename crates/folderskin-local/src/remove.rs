//! Taking back what setup put on this computer: the model's files, the runtimes it installed and
//! their downloads, so the space comes back and a later setup starts again from nothing.

use crate::event::{Level, Reporter};
use crate::{paths, Error};
use std::path::{Path, PathBuf};

/// Left in the engine's home when setup installed mflux itself, so removing takes away only an
/// mflux FolderSkin put there, never one someone installed for their own use.
pub(crate) const MFLUX_MARKER: &str = "mflux.installed";

/// The folders under the engine's home that are setup's alone: the models (every tier, partial
/// downloads included), the runtimes' zips, the runtimes, and the app's work folders. Only these
/// are removed, never the home itself, which `FOLDERSKIN_LOCALGEN_HOME` can point anywhere.
fn setup_dirs(home: &Path) -> [PathBuf; 4] {
    [
        home.join("models"),
        home.join("downloads"),
        home.join("bin"),
        home.join("app"),
    ]
}

/// How much disk what setup downloaded takes now, partial downloads included; mflux, which lives
/// with uv's tools, isn't counted.
pub fn kept_bytes() -> u64 {
    setup_dirs(&paths::home()).iter().map(|d| dir_size(d)).sum()
}

/// The bytes under `dir`, not following links; 0 when it isn't there.
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.path().symlink_metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Removes everything setup put here ([`setup_dirs`]), and mflux when setup installed it. It
/// waits for no one: a setup under way, here or in a terminal, holds the lock and it is refused
/// ("busy"). Returns the bytes given back.
pub fn remove(reporter: &Reporter) -> Result<u64, Error> {
    let home = paths::home();
    let _only_one = crate::setup::lock(&home)?;
    // A painting just stopped may still be letting go of the model's files, which Windows won't
    // delete while they're open.
    crate::run::wait_for_the_stopped(&crate::CancelToken::new());
    let mut freed = 0;
    for dir in setup_dirs(&home) {
        let size = dir_size(&dir);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {
                freed += size;
                reporter.log(Level::Info, format!("removed {}", dir.display()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io("remove", &dir, &e)),
        }
    }
    let marker = home.join(MFLUX_MARKER);
    if marker.is_file() {
        freed += uninstall_mflux(reporter);
        let _ = std::fs::remove_file(&marker);
    }
    Ok(freed)
}

/// `uv tool uninstall mflux`, and the bytes its tool folder held; 0 when uv isn't there or says no,
/// which is reported and otherwise left alone (the model's files are what took the space).
fn uninstall_mflux(reporter: &Reporter) -> u64 {
    let Some(uv) = paths::find_tool("uv") else {
        reporter.log(
            Level::Warn,
            "mflux wasn't uninstalled: uv isn't there any more",
        );
        return 0;
    };
    let quiet = |args: &[&str]| {
        let mut cmd = std::process::Command::new(&uv);
        cmd.args(args).stdin(std::process::Stdio::null());
        crate::run::hide_window(&mut cmd);
        cmd.output()
    };
    let size = quiet(&["tool", "dir"])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| dir_size(&PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()).join("mflux")))
        .unwrap_or(0);
    match quiet(&["tool", "uninstall", "mflux"]) {
        Ok(out) if out.status.success() => {
            reporter.log(Level::Info, "uninstalled mflux, which setup had installed");
            size
        }
        Ok(out) => {
            reporter.log(
                Level::Warn,
                format!(
                    "mflux wasn't uninstalled: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            );
            0
        }
        Err(e) => {
            reporter.log(Level::Warn, format!("mflux wasn't uninstalled: {e}"));
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_measured_file_by_file_and_a_missing_one_is_nothing() {
        let dir = std::env::temp_dir().join(format!("fs-remove-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mlx").join("vae")).unwrap();
        std::fs::write(
            dir.join("mlx").join("vae").join("0.safetensors"),
            [0u8; 700],
        )
        .unwrap();
        std::fs::write(dir.join("a.part"), [0u8; 300]).unwrap();
        assert_eq!(dir_size(&dir), 1000);
        assert_eq!(dir_size(&dir.join("nothing here")), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_setups_own_folders_are_removed() {
        let home = Path::new("/somewhere/folderskin-localgen");
        let dirs = setup_dirs(home);
        assert!(dirs.iter().all(|d| d.parent() == Some(home)), "{dirs:?}");
        assert!(!dirs.iter().any(|d| d == home), "never the home itself");
    }
}
