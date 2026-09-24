//! Taking back what setup put on this computer: the model's files, the runtimes it installed and
//! their downloads, so the space comes back and a later setup starts again from nothing.

use crate::event::{Level, Reporter};
use crate::generate::Settings;
use crate::machine::Backend;
use crate::manifest::MODELS;
use crate::{paths, Error};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
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

/// Model files in the models' folder that the model at `settings` doesn't use: the other tier's,
/// Z-Image Turbo's (which an earlier build set up beside klein), or another backend's (MLX
/// weights beside GGUF files). A file comes with its check mark and any part of it downloaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Unused {
    /// The file's name, or `mlx/<folder>` for an MLX model's folder.
    pub name: String,
    pub bytes: u64,
    /// What goes with it: the file and its `.ok` and `.part`, or the folder.
    #[serde(skip)]
    pub paths: Vec<PathBuf>,
}

/// [`Unused`] model files, biggest first.
pub fn unused(settings: &Settings) -> Vec<Unused> {
    unused_in(&paths::models_dir(), settings)
}

/// The bytes [`unused`] files take.
pub fn unused_bytes(settings: &Settings) -> u64 {
    unused(settings).iter().map(|u| u.bytes).sum()
}

/// [`unused`] in `dir`, the models' folder.
fn unused_in(dir: &Path, settings: &Settings) -> Vec<Unused> {
    // What the model runs with, by where it is kept in the models' folder.
    let models = paths::models_dir();
    let inside = |p: PathBuf| p.strip_prefix(&models).map(Path::to_path_buf).unwrap_or(p);
    let used: HashSet<PathBuf> = MODELS
        .iter()
        .flat_map(|m| m.files_for(settings.backend, settings.tier))
        .map(|f| inside(f.local))
        .collect();
    let used_mlx: HashSet<PathBuf> = if settings.backend == Backend::Mlx {
        MODELS
            .iter()
            .map(|m| inside(m.mlx(settings.tier).dir()))
            .collect()
    } else {
        HashSet::new()
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: BTreeMap<String, Unused> = BTreeMap::new();
    let mut add = |name: String, path: PathBuf, bytes: u64| {
        let entry = found.entry(name.clone()).or_insert_with(|| Unused {
            name,
            bytes: 0,
            paths: Vec::new(),
        });
        entry.bytes += bytes;
        entry.paths.push(path);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = path.symlink_metadata() else {
            continue;
        };
        if meta.is_dir() {
            if name != "mlx" {
                continue;
            }
            // Each MLX model is a folder of its own, one per tier.
            for model in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                let folder = model.path();
                let named = Path::new("mlx").join(model.file_name());
                if folder.is_dir() && !used_mlx.contains(&named) {
                    let size = dir_size(&folder);
                    add(named.to_string_lossy().replace('\\', "/"), folder, size);
                }
            }
            continue;
        }
        // A file goes with its check mark and its part: `x.gguf`, `x.gguf.ok`, `x.gguf.part`.
        let base = [".ok", ".part"]
            .iter()
            .find_map(|s| name.strip_suffix(s))
            .unwrap_or(&name)
            .to_string();
        if !used.contains(Path::new(&base)) {
            add(base, path, meta.len());
        }
    }
    let mut out: Vec<Unused> = found.into_values().collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Removes the [`unused`] model files, and leaves everything the model at `settings` runs with.
/// Under setup's lock, as [`remove`] is. Returns the bytes given back.
pub fn remove_unused(settings: &Settings, reporter: &Reporter) -> Result<u64, Error> {
    let home = paths::home();
    let _only_one = crate::setup::lock(&home)?;
    crate::run::wait_for_the_stopped(&crate::CancelToken::new());
    let mut freed = 0;
    for file in unused(settings) {
        for path in &file.paths {
            let gone = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
            match gone {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::io("remove", path, &e)),
            }
        }
        freed += file.bytes;
        reporter.log(Level::Info, format!("removed {}", file.name));
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
    fn files_the_model_doesnt_use_are_told_apart_from_its_own() {
        use crate::machine::Tier;
        let dir = std::env::temp_dir().join(format!("fs-unused-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mlx/flux2-klein-4b-mflux-q8/vae")).unwrap();
        std::fs::create_dir_all(dir.join("mlx/flux2-klein-4b-mflux-q4/vae")).unwrap();
        let write =
            |name: &str, bytes: usize| std::fs::write(dir.join(name), vec![0u8; bytes]).unwrap();
        // What an earlier build set up: klein at 8-bit, and Z-Image Turbo beside it.
        write("flux-2-klein-4b-Q8_0.gguf", 400);
        write("flux-2-klein-4b-Q8_0.gguf.ok", 64);
        write("Qwen3-4B-Q8_0.gguf", 300);
        write("z_image_turbo-Q8_0.gguf", 600);
        write("z_image_turbo-Q8_0.gguf.part", 50);
        write("mlx/flux2-klein-4b-mflux-q8/vae/0.safetensors", 70);
        write("mlx/flux2-klein-4b-mflux-q4/vae/0.safetensors", 30);
        // And what klein at 4-bit runs with, some of it still coming.
        write("full_encoder_small_decoder.safetensors", 20);
        write("full_encoder_small_decoder.safetensors.ok", 64);
        write("flux-2-klein-4b-Q4_0.gguf.part", 90);

        let unused = |backend, tier| {
            let settings = Settings {
                backend,
                tier,
                vram_gb: 4.0,
            };
            unused_in(&dir, &settings)
                .into_iter()
                .map(|u| (u.name, u.bytes))
                .collect::<Vec<_>>()
        };
        let named = |list: &[(&str, u64)]| {
            list.iter()
                .map(|(n, b)| (n.to_string(), *b))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            unused(Backend::Cuda, Tier::Q4),
            named(&[
                ("z_image_turbo-Q8_0.gguf", 650),
                ("flux-2-klein-4b-Q8_0.gguf", 464),
                ("Qwen3-4B-Q8_0.gguf", 300),
                ("mlx/flux2-klein-4b-mflux-q8", 70),
                ("mlx/flux2-klein-4b-mflux-q4", 30),
            ])
        );
        // At 8-bit the 8-bit files are its own, and the 4-bit ones aren't.
        assert_eq!(
            unused(Backend::Vulkan, Tier::Q8),
            named(&[
                ("z_image_turbo-Q8_0.gguf", 650),
                ("flux-2-klein-4b-Q4_0.gguf", 90),
                ("mlx/flux2-klein-4b-mflux-q8", 70),
                ("mlx/flux2-klein-4b-mflux-q4", 30),
            ])
        );
        // mflux runs from its own folder only.
        let mac = unused(Backend::Mlx, Tier::Q4);
        assert!(
            !mac.iter().any(|(n, _)| n == "mlx/flux2-klein-4b-mflux-q4"),
            "{mac:?}"
        );
        assert!(
            mac.iter()
                .any(|(n, _)| n == "full_encoder_small_decoder.safetensors"),
            "{mac:?}"
        );
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
