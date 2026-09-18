//! FolderSkin desktop app (Tauri v2).

pub mod ai;
pub mod commands;
pub mod folder_icon;
pub mod skins;
pub mod state;
pub mod window;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_skins,
            commands::inspect_path,
            commands::import_image,
            commands::apply_skin,
            commands::revert_skin,
            commands::folder_icon,
            commands::platform_info,
            ai::ai_catalogue,
            ai::ai_set_key,
            ai::ai_clear_key,
            ai::ai_test_key,
            ai::ai_generate,
        ])
        .setup(|app| {
            folder_icon::set_dock_icon(include_bytes!("../icons/icon.png"));
            window::create_main(app)
        })
        .run(tauri::generate_context!())
        .expect("error while running FolderSkin");
}
