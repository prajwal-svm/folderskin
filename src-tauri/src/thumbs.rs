//! The gallery's thumbnails, served at an address instead of sent inside the gallery's reply.
//!
//! A library of skins is tens of megabytes of PNG. Carried as data URLs in one `list_skins` reply,
//! every thumbnail is encoded to base64 (half as big again), parsed back out of the JSON and
//! decoded by the webview, all at once — and all of it stays in memory for as long as the gallery
//! does. An address costs a hundred bytes, so the reply stays small whatever the library holds:
//! the webview asks for the pictures it is showing, caches them itself, and drops the rest.
//!
//! A skin's id comes from its own pixels, so an address always answers with the same picture and
//! the answer can be cached forever. The thumbnail version is in the path, so a change to the
//! compositor asks a different address rather than reading a stale picture from that cache.

use crate::commands::{default_thumbnail_png, DEFAULT_ID, THUMB_CACHE_VERSION};
use crate::state::AppState;
use tauri::http::{Request, Response};
use tauri::{AppHandle, Manager, Runtime, UriSchemeContext, UriSchemeResponder};

/// The scheme thumbnails are served on, registered in [`crate::run`].
pub const SCHEME: &str = "skin";

/// Where the webview can fetch the thumbnail of `id`: a skin's, or [`DEFAULT_ID`]'s for the plain
/// default folder.
pub fn url(id: &str) -> String {
    // Windows and Android serve custom schemes over http; elsewhere the scheme stands on its own.
    let origin = if cfg!(any(windows, target_os = "android")) {
        format!("http://{SCHEME}.localhost")
    } else {
        format!("{SCHEME}://localhost")
    };
    format!("{origin}/{THUMB_CACHE_VERSION}/{}.png", encode(id))
}

/// Answers a request for a thumbnail. Reading one can mean reading a picture off disk, or drawing
/// it again, so the work leaves the thread the webview asked on.
pub fn serve<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn_blocking(move || responder.respond(answer(&app, &path)));
}

fn answer<R: Runtime>(app: &AppHandle<R>, path: &str) -> Response<Vec<u8>> {
    let Some(png) = png_at(app, path) else {
        return empty(404);
    };
    Response::builder()
        .header("Content-Type", "image/png")
        .header("Cache-Control", "public, max-age=31536000, immutable")
        // No `Access-Control-Allow-Origin`: nothing reads these pixels back out, it only shows
        // them. Colours are read in Rust now (`folderskin_core::palette`), where the picture
        // already is. Add the header again if something ever needs a canvas read of a thumbnail.
        .body(png)
        .unwrap_or_else(|_| empty(500))
}

/// The PNG an address names, or `None` when it names nothing FolderSkin has: an id it doesn't
/// know, a skin whose picture has gone, or a thumbnail from a version it no longer draws.
fn png_at<R: Runtime>(app: &AppHandle<R>, path: &str) -> Option<Vec<u8>> {
    let id = id_at(path)?;
    let state = app.state::<AppState>();
    if id == DEFAULT_ID {
        return Some(default_thumbnail_png(app, &state));
    }
    state.find_saved(&id).map(|(_, png)| png)
}

/// The id an address names, or `None` for a path this version doesn't serve.
fn id_at(path: &str) -> Option<String> {
    let (version, file) = path.trim_start_matches('/').split_once('/')?;
    if version.parse::<u32>().ok()? != THUMB_CACHE_VERSION {
        return None;
    }
    Some(decode(file.strip_suffix(".png")?))
}

fn empty(status: u16) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .body(Vec::new())
        .expect("a status and an empty body")
}

/// A skin id as one path segment. Ids hold a colon (`user:8f3a…`), which is legal in a path but
/// means something to enough URL parsers to be worth encoding.
fn encode(id: &str) -> String {
    id.replace(':', "%3A")
}

fn decode(segment: &str) -> String {
    segment.replace("%3A", ":").replace("%3a", ":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_names_the_version_and_the_skin() {
        let url = url("user:8f3a1b2c3d4e");
        assert!(
            url.ends_with(&format!("/{THUMB_CACHE_VERSION}/user%3A8f3a1b2c3d4e.png")),
            "{url}"
        );
        assert!(url.starts_with(&format!("{SCHEME}://localhost/")) || cfg!(windows), "{url}");
    }

    /// The path the webview asks for, out of the address it was given.
    fn path_of(url: &str) -> String {
        url[url.find("/localhost").expect("a localhost address") + "/localhost".len()..].to_string()
    }

    #[test]
    fn a_path_is_read_back_into_the_id_it_was_made_from() {
        let path = |id: &str| path_of(&url(id));
        assert_eq!(
            id_at(&path("user:8f3a1b2c3d4e")).as_deref(),
            Some("user:8f3a1b2c3d4e")
        );
        assert_eq!(id_at(&path(DEFAULT_ID)).as_deref(), Some(DEFAULT_ID));
        // A lower-case escape is the same address; anything else is nothing we serve.
        assert_eq!(decode("user%3aff"), "user:ff");
        assert_eq!(id_at("/0/user%3Aff.png"), None, "an older thumbnail version");
        assert_eq!(id_at(&format!("/{THUMB_CACHE_VERSION}/user%3Aff.jpg")), None);
        assert_eq!(id_at("/nothing"), None);
    }

    /// The webview's side of it: a real request, answered by the real handler. Excluded on
    /// Windows with the rest of what needs tauri's `test` feature (see `Cargo.toml`).
    #[cfg(not(windows))]
    mod served {
        use super::*;
        use crate::store::{NewSkin, SkinImage, SkinSource};
        use std::sync::Arc;

        fn a_skin() -> (NewSkin, SkinImage) {
            let rgba = image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 40, 60, 255]));
            (
                NewSkin {
                    id: crate::store::skin_id(b"served"),
                    name: "Served".into(),
                    source: SkinSource::Import,
                    provider: None,
                    model: None,
                    idea: None,
                    tags: Vec::new(),
                    pack: None,
                    pack_name: None,
                    author: None,
                    license: None,
                    pack_hash: None,
                },
                SkinImage::Folder(Arc::new(rgba)),
            )
        }

        #[test]
        fn a_skin_is_answered_with_its_picture_and_nothing_else_is() {
            let app = tauri::test::mock_app();
            let state = AppState::default();
            let (new, image) = a_skin();
            let entry = state.keep_unsaved(new, image);
            app.manage(state);

            let served = answer(app.handle(), &path_of(&url(&entry.id)));
            assert_eq!(served.status(), 200);
            assert!(served.body().starts_with(b"\x89PNG"), "a PNG, not a data URL");
            assert_eq!(served.headers()["content-type"], "image/png");
            assert!(
                !served.headers().contains_key("access-control-allow-origin"),
                "nothing reads these pixels back out, so nothing needs permission to"
            );
            assert!(served.headers()["cache-control"]
                .to_str()
                .unwrap()
                .contains("immutable"));

            let unknown = url("user:000000000000");
            assert_eq!(answer(app.handle(), &path_of(&unknown)).status(), 404);
            assert_eq!(answer(app.handle(), "/nonsense").status(), 404);
        }
    }
}
