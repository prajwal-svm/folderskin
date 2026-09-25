//! FolderSkin desktop app (Tauri v2).

pub mod ai;
pub mod catalog;
pub mod chats;
pub mod commands;
pub mod community;
pub mod composer;
pub mod deep_link;
pub mod folder_icon;
pub mod icons;
pub mod installs;
pub mod language;
pub mod look;
pub mod onboarding;
pub mod pack_views;
pub mod previews;
pub mod share;
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
    let builder = tauri::Builder::default();
    // First, so a second FolderSkin ends before anything else starts. On Windows and Linux a
    // folderskin:// link starts one: this hands the link to the one already running (its
    // `deep-link` feature does that) and brings that one's window forward, as it does for any
    // second launch. macOS never starts a second copy of the app for either.
    #[cfg(any(windows, target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        deep_link::bring_forward(app)
    }));
    builder
        // folderskin://install links (deep_link.rs).
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Updates: the newest GitHub release's latest.json, signed with the key whose public half
        // is in tauri.conf.json; the page restarts the app once one is installed.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(state::AppState::default())
        .manage(keys::Keys::default())
        // The AI runs that can be stopped, and this computer as the local models see it.
        .manage(ai::jobs::Jobs::default())
        .manage(ai::local::Local::default())
        .manage(chats::Chats::default())
        .manage(catalog::Community::default())
        // Community strips and thumbnails, fetched as the cards that show them scroll in.
        .register_asynchronous_uri_scheme_protocol(previews::SCHEME, previews::handle)
        .manage(share::Waiting::default())
        .manage(deep_link::InstallLinks::default())
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
            ai::ai_cancel,
            ai::ai_local_status,
            ai::ai_local_setup,
            ai::ai_local_remove,
            ai::ai_local_remove_unused,
            icons::icon_pack_download,
            icons::icon_packs_installed,
            icons::icon_pack_read,
            icons::icon_pack_remove,
            look::folder_look,
            look::set_folder_look,
            language::set_language,
            chats::chats_list,
            chats::chat_read,
            chats::chat_save,
            chats::chat_delete,
            chats::chat_keep_reference,
            community::community_search,
            community::community_refresh,
            community::community_installed,
            community::community_pack,
            deep_link::install_link_take,
            share::share_status,
            share::share_verify,
            share::share_wait,
            share::share_cancel,
            share::share_save_key,
            share::share_load_key,
            share::share_submit,
            share::share_submissions,
            share::share_withdraw,
        ])
        .setup(|app| {
            // Before anything that could panic, so a crash report has somewhere to land.
            if let Ok(dir) = app.path().app_log_dir() {
                log_panics_to(dir);
            }
            // Open the saved skins before the window exists, so the first list_skins sees them.
            // The onboarding's marker sits beside them (onboarding.rs).
            match app.path().app_data_dir() {
                Ok(dir) => {
                    // Before the skins: their thumbnails are drawn on the folder chosen.
                    look::load(&dir);
                    app.state::<chats::Chats>().open(dir.join("chats"));
                    app.state::<state::AppState>().open_store(dir.join("skins"))
                }
                Err(e) => eprintln!(
                    "folderskin: no app data folder ({e}); skins added now last until you quit"
                ),
            }
            // API keys live here, encrypted, not in the keychain (see keys.rs).
            match app.path().app_config_dir() {
                Ok(dir) => {
                    let keys = app.state::<keys::Keys>();
                    keys.open(&dir);
                    share::forget_github_sign_in(&keys);
                }
                Err(e) => {
                    eprintln!("folderskin: no app config folder ({e}); keys last until you quit")
                }
            }
            folder_icon::set_dock_icon(include_bytes!("../icons/icon.png"));
            window::create_main(app)?;
            // Once the window is there to bring forward: a link FolderSkin was started with waits
            // for the webview until it asks.
            deep_link::watch(app);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building FolderSkin")
        .run(|_app, event| {
            // A painting, or mflux's install, doesn't end with the app on macOS and Linux: it
            // would go on for minutes after FolderSkin quit, holding gigabytes of memory and the
            // graphics card. (On Windows the job object ends it anyway.)
            if let tauri::RunEvent::Exit = event {
                folderskin_local::run::end_all();
            }
        });
}
