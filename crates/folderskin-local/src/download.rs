//! Downloads that survive a dropped connection: a file comes down into `<name>.part`, a second
//! run asks for the rest with a `Range` header, and the whole file is checked against its
//! published size and SHA-256 once, leaving `<name>.ok` beside it so it is never checked again.
//!
//! A big file comes down in pieces over several connections at once, each piece recorded in
//! `<name>.part.pieces` as it is finished, so a later run asks only for the pieces still missing.
//! One connection to a CDN on another continent carries a fraction of what the line can.

use crate::event::{Event, Level, Reporter, Stage};
use crate::{CancelToken, Error};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A file to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub url: String,
    pub size: u64,
    /// The published SHA-256, lower-case hex; `None` checks the size only.
    pub sha256: Option<String>,
    /// How it is named in its progress and its errors; `None` names it by its file name.
    pub label: Option<String>,
}

/// The HTTP client for downloads: no overall time limit (a model is gigabytes), but one on
/// connecting and on silence, so a stalled connection fails instead of hanging. HTTP/1.1 only, so
/// the pieces of a file come down over connections of their own: over HTTP/2 a CDN would carry
/// them all on one, no faster than asking for the file whole.
pub fn client() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .user_agent(concat!("folderskin-local/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .http1_only()
        .build()
        .map_err(|e| Error::bug("The downloader couldn't start.", e.to_string()))
}

/// How a big file is split up to come down over several connections.
#[derive(Clone, Copy, Debug)]
struct Plan {
    /// A file of two pieces or more comes down in pieces; a smaller one in one go.
    piece: u64,
    /// How many connections the pieces share.
    connections: usize,
    /// How many times a piece whose connection drops is asked for again, from where it got to.
    tries: u32,
    /// The wait before the first of those, doubling each time.
    backoff: Duration,
}

/// Eight connections at once: from here to Hugging Face's CDN one carried 28 MB/s and eight 68.5
/// MB/s together, and the further away the CDN, the more they gain.
const PLAN: Plan = Plan {
    piece: 32 << 20,
    connections: 8,
    tries: 4,
    backoff: Duration::from_secs(2),
};

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
    remaining_with(dest, size, PLAN.piece)
}

/// [`remaining`], for a download in pieces of `piece` bytes.
fn remaining_with(dest: &Path, size: u64, piece: u64) -> u64 {
    if file_size(dest) == Some(size) {
        return 0;
    }
    let part = with_suffix(dest, ".part");
    if let Some(done) = pieces_done(&part, size, piece) {
        return (0..done.len())
            .filter(|&i| !done[i])
            .map(|i| piece_at(size, i, piece).1)
            .sum();
    }
    let have = file_size(&part).unwrap_or(0);
    if have > size {
        size // too long to be this file: it starts again
    } else {
        size - have
    }
}

/// Removes a download that has served its purpose, an archive once unpacked, with the marker
/// that says it was checked and any part left of it. Best effort: at worst it is downloaded again.
pub fn discard(dest: &Path) {
    for path in [
        dest.to_path_buf(),
        with_suffix(dest, ".ok"),
        with_suffix(dest, ".part"),
        with_suffix(dest, ".part.pieces"),
    ] {
        let _ = std::fs::remove_file(path);
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
    fetch_with(&PLAN, client, remote, dest, reporter, cancel).await
}

/// [`fetch`], with big files split up as `plan` says.
async fn fetch_with(
    plan: &Plan,
    client: &reqwest::Client,
    remote: &Remote,
    dest: &Path,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    let name = remote.label.clone().unwrap_or_else(|| {
        dest.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
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
        download(plan, client, remote, &part, &name, reporter, cancel).await?;
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
            let _ = std::fs::remove_file(record_path(&part));
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
        let _ = std::fs::remove_file(record_path(&part));
    }
    let ok = with_suffix(dest, ".ok");
    std::fs::write(&ok, remote.sha256.as_deref().unwrap_or("size"))
        .map_err(|e| Error::io("record the finished download", &ok, &e))?;
    Ok(())
}

/// Brings `part` up to `remote.size` bytes, asking only for what is missing: in pieces over
/// several connections when the file is big and the server sends pieces, in one go otherwise.
async fn download(
    plan: &Plan,
    client: &reqwest::Client,
    remote: &Remote,
    part: &Path,
    name: &str,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<(), Error> {
    if remote.size >= 2 * plan.piece {
        match download_pieces(plan, client, remote, part, name, reporter, cancel).await? {
            Pieces::Done => return Ok(()),
            Pieces::NotSent => reporter.log(
                Level::Info,
                format!("{name}: the server sends it whole only, so it comes down in one go"),
            ),
        }
    }
    // What a download in pieces left is no start for one in one go: its part is full length.
    if record_path(part).exists() {
        let _ = std::fs::remove_file(part);
        let _ = std::fs::remove_file(record_path(part));
    }
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

/// What became of a download in pieces.
enum Pieces {
    Done,
    /// The server sends the file whole only: it comes down in one go instead.
    NotSent,
}

/// How much of a piece is gathered before it is written.
const BUFFER: usize = 1 << 20;

/// Piece `i` of a file of `size` bytes cut in `piece`s: where it starts, and how long it is.
fn piece_at(size: u64, i: usize, piece: u64) -> (u64, u64) {
    let start = i as u64 * piece;
    (start, piece.min(size.saturating_sub(start)))
}

fn piece_count(size: u64, piece: u64) -> usize {
    size.div_ceil(piece) as usize
}

/// Where the pieces of `part` that are down are recorded.
fn record_path(part: &Path) -> PathBuf {
    with_suffix(part, ".pieces")
}

/// A record's first line: the size of piece it was made with.
fn record_head(piece: u64) -> String {
    format!("pieces of {piece} bytes")
}

/// Which pieces of `part` are down, from its record; `None` when there is no record to go by: none
/// at all, one made with pieces of another size, or one beside a part that isn't full length.
fn pieces_done(part: &Path, size: u64, piece: u64) -> Option<Vec<bool>> {
    let text = std::fs::read_to_string(record_path(part)).ok()?;
    let mut lines = text.lines();
    if lines.next()? != record_head(piece) || file_size(part) != Some(size) {
        return None;
    }
    let mut done = vec![false; piece_count(size, piece)];
    for i in lines.filter_map(|l| l.trim().parse::<usize>().ok()) {
        if let Some(d) = done.get_mut(i) {
            *d = true;
        }
    }
    Some(done)
}

/// The total size a `Content-Range: bytes 0-0/1234` header gives.
fn content_range_total(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .rsplit('/')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// Writes all of `buf` at `offset` in `file`, whatever else is being written elsewhere in it.
fn write_at(file: &std::fs::File, buf: &[u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.write_all_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut written = 0;
        while written < buf.len() {
            let n = file.seek_write(&buf[written..], offset + written as u64)?;
            if n == 0 {
                return Err(std::io::ErrorKind::WriteZero.into());
            }
            written += n;
        }
        Ok(())
    }
}

/// Downloads `remote` into `part` in pieces over several connections at once, as `plan` says,
/// carrying on from its record, or from the start a download in one go left.
async fn download_pieces(
    plan: &Plan,
    client: &reqwest::Client,
    remote: &Remote,
    part: &Path,
    name: &str,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> Result<Pieces, Error> {
    let size = remote.size;
    let record = record_path(part);
    let done = match pieces_done(part, size, plan.piece) {
        Some(done) => done,
        None => {
            // A record that can't be gone by goes, with the part it was about.
            if record.exists() {
                let _ = std::fs::remove_file(&record);
                let _ = std::fs::remove_file(part);
            }
            // What a download in one go brought: the pieces it holds whole are down.
            let have = match file_size(part) {
                Some(have) if have <= size => have,
                Some(_) => {
                    let _ = std::fs::remove_file(part);
                    0
                }
                None => 0,
            };
            (0..piece_count(size, plan.piece))
                .map(|i| {
                    let (start, len) = piece_at(size, i, plan.piece);
                    start + len <= have
                })
                .collect()
        }
    };
    let todo: VecDeque<usize> = (0..done.len()).filter(|&i| !done[i]).collect();
    if todo.is_empty() {
        return Ok(Pieces::Done);
    }
    cancel.check()?;
    reporter.stage(
        Stage::Download,
        if todo.len() < done.len() {
            format!("Resuming {name}")
        } else {
            format!("Downloading {name}")
        },
    );
    // Where the bytes are: the file's own address redirects there (Hugging Face to its CDN,
    // GitHub to its storage). Asked once, for the first byte, which also says whether the server
    // sends pieces at all.
    let probe = client
        .get(&remote.url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await
        .map_err(|e| unreachable_error(name, &remote.url, &e))?;
    let status = probe.status();
    if !status.is_success() {
        return Err(status_error(name, &remote.url, status.as_u16()));
    }
    if status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Ok(Pieces::NotSent);
    }
    if let Some(total) = content_range_total(probe.headers()).filter(|t| *t != size) {
        return Err(Error::environment(
            "download_failed",
            format!("Couldn't download {name}."),
            format!("The server has a file of {total} bytes there, not the {size} expected."),
        )
        .fix("Update folderskin: a newer release knows where the file is now."));
    }
    let at = probe.url().clone();
    drop(probe);

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(part)
        .map_err(|e| Error::io("save the download", part, &e))?;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&record)
        .map_err(|e| Error::io("save the download", &record, &e))?;
    // A new record says what it counts in, and which pieces a download in one go brought. It is
    // written before the part is made full length, so a part that is has a record beside it.
    if log.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        let mut head = record_head(plan.piece) + "\n";
        for i in (0..done.len()).filter(|&i| done[i]) {
            head += &format!("{i}\n");
        }
        log.write_all(head.as_bytes())
            .map_err(|e| Error::io("save the download", &record, &e))?;
    }
    file.set_len(size)
        .map_err(|e| Error::io("save the download", part, &e))?;

    let already = size
        - todo
            .iter()
            .map(|&i| piece_at(size, i, plan.piece).1)
            .sum::<u64>();
    let connections = plan.connections.min(todo.len()).max(1);
    let shared = Arc::new(Shared {
        file,
        log: Mutex::new(log),
        todo: Mutex::new(todo),
        at: Mutex::new(at),
        done: AtomicU64::new(already),
        told: Mutex::new(Instant::now()),
    });
    reporter.emit(Event::Download {
        file: name.to_string(),
        done: already,
        total: size,
    });
    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..connections {
        let worker = Worker {
            plan: *plan,
            client: client.clone(),
            remote: remote.clone(),
            part: part.to_path_buf(),
            name: name.to_string(),
            reporter: reporter.clone(),
            cancel: cancel.clone(),
            shared: shared.clone(),
        };
        workers.spawn(worker.run());
    }
    let mut failed = None;
    while let Some(ended) = workers.join_next().await {
        let outcome = match ended {
            Ok(outcome) => outcome,
            Err(e) if e.is_cancelled() => continue,
            Err(e) => Err(Error::bug(
                "A download stopped unexpectedly.",
                e.to_string(),
            )),
        };
        if let Err(e) = outcome {
            if failed.is_none() {
                // The others stop too; what they had of their pieces comes down again next time.
                workers.abort_all();
                failed = Some(e);
            }
        }
    }
    reporter.emit(Event::Download {
        file: name.to_string(),
        done: shared.done.load(Ordering::Relaxed),
        total: size,
    });
    match failed {
        Some(e) => Err(e),
        None => Ok(Pieces::Done),
    }
}

/// What the connections of a download in pieces share.
struct Shared {
    file: std::fs::File,
    /// The record, a line added as each piece is finished.
    log: Mutex<std::fs::File>,
    /// The pieces no connection has taken yet.
    todo: Mutex<VecDeque<usize>>,
    /// Where the bytes are: the address the file's own redirected to.
    at: Mutex<reqwest::Url>,
    /// Bytes of the file down so far, all pieces together.
    done: AtomicU64,
    /// When progress was last reported.
    told: Mutex<Instant>,
}

/// One connection of a download in pieces: it takes pieces until none are left.
struct Worker {
    plan: Plan,
    client: reqwest::Client,
    remote: Remote,
    part: PathBuf,
    name: String,
    reporter: Reporter,
    cancel: CancelToken,
    shared: Arc<Shared>,
}

/// Why one go at a piece ended early.
enum Attempt {
    /// The connection dropped, or couldn't be made: worth asking again.
    Dropped(String),
    /// Anything else, which stops the download.
    Failed(Error),
}

impl Worker {
    async fn run(self) -> Result<(), Error> {
        loop {
            let next = lock(&self.shared.todo).pop_front();
            let Some(i) = next else {
                return Ok(());
            };
            self.piece(i).await?;
            // Recorded once it is whole, so no later run asks for it again.
            writeln!(lock(&self.shared.log), "{i}")
                .map_err(|e| Error::io("record the download", &record_path(&self.part), &e))?;
        }
    }

    /// Brings piece `i` down, asking again from where it got to when its connection drops.
    async fn piece(&self, i: usize) -> Result<(), Error> {
        let (start, len) = piece_at(self.remote.size, i, self.plan.piece);
        let mut got = 0;
        let mut tries = 0;
        loop {
            match self.attempt(start + got, start + len - 1, &mut got).await {
                Ok(()) => return Ok(()),
                Err(Attempt::Failed(e)) => return Err(e),
                Err(Attempt::Dropped(why)) => {
                    tries += 1;
                    if tries > self.plan.tries {
                        return Err(interrupted_by(
                            &self.name,
                            self.shared.done.load(Ordering::Relaxed),
                            self.remote.size,
                            why,
                        ));
                    }
                    tokio::time::sleep(self.plan.backoff * 2u32.pow(tries - 1)).await;
                    self.cancel.check()?;
                }
            }
        }
    }

    /// One go at bytes `from` to `to` (both included), adding what it writes to `got`.
    async fn attempt(&self, from: u64, to: u64, got: &mut u64) -> Result<(), Attempt> {
        let at = lock(&self.shared.at).clone();
        let mut response = self.ask(at.as_str(), from, to).await?;
        // An address that has stopped working (GitHub's last minutes) is asked for afresh.
        if matches!(response.status().as_u16(), 401 | 403 | 410) && at.as_str() != self.remote.url {
            response = self.ask(&self.remote.url, from, to).await?;
            *lock(&self.shared.at) = response.url().clone();
        }
        let status = response.status();
        if !status.is_success() {
            return Err(Attempt::Failed(status_error(
                &self.name,
                &self.remote.url,
                status.as_u16(),
            )));
        }
        if status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(Attempt::Failed(
                Error::environment(
                    "download_failed",
                    format!("Couldn't download {}.", self.name),
                    "The server stopped sending it in pieces.",
                )
                .fix("Run the same command again: it carries on from where it stopped."),
            ));
        }
        let mut buf = Vec::with_capacity(BUFFER);
        let mut offset = from;
        let mut left = to + 1 - from;
        while left > 0 {
            if self.cancel.is_cancelled() {
                return Err(Attempt::Failed(Error::cancelled()));
            }
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    // A server that sends more than was asked for is only listened to that far.
                    let take = chunk.len().min(left as usize);
                    buf.extend_from_slice(&chunk[..take]);
                    left -= take as u64;
                    self.shared.done.fetch_add(take as u64, Ordering::Relaxed);
                    if buf.len() >= BUFFER {
                        self.write(&mut buf, &mut offset, got)?;
                    }
                    self.tell();
                }
                Ok(None) => break,
                Err(e) => {
                    // What arrived is kept: the piece carries on from there.
                    self.write(&mut buf, &mut offset, got)?;
                    return Err(Attempt::Dropped(root_cause(&e)));
                }
            }
        }
        self.write(&mut buf, &mut offset, got)?;
        if left > 0 {
            return Err(Attempt::Dropped(
                "the server closed the connection early".into(),
            ));
        }
        Ok(())
    }

    async fn ask(&self, url: &str, from: u64, to: u64) -> Result<reqwest::Response, Attempt> {
        self.client
            .get(url)
            .header(reqwest::header::RANGE, format!("bytes={from}-{to}"))
            .send()
            .await
            .map_err(|e| Attempt::Dropped(root_cause(&e)))
    }

    /// Writes what `buf` holds at `offset`, and moves both on.
    fn write(&self, buf: &mut Vec<u8>, offset: &mut u64, got: &mut u64) -> Result<(), Attempt> {
        if buf.is_empty() {
            return Ok(());
        }
        write_at(&self.shared.file, buf, *offset)
            .map_err(|e| Attempt::Failed(Error::io("save the download", &self.part, &e)))?;
        *offset += buf.len() as u64;
        *got += buf.len() as u64;
        buf.clear();
        Ok(())
    }

    /// Reports how far the whole file has got, twice a second at most, whichever connection asks.
    fn tell(&self) {
        let Ok(mut told) = self.shared.told.try_lock() else {
            return;
        };
        if told.elapsed() >= Duration::from_millis(500) {
            *told = Instant::now();
            self.reporter.emit(Event::Download {
                file: self.name.clone(),
                done: self.shared.done.load(Ordering::Relaxed),
                total: self.remote.size,
            });
        }
    }
}

/// `mutex`'s value, whether or not a connection panicked while it held it.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
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
    interrupted_by(name, done, total, root_cause(e))
}

fn interrupted_by(name: &str, done: u64, total: u64, why: String) -> Error {
    Error::environment(
        "download_interrupted",
        format!("The download of {name} stopped part-way."),
        format!(
            "{:.2} of {:.2} GB had arrived when the connection dropped: {why}.",
            done as f64 / 1e9,
            total as f64 / 1e9,
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
        /// Like a CDN: a file's address redirects to a signed one, `/signed/<n>/<file>`, which
        /// answers this many requests and then says 403.
        Expiring(usize),
    }

    struct Server {
        base: String,
        requests: Arc<AtomicUsize>,
        ranges: Arc<Mutex<Vec<String>>>,
        /// Expiring: how many requests each signed address has answered, and how many were made.
        signed: Arc<Mutex<(HashMap<String, usize>, usize)>>,
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
        let signed = Arc::new(Mutex::new((HashMap::<String, usize>::new(), 0usize)));
        let (count, seen, made) = (requests.clone(), ranges.clone(), signed.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (files, count, seen, signed) =
                    (files.clone(), count.clone(), seen.clone(), made.clone());
                std::thread::spawn(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                    let mut reader = BufReader::new(&stream);
                    let mut request = String::new();
                    let _ = reader.read_line(&mut request);
                    // "bytes=10-19" or "bytes=10-": where it starts, and where it ends if it says.
                    let mut range: Option<(usize, Option<usize>)> = None;
                    let mut header = String::new();
                    while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                        if let Some(v) = header.to_ascii_lowercase().strip_prefix("range: bytes=") {
                            let (start, end) = v.trim().split_once('-').unwrap_or((v.trim(), ""));
                            range = start
                                .parse::<usize>()
                                .ok()
                                .map(|s| (s, end.parse::<usize>().ok()));
                            seen.lock()
                                .unwrap()
                                .push(header.trim().to_ascii_lowercase());
                        }
                        header.clear();
                    }
                    let mut path = request.split(' ').nth(1).unwrap_or("").to_string();
                    let mut stream = &stream;
                    if let Mode::Expiring(uses) = mode {
                        let mut signed = signed.lock().unwrap();
                        if let Some(rest) = path.strip_prefix("/signed/") {
                            let (token, file) = rest.split_once('/').unwrap_or((rest, ""));
                            let used = signed.0.entry(token.to_string()).or_insert(0);
                            *used += 1;
                            if *used > uses {
                                let _ = stream.write_all(
                                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                );
                                return;
                            }
                            path = format!("/{file}");
                        } else {
                            signed.1 += 1;
                            let head = format!(
                                "HTTP/1.1 302 Found\r\nLocation: /signed/{}{path}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                signed.1
                            );
                            let _ = stream.write_all(head.as_bytes());
                            return;
                        }
                    }
                    let Some(body) = files.get(&path) else {
                        let _ = stream.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                        return;
                    };
                    let (status, from, to) = match (mode, range) {
                        (Mode::Whole, _) | (_, None) => ("200 OK", 0, body.len()),
                        (_, Some((start, end))) => (
                            "206 Partial Content",
                            start.min(body.len()),
                            end.map_or(body.len(), |e| (e + 1).min(body.len())),
                        ),
                    };
                    let rest = &body[from..to];
                    let content_range = if status.starts_with("206") {
                        format!(
                            "Content-Range: bytes {from}-{}/{}\r\n",
                            to.saturating_sub(1),
                            body.len()
                        )
                    } else {
                        String::new()
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\n{content_range}Content-Length: {}\r\nConnection: close\r\n\r\n",
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
            signed,
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

    #[test]
    fn a_discarded_download_leaves_nothing_behind_and_counts_as_to_come() {
        let dir = temp_dir("discard");
        let zip = dir.join("sd.zip");
        for path in [&zip, &with_suffix(&zip, ".ok"), &with_suffix(&zip, ".part")] {
            std::fs::write(path, b"x").unwrap();
        }
        std::fs::write(dir.join("other.zip"), b"x").unwrap();
        discard(&zip);
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["other.zip"]);
        assert_eq!(remaining(&zip, 10), 10);
        discard(&zip); // and again, with nothing there
        std::fs::remove_dir_all(&dir).unwrap();
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
            label: None,
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
            label: None,
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
            label: None,
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
            label: None,
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

    /// Pieces small enough for a test, four at a time, retried at once.
    const SMALL: Plan = Plan {
        piece: 16_384,
        connections: 4,
        tries: 3,
        backoff: Duration::from_millis(5),
    };

    fn get_in_pieces(server: &Server, bytes: &[u8], dest: &Path) -> Result<(), Error> {
        let remote = Remote {
            url: format!("{}/m.gguf", server.base),
            size: bytes.len() as u64,
            sha256: Some(sha(bytes)),
            label: None,
        };
        run(async {
            fetch_with(
                &SMALL,
                &client().unwrap(),
                &remote,
                dest,
                &Reporter::silent(),
                &CancelToken::new(),
            )
            .await
        })
    }

    /// The ranges the server was asked for, in order of where they start.
    fn asked(server: &Server) -> Vec<String> {
        let mut ranges = server.ranges.lock().unwrap().clone();
        ranges.sort_by_key(|r| {
            r.trim_start_matches("range: bytes=")
                .split('-')
                .next()
                .and_then(|n| n.parse::<u64>().ok())
        });
        ranges
    }

    /// The range for piece `i` of a file `size` bytes long, as a request header.
    fn piece_range(size: usize, i: usize) -> String {
        let start = i * SMALL.piece as usize;
        let end = (start + SMALL.piece as usize).min(size) - 1;
        format!("range: bytes={start}-{end}")
    }

    #[test]
    fn a_big_file_comes_down_in_pieces_over_several_connections() {
        let data = body(100_000); // six pieces of 16 KB and one of 1,696 bytes
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("pieces");
        let dest = dir.join("m.gguf");
        get_in_pieces(&server, &data, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert!(is_done(&dest, data.len() as u64));
        assert!(!dir.join("m.gguf.part").exists());
        assert!(!dir.join("m.gguf.part.pieces").exists(), "the record goes");
        // The first byte, to learn where the pieces are and that they're sent, then each piece.
        let mut want = vec!["range: bytes=0-0".to_string()];
        want.extend((0..7).map(|i| piece_range(data.len(), i)));
        assert_eq!(asked(&server), want);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_download_in_pieces_carries_on_with_the_pieces_still_missing() {
        let data = body(100_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("pieces-resume");
        // Pieces 0 and 2 came down before; the rest of the part is rubbish.
        let mut part = vec![9u8; data.len()];
        for i in [0, 2] {
            let (start, len) = piece_at(data.len() as u64, i, SMALL.piece);
            let range = start as usize..(start + len) as usize;
            part[range.clone()].copy_from_slice(&data[range]);
        }
        std::fs::write(dir.join("m.gguf.part"), &part).unwrap();
        std::fs::write(
            dir.join("m.gguf.part.pieces"),
            "pieces of 16384 bytes\n0\n2\n",
        )
        .unwrap();
        let dest = dir.join("m.gguf");
        assert_eq!(
            remaining_with(&dest, data.len() as u64, SMALL.piece),
            100_000 - 2 * 16_384
        );
        get_in_pieces(&server, &data, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        let mut want = vec!["range: bytes=0-0".to_string()];
        want.extend([1, 3, 4, 5, 6].map(|i| piece_range(data.len(), i)));
        assert_eq!(asked(&server), want, "only what was missing");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_a_download_in_one_go_brought_counts_as_pieces() {
        let data = body(100_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("pieces-from-one");
        // An earlier build's download, stopped 40,000 bytes in: pieces 0 and 1 are whole.
        std::fs::write(dir.join("m.gguf.part"), &data[..40_000]).unwrap();
        get_in_pieces(&server, &data, &dir.join("m.gguf")).unwrap();
        assert_eq!(std::fs::read(dir.join("m.gguf")).unwrap(), data);
        let mut want = vec!["range: bytes=0-0".to_string()];
        want.extend((2..7).map(|i| piece_range(data.len(), i)));
        assert_eq!(asked(&server), want);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_server_that_sends_files_whole_is_asked_for_it_in_one_go() {
        let data = body(100_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Whole);
        let dir = temp_dir("pieces-whole");
        // What a download in pieces left, which is no start for one in one go.
        std::fs::write(dir.join("m.gguf.part"), vec![7u8; data.len()]).unwrap();
        std::fs::write(dir.join("m.gguf.part.pieces"), "pieces of 16384 bytes\n0\n").unwrap();
        get_in_pieces(&server, &data, &dir.join("m.gguf")).unwrap();
        assert_eq!(std::fs::read(dir.join("m.gguf")).unwrap(), data);
        assert!(!dir.join("m.gguf.part.pieces").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_piece_whose_connection_drops_carries_on_from_where_it_got_to() {
        let data = body(100_000);
        // Every answer stops 10,000 bytes in: each piece comes in two goes.
        let server = serve(vec![("/m.gguf", data.clone())], Mode::CutAfter(10_000));
        let dir = temp_dir("pieces-dropped");
        get_in_pieces(&server, &data, &dir.join("m.gguf")).unwrap();
        assert_eq!(std::fs::read(dir.join("m.gguf")).unwrap(), data);
        let asked = asked(&server);
        assert!(
            asked.contains(&"range: bytes=10000-16383".to_string()),
            "the first piece's rest: {asked:?}"
        );
        // Each piece is asked for from its start once, then for what the cut left.
        for i in 0..7 {
            let whole = piece_range(data.len(), i);
            assert_eq!(
                asked.iter().filter(|r| **r == whole).count(),
                1,
                "{whole}: {asked:?}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_piece_that_never_comes_stops_the_download_and_keeps_the_rest() {
        let data = body(100_000);
        // Nothing but the first 500 bytes of any answer: the probe works, no piece ever finishes.
        let server = serve(vec![("/m.gguf", data.clone())], Mode::CutAfter(500));
        let dir = temp_dir("pieces-never");
        let err = get_in_pieces(&server, &data, &dir.join("m.gguf")).unwrap_err();
        assert_eq!(err.code, "download_interrupted", "{err:?}");
        assert!(err.fix[0].contains("carries on"), "{err:?}");
        // The part and its record stay for the next run.
        assert!(dir.join("m.gguf.part.pieces").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_address_that_stops_working_is_asked_for_afresh() {
        let data = body(100_000);
        // Signed addresses that answer three requests each, like GitHub's that last minutes.
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Expiring(3));
        let dir = temp_dir("pieces-expiring");
        get_in_pieces(&server, &data, &dir.join("m.gguf")).unwrap();
        assert_eq!(std::fs::read(dir.join("m.gguf")).unwrap(), data);
        // Eight requests, three to an address: the file's own was asked for new ones.
        let made = server.signed.lock().unwrap().1;
        assert!(made >= 3, "{made} signed addresses");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_cancelled_download_in_pieces_stops() {
        let data = body(100_000);
        let server = serve(vec![("/m.gguf", data.clone())], Mode::Ranges);
        let dir = temp_dir("pieces-cancel");
        let cancel = CancelToken::new();
        cancel.cancel();
        let remote = Remote {
            url: format!("{}/m.gguf", server.base),
            size: data.len() as u64,
            sha256: None,
            label: None,
        };
        let err = run(fetch_with(
            &SMALL,
            &client().unwrap(),
            &remote,
            &dir.join("m.gguf"),
            &Reporter::silent(),
            &cancel,
        ))
        .unwrap_err();
        assert!(err.is_cancelled(), "{err:?}");
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
