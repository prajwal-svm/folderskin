//! The library's thumbnails, handed to the webview as files under the `fsthumb:` scheme.
//!
//! Each skin's thumbnail on a folder is a 512 px PNG of about 450 KB, and the library lists every
//! skin: sent inside the list as data URLs, 150 skins came to 90 MB of text, which every switch
//! between the Mac's, Windows' and the Linux folder sent again and the window waited for. So the
//! list names each thumbnail by an address instead ([`url`]), and the webview asks for the ones
//! it shows. A thumbnail is read from where it's kept, or drawn the first time
//! ([`crate::state::AppState::thumbnail_in`]), at most [`AT_ONCE`] at a time, and the rest are
//! drawn ahead in the background ([`crate::state::AppState::predraw_thumbnails`]).
//!
//! Its address names the renderer's version, the folder and the skin, whose picture never changes
//! under its id, so the webview may keep it for good. The webview reaches the scheme as
//! `fsthumb://localhost/<path>` on macOS and Linux and as `http://fsthumb.localhost/<path>` on
//! Windows, which is why [`url`] builds both and the Content-Security-Policy in tauri.conf.json
//! allows both.

use crate::commands::THUMB_CACHE_VERSION;
use crate::state::AppState;
use crate::store::is_skin_id;
use folderskin_core::compositor::Style;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};
use tokio::sync::Semaphore;

/// The scheme's name.
pub const SCHEME: &str = "fsthumb";

/// How many thumbnails are read or drawn at once.
const AT_ONCE: usize = 6;
static TURNS: Semaphore = Semaphore::const_new(AT_ONCE);

/// Where the webview finds skin `id`'s thumbnail on the folder of `style`.
pub fn url(style: Style, id: &str) -> String {
    let path = format!("v{THUMB_CACHE_VERSION}/{}/{id}", style.id());
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{SCHEME}.localhost/{path}")
    } else {
        format!("{SCHEME}://localhost/{path}")
    }
}

/// The folder and the skin a request's path names: `/v3/linux/<skin id>`. Every part is checked
/// to be what it claims, and a thumbnail from another version of the renderer isn't one this can
/// draw.
pub fn parse(path: &str) -> Option<(Style, String)> {
    let path = path.trim_start_matches('/').split('?').next()?;
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [version, style, id]
            if version.strip_prefix('v') == Some(&THUMB_CACHE_VERSION.to_string())
                && is_skin_id(id) =>
        {
            Some((Style::from_id(style)?, id.to_string()))
        }
        _ => None,
    }
}

/// The scheme's handler: reads or draws on a blocking thread, so the webview never waits on it.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let state = ctx.app_handle().state::<AppState>().inner().clone();
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn(async move {
        let answer = match parse(&path) {
            Some((style, id)) => thumbnail(state, style, id).await,
            None => Err(StatusCode::BAD_REQUEST),
        };
        let response = match answer {
            Ok(png) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/png")
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(png),
            Err(status) => Response::builder().status(status).body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
    });
}

/// Skin `id` on the folder of `style`, as a PNG, once it's this request's turn.
async fn thumbnail(state: AppState, style: Style, id: String) -> Result<Vec<u8>, StatusCode> {
    let _turn = TURNS
        .acquire()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    tauri::async_runtime::spawn_blocking(move || state.thumbnail_in(&id, style))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnail_is_asked_for_by_version_folder_and_skin() {
        let id = crate::store::skin_id(b"a picture");
        let url = url(Style::Linux, &id);
        assert!(
            url.ends_with(&format!("/v{THUMB_CACHE_VERSION}/linux/{id}")),
            "{url}"
        );
        for style in Style::ALL {
            let path = format!("/v{THUMB_CACHE_VERSION}/{}/{id}", style.id());
            assert_eq!(parse(&path), Some((style, id.clone())));
        }
        let path = format!("/v{THUMB_CACHE_VERSION}/windows/{id}");
        assert_eq!(
            parse(&format!("{path}?x=1")),
            Some((Style::Windows, id.clone()))
        );
        // Anything else is refused, before any file is looked for.
        for wrong in [
            format!("/v0/mac/{id}"),
            format!("/v{THUMB_CACHE_VERSION}/amiga/{id}"),
            format!("/v{THUMB_CACHE_VERSION}/mac/../../etc/passwd"),
            format!("/v{THUMB_CACHE_VERSION}/mac/user:NOT-A-SKIN"),
            format!("/v{THUMB_CACHE_VERSION}/mac"),
            String::new(),
        ] {
            assert_eq!(parse(&wrong), None, "{wrong}");
        }
    }
}
