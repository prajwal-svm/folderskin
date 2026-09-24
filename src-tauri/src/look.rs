//! Which folder FolderSkin puts skins on: FolderSkin's own, as a Mac shows it, or the one Windows
//! draws. The user switches it in the folder panel; every computer starts on the Mac's. Artwork
//! skins (photos, paintings, AI pictures) are drawn and applied on it, and cached thumbnails are
//! kept apart per folder (`commands::thumb_tag`).
//!
//! It is kept in the app's data folder, beside the skins, so the library's first thumbnails at
//! launch are already drawn on it.

use folderskin_core::compositor::Style;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Whether it's Windows' folder; the Mac's otherwise.
static WINDOWS: AtomicBool = AtomicBool::new(false);
/// Where the choice is kept, once the data folder is known.
static FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

const FILE_NAME: &str = "folder-look.txt";

/// The folder skins go on now.
pub fn current() -> Style {
    if WINDOWS.load(Ordering::Relaxed) {
        Style::Windows
    } else {
        Style::Mac
    }
}

fn set(style: Style) {
    WINDOWS.store(style == Style::Windows, Ordering::Relaxed);
}

/// The choice saved in `dir`, the app's data folder. Nothing saved, or anything unreadable, is
/// the Mac's.
fn saved_in(dir: &Path) -> Style {
    std::fs::read_to_string(dir.join(FILE_NAME))
        .ok()
        .and_then(|s| Style::from_id(s.trim()))
        .unwrap_or(Style::Mac)
}

/// Reads the saved choice from `dir`, the app's data folder, and keeps later choices there.
pub fn load(dir: &Path) {
    set(saved_in(dir));
    *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.join(FILE_NAME));
}

fn parse(look: &str) -> Result<Style, String> {
    Style::from_id(look).ok_or_else(|| format!("FolderSkin doesn't know a folder called {look:?}"))
}

/// The folder skins go on, as the webview names it: "mac" or "windows".
#[tauri::command]
pub fn folder_look() -> &'static str {
    current().id()
}

/// Puts skins on another folder from now on, and keeps the choice.
#[tauri::command]
pub fn set_folder_look(look: String) -> Result<(), String> {
    let style = parse(&look)?;
    set(style);
    let file = FILE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(file) = file {
        if let Err(e) = std::fs::write(&file, style.id()) {
            eprintln!(
                "folderskin: couldn't keep the folder look in {}: {e}",
                file.display()
            );
        }
    }
    Ok(())
}

// The tests leave the app-wide choice alone: other tests draw thumbnails with it at the same time.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_choice_is_read_back_and_anything_else_is_the_macs() {
        let dir = std::env::temp_dir().join(format!("folderskin-look-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(saved_in(&dir), Style::Mac, "nothing saved yet");
        std::fs::write(dir.join(FILE_NAME), "windows\n").unwrap();
        assert_eq!(saved_in(&dir), Style::Windows);
        std::fs::write(dir.join(FILE_NAME), "rubbish").unwrap();
        assert_eq!(saved_in(&dir), Style::Mac);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_the_two_folders_are_known() {
        assert_eq!(parse("mac"), Ok(Style::Mac));
        assert_eq!(parse("windows"), Ok(Style::Windows));
        assert!(parse("linux").is_err());
    }

    #[test]
    fn every_computer_starts_on_the_macs() {
        // No test loads or sets a choice, so this is the value the app starts with.
        assert_eq!(current(), Style::Mac);
    }
}
