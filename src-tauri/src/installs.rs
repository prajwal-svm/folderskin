//! Counting how often each community pack is added, for the numbers on folderskin.app's gallery.
//!
//! Once a pack from Community is added, FolderSkin tells the community service (services/community)
//! the pack's id: `POST <service>/v1/packs/<id>/installs`, with no body. Nothing else goes with it:
//! no account, no device id, nothing about the library. Like every request FolderSkin makes, it
//! names the app's version in its User-Agent. The service keeps a count per pack and, until the
//! UTC day is over, a salted hash of the network the request came from, so adding the same pack
//! again that day doesn't count twice; the daily clean-up deletes those hashes.
//!
//! It is fire and forget: the request runs on a task of its own once the pack is saved, gives up
//! after a few seconds, and whatever becomes of it (offline, the service down, a pack it doesn't
//! know) is dropped without a word. Nothing waits for it.
//!
//! The service is the one `FOLDERSKIN_COMMUNITY_API` names, or community.folderskin.app for a
//! release build. A development build, and a build reading the packs from another copy of
//! folderskin-community (`FOLDERSKIN_COMMUNITY_URL`), count nothing unless a service is named that
//! way, so trying things out doesn't add to the real numbers.

use folderskin_core::pack;
use std::sync::OnceLock;
use std::time::Duration;

/// Where installs are counted when nothing names another service.
pub const COUNTS_API: &str = "https://community.folderskin.app";
/// How long the whole request may take, and connecting.
const TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Tells the community service that pack `pack_id` was added. Returns at once.
pub fn report(pack_id: &str) {
    let other_packs =
        std::env::var("FOLDERSKIN_COMMUNITY_URL").is_ok_and(|url| !url.trim().is_empty());
    let service = service(
        crate::share::api_override(),
        other_packs,
        cfg!(debug_assertions),
    );
    if let Some(url) = service.and_then(|base| install_url(&base, pack_id)) {
        tauri::async_runtime::spawn(send(url));
    }
}

/// The service to count at: the one `named` on purpose, or [`COUNTS_API`] for a release build that
/// reads the real packs; `None` otherwise.
fn service(named: Option<String>, other_packs: bool, development: bool) -> Option<String> {
    named.or_else(|| (!development && !other_packs).then(|| COUNTS_API.to_string()))
}

/// `<base>/v1/packs/<pack_id>/installs`, or `None` when `pack_id` isn't a pack id or `base` isn't
/// an address to send it to: https, or plain http to this computer for `wrangler dev`, with a host
/// and no user name, password, query or fragment.
pub fn install_url(base: &str, pack_id: &str) -> Option<String> {
    if !pack::is_pack_id(pack_id) {
        return None;
    }
    let url = reqwest::Url::parse(base.trim()).ok()?;
    // The parser has already lower-cased the host and written IPv4 and IPv6 addresses out in full.
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let secure = url.scheme() == "https" || (url.scheme() == "http" && local);
    let plain = url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    (secure && plain).then(|| {
        format!(
            "{}/v1/packs/{pack_id}/installs",
            url.as_str().trim_end_matches('/')
        )
    })
}

/// Sends the count. Whatever comes back, or doesn't, is dropped.
async fn send(url: String) {
    if let Some(client) = client() {
        // No body, and saying so: a POST without a length can be turned away with a 411.
        let _ = client
            .post(url)
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .send()
            .await;
    }
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(concat!("FolderSkin/", env!("CARGO_PKG_VERSION")))
                .build()
                .ok()
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    #[test]
    fn the_count_goes_to_the_service_by_pack_id_alone() {
        assert_eq!(
            install_url("https://community.folderskin.app", "classic-art").as_deref(),
            Some("https://community.folderskin.app/v1/packs/classic-art/installs")
        );
        assert_eq!(
            install_url(" https://example.org/community/ ", "colours").as_deref(),
            Some("https://example.org/community/v1/packs/colours/installs")
        );
        assert_eq!(
            install_url("http://127.0.0.1:8787", "colours").as_deref(),
            Some("http://127.0.0.1:8787/v1/packs/colours/installs"),
            "wrangler dev on this computer"
        );
        for base in [
            "http://community.folderskin.app",
            "https://user:secret@community.folderskin.app",
            "https://community.folderskin.app/?x=1",
            "https://community.folderskin.app/#top",
            "ftp://community.folderskin.app",
            "not an address",
            "",
        ] {
            assert_eq!(install_url(base, "colours"), None, "{base}");
        }
        for id in ["", "../admin", "Colours", "colours/installs", "a--b", "con"] {
            assert_eq!(install_url(COUNTS_API, id), None, "{id}");
        }
    }

    #[test]
    fn only_a_release_build_reading_the_real_packs_counts_without_being_told_where() {
        let named = || Some("http://127.0.0.1:8787".to_string());
        assert_eq!(service(None, false, false).as_deref(), Some(COUNTS_API));
        assert_eq!(service(None, false, true), None, "a development build");
        assert_eq!(
            service(None, true, false),
            None,
            "another copy of the packs"
        );
        for (other_packs, development) in [(false, false), (true, true)] {
            assert_eq!(service(named(), other_packs, development), named());
        }
    }

    #[test]
    fn the_request_is_a_bare_post_that_nothing_waits_on() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let heard = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut lines = Vec::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                lines.push(line.trim().to_ascii_lowercase());
            }
            let length: usize = lines
                .iter()
                .find_map(|l| l.strip_prefix("content-length:"))
                .map_or(0, |n| n.trim().parse().unwrap());
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let mut stream = &stream;
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            (lines, body)
        });
        let url = install_url(&base, "classic-art").unwrap();
        // An answer the app can't use is dropped like any other.
        tauri::async_runtime::block_on(send(url));
        let (lines, body) = heard.join().unwrap();
        assert_eq!(lines[0], "post /v1/packs/classic-art/installs http/1.1");
        assert!(
            lines.contains(&"content-length: 0".to_string()),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("user-agent: folderskin/")),
            "{lines:?}"
        );
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("cookie") || l.starts_with("authorization")),
            "{lines:?}"
        );
        assert!(body.is_empty());
    }
}
