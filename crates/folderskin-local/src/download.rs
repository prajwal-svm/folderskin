//! Downloads that survive a dropped connection: a file comes down into `<name>.part`, a second
//! run asks for the rest with a `Range` header, and the whole file is checked against its
//! published size and SHA-256 once, leaving `<name>.ok` beside it so it is never checked again.

use crate::event::{Event, Level, Reporter, Stage};
use crate::{CancelToken, Error};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A file to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub url: String,
    pub size: u64,
    /// The published SHA-256, lower-case hex; `None` checks the size only.
    pub sha256: Option<String>,
}

/// The HTTP client for downloads: no overall time limit (a model is gigabytes), but one on
/// connecting and on silence, so a stalled connection fails instead of hanging.
pub fn client() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .user_agent(concat!("folderskin-local/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| Error::bug("The downloader couldn't start.", e.to_string()))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

/// True when `dest` is already downloaded and checked.
pub fn is_done(dest: &Path, size: u64) -> bool {
    file_size(dest) == Some(size) && with_suffix(dest, ".ok").is_file()
}

/// How many of `size` bytes [`fetch`] still has to download for `dest`: none once the whole
/// file is there (checked or not), and what an interrupted download hasn't brought yet.
pub fn remaining(dest: &Path, size: u64) -> u64 {
    if file_size(dest) == Some(size) {
        return 0;
    }
    let have = file_size(&with_suffix(dest, ".part")).unwrap_or(0);
    if have > size {
        size // too long to be this file: it starts again
    } else {
        size - have
    }
}

/// Downloads `remote` to `dest`, resuming a partial download, and checks its size and hash once.
pub async fn fetch(
    client: &reqwest::Client,
    remote: &Remote,
    dest: &Path,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if is_done(dest, remote.size) {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io("make the download folder", dir, &e))?;
    }
    let part = if file_size(dest) == Some(remote.size) {
        dest.to_path_buf() // downloaded earlier, never checked
    } else {
        let part = with_suffix(dest, ".part");
        download(client, remote, &part, &name, reporter, cancel).await?;
        part
    };

    if let Some(want) = &remote.sha256 {
        reporter.stage(Stage::Verify, format!("Checking {name}"));
        let path = part.clone();
        let cancelled = cancel.clone();
        let got = tokio::task::spawn_blocking(move || sha256_file(&path, &cancelled))
            .await
            .map_err(|e| Error::bug("The download check stopped unexpectedly.", e.to_string()))?;
        // Stopped part-way through the check is a cancel, not a file that can't be read.
        cancel.check()?;
        let got = got.map_err(|e| Error::io("check the download", &part, &e))?;
        if !got.eq_ignore_ascii_case(want) {
            let _ = std::fs::remove_file(&part);
            return Err(Error::fixable(
                "hash_mismatch",
                format!("{name} didn't download correctly."),
                format!(
                    "Its SHA-256 is {got}, not the published {want}, so it was deleted rather \
                     than used."
                ),
            )
            .fix("Run the same command again to download it afresh.")
            .fix("If it happens twice, your network may be changing downloads; try another one."));
        }
    }
    if part != dest {
        std::fs::rename(&part, dest)
            .map_err(|e| Error::io("put the download in place", dest, &e))?;
    }
    let ok = with_suffix(dest, ".ok");
    std::fs::write(&ok, remote.sha256.as_deref().unwrap_or("size"))
        .map_err(|e| Error::io("record the finished download", &ok, &e))?;
    Ok(())
}

/// Brings `part` up to `remote.size` bytes, asking only for what is missing.
async fn download(
    client: &reqwest::Client,
    remote: &Remote,
    part: &Path,
    name: &str,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    let mut have = file_size(part).unwrap_or(0);
    if have > remote.size {
        let _ = std::fs::remove_file(part);
        have = 0;
    }
    if have < remote.size {
        cancel.check()?;
        reporter.stage(
            Stage::Download,
            if have > 0 {
                format!("Resuming {name}")
            } else {
                format!("Downloading {name}")
            },
        );
        let mut request = client.get(&remote.url);
        if have > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let mut response = request
            .send()
            .await
            .map_err(|e| unreachable_error(name, &remote.url, &e))?;
        let status = response.status();
        let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
        if !status.is_success() {
            return Err(status_error(name, &remote.url, status.as_u16()));
        }
        if have > 0 && !resumed {
            // The server sent the whole file instead of the rest: start again from its first byte.
            reporter.log(
                Level::Info,
                format!("{name}: the server can't resume, so it starts from the beginning"),
            );
            have = 0;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(part)
            .map_err(|e| Error::io("save the download", part, &e))?;
        let mut done = have;
        let mut last = Instant::now();
        reporter.emit(Event::Download {
            file: name.to_string(),
            done,
            total: remote.size,
        });
        loop {
            if cancel.is_cancelled() {
                let _ = file.flush();
                return Err(Error::cancelled());
            }
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(e) => {
                    let _ = file.flush();
                    return Err(interrupted(name, done, remote.size, &e));
                }
            };
            file.write_all(&chunk)
                .map_err(|e| Error::io("save the download", part, &e))?;
            done += chunk.len() as u64;
            if last.elapsed() >= Duration::from_millis(500) {
                last = Instant::now();
                reporter.emit(Event::Download {
                    file: name.to_string(),
                    done,
                    total: remote.size,
                });
            }
        }
        file.flush()
            .map_err(|e| Error::io("save the download", part, &e))?;
        reporter.emit(Event::Download {
            file: name.to_string(),
            done,
            total: remote.size,
        });
    }
    let got = file_size(part).unwrap_or(0);
    if got != remote.size {
        return Err(Error::fixable(
            "download_incomplete",
            format!("{name} didn't finish downloading."),
            format!(
                "It came down as {got} bytes, not {}. What arrived is kept.",
                remote.size
            ),
        )
        .fix("Run the same command again: it carries on from where it stopped."));
    }
    Ok(())
}

fn unreachable_error(name: &str, url: &str, e: &reqwest::Error) -> Error {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string());
    Error::environment(
        "download_failed",
        format!("Couldn't download {name}."),
        format!("{host} couldn't be reached: {}.", root_cause(e)),
    )
    .fix("Check the internet connection (and any proxy or firewall), then run the command again.")
    .fix("Whatever was already downloaded is kept; the next run carries on from there.")
}

fn status_error(name: &str, url: &str, status: u16) -> Error {
    let why = match status {
        404 | 410 => format!("It is no longer at {url} (HTTP {status})."),
        401 | 403 => format!("The server refused it (HTTP {status})."),
        429 => "The server is limiting downloads right now (HTTP 429).".to_string(),
        _ => format!("The server answered HTTP {status} for {url}."),
    };
    let error = Error::environment("download_failed", format!("Couldn't download {name}."), why);
    match status {
        404 | 410 => error.fix("Update folderskin: a newer release knows where the file moved."),
        429 => error.fix("Wait a few minutes, then run the command again."),
        _ => error.fix("Run the command again in a little while."),
    }
}

fn interrupted(name: &str, done: u64, total: u64, e: &reqwest::Error) -> Error {
    Error::environment(
        "download_interrupted",
        format!("The download of {name} stopped part-way."),
        format!(
            "{:.2} of {:.2} GB had arrived when the connection dropped: {}.",
            done as f64 / 1e9,
            total as f64 / 1e9,
            root_cause(e)
        ),
    )
    .fix("Run the same command again: it carries on from where it stopped.")
}

/// The innermost cause of a reqwest error, which says what actually went wrong.
fn root_cause(e: &reqwest::Error) -> String {
    let mut source: &dyn std::error::Error = e;
    while let Some(next) = source.source() {
        source = next;
    }
    let text = source.to_string();
    if e.is_timeout() {
        format!("it stopped answering ({text})")
    } else {
        text
    }
}

/// The SHA-256 of a file, in lower-case hex, read in 16 MB pieces. Cancelling stops it with an
/// [`std::io::ErrorKind::Interrupted`] error.
pub fn sha256_file(path: &Path, cancel: &CancelToken) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 16 << 20];
    loop {
        if cancel.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "the check was cancelled",
            ));
        }
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// How the test server answers.
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        /// Honours `Range` with 206 Partial Content.
        Ranges,
        /// Ignores `Range` and always sends the whole file with 200.
        Whole,
        /// Honours `Range`, but hangs up after this many bytes of each answer.
        CutAfter(usize),
    }

    struct Server {
        base: String,
        requests: Arc<AtomicUsize>,
        ranges: Arc<Mutex<Vec<String>>>,
    }

    /// Serves `files` over HTTP on a free local port, the way `mode` says.
    fn serve(files: Vec<(&str, Vec<u8>)>, mode: Mode) -> Server {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let files: Arc<HashMap<String, Vec<u8>>> =
            Arc::new(files.into_iter().map(|(p, b)| (p.to_string(), b)).collect());
        let requests = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let (count, seen) = (requests.clone(), ranges.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (files, count, seen) = (files.clone(), count.clone(), seen.clone());
                std::thread::spawn(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                    let mut reader = BufReader::new(&stream);
                    let mut request = String::new();
                    let _ = reader.read_line(&mut request);
                    let mut range = None;
                    let mut header = String::new();
                    while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                        if let Some(v) = header.to_ascii_lowercase().strip_prefix("range: bytes=") {
                            range = v.trim().trim_end_matches('-').parse::<usize>().ok();
                            seen.lock()
                                .unwrap()
                                .push(header.trim().to_ascii_lowercase());
                        }
                        header.clear();
                    }
                    let path = request.split(' ').nth(1).unwrap_or("").to_string();
                    let mut stream = &stream;
                    let Some(body) = files.get(&path) else {
                        let _ = stream.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                        return;
                    };
                    let (status, from) = match (mode, range) {
                        (Mode::Whole, _) | (_, None) => ("200 OK", 0),
                        (_, Some(start)) => ("206 Partial Content", start.min(body.len())),
                    };
                    let rest = &body[from..];
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        rest.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let sent = match mode {
                        Mode::CutAfter(n) => &rest[..n.min(rest.len())],
                        _ => rest,
                    };
                    let _ = stream.write_all(sent);
                });
            }
        });
        Server {
            base,
            requests,
            ranges,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-download-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 % 251) as u8).collect()
    }

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn run<T>(fut: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(fut)
    }

    fn get(
        server: &Server,
        file: &str,
        bytes: &[u8],
        sha256: Option<String>,
        dest: &Path,
    ) -> Result<(), Error> {
        let remote = Remote {
            url: format!("{}/{file}", server.base),
            size: bytes.len() as u64,
            sha256,
        };
        run(async {
            fetch(
                &client().unwrap(),
                &remote,
                dest,
                &Reporter::silent(),
                &CancelToken::new(),
            )
            .await
        })
    }

    #[test]
    fn a_file_comes_down_is_checked_and_is_not_fetched_twice() {
        let data = body(300_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("fresh");
        let dest = dir.join("m.gguf");
        get(&server, "m.gguf", &data, Some(sha(&data)), &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(
            std::fs::read_to_string(dir.join("m.gguf.ok")).unwrap(),
            sha(&data)
        );
        assert!(!dir.join("m.gguf.part").exists());
        assert!(is_done(&dest, data.len() as u64));

        get(&server, "m.gguf", &data, Some(sha(&data)), &dest).unwrap();
        assert_eq!(
            server.requests.load(Ordering::SeqCst),
            1,
            "the .ok marker is trusted"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_partial_download_asks_for_the_rest_and_resumes_on_206() {
        let data = body(200_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("resume");
        std::fs::write(dir.join("m.gguf.part"), &data[..70_000]).unwrap();
        get(
            &server,
            "m.gguf",
            &data,
            Some(sha(&data)),
            &dir.join("m.gguf"),
        )
        .unwrap();
        assert_eq!(std::fs::read(dir.join("m.gguf")).unwrap(), data);
        assert_eq!(*server.ranges.lock().unwrap(), ["range: bytes=70000-"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_server_that_answers_200_to_a_range_starts_the_file_again() {
        let data = body(120_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Whole);
        let dir = temp_dir("whole");
        // Rubbish in the partial file proves it is thrown away, not appended to.
        std::fs::write(dir.join("m.gguf.part"), vec![7u8; 50_000]).unwrap();
        get(
            &server,
            "m.gguf",
            &data,
            Some(sha(&data)),
            &dir.join("m.gguf"),
        )
        .unwrap();
        assert_eq!(std::fs::read(dir.join("m.gguf")).unwrap(), data);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_dropped_connection_keeps_what_arrived_for_the_next_run() {
        let data = body(150_000);
        let dir = temp_dir("dropped");
        let dest = dir.join("m.gguf");
        let cut = serve(vec![("/m.gguf", data.clone())], Mode::CutAfter(60_000));
        let err = get(&cut, "m.gguf", &data, Some(sha(&data)), &dest).unwrap_err();
        assert!(
            matches!(err.code, "download_incomplete" | "download_interrupted"),
            "{err:?}"
        );
        assert!(err.fix[0].contains("carries on"), "{err:?}");
        assert_eq!(
            std::fs::metadata(dir.join("m.gguf.part")).unwrap().len(),
            60_000
        );

        let whole = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        get(&whole, "m.gguf", &data, Some(sha(&data)), &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(*whole.ranges.lock().unwrap(), ["range: bytes=60000-"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_doesnt_match_its_hash_is_deleted() {
        let data = body(80_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("hash");
        let err = get(
            &server,
            "m.gguf",
            &data,
            Some(sha(b"something else")),
            &dir.join("m.gguf"),
        )
        .unwrap_err();
        assert_eq!(err.code, "hash_mismatch");
        assert!(err.what.contains("m.gguf"), "{err:?}");
        assert!(!dir.join("m.gguf").exists() && !dir.join("m.gguf.part").exists());
        assert!(!dir.join("m.gguf.ok").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_complete_file_without_its_marker_is_checked_without_downloading() {
        let data = body(90_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("unchecked");
        std::fs::write(dir.join("m.gguf"), &data).unwrap();
        get(
            &server,
            "m.gguf",
            &data,
            Some(sha(&data)),
            &dir.join("m.gguf"),
        )
        .unwrap();
        assert_eq!(server.requests.load(Ordering::SeqCst), 0);
        assert!(dir.join("m.gguf.ok").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_says_where_it_was_looked_for() {
        let server = serve(vec![], Mode::Ranges);
        let dir = temp_dir("missing");
        let err = get(
            &server,
            "gone.gguf",
            &body(10),
            None,
            &dir.join("gone.gguf"),
        )
        .unwrap_err();
        assert_eq!(err.code, "download_failed");
        assert!(err.why.contains("HTTP 404"), "{err:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nothing_listening_is_a_network_problem_not_a_bug() {
        let dir = temp_dir("refused");
        // A port that was free a moment ago, so nothing answers there.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let remote = Remote {
            url: format!("http://127.0.0.1:{port}/m.gguf"),
            size: 10,
            sha256: None,
        };
        let err = run(fetch(
            &client().unwrap(),
            &remote,
            &dir.join("m.gguf"),
            &Reporter::silent(),
            &CancelToken::new(),
        ))
        .unwrap_err();
        assert_eq!(err.code, "download_failed");
        assert_eq!(err.class, crate::Class::Environment);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_cancelled_download_stops_and_keeps_its_part() {
        let data = body(50_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("cancel");
        let cancel = CancelToken::new();
        cancel.cancel();
        let remote = Remote {
            url: format!("{}/m.gguf", server.base),
            size: data.len() as u64,
            sha256: None,
        };
        let err = run(fetch(
            &client().unwrap(),
            &remote,
            &dir.join("m.gguf"),
            &Reporter::silent(),
            &cancel,
        ))
        .unwrap_err();
        assert!(err.is_cancelled());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancelling_while_a_download_is_checked_is_a_cancel_not_a_read_error() {
        // A whole file without its marker is only checked, which a cancel interrupts.
        let data = body(70_000);
        let dir = temp_dir("cancel-check");
        let dest = dir.join("m.gguf");
        std::fs::write(&dest, &data).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        let remote = Remote {
            url: "http://127.0.0.1:9/m.gguf".into(),
            size: data.len() as u64,
            sha256: Some(sha(&data)),
        };
        let err = run(fetch(
            &client().unwrap(),
            &remote,
            &dest,
            &Reporter::silent(),
            &cancel,
        ))
        .unwrap_err();
        assert!(err.is_cancelled(), "{err:?}");
        assert!(dest.is_file(), "the file is kept for the next run");
        assert!(!dir.join("m.gguf.ok").exists());
        let e = sha256_file(&dest, &cancel).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::Interrupted);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_is_left_to_download_counts_what_already_arrived() {
        let dir = temp_dir("remaining");
        let dest = dir.join("m.gguf");
        assert_eq!(remaining(&dest, 1000), 1000, "nothing here yet");
        std::fs::write(dir.join("m.gguf.part"), vec![0u8; 400]).unwrap();
        assert_eq!(
            remaining(&dest, 1000),
            600,
            "an interrupted download carries on"
        );
        std::fs::write(dir.join("m.gguf.part"), vec![0u8; 1200]).unwrap();
        assert_eq!(remaining(&dest, 1000), 1000, "too long: it starts again");
        std::fs::write(&dest, vec![0u8; 1000]).unwrap();
        assert_eq!(remaining(&dest, 1000), 0, "whole, if not yet checked");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
