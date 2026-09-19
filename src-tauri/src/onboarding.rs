//! The first-launch onboarding, shown until it is finished once on this computer.
//!
//! Finishing it writes a marker beside the saved-skin store, in the app data directory:
//!
//! ```text
//! <app data dir>/onboarding.json   {"version":1,"finished_at":1790000000000,"app":"0.1.0"}
//! ```
//!
//! Only whether the marker exists decides anything today; what it records is there for a later
//! version that wants to show new users something new. `FOLDERSKIN_ONBOARDING=1` shows the
//! onboarding whatever the marker says, for trying it out.

use folderskin_core::apply::paths::write_atomic;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Manager};

const MARKER_FILE: &str = "onboarding.json";
/// Bump when the marker's format changes in a way an older build could misread.
const MARKER_VERSION: u32 = 1;
/// Set to `1` to show the onboarding at every launch.
const FORCE_VAR: &str = "FOLDERSKIN_ONBOARDING";

/// What the marker records.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Marker {
    version: u32,
    /// When the onboarding was finished, in Unix milliseconds.
    finished_at: u64,
    /// The FolderSkin version it was finished in.
    app: String,
}

/// Whether to show the onboarding. Always when `forced`; never when there is no data folder
/// (`marker_exists` is `None`), since nothing could remember that it was finished; otherwise
/// until its marker exists.
fn needed(forced: bool, marker_exists: Option<bool>) -> bool {
    forced || marker_exists == Some(false)
}

/// Whether the value of `FOLDERSKIN_ONBOARDING` forces the onboarding.
fn forced(var: Option<&str>) -> bool {
    var.is_some_and(|v| v.trim() == "1")
}

/// Writes the marker into `data_dir`, creating the folder if need be.
fn write_marker(data_dir: &Path, finished_at: u64) -> Result<(), String> {
    let marker = Marker {
        version: MARKER_VERSION,
        finished_at,
        app: env!("CARGO_PKG_VERSION").to_string(),
    };
    let mut json = serde_json::to_vec(&marker).map_err(|e| e.to_string())?;
    json.push(b'\n');
    std::fs::create_dir_all(data_dir)
        .and_then(|()| write_atomic(&data_dir.join(MARKER_FILE), &json))
        .map_err(|e| format!("couldn't save that FolderSkin is set up: {e}"))
}

/// True until the onboarding has been finished on this computer.
#[tauri::command]
pub fn onboarding_needed(app: AppHandle) -> bool {
    let marker = app
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(MARKER_FILE).is_file());
    needed(forced(std::env::var(FORCE_VAR).ok().as_deref()), marker)
}

/// Remembers that the onboarding is finished, so it doesn't show again. Without a data folder
/// there is nowhere to remember it, which is not an error.
#[tauri::command]
pub async fn finish_onboarding(app: AppHandle) -> Result<(), String> {
    let Ok(dir) = app.path().app_data_dir() else {
        return Ok(());
    };
    tauri::async_runtime::spawn_blocking(move || write_marker(&dir, crate::store::now_ms()))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_shows_until_it_is_finished_unless_forced_or_nothing_could_remember() {
        assert!(needed(false, Some(false)), "a first launch");
        assert!(!needed(false, Some(true)), "finished before");
        assert!(!needed(false, None), "no data folder");
        assert!(needed(true, Some(true)), "forced");
        assert!(needed(true, None), "forced without a data folder");
    }

    #[test]
    fn only_one_forces_it() {
        assert!(forced(Some("1")));
        assert!(forced(Some(" 1\n")));
        for other in [None, Some(""), Some("0"), Some("true"), Some("yes")] {
            assert!(!forced(other), "{other:?}");
        }
    }

    #[test]
    fn finishing_writes_the_marker_beside_the_store() {
        let root =
            std::env::temp_dir().join(format!("folderskin-onboarding-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data_dir = root.join("app.folderskin.desktop");
        assert!(needed(false, Some(data_dir.join(MARKER_FILE).is_file())));

        write_marker(&data_dir, 1_790_000_000_000).unwrap();
        let path = data_dir.join(MARKER_FILE);
        assert!(!needed(false, Some(path.is_file())), "not again");
        let marker: Marker = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            marker,
            Marker {
                version: 1,
                finished_at: 1_790_000_000_000,
                app: env!("CARGO_PKG_VERSION").to_string(),
            }
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with(r#"{"version":1,"finished_at":1790000000000,"app":""#),
            "{text}"
        );

        // Finishing again, after FOLDERSKIN_ONBOARDING=1 showed it once more, is fine.
        write_marker(&data_dir, 1_790_000_000_001).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_marker_that_cannot_be_written_is_an_error_sentence() {
        let root = std::env::temp_dir().join(format!(
            "folderskin-onboarding-blocked-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(MARKER_FILE)).unwrap();
        let err = write_marker(&root, 1).unwrap_err();
        assert!(
            err.starts_with("couldn't save that FolderSkin is set up"),
            "{err}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
