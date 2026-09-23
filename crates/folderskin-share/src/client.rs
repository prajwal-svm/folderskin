//! Signed requests to the community service.
//!
//! Every request after the first public one is signed with the computer's key (see [`crate::sign`]).
//! Two things about time: a computer whose clock is off by more than five minutes is told so, with
//! the service's own time, and the client sets its clock by that and tries once more; and no two
//! requests are ever signed with the same second, so a retry of the same upload is never mistaken
//! for a replay of the first.
//!
//! Redirects are never followed: a signed request goes to the address it was signed for or nowhere.

use crate::api::{self, Created, Export, Me, NewSubmission, Status, Submission};
use crate::key::DeviceKey;
use crate::sign;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The largest answer read as JSON, and the largest file an export hands over.
const MAX_JSON: usize = 1024 * 1024;
const MAX_FILE: usize = 2 * 1024 * 1024;

/// Why a request didn't work. `Display` is a sentence the app shows as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Nothing answered.
    Offline,
    /// The service answered with an error of its own, already in words.
    Service {
        status: u16,
        code: String,
        message: String,
    },
    /// It answered something this version can't read.
    Unreadable,
    /// The service's address isn't one a signed request may go to.
    BadAddress,
}

impl Error {
    /// The service's code for the error, such as `quota` or `not_verified`.
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Service { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Offline => f.write_str(
                "FolderSkin's sharing service couldn't be reached. Check your connection and try again.",
            ),
            Error::Service { message, .. } => f.write_str(message),
            Error::Unreadable => f.write_str(
                "FolderSkin's sharing service answered in a way this version can't read. Update FolderSkin and try again.",
            ),
            Error::BadAddress => {
                f.write_str("The sharing service's address has to start with https://.")
            }
        }
    }
}

impl std::error::Error for Error {}

struct Clock {
    /// Seconds to add to this computer's clock to get the service's.
    offset: i64,
    /// The last time a request was signed with, so the next is always later.
    last: u64,
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    /// The path the service sits under, if any: part of what the service sees, so part of what
    /// is signed.
    prefix: String,
    key: DeviceKey,
    clock: Mutex<Clock>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

/// Whether `url` is an address a key's signatures may be sent to: https, or plain http to this
/// computer itself for `wrangler dev`. Decided from the parsed address, so `http://localhost@elsewhere`
/// (a user name of "localhost" on another host) is seen for what it is; and with no user name or
/// password at all, which a service address never needs.
fn allowed(url: &reqwest::Url) -> bool {
    // The parser has already lower-cased the host and written IPv4 and IPv6 addresses out in full.
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let secure = url.scheme() == "https" || (url.scheme() == "http" && local);
    secure && url.username().is_empty() && url.password().is_none()
}

/// A piece of a path, from the service or the user, before it goes into a URL: ids, hashes and
/// file names only ever hold these characters, so anything else is refused rather than escaped.
fn segment(value: &str) -> Result<&str, Error> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    ok.then_some(value).ok_or(Error::Unreadable)
}

impl Client {
    /// A client for the service at `base` (no trailing slash needed), signing as `key`.
    pub fn new(base: &str, key: DeviceKey) -> Result<Client, Error> {
        let url = reqwest::Url::parse(base.trim()).map_err(|_| Error::BadAddress)?;
        if !allowed(&url)
            || url.host_str().is_none_or(str::is_empty)
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::BadAddress);
        }
        // Requests go to the address as it was parsed and checked, not as it was typed.
        let base = url.as_str().trim_end_matches('/').to_string();
        let prefix = url.path().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Long enough for a 2 MB picture on a slow connection.
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("FolderSkin/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::Offline)?;
        Ok(Client {
            http,
            base,
            prefix,
            key,
            clock: Mutex::new(Clock { offset: 0, last: 0 }),
        })
    }

    pub fn key(&self) -> &DeviceKey {
        &self.key
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn clock(&self) -> std::sync::MutexGuard<'_, Clock> {
        self.clock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn local_now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    /// The time to sign the next request with: the service's time as best known, and always
    /// after the last one.
    fn stamp(&self) -> u64 {
        let mut clock = self.clock();
        let now = Self::local_now().saturating_add_signed(clock.offset);
        let ts = now.max(clock.last + 1);
        clock.last = ts;
        ts
    }

    /// The page that verifies this computer, signed so the service knows the key asked for it.
    /// It opens in the browser, since the check can't run inside the app.
    pub fn verify_url(&self, handle: &str) -> Result<String, Error> {
        if !crate::is_handle(handle) {
            return Err(Error::Service {
                status: 400,
                code: "bad_handle".into(),
                message: "A name is 3 to 39 letters, digits and single dashes, not starting or ending with a dash.".into(),
            });
        }
        let mut nonce = [0u8; 16];
        aws_lc_rs::rand::fill(&mut nonce).map_err(|_| Error::Unreadable)?;
        let nonce = base64_url(&nonce);
        let ts = self.stamp();
        let key = self.key.public();
        let signature = self
            .key
            .sign(sign::verify_message(&key, &nonce, ts, handle).as_bytes());
        // Every value is base64url, digits or a handle, none of which needs escaping in a query.
        Ok(format!(
            "{}/verify?k={key}&n={nonce}&t={ts}&h={handle}&s={signature}",
            self.base
        ))
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<(Vec<u8>, &'static str)>,
        signed: bool,
        max: usize,
    ) -> Result<Vec<u8>, Error> {
        let mut retried_clock = false;
        loop {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{path}", self.base));
            let bytes = body.as_ref().map(|(b, _)| b.as_slice()).unwrap_or_default();
            if signed {
                let ts = self.stamp();
                let full = format!("{}{path}", self.prefix);
                let message = sign::request_message(method.as_str(), &full, ts, bytes);
                request = request
                    .header(sign::KEY_HEADER, self.key.public())
                    .header(sign::TIME_HEADER, ts.to_string())
                    .header(sign::SIGNATURE_HEADER, self.key.sign(message.as_bytes()));
            }
            if let Some((bytes, content_type)) = &body {
                request = request
                    .header("Content-Type", *content_type)
                    .body(bytes.clone());
            }
            let mut response = request.send().await.map_err(|_| Error::Offline)?;
            let status = response.status();
            let server_time = response
                .headers()
                .get(sign::SERVER_TIME_HEADER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok());
            let mut answer = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::Offline)? {
                answer.extend_from_slice(&chunk);
                if answer.len() > max {
                    return Err(Error::Unreadable);
                }
            }
            if status.is_success() {
                return Ok(answer);
            }
            let error = service_error(status.as_u16(), &answer);
            // Set the clock by the service's and sign once more.
            if error.code() == Some("clock") && signed && !retried_clock {
                if let Some(theirs) = server_time {
                    let mut clock = self.clock();
                    clock.offset = theirs - Self::local_now() as i64;
                    clock.last = 0;
                    retried_clock = true;
                    continue;
                }
            }
            return Err(error);
        }
    }

    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        signed: bool,
    ) -> Result<T, Error> {
        let body = body.map(|v| {
            (
                serde_json::to_vec(&v).unwrap_or_default(),
                "application/json",
            )
        });
        let bytes = self.request(method, path, body, signed, MAX_JSON).await?;
        serde_json::from_slice(&bytes).map_err(|_| Error::Unreadable)
    }

    // ---- anyone ----

    /// Whether the service is taking packs at all. Unsigned: asked before there is a key.
    pub async fn status(&self) -> Result<Status, Error> {
        self.json(Method::GET, "/v1/status", None, false).await
    }

    /// This computer, as the service knows it.
    pub async fn me(&self) -> Result<Me, Error> {
        self.json(Method::GET, "/v1/me", None, true).await
    }

    /// Changes the name this computer's packs are credited to; resolves to the name it has now.
    pub async fn rename(&self, handle: &str) -> Result<String, Error> {
        let answer: Value = self
            .json(
                Method::POST,
                "/v1/me",
                Some(json!({ "handle": handle })),
                true,
            )
            .await?;
        answer["handle"]
            .as_str()
            .map(str::to_string)
            .ok_or(Error::Unreadable)
    }

    pub async fn create(&self, submission: &NewSubmission) -> Result<Created, Error> {
        let body = serde_json::to_value(submission).map_err(|_| Error::Unreadable)?;
        self.json(Method::POST, "/v1/submissions", Some(body), true)
            .await
    }

    pub async fn put_item(&self, id: &str, sha256: &str, bytes: Vec<u8>) -> Result<(), Error> {
        let path = format!(
            "/v1/submissions/{}/items/{}",
            segment(id)?,
            segment(sha256)?
        );
        self.request(
            Method::PUT,
            &path,
            Some((bytes, "application/octet-stream")),
            true,
            MAX_JSON,
        )
        .await
        .map(drop)
    }

    pub async fn put_sheet(&self, id: &str, n: usize, bytes: Vec<u8>) -> Result<(), Error> {
        let path = format!("/v1/submissions/{}/sheets/{n}", segment(id)?);
        self.request(
            Method::PUT,
            &path,
            Some((bytes, "application/octet-stream")),
            true,
            MAX_JSON,
        )
        .await
        .map(drop)
    }

    pub async fn finalize(&self, id: &str) -> Result<(), Error> {
        let path = format!("/v1/submissions/{}/finalize", segment(id)?);
        self.json::<Value>(Method::POST, &path, Some(json!({})), true)
            .await
            .map(drop)
    }

    /// The packs this computer has sent, newest first.
    pub async fn submissions(&self) -> Result<Vec<Submission>, Error> {
        let list: api::Submissions = self
            .json(Method::GET, "/v1/submissions", None, true)
            .await?;
        Ok(list.submissions)
    }

    /// Takes one of this computer's packs back, out of the queue or out of the public bucket.
    pub async fn withdraw(&self, id: &str) -> Result<(), Error> {
        let path = format!("/v1/packs/{}", segment(id)?);
        self.request(Method::DELETE, &path, None, true, MAX_JSON)
            .await
            .map(drop)
    }

    // ---- the maintainer ----

    /// Packs waiting for a decision: `waiting`, `flagged`, `pending`, `approved` or `open`.
    pub async fn queue(&self, status: &str) -> Result<Value, Error> {
        let path = format!("/v1/admin/queue?status={}", segment(status)?);
        self.json(Method::GET, &path, None, true).await
    }

    /// Approves a pack, or turns it down with reason codes from the pack terms.
    pub async fn decide(
        &self,
        id: &str,
        decision: &str,
        reasons: &[String],
        note: &str,
    ) -> Result<Value, Error> {
        let path = format!("/v1/admin/submissions/{}/decision", segment(id)?);
        let body = json!({ "decision": decision, "reasons": reasons, "note": note });
        self.json(Method::POST, &path, Some(body), true).await
    }

    pub async fn takedown(&self, id: &str, reasons: &[String], note: &str) -> Result<Value, Error> {
        let path = format!("/v1/admin/submissions/{}/takedown", segment(id)?);
        let body = json!({ "reasons": reasons, "note": note });
        self.json(Method::POST, &path, Some(body), true).await
    }

    /// The kill switch.
    pub async fn pause(&self, paused: bool, message: &str) -> Result<Value, Error> {
        let body = json!({ "paused": paused, "message": message });
        self.json(Method::POST, "/v1/admin/pause", Some(body), true)
            .await
    }

    /// Approved packs not pulled into the repository yet.
    pub async fn exports(&self) -> Result<Vec<Export>, Error> {
        let list: api::Exports = self
            .json(Method::GET, "/v1/admin/exports", None, true)
            .await?;
        Ok(list.packs)
    }

    pub async fn export_manifest(&self, id: &str) -> Result<Vec<u8>, Error> {
        let path = format!("/v1/admin/exports/{}/pack.json", segment(id)?);
        self.request(Method::GET, &path, None, true, 64 * 1024)
            .await
    }

    pub async fn export_file(&self, id: &str, file: &str) -> Result<Vec<u8>, Error> {
        let path = format!(
            "/v1/admin/exports/{}/files/{}",
            segment(id)?,
            segment(file)?
        );
        self.request(Method::GET, &path, None, true, MAX_FILE).await
    }

    pub async fn export_done(&self, id: &str) -> Result<(), Error> {
        let path = format!("/v1/admin/exports/{}/done", segment(id)?);
        self.json::<Value>(Method::POST, &path, Some(json!({})), true)
            .await
            .map(drop)
    }
}

/// The error in a failed answer: the service's own sentence when it sent one.
fn service_error(status: u16, body: &[u8]) -> Error {
    match serde_json::from_slice::<api::ErrorBody>(body) {
        Ok(body) => Error::Service {
            status,
            code: body.error.code,
            message: body.error.message,
        },
        Err(_) => Error::Service {
            status,
            code: "http".into(),
            message: format!(
                "FolderSkin's sharing service answered {status}. Please try again in a while."
            ),
        },
    }
}

fn base64_url(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// A request as the stand-in service saw it.
    #[derive(Debug)]
    struct Seen {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Seen {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
    }

    /// An answer from the stand-in service: status, extra headers, body.
    type Answer = (u16, Vec<(&'static str, String)>, String);

    /// A stand-in service on localhost that answers each request with the next of `answers` and
    /// reports what it was sent.
    fn serve(answers: Vec<Answer>) -> (String, mpsc::Receiver<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for (status, headers, body) in answers {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut parts = line.split_whitespace();
                let (method, path) = (
                    parts.next().unwrap_or("").to_string(),
                    parts.next().unwrap_or("").to_string(),
                );
                let mut seen_headers = Vec::new();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end().to_string();
                    if header.is_empty() {
                        break;
                    }
                    if let Some((k, v)) = header.split_once(':') {
                        seen_headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let length = seen_headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut sent = vec![0; length];
                reader.read_exact(&mut sent).unwrap();
                tx.send(Seen {
                    method,
                    path,
                    headers: seen_headers,
                    body: sent,
                })
                .unwrap();
                let mut stream = stream;
                let extra: String = headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}\r\n"))
                    .collect();
                write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        (base, rx)
    }

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn requests_are_signed_over_exactly_what_is_sent() {
        let (base, seen) = serve(vec![(
            201,
            vec![],
            r#"{"submission_id": "sub_aaaaaaaaaaaaaaaaaaaa", "need": ["ab"], "sheets": 1}"#.into(),
        )]);
        let client = Client::new(&base, DeviceKey::generate().unwrap()).unwrap();
        let submission = NewSubmission {
            manifest: api::Manifest {
                name: "Night prints".into(),
                tags: vec!["woodblock".into()],
                skins: vec![],
            },
            license: "CC0-1.0".into(),
            source: "own".into(),
            notes: String::new(),
            terms_version: 1,
            items: vec![],
        };
        let created = run(client.create(&submission)).unwrap();
        assert_eq!(created.submission_id, "sub_aaaaaaaaaaaaaaaaaaaa");

        let request = seen.recv().unwrap();
        assert_eq!(
            (request.method.as_str(), request.path.as_str()),
            ("POST", "/v1/submissions")
        );
        let ts: u64 = request.header("x-fs-ts").unwrap().parse().unwrap();
        let message = sign::request_message("POST", "/v1/submissions", ts, &request.body);
        assert_eq!(
            request.header("x-fs-key"),
            Some(client.key().public().as_str())
        );
        assert!(sign::verify(
            request.header("x-fs-key").unwrap(),
            message.as_bytes(),
            request.header("x-fs-sig").unwrap()
        ));
        let sent: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(sent["manifest"]["name"], "Night prints");
    }

    #[test]
    fn a_clock_that_is_out_is_set_by_the_service_and_the_request_sent_again() {
        let theirs = Client::local_now() + 3600;
        let (base, seen) = serve(vec![
            (
                401,
                vec![("X-FS-Time", theirs.to_string())],
                r#"{"error": {"code": "clock", "message": "Your computer's clock is more than five minutes out."}}"#.into(),
            ),
            (200, vec![], r#"{"verified": true, "handle": "sunny-otter", "tier": "probation", "accepting": true}"#.into()),
        ]);
        let client = Client::new(&base, DeviceKey::generate().unwrap()).unwrap();
        let me = run(client.me()).unwrap();
        assert_eq!(me.handle.as_deref(), Some("sunny-otter"));
        let _first = seen.recv().unwrap();
        let second = seen.recv().unwrap();
        let ts: u64 = second.header("x-fs-ts").unwrap().parse().unwrap();
        assert!(ts.abs_diff(theirs) < 60, "signed with the service's time");
    }

    #[test]
    fn the_services_own_words_come_back_as_the_error() {
        let (base, _seen) = serve(vec![
            (
                429,
                vec![],
                r#"{"error": {"code": "quota", "message": "You've shared as many packs as you can today. Please try again tomorrow."}}"#.into(),
            ),
            (502, vec![], "<html>bad gateway</html>".into()),
        ]);
        let client = Client::new(&base, DeviceKey::generate().unwrap()).unwrap();
        let error = run(client.submissions()).unwrap_err();
        assert_eq!(error.code(), Some("quota"));
        assert_eq!(
            error.to_string(),
            "You've shared as many packs as you can today. Please try again tomorrow."
        );
        let error = run(client.submissions()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "FolderSkin's sharing service answered 502. Please try again in a while."
        );
    }

    #[test]
    fn a_service_under_a_path_is_signed_with_that_path() {
        let (base, seen) = serve(vec![(200, vec![], r#"{"verified": false}"#.into())]);
        let client = Client::new(&format!("{base}/api/"), DeviceKey::generate().unwrap()).unwrap();
        assert!(!run(client.me()).unwrap().verified);
        let request = seen.recv().unwrap();
        assert_eq!(request.path, "/api/v1/me");
        let ts: u64 = request.header("x-fs-ts").unwrap().parse().unwrap();
        let message = sign::request_message("GET", "/api/v1/me", ts, b"");
        assert!(sign::verify(
            &client.key().public(),
            message.as_bytes(),
            request.header("x-fs-sig").unwrap()
        ));
    }

    #[test]
    fn nothing_answering_is_said_plainly() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        drop(listener);
        let client = Client::new(&base, DeviceKey::generate().unwrap()).unwrap();
        assert_eq!(run(client.status()).unwrap_err(), Error::Offline);
    }

    #[test]
    fn signatures_only_go_to_https_or_this_computer() {
        let key = || DeviceKey::generate().unwrap();
        assert!(Client::new("https://community.example.org/", key()).is_ok());
        assert!(Client::new("http://localhost:8787", key()).is_ok());
        assert_eq!(
            Client::new("http://LOCALHOST:8787/", key()).unwrap().base(),
            "http://localhost:8787"
        );
        assert!(Client::new("http://127.0.0.1", key()).is_ok());
        assert!(Client::new("http://[::1]:8787/", key()).is_ok());
        for bad in [
            "http://community.example.org",
            "http://localhost.evil.test",
            // A user name (and password) of "localhost" on somebody else's host.
            "http://localhost:80@evil.example",
            "http://127.0.0.1:1@evil.example/",
            "http://localhost@evil.example",
            "https://someone:secret@community.example.org",
            "ftp://x",
            "",
        ] {
            assert_eq!(
                Client::new(bad, key()).unwrap_err(),
                Error::BadAddress,
                "{bad}"
            );
        }
    }

    #[test]
    fn ids_that_could_leave_their_place_in_a_path_are_refused() {
        let client = Client::new(
            "https://community.example.org",
            DeviceKey::generate().unwrap(),
        )
        .unwrap();
        for bad in ["../admin", "a/b", "", "..", "sub_x?y=1", "a b"] {
            assert_eq!(
                run(client.withdraw(bad)).unwrap_err(),
                Error::Unreadable,
                "{bad}"
            );
        }
    }

    #[test]
    fn the_verify_link_is_signed_for_this_key_and_handle() {
        let client = Client::new(
            "https://community.example.org",
            DeviceKey::generate().unwrap(),
        )
        .unwrap();
        let url = client.verify_url("sunny-otter").unwrap();
        let query = url.split_once('?').unwrap().1;
        let get = |name: &str| {
            query
                .split('&')
                .find_map(|kv| kv.strip_prefix(&format!("{name}=")))
                .unwrap()
                .to_string()
        };
        assert!(url.starts_with("https://community.example.org/verify?"));
        assert_eq!(get("k"), client.key().public());
        assert_eq!(get("h"), "sunny-otter");
        let message = sign::verify_message(
            &get("k"),
            &get("n"),
            get("t").parse().unwrap(),
            "sunny-otter",
        );
        assert!(sign::verify(&get("k"), message.as_bytes(), &get("s")));
        assert!(client.verify_url("no").is_err());
    }

    #[test]
    fn no_two_requests_share_a_second() {
        let client = Client::new(
            "https://community.example.org",
            DeviceKey::generate().unwrap(),
        )
        .unwrap();
        let stamps: Vec<u64> = (0..5).map(|_| client.stamp()).collect();
        assert!(stamps.windows(2).all(|w| w[1] > w[0]), "{stamps:?}");
    }
}
