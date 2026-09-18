//! FolderSkin desktop app (Tauri v2).

pub mod skins;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running FolderSkin");
}
