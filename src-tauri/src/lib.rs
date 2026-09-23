//! FolderSkin desktop app (Tauri v2).

pub mod ai;
pub mod commands;
pub mod community;
pub mod composer;
pub mod folder_icon;
pub mod github;
pub mod onboarding;
pub mod pack_views;
pub mod state;
pub mod store;
pub mod tree;
pub mod window;

/// The saved API keys, sealed on disk; a crate of their own so the command line opens them too.
pub use folderskin_keys as keys;

use tauri::Manager;

/// Writes panics to `<app log dir>/panic.log`, and lets the default hook run as well.
///
/// A release build has no console to print to (`main.rs` makes it a `windows_subsystem` binary),
/// so the message the default hook writes to stderr is lost exactly when it is wanted: in the
/// build a user is running. The commands unwind rather than abort (see `panic = "abort"` in the
/// workspace `Cargo.toml`), so a panic here surfaces as a failed command rather than a vanished
/// window — this is what says where it came from. Appending, so a second one doesn't erase the
/// first, and every step is allowed to fail: logging a panic must not cause one.
fn log_panics_to(dir: std::path::PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("unnamed").to_string();
        let where_ = info
            .location()
            .map_or_else(|| "unknown".into(), |l| l.to_string());
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("panic.log"))
            {
                let _ = writeln!(
                    file,
                    "{at} v{} thread \"{thread}\" panicked at {where_}: {}",
                    env!("CARGO_PKG_VERSION"),
                    info.payload_as_str().unwrap_or("(no message)"),
                );
            }
        }
        previous(info);
    }));
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Updates: the newest GitHub release's latest.json, signed with the key whose public half
        // is in tauri.conf.json; the page restarts the app once one is installed.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(state::AppState::default())
        .manage(keys::Keys::default())
        .manage(github::Pending::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_skins,
            commands::inspect_path,
            commands::import_image,
            commands::apply_skin,
            commands::revert_skin,
            commands::delete_skin,
            commands::edit_skin,
            commands::skins_folder,
            community::community_packs,
            community::community_preview,
            community::community_add,
            community::community_update,
            community::community_pack_skins,
            community::community_remove,
            community::import_pack,
            community::export_pack,
            github::github_connect,
            github::github_wait,
            github::github_cancel,
            github::github_account,
            github::github_sign_out,
            github::publish_pack,
            composer::composer_template,
            composer::composer_save,
            composer::composer_preview,
            composer::composer_image,
            composer::composer_skin_image,
            composer::composer_design,
            commands::folder_icon,
            commands::platform_info,
            tree::subfolder_count,
            tree::tree_bytes,
            tree::apply_skin_tree,
            tree::revert_skin_tree,
            tree::stop_tree_run,
            onboarding::onboarding_needed,
            onboarding::finish_onboarding,
            ai::ai_catalogue,
            ai::ai_set_key,
            ai::ai_clear_key,
            ai::ai_test_key,
            ai::ai_generate,
        ])
        .setup(|app| {
            // Before anything that could panic, so a crash report has somewhere to land.
            if let Ok(dir) = app.path().app_log_dir() {
                log_panics_to(dir);
            }
            // Open the saved skins before the window exists, so the first list_skins sees them.
            // The onboarding's marker sits beside them (onboarding.rs).
            match app.path().app_data_dir() {
                Ok(dir) => app.state::<state::AppState>().open_store(dir.join("skins")),
                Err(e) => eprintln!(
                    "folderskin: no app data folder ({e}); skins added now last until you quit"
                ),
            }
            // API keys live here, encrypted, not in the keychain (see keys.rs).
            match app.path().app_config_dir() {
                Ok(dir) => app.state::<keys::Keys>().open(&dir),
                Err(e) => {
                    eprintln!("folderskin: no app config folder ({e}); keys last until you quit")
                }
            }
            folder_icon::set_dock_icon(include_bytes!("../icons/icon.png"));
            window::create_main(app)
        })
        .run(tauri::generate_context!())
        .expect("error while running FolderSkin");
}
