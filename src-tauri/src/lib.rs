//! FolderSkin desktop app (Tauri v2).

pub mod ai;
pub mod commands;
pub mod community;
pub mod composer;
pub mod folder_icon;
pub mod keys;
pub mod onboarding;
pub mod pack_views;
pub mod state;
pub mod store;
pub mod thumbs;
pub mod tree;
pub mod window;

use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        // The gallery's thumbnails are fetched from here rather than carried in its reply.
        .register_asynchronous_uri_scheme_protocol(thumbs::SCHEME, thumbs::serve)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Updates: the newest GitHub release's latest.json, signed with the key whose public half
        // is in tauri.conf.json; the page restarts the app once one is installed.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(state::AppState::default())
        .manage(keys::Keys::default())
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
            // Open the saved skins before the window exists, so the first list_skins sees them.
            // The onboarding's marker sits beside them (onboarding.rs).
            match app.path().app_data_dir() {
                Ok(dir) => app.state::<state::AppState>().open_store(dir.join("skins")),
                Err(e) => eprintln!(
                    "folderskin: no app data folder ({e}); skins added now last until you quit"
                ),
            }
            read_palettes_in_the_background(app.handle());
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

/// Reads the colours of skins saved before FolderSkin kept them, off the startup path, and tells
/// the gallery when there are some to show.
///
/// A skin saved now is read as it is saved (`Store::add`), so this only ever has work to do for a
/// library from an older version or after the reading itself changes. The gallery works
/// throughout: a skin with no colours yet simply isn't offered under a colour, exactly as it
/// wasn't while the webview was still reading them.
fn read_palettes_in_the_background(app: &tauri::AppHandle) {
    let handle = app.clone();
    let state = app.state::<state::AppState>().inner().clone();
    // Set before the thread starts, so a gallery that asks first is told to expect them.
    state.set_reading_palettes(true);
    std::thread::spawn(move || {
        if let Some(store) = state.store() {
            store.read_missing_palettes();
        }
        state.set_reading_palettes(false);
        // Always, even when there was nothing to read: the gallery waits on this to stop saying
        // it is still working them out.
        if let Err(e) = handle.emit("palettes-read", ()) {
            eprintln!("folderskin: couldn't tell the gallery about the colours: {e}");
        }
    });
}
