//! `community mirror`: the published tree (`v2/`) copied to its public mirror, the R2 bucket
//! behind packs.folderskin.app, through the community service. The service holds the bucket's
//! keys, so the maintainer's signing key is all this needs.
//!
//! Every file but `head.json` is named after what is in it, so one the mirror already serves at
//! the same length is the same file, and is left alone. The rest are uploaded
//! (`PUT /v1/admin/tree/<path>`), each with its SHA-256 for the bucket to check. `head.json` goes
//! last, and only once every other file is there, so the mirror never names a catalog it doesn't
//! hold. A request that may pass on its own (no answer, a 5xx, a 429) is tried again after a wait
//! that doubles each time.

use folderskin_share::{sign, DeviceKey};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The tree's folder, in the folder `--tree` names and on the mirror.
pub const TREE_DIR: &str = "v2";
/// The one file whose name doesn't say what is in it, uploaded last.
const HEAD_PATH: &str = "v2/head.json";
/// The header that carries a file's SHA-256, which the bucket checks the bytes against.
pub const SHA256_HEADER: &str = "X-Content-SHA256";

/// How often, and how patiently, a request that may pass on its own is tried again.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// Tries in all, the first one included.
    pub tries: u32,
    /// The wait before the second try. Each wait after doubles it.
    pub first: Duration,
    /// The longest wait, whatever the doubling or the service's Retry-After says.
    pub most: Duration,
}

impl Default for Backoff {
    /// Five tries over about fifteen seconds.
    fn default() -> Self {
        Backoff {
            tries: 5,
            first: Duration::from_secs(1),
            most: Duration::from_secs(30),
        }
    }
}

/// What a mirror run did.
#[derive(Debug, Default)]
pub struct Summary {
    /// How many files the mirror had already, at the right length.
    pub there: usize,
    /// The files uploaded, as paths such as `v2/pictures/<sha256>.png`, `head.json` last when it
    /// went.
    pub uploaded: Vec<String>,
    /// The files that couldn't be uploaded, each with why.
    pub failed: Vec<(String, String)>,
    /// Whether `head.json` went, which it does only when nothing failed.
    pub head: bool,
}

/// The mirror at a public address, written through the service.
pub struct Mirror {
    http: reqwest::Client,
    /// Where the mirror serves the tree from, such as `https://packs.folderskin.app`.
    public: String,
    /// The service's address.
    api: String,
    /// The path the service sits under, if any, which is part of what is signed.
    prefix: String,
    key: DeviceKey,
    backoff: Backoff,
    /// Seconds to add to this computer's clock to get the service's.
    offset: i64,
}

impl Mirror {
    /// A mirror serving from `public`, written through the service at `api` with the maintainer's
    /// `key`. Both are https:// addresses, or http:// on this computer for a local service.
    pub fn new(
        public: &str,
        api: &str,
        key: DeviceKey,
        backoff: Backoff,
    ) -> Result<Mirror, String> {
        let (public, _) = address(public, "the mirror's address")?;
        let (api, prefix) = address(api, "the service's address")?;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Long enough for a 2 MB picture on a slow connection.
            .timeout(Duration::from_secs(120))
            // A signed request goes to the address it was signed for or nowhere.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("folderskin-tools/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("couldn't start a web client: {e}"))?;
        Ok(Mirror {
            http,
            public,
            api,
            prefix,
            key,
            backoff,
            offset: 0,
        })
    }

    /// Brings the mirror up to date with `<tree>/v2`: every file it lacks, then `head.json`.
    /// Fails only when the tree can't be read; a file that can't be uploaded is in the summary,
    /// and keeps `head.json` back.
    pub async fn run(&mut self, tree: &Path) -> Result<Summary, String> {
        let files = tree_files(tree)?;
        if !files.iter().any(|f| f == HEAD_PATH) {
            return Err(format!(
                "there's no {HEAD_PATH} in {}; build the tree with `packs catalog` first",
                tree.display()
            ));
        }
        let mut summary = Summary::default();
        for path in files.iter().filter(|f| *f != HEAD_PATH) {
            // The length is enough to ask about; the bytes are read only to upload.
            let len = std::fs::metadata(tree.join(path))
                .map_err(|e| format!("couldn't read {path}: {e}"))?
                .len();
            if self.serves(path, len).await {
                summary.there += 1;
                continue;
            }
            let bytes =
                std::fs::read(tree.join(path)).map_err(|e| format!("couldn't read {path}: {e}"))?;
            match self.upload(path, bytes).await {
                Ok(()) => summary.uploaded.push(path.clone()),
                Err(why) => summary.failed.push((path.clone(), why)),
            }
        }
        if summary.failed.is_empty() {
            let bytes = std::fs::read(tree.join(HEAD_PATH))
                .map_err(|e| format!("couldn't read {HEAD_PATH}: {e}"))?;
            match self.upload(HEAD_PATH, bytes).await {
                Ok(()) => {
                    summary.uploaded.push(HEAD_PATH.to_string());
                    summary.head = true;
                }
                Err(why) => summary.failed.push((HEAD_PATH.to_string(), why)),
            }
        }
        Ok(summary)
    }

    /// Whether the mirror serves `path` at `len` bytes. When it can't be asked, the answer is no,
    /// and the file is uploaded again, which does no harm.
    async fn serves(&self, path: &str, len: u64) -> bool {
        let url = format!("{}/{path}", self.public);
        let mut wait = self.backoff.first;
        for attempt in 1..=self.backoff.tries {
            match self.http.head(&url).send().await {
                Ok(answer) if !may_pass(answer.status().as_u16()) => {
                    // Read from the header: reqwest's content_length() is the body's, and a HEAD
                    // answer has none.
                    let length = answer
                        .headers()
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok());
                    return answer.status() == reqwest::StatusCode::OK && length == Some(len);
                }
                Ok(answer) if attempt < self.backoff.tries => {
                    self.pause(retry_after(&answer), &mut wait).await
                }
                Err(_) if attempt < self.backoff.tries => self.pause(None, &mut wait).await,
                _ => return false,
            }
        }
        false
    }

    /// Uploads one file of the tree through the service, signed.
    async fn upload(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), String> {
        let route = format!("/v1/admin/tree/{path}");
        let url = format!("{}{route}", self.api);
        let signed = format!("{}{route}", self.prefix);
        let sha256 = sign::sha256_hex(&bytes);
        let mut wait = self.backoff.first;
        let mut last = 0;
        let mut clock_set = false;
        let mut attempt = 1;
        loop {
            // Each try is signed later than the one before, so the service never takes a try for
            // a replay of an earlier one it already saw.
            let ts = service_now(self.offset).max(last + 1);
            last = ts;
            let message = sign::request_message("PUT", &signed, ts, &bytes);
            let sent = self
                .http
                .put(&url)
                .header(sign::KEY_HEADER, self.key.public())
                .header(sign::TIME_HEADER, ts.to_string())
                .header(sign::SIGNATURE_HEADER, self.key.sign(message.as_bytes()))
                .header(SHA256_HEADER, &sha256)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes.clone())
                .send()
                .await;
            let (why, asked) = match sent {
                Ok(answer) if answer.status().is_success() => return Ok(()),
                Ok(answer) => {
                    let status = answer.status().as_u16();
                    let asked = retry_after(&answer);
                    let theirs = answer
                        .headers()
                        .get(sign::SERVER_TIME_HEADER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<i64>().ok());
                    let body = answer.bytes().await.unwrap_or_default();
                    let (code, why) = service_error(status, &body);
                    // Set the clock by the service's, once, and sign again: that try is free.
                    if code == "clock" && !clock_set {
                        if let Some(theirs) = theirs {
                            self.offset = theirs - local_now() as i64;
                            last = 0;
                            clock_set = true;
                            continue;
                        }
                    }
                    if !may_pass(status) {
                        return Err(why);
                    }
                    (why, asked)
                }
                Err(e) => (format!("the service didn't answer ({e})"), None),
            };
            if attempt >= self.backoff.tries {
                return Err(why);
            }
            attempt += 1;
            self.pause(asked, &mut wait).await;
        }
    }

    /// Waits before trying again: what the service asked for, or the next doubling, and never
    /// longer than [`Backoff::most`].
    async fn pause(&self, asked: Option<Duration>, wait: &mut Duration) {
        tokio::time::sleep(asked.unwrap_or(*wait).min(self.backoff.most)).await;
        *wait = (*wait * 2).min(self.backoff.most);
    }
}

/// Whether an answer with `status` may be different on another try: a 5xx, or a 429 asking to
/// slow down.
fn may_pass(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// The wait a Retry-After header asks for, in seconds.
fn retry_after(answer: &reqwest::Response) -> Option<Duration> {
    answer
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// The service's code and sentence from a failed answer, or a sentence of our own.
fn service_error(status: u16, body: &[u8]) -> (String, String) {
    let json: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    let code = json["error"]["code"].as_str().unwrap_or("http").to_string();
    let why = match json["error"]["message"].as_str() {
        Some(message) => format!("{message} ({status})"),
        None => format!("the service answered {status}"),
    };
    (code, why)
}

fn local_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The service's time as best known: this computer's clock, set by `offset`.
fn service_now(offset: i64) -> u64 {
    local_now().saturating_add_signed(offset)
}

/// `url` as a base address, and its path, without a trailing slash: https://, or http:// to this
/// computer, with no user, query or fragment. `what` names it in the error.
fn address(url: &str, what: &str) -> Result<(String, String), String> {
    let bad = || format!("{what} has to be an https:// address with no ? or #, not {url:?}");
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| bad())?;
    let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let secure = parsed.scheme() == "https" || (parsed.scheme() == "http" && local);
    if !secure
        || parsed.host_str().is_none_or(str::is_empty)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(bad());
    }
    let base = parsed.as_str().trim_end_matches('/').to_string();
    let path = parsed.path().trim_end_matches('/').to_string();
    Ok((base, path))
}

/// Every file under `<tree>/v2`, as paths such as `v2/catalog/<generation>.sqlite.gz`, sorted.
/// Dotfiles such as `.DS_Store` are left out, and so is anything but a plain file.
fn tree_files(tree: &Path) -> Result<Vec<String>, String> {
    fn walk(dir: &Path, relative: &str, out: &mut Vec<String>) -> Result<(), String> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("couldn't read {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("couldn't read {}: {e}", dir.display()))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = format!("{relative}/{name}");
            let kind = entry
                .file_type()
                .map_err(|e| format!("couldn't read {path}: {e}"))?;
            if kind.is_dir() {
                walk(&entry.path(), &path, out)?;
            } else if kind.is_file() {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(&tree.join(TREE_DIR), TREE_DIR, &mut files)?;
    // Only names the tree itself uses reach a URL: letters, digits, dots, dashes, underscores.
    if let Some(bad) = files.iter().find(|f| {
        !f.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'/'))
    }) {
        return Err(format!(
            "{bad} isn't a name the tree uses, so it can't be mirrored; take it out of {}",
            tree.join(TREE_DIR).display()
        ));
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// A request as the stand-in saw it.
    #[derive(Debug, Clone)]
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

    /// An answer: status, headers, body. A HEAD answer names its length in its headers.
    type Answer = (u16, Vec<(String, String)>, Vec<u8>);

    /// A stand-in for the mirror and the service at once, on localhost: `answer` decides what
    /// each request gets, and every request is kept, in order.
    fn serve(answer: impl Fn(&Seen) -> Answer + Send + 'static) -> (String, Arc<Mutex<Vec<Seen>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let (method, path) = (
                    parts.next().unwrap_or("").to_string(),
                    parts.next().unwrap_or("").to_string(),
                );
                let mut headers = Vec::new();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end().to_string();
                    if header.is_empty() {
                        break;
                    }
                    if let Some((k, v)) = header.split_once(':') {
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let length = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let request = Seen {
                    method,
                    path,
                    headers,
                    body,
                };
                let (status, extra, body) = answer(&request);
                log.lock().unwrap().push(request.clone());
                let mut head = format!("HTTP/1.1 {status} Whatever\r\nConnection: close\r\n");
                if !extra
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                {
                    head.push_str(&format!("Content-Length: {}\r\n", body.len()));
                }
                for (k, v) in &extra {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                let mut stream = stream;
                let _ = stream.write_all(head.as_bytes());
                if request.method != "HEAD" {
                    let _ = stream.write_all(&body);
                }
            }
        });
        (base, seen)
    }

    /// A tree in the system temp folder, removed when the test ends.
    struct Tree(PathBuf);

    impl Tree {
        fn new(test: &str) -> Tree {
            let dir = std::env::temp_dir()
                .join(format!("folderskin-mirror-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Tree(dir)
        }

        fn put(&self, path: &str, bytes: &[u8]) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const CATALOG: &str = "v2/catalog/0123456789abcdef.sqlite.gz";
    const PICTURE: &str =
        "v2/pictures/1111111111111111111111111111111111111111111111111111111111111111.png";
    const THUMB: &str =
        "v2/thumbs/2222222222222222222222222222222222222222222222222222222222222222.webp";

    /// A head.json, a catalog, a picture and a thumbnail, and a dotfile that isn't mirrored.
    fn tree(test: &str) -> Tree {
        let tree = Tree::new(test);
        tree.put(HEAD_PATH, b"{ \"version\": 2 }\n");
        tree.put(CATALOG, b"a catalog");
        tree.put(PICTURE, b"a picture");
        tree.put(THUMB, b"a thumbnail");
        tree.put("v2/.DS_Store", b"Finder's");
        tree
    }

    /// Short waits, so the tests don't sit through real ones.
    fn quick() -> Backoff {
        Backoff {
            tries: 3,
            first: Duration::from_millis(5),
            most: Duration::from_millis(20),
        }
    }

    fn run(mirror: &mut Mirror, tree: &Tree) -> Result<Summary, String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(mirror.run(&tree.0))
    }

    fn ok() -> Answer {
        (200, vec![], br#"{"stored": true}"#.to_vec())
    }

    fn length(n: usize) -> Vec<(String, String)> {
        vec![("Content-Length".into(), n.to_string())]
    }

    #[test]
    fn only_what_the_mirror_lacks_is_uploaded_and_head_json_goes_last() {
        let tree = tree("lacks");
        let (base, seen) = serve(|r| match (r.method.as_str(), r.path.as_str()) {
            // The catalog is there, the picture isn't, and the thumbnail is the wrong length.
            ("HEAD", p) if p == format!("/{CATALOG}") => (200, length(9), vec![]),
            ("HEAD", p) if p == format!("/{THUMB}") => (200, length(99), vec![]),
            ("HEAD", _) => (404, vec![], vec![]),
            ("PUT", _) => ok(),
            _ => (405, vec![], vec![]),
        });
        let key = DeviceKey::generate().unwrap();
        let public_key = key.public();
        let mut mirror = Mirror::new(&base, &base, key, quick()).unwrap();
        let summary = run(&mut mirror, &tree).unwrap();
        assert_eq!(summary.there, 1);
        assert_eq!(summary.uploaded, [PICTURE, THUMB, HEAD_PATH]);
        assert!(summary.failed.is_empty() && summary.head);

        let seen = seen.lock().unwrap();
        let asked: Vec<(&str, &str)> = seen
            .iter()
            .map(|r| (r.method.as_str(), r.path.as_str()))
            .collect();
        let picture = format!("/v1/admin/tree/{PICTURE}");
        let thumb = format!("/v1/admin/tree/{THUMB}");
        let head = format!("/v1/admin/tree/{HEAD_PATH}");
        assert_eq!(
            asked,
            [
                ("HEAD", format!("/{CATALOG}").as_str()),
                ("HEAD", format!("/{PICTURE}").as_str()),
                ("PUT", picture.as_str()),
                ("HEAD", format!("/{THUMB}").as_str()),
                ("PUT", thumb.as_str()),
                ("PUT", head.as_str()),
            ],
            "head.json is never asked about, and goes last"
        );
        // Each upload is the file, with its SHA-256, signed by the maintainer's key.
        for (request, bytes) in [
            (&seen[2], b"a picture".as_slice()),
            (&seen[5], b"{ \"version\": 2 }\n".as_slice()),
        ] {
            assert_eq!(request.body, bytes);
            assert_eq!(
                request.header(SHA256_HEADER),
                Some(sign::sha256_hex(bytes).as_str())
            );
            assert_eq!(request.header(sign::KEY_HEADER), Some(public_key.as_str()));
            let ts: u64 = request.header(sign::TIME_HEADER).unwrap().parse().unwrap();
            let message = sign::request_message("PUT", &request.path, ts, bytes);
            assert!(sign::verify(
                &public_key,
                message.as_bytes(),
                request.header(sign::SIGNATURE_HEADER).unwrap()
            ));
        }
    }

    #[test]
    fn a_failure_that_may_pass_is_tried_again_and_one_that_lasts_holds_head_json_back() {
        let tree = tree("retry");
        let picture_tries = Arc::new(AtomicUsize::new(0));
        let catalog_asks = Arc::new(AtomicUsize::new(0));
        let (tries, asks) = (Arc::clone(&picture_tries), Arc::clone(&catalog_asks));
        let (base, seen) = serve(move |r| {
            let path = r.path.as_str();
            match r.method.as_str() {
                // Asked to slow down once, for longer than the longest wait, then it's there.
                "HEAD" if path == format!("/{CATALOG}") => {
                    if asks.fetch_add(1, Ordering::SeqCst) == 0 {
                        (429, vec![("Retry-After".into(), "600".into())], vec![])
                    } else {
                        (200, length(9), vec![])
                    }
                }
                "HEAD" => (404, vec![], vec![]),
                // The picture fails twice and then goes; the thumbnail is turned down for good.
                "PUT" if path.ends_with(".png") => {
                    if tries.fetch_add(1, Ordering::SeqCst) < 2 {
                        (503, vec![], b"busy".to_vec())
                    } else {
                        ok()
                    }
                }
                "PUT" if path.ends_with(".webp") => (
                    400,
                    vec![],
                    br#"{"error":{"code":"bad_path","message":"That isn't a file of the tree."}}"#
                        .to_vec(),
                ),
                _ => ok(),
            }
        });
        let key = DeviceKey::generate().unwrap();
        let mut mirror = Mirror::new(&base, &base, key, quick()).unwrap();
        let started = std::time::Instant::now();
        let summary = run(&mut mirror, &tree).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "Retry-After is held to the longest wait"
        );
        assert_eq!(summary.there, 1);
        assert_eq!(summary.uploaded, [PICTURE]);
        assert_eq!(
            summary.failed,
            [(
                THUMB.to_string(),
                "That isn't a file of the tree. (400)".to_string()
            )]
        );
        assert!(
            !summary.head,
            "head.json waits until everything else is there"
        );
        assert_eq!(picture_tries.load(Ordering::SeqCst), 3);
        let seen = seen.lock().unwrap();
        assert!(!seen.iter().any(|r| r.path.ends_with("head.json")));
        let thumb_puts = seen
            .iter()
            .filter(|r| r.method == "PUT" && r.path.ends_with(".webp"))
            .count();
        assert_eq!(thumb_puts, 1, "a 400 isn't tried again");
        // Every try was signed later than the one before it.
        let times: Vec<u64> = seen
            .iter()
            .filter(|r| r.method == "PUT" && r.path.ends_with(".png"))
            .map(|r| r.header(sign::TIME_HEADER).unwrap().parse().unwrap())
            .collect();
        assert!(times.windows(2).all(|w| w[1] > w[0]), "{times:?}");
    }

    #[test]
    fn a_failure_that_keeps_on_uses_up_the_tries_and_says_so() {
        let tree = tree("gone");
        let (base, seen) = serve(|r| match r.method.as_str() {
            "HEAD" => (404, vec![], vec![]),
            _ => (502, vec![], b"bad gateway".to_vec()),
        });
        let mut mirror =
            Mirror::new(&base, &base, DeviceKey::generate().unwrap(), quick()).unwrap();
        let summary = run(&mut mirror, &tree).unwrap();
        assert_eq!(summary.failed.len(), 3);
        assert!(summary
            .failed
            .iter()
            .all(|(_, why)| why == "the service answered 502"));
        assert!(!summary.head);
        let puts = seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.method == "PUT")
            .count();
        assert_eq!(puts, 3 * 3, "three tries for each of three files");
    }

    #[test]
    fn a_clock_that_is_out_is_set_by_the_service_and_signed_again() {
        let tree = Tree::new("clock");
        tree.put(HEAD_PATH, b"{}");
        let theirs = local_now() + 3600;
        let turned = Arc::new(AtomicUsize::new(0));
        let once = Arc::clone(&turned);
        let (base, seen) = serve(move |r| {
            if r.method == "PUT" && once.fetch_add(1, Ordering::SeqCst) == 0 {
                let body = br#"{"error":{"code":"clock","message":"Your clock is out."}}"#;
                (
                    401,
                    vec![(sign::SERVER_TIME_HEADER.into(), theirs.to_string())],
                    body.to_vec(),
                )
            } else {
                ok()
            }
        });
        let mut mirror =
            Mirror::new(&base, &base, DeviceKey::generate().unwrap(), quick()).unwrap();
        let summary = run(&mut mirror, &tree).unwrap();
        assert!(summary.head, "{summary:?}");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        let second: u64 = seen[1].header(sign::TIME_HEADER).unwrap().parse().unwrap();
        assert!(second >= theirs, "signed by the service's clock: {second}");
    }

    #[test]
    fn a_tree_without_head_json_or_a_bad_address_is_refused() {
        let tree = Tree::new("no-head");
        tree.put(CATALOG, b"a catalog");
        let key = || DeviceKey::generate().unwrap();
        let mut mirror =
            Mirror::new("http://127.0.0.1:9", "http://127.0.0.1:9", key(), quick()).unwrap();
        let err = run(&mut mirror, &tree).unwrap_err();
        assert!(err.contains("there's no v2/head.json"), "{err}");
        let missing = Tree::new("no-tree");
        assert!(run(&mut mirror, &missing)
            .unwrap_err()
            .contains("couldn't read"));

        for (public, api) in [
            ("http://packs.example.org", "https://community.example.org"),
            (
                "https://packs.example.org/?x=1",
                "https://community.example.org",
            ),
            ("https://packs.example.org", "ftp://community.example.org"),
            (
                "https://user@packs.example.org",
                "https://community.example.org",
            ),
        ] {
            let err = Mirror::new(public, api, key(), quick())
                .map(drop)
                .unwrap_err();
            assert!(err.contains("has to be an https:// address"), "{err}");
        }
    }

    #[test]
    fn a_name_the_tree_never_uses_is_turned_down_before_anything_is_sent() {
        let tree = tree("odd-name");
        tree.put("v2/pictures/a picture.png", b"a picture");
        let err = tree_files(&tree.0).unwrap_err();
        assert!(err.contains("isn't a name the tree uses"), "{err}");
        let tidy = self::tree("tidy");
        assert_eq!(
            tree_files(&tidy.0).unwrap(),
            [CATALOG, HEAD_PATH, PICTURE, THUMB],
            "sorted, without the dotfile"
        );
    }
}
