//! Tauri's build step: the app's configuration, capabilities and icons. FolderSkin ships no skins
//! of its own, so there is nothing else to embed; every skin comes from the user's library.

fn main() {
    tauri_build::build()
}
