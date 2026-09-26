//! Skins on a drive, handed to the webview as files under the `fsdrive:` scheme.
//!
//! While a drive is picked, the library shows each skin on it. There can be thousands of skins,
//! so a card's picture is only drawn when its `<img>` asks for it, which the webview does for the
//! cards on screen, and at most [`AT_ONCE`] are drawn at a time, so a fast scroll queues the rest
//! rather than starting them all. Each is kept beside the skin once drawn
//! ([`crate::store::Store::drive_thumbnail_png`]). Its address names the renderer's version, the
//! drive's shape and the skin, whose picture never changes under its id, so the webview may keep
//! it for good as well.
//!
//! The webview reaches the scheme as `fsdrive://localhost/<path>` on macOS and Linux and as
//! `http://fsdrive.localhost/<path>` on Windows, which is why [`base`] builds both and the
//! Content-Security-Policy in tauri.conf.json allows both.

use crate::commands::THUMB_CACHE_VERSION;
use crate::state::AppState;
use crate::store::is_skin_id;
use folderskin_core::drive::DriveShape;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};
use tokio::sync::Semaphore;

/// The scheme's name.
pub const SCHEME: &str = "fsdrive";

/// How many drive thumbnails are drawn at once.
const AT_ONCE: usize = 4;
static TURNS: Semaphore = Semaphore::const_new(AT_ONCE);

/// Where the webview finds each skin's thumbnail on the drive of `shape`: this, then the skin's
/// id.
pub fn base(shape: DriveShape) -> String {
    let path = format!("v{THUMB_CACHE_VERSION}/{}/", shape.id());
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{SCHEME}.localhost/{path}")
    } else {
        format!("{SCHEME}://localhost/{path}")
    }
}

/// The drive's shape and the skin a request's path names: `/v3/mac-external/<skin id>`. Every
/// part is checked to be what it claims, and a thumbnail from another version of the renderer
/// isn't one this can draw.
pub fn parse(path: &str) -> Option<(DriveShape, String)> {
    let path = path.trim_start_matches('/').split('?').next()?;
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [version, shape, id]
            if version.strip_prefix('v') == Some(&THUMB_CACHE_VERSION.to_string())
                && is_skin_id(id) =>
        {
            Some((DriveShape::from_id(shape)?, id.to_string()))
        }
        _ => None,
    }
}

/// The scheme's handler: draws on a blocking thread, so the webview never waits on it.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let state = ctx.app_handle().state::<AppState>().inner().clone();
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn(async move {
        let answer = match parse(&path) {
            Some((shape, id)) => thumbnail(state, shape, id).await,
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

/// Skin `id` on the drive of `shape`, as a PNG, once it's this request's turn.
async fn thumbnail(state: AppState, shape: DriveShape, id: String) -> Result<Vec<u8>, StatusCode> {
    let _turn = TURNS
        .acquire()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    tauri::async_runtime::spawn_blocking(move || state.drive_thumbnail(&id, shape))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnail_is_asked_for_by_version_drive_and_skin() {
        let shape = DriveShape::from_id("mac-external").unwrap();
        let base = base(shape);
        assert!(
            base.ends_with(&format!("/v{THUMB_CACHE_VERSION}/mac-external/")),
            "{base}"
        );
        let id = crate::store::skin_id(b"a picture");
        let path = format!("/v{THUMB_CACHE_VERSION}/mac-external/{id}");
        assert_eq!(parse(&path), Some((shape, id.clone())));
        assert_eq!(parse(&format!("{path}?x=1")), Some((shape, id.clone())));
        // Anything else is refused, before any file is looked for.
        for wrong in [
            format!("/v0/mac-external/{id}"),
            format!("/v{THUMB_CACHE_VERSION}/mac-solid-state/{id}"),
            format!("/v{THUMB_CACHE_VERSION}/mac-external/../../etc/passwd"),
            format!("/v{THUMB_CACHE_VERSION}/mac-external/user:NOT-A-SKIN"),
            format!("/v{THUMB_CACHE_VERSION}/mac-external"),
            String::new(),
        ] {
            assert_eq!(parse(&wrong), None, "{wrong}");
        }
    }
}
