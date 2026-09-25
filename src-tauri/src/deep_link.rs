//! `folderskin://install?pack=<id>` links: the Install buttons on folderskin.app's gallery.
//!
//! The scheme is in tauri.conf.json (`plugins > deep-link`), and the bundles register it with the
//! system: the macOS app's Info.plist, the Windows installers, and the desktop entry of the Linux
//! .deb and .rpm. An AppImage, which nothing installs, and a development build on Windows or
//! Linux register it for themselves as they start ([`register_scheme`]). macOS routes the scheme
//! only to a bundled app, so `pnpm tauri dev` there never sees a link: try one on a built
//! FolderSkin.app.
//!
//! How a link arrives:
//! - macOS: as an event to the running app, which the system starts first when it isn't running.
//! - Windows and Linux: as the only argument of a new FolderSkin. When one is already running, the
//!   single-instance plugin (lib.rs) hands the link to it and ends the new one.
//!
//! A link FolderSkin was started with is there as it sets up (`get_current`); later ones come as
//! events (`on_open_url`). Every one goes through [`install_pack`], which takes nothing but an
//! install link that names a pack id. A good one brings the window forward and waits in
//! [`InstallLinks`] until the webview takes it with `install_link_take`. The webview asks as it
//! starts, since a link can come before it has (or during the first-launch welcome, which it
//! waits out), and again whenever [`EVENT`] says another has come. It opens Community on that
//! pack and adds it the way the pack's Add button does. A link with an id the pack had before it
//! moved still finds it: `community_pack` follows head.json's `moved` (community.rs).

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// The scheme, as tauri.conf.json registers it.
pub const SCHEME: &str = "folderskin";
/// What the webview is told when a link is waiting for it.
pub const EVENT: &str = "install-link";
/// The longest link read at all. An install link is a small fraction of it.
const MAX_LINK: usize = 512;

/// The pack id in a `folderskin://install?pack=<id>` link, or `None` for anything else.
///
/// The scheme has to be `folderskin`, and the link has to say `install`: as its host
/// (`folderskin://install?pack=…`, the way the website writes it, with or without a `/` after it)
/// or as its whole path (`folderskin:install?pack=…`). It may have no user, password or port. It
/// needs exactly one `pack`, and that has to be a pack id as `folderskin_core::pack::is_pack_id`
/// has it: lower-case letters and digits in words joined by single dashes, at most 40
/// characters. Other parameters are passed over, so the website can add one later without the
/// apps already installed turning its links down.
pub fn install_pack(link: &str) -> Option<String> {
    let link = link.trim();
    if link.len() > MAX_LINK {
        return None;
    }
    let url = reqwest::Url::parse(link).ok()?;
    if url.scheme() != SCHEME
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let install = match url.host_str() {
        Some("install") => matches!(url.path(), "" | "/"),
        None | Some("") => matches!(url.path(), "install" | "/install"),
        Some(_) => false,
    };
    if !install {
        return None;
    }
    let mut packs = url
        .query_pairs()
        .filter(|(key, _)| key == "pack")
        .map(|(_, value)| value.into_owned());
    match (packs.next(), packs.next()) {
        (Some(pack), None) if folderskin_core::pack::is_pack_id(&pack) => Some(pack),
        _ => None,
    }
}

/// The pack the newest good link asked for, until the webview takes it. Only the newest is kept:
/// packs are added one at a time, and a link that comes before the webview has taken the one
/// before it stands for what was clicked last.
#[derive(Default)]
pub struct InstallLinks(Mutex<Option<String>>);

impl InstallLinks {
    fn put(&self, pack: String) {
        *self.lock() = Some(pack);
    }

    fn take(&self) -> Option<String> {
        self.lock().take()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The pack a `folderskin://install` link asked for, once: `None` when none is waiting, or the
/// one that was has been taken.
#[tauri::command]
pub fn install_link_take(links: State<'_, InstallLinks>) -> Option<String> {
    links.take()
}

/// Listens for links from now on, and takes the one FolderSkin was started with, if any. Called
/// once, as the app sets up.
pub fn watch(app: &tauri::App) {
    use tauri_plugin_deep_link::DeepLinkExt;
    register_scheme(app);
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        arrived(&handle, event.urls().iter().map(|url| url.to_string()));
    });
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        arrived(app.handle(), urls.iter().map(|url| url.to_string()));
    }
}

/// The links the system handed over. The last good one waits for the webview, which is told, and
/// the window comes to the front; anything else is ignored.
fn arrived<R: Runtime>(app: &AppHandle<R>, links: impl IntoIterator<Item = String>) {
    let Some(pack) = links.into_iter().filter_map(|l| install_pack(&l)).last() else {
        return;
    };
    app.state::<InstallLinks>().put(pack);
    bring_forward(app);
    let _ = app.emit(EVENT, ());
}

/// Shows the main window, unminimised and in front of the others.
pub fn bring_forward<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Registers the scheme for this copy of FolderSkin where no installer did: an AppImage, and a
/// development build. A failure (a Linux without `xdg-mime`, say) only means links don't reach
/// this copy, so it is written to the log and nothing else.
#[cfg(any(windows, target_os = "linux"))]
fn register_scheme(app: &tauri::App) {
    use tauri_plugin_deep_link::DeepLinkExt;
    #[cfg(target_os = "linux")]
    let uninstalled = app.env().appimage.is_some();
    #[cfg(windows)]
    let uninstalled = false;
    if cfg!(debug_assertions) || uninstalled {
        if let Err(e) = app.deep_link().register_all() {
            eprintln!("folderskin: couldn't register folderskin:// links ({e})");
        }
    }
}

/// macOS reads the scheme from the app bundle alone.
#[cfg(not(any(windows, target_os = "linux")))]
fn register_scheme(_app: &tauri::App) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_install_link_gives_its_pack() {
        for link in [
            "folderskin://install?pack=classic-art",
            "folderskin://install/?pack=classic-art",
            "FolderSkin://install?pack=classic-art",
            "folderskin:install?pack=classic-art",
            "folderskin:/install?pack=classic-art",
            "folderskin:///install?pack=classic-art",
            "  folderskin://install?pack=classic-art\n",
            "folderskin://install?pack=classic%2Dart",
            "folderskin://install?from=gallery&pack=classic-art#top",
        ] {
            assert_eq!(
                install_pack(link).as_deref(),
                Some("classic-art"),
                "{link:?}"
            );
        }
        let longest = "a".repeat(40);
        assert_eq!(
            install_pack(&format!("folderskin://install?pack={longest}")),
            Some(longest)
        );
    }

    #[test]
    fn anything_else_is_ignored() {
        for link in [
            "",
            "classic-art",
            "https://folderskin.app/install?pack=classic-art",
            "folderskins://install?pack=classic-art",
            "folderskin://open?pack=classic-art",
            "folderskin://INSTALL?pack=classic-art",
            "folderskin://install/more?pack=classic-art",
            "folderskin://install.example?pack=classic-art",
            "folderskin://user@install?pack=classic-art",
            "folderskin://user:secret@install?pack=classic-art",
            "folderskin://install:8080?pack=classic-art",
            "folderskin:uninstall?pack=classic-art",
            "folderskin://install",
            "folderskin://install?pack=",
            "folderskin://install?packs=classic-art",
            "folderskin://install?pack=classic-art&pack=colours",
            "folderskin://install?pack=Classic-Art",
            "folderskin://install?pack=../../etc",
            "folderskin://install?pack=classic%2Fart",
            "folderskin://install?pack=classic--art",
            "folderskin://install?pack=-classic",
            "folderskin://install?pack=classic art",
            "folderskin://install?pack=con",
        ] {
            assert_eq!(install_pack(link), None, "{link:?}");
        }
        let too_long = "a".repeat(41);
        assert_eq!(
            install_pack(&format!("folderskin://install?pack={too_long}")),
            None
        );
        let padded = format!("folderskin://install?pack=colours&x={}", "y".repeat(600));
        assert_eq!(install_pack(&padded), None, "longer than any link needs");
    }

    // With the mock runtime, which Windows' test binary can't load (see tree.rs).
    #[cfg(not(windows))]
    #[test]
    fn a_good_link_waits_for_the_webview_which_is_told_and_takes_it_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tauri::Listener;

        let app = tauri::test::mock_builder()
            .manage(InstallLinks::default())
            .invoke_handler(tauri::generate_handler![install_link_take])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let told = Arc::new(AtomicUsize::new(0));
        let heard = told.clone();
        app.listen(EVENT, move |_| {
            heard.fetch_add(1, Ordering::SeqCst);
        });
        let take = || {
            tauri::test::get_ipc_response(
                &webview,
                tauri::webview::InvokeRequest {
                    cmd: "install_link_take".into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "tauri://localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(serde_json::json!({})),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .unwrap()
            .deserialize::<Option<String>>()
            .unwrap()
        };

        arrived(
            app.handle(),
            ["https://folderskin.app/?pack=colours".to_string()],
        );
        assert_eq!(told.load(Ordering::SeqCst), 0, "not a link to act on");
        assert_eq!(take(), None);

        // Two at once: the one clicked last is the one added.
        arrived(
            app.handle(),
            [
                "folderskin://install?pack=colours".to_string(),
                "folderskin://install?pack=classic-art".to_string(),
                "folderskin://install?pack=Nope".to_string(),
            ],
        );
        assert_eq!(told.load(Ordering::SeqCst), 1);
        assert_eq!(take().as_deref(), Some("classic-art"));
        assert_eq!(take(), None, "taken once");
    }
}
