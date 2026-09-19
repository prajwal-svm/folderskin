//! FolderSkin desktop app (Tauri v2).

pub mod ai;
pub mod commands;
pub mod community;
pub mod folder_icon;
pub mod keys;
pub mod skins;
pub mod state;
pub mod store;
pub mod window;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            community::community_remove,
            community::import_pack,
            community::export_pack,
            commands::folder_icon,
            commands::platform_info,
            ai::ai_catalogue,
            ai::ai_set_key,
            ai::ai_clear_key,
            ai::ai_test_key,
            ai::ai_generate,
        ])
        .setup(|app| {
            // Open the saved skins before the window exists, so the first list_skins sees them.
            match app.path().app_data_dir() {
                Ok(dir) => app.state::<state::AppState>().open_store(dir.join("skins")),
                Err(e) => eprintln!(
                    "folderskin: no app data folder ({e}); skins added now last until you quit"
                ),
            }
            // API keys live in a private file here, not in the keychain (see keys.rs).
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
