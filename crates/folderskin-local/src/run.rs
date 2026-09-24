//! Running a runtime: its output read as it comes, turned into events, and the process ended
//! when the job is cancelled.

use crate::event::{Level, Reporter};
use crate::progress::{Line, OutputParser, Splitter};
use crate::CancelToken;
use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// How many of the runtime's last log lines are kept for an error report.
const TAIL: usize = 15;

/// How a run ended.
#[derive(Debug)]
pub struct Finished {
    /// `None` when it was cancelled and ended by us.
    pub status: Option<ExitStatus>,
    /// Its last log lines, oldest first, for saying why it failed.
    pub tail: Vec<String>,
    /// Whether it got as far as painting.
    pub painted: bool,
}

/// The runtimes running now, by process id, for [`end_all`]. An id leaves the list before its
/// process is reaped, and a process's id can only go to another once it has been reaped, so an id
/// here is always one of ours.
static RUNNING: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Set by [`end_all`]: a runtime started after it is ended as soon as it is listed.
static ENDED: AtomicBool = AtomicBool::new(false);

fn running() -> MutexGuard<'static, Vec<u32>> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runtimes that were stopped and are still ending. Ending can take seconds after the stop: on
/// Windows sd-cli is gone only once the system has let go of the gigabytes it was streaming from
/// the disk and of the graphics card, measured at 6 to 12 s. A stopped run doesn't wait for that
/// (the stop is answered at once); the next run does, before it starts, so it never finds the
/// graphics card still full.
struct Ending {
    count: Mutex<usize>,
    gone: Condvar,
}

static ENDING: Ending = Ending {
    count: Mutex::new(0),
    gone: Condvar::new(),
};

fn ending() -> MutexGuard<'static, usize> {
    ENDING.count.lock().unwrap_or_else(|e| e.into_inner())
}

/// Waits until every runtime stopped earlier has ended, or `cancel` is set. Says whether it was.
pub(crate) fn wait_for_the_stopped(cancel: &CancelToken) -> bool {
    let mut count = ending();
    while *count > 0 {
        if cancel.is_cancelled() {
            return true;
        }
        count = match ENDING.gone.wait_timeout(count, Duration::from_millis(100)) {
            Ok((count, _)) => count,
            Err(e) => e.into_inner().0,
        };
    }
    cancel.is_cancelled()
}

/// Lets `child`, which has been told to end, finish ending on a thread of its own, and reaps it.
fn reap_later(mut child: Child, listed: Listed) {
    *ending() += 1;
    std::thread::spawn(move || {
        // Off the list before it is reaped: after that its id can be another process's.
        drop(listed);
        let _ = child.wait();
        *ending() -= 1;
        ENDING.gone.notify_all();
    });
}

/// A runtime on the list, for as long as this is kept.
struct Listed(u32);

impl Listed {
    /// Lists `pid`, and says whether everything is being ended, in which case it is too.
    fn new(pid: u32) -> (Listed, bool) {
        let mut list = running();
        list.push(pid);
        (Listed(pid), ENDED.load(Ordering::SeqCst))
    }
}

impl Drop for Listed {
    fn drop(&mut self) {
        running().retain(|p| *p != self.0);
    }
}

/// Ends every runtime still running, and any started from now on. For a program on its way out:
/// a painting (or mflux's install) doesn't end with the program that started it on macOS and
/// Linux, and would go on holding gigabytes of memory and the graphics card for minutes. On
/// Windows the job object (see `job` below) already ends them when the program exits.
pub fn end_all() {
    let list = running();
    ENDED.store(true, Ordering::SeqCst);
    for &pid in list.iter() {
        kill(pid);
    }
}

#[cfg(unix)]
fn kill(pid: u32) {
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: kill(2) takes any pid; one on the list is ours and not yet reaped (see
        // RUNNING), so it can't be another process's.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn kill(_pid: u32) {}

/// Keeps a console window from opening for a child of the app, which has no console of its own.
pub(crate) fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Runs `cmd` to the end, reporting its progress, and ends it early if `cancel` is set: then it
/// returns as soon as the runtime has been told to end, and the runtime finishes ending on its own
/// (see [`ENDING`]). `steps` is how many painting steps the run takes. A run waits for runtimes
/// stopped earlier to have ended before it starts its own.
pub fn run(
    mut cmd: Command,
    steps: u32,
    reporter: &Reporter,
    cancel: &CancelToken,
) -> std::io::Result<Finished> {
    if wait_for_the_stopped(cancel) {
        return Ok(Finished {
            status: None,
            tail: Vec::new(),
            painted: false,
        });
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    let mut child = cmd.spawn()?;
    job::adopt(&child);
    let (listed, ending) = Listed::new(child.id());
    if ending {
        let _ = child.kill();
    }

    // Both streams are read on threads of their own, so neither fills up and blocks the runtime.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|mut stream| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        })
    })
    .collect();
    drop(tx);

    let mut parser = OutputParser::new(steps);
    let mut splitter = Splitter::default();
    let mut tail = VecDeque::with_capacity(TAIL);
    let mut handle = |piece: &str| match parser.line(piece) {
        Line::Event(e) => reporter.emit(e),
        Line::Log(text) => {
            reporter.log(Level::Debug, text.clone());
            if tail.len() == TAIL {
                tail.pop_front();
            }
            tail.push_back(text);
        }
        Line::Nothing => {}
    };

    let mut stopped = false;
    loop {
        // Stopped: the runtime is told to end, and the run is over. Its output isn't waited
        // for, nor is it: ending can take seconds, and a launcher (uv's, for mflux) can leave a
        // process of its own holding the output open after it is ended.
        if cancel.is_cancelled() {
            let _ = child.kill();
            stopped = true;
            break;
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => splitter.push(&bytes).iter().for_each(|p| handle(p)),
            // Both streams closed: the runtime is finishing.
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    if let Some(rest) = splitter.finish() {
        handle(&rest);
    }
    if stopped {
        reap_later(child, listed);
        return Ok(Finished {
            status: None,
            tail: tail.into(),
            painted: parser.painted(),
        });
    }
    for reader in readers {
        let _ = reader.join();
    }
    // Off the list before it is reaped: after that its id can be another process's.
    drop(listed);
    let status = child.wait()?;
    Ok(Finished {
        status: Some(status),
        tail: tail.into(),
        painted: parser.painted(),
    })
}

/// On Windows, every runtime goes into one job object that ends its processes when FolderSkin's
/// last handle to it closes, which happens when FolderSkin exits however it exits. A runtime
/// started without a window gets no Ctrl+C of its own, and would otherwise keep painting (and
/// holding the GPU) after the command line was stopped or the app quit.
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// The job's handle, as an integer so it can live in a static; 0 when it couldn't be made.
    fn job() -> isize {
        static JOB: OnceLock<isize> = OnceLock::new();
        *JOB.get_or_init(|| {
            // SAFETY: no security attributes and no name make an anonymous job; the handle is
            // checked before use and deliberately never closed (closing it ends the runtimes).
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return 0;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    return 0;
                }
                job as isize
            }
        })
    }

    pub fn adopt(child: &std::process::Child) {
        let job = job();
        if job != 0 {
            // SAFETY: both handles are live: the job's for the life of the process, the child's
            // for as long as `child` is borrowed. A failure only means it isn't tied to us.
            unsafe {
                AssignProcessToJobObject(job as _, child.as_raw_handle() as _);
            }
        }
    }
}

#[cfg(not(windows))]
mod job {
    /// A runtime is in our process group, so the terminal's Ctrl+C reaches it too.
    pub fn adopt(_child: &std::process::Child) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    /// A command that prints `script`'s output: PowerShell on Windows, sh elsewhere.
    fn shell(script_unix: &str, script_windows: &str) -> Command {
        if cfg!(windows) {
            let mut cmd = Command::new("powershell");
            cmd.args(["-NoProfile", "-Command", script_windows]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", script_unix]);
            cmd
        }
    }

    fn collect() -> (Reporter, Arc<Mutex<Vec<Event>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (Reporter::new(move |e| sink.lock().unwrap().push(e)), seen)
    }

    #[test]
    fn output_is_streamed_as_events_and_the_tail_is_kept() {
        let (reporter, seen) = collect();
        let cmd = shell(
            r"printf 'sampling using Euler\n\r  |==>  | 1/2 - 1.0s/it\r  |=====| 2/2 - 1.0s/it\n' ; echo oops >&2 ; exit 3",
            "Write-Output 'sampling using Euler'; [Console]::Out.Write([string][char]13 + '  |==>  | 1/2 - 1.0s/it' + [char]13 + '  |=====| 2/2 - 1.0s/it' + [char]10); [Console]::Error.WriteLine('oops'); exit 3",
        );
        let done = run(cmd, 2, &reporter, &CancelToken::new()).unwrap();
        assert_eq!(done.status.and_then(|s| s.code()), Some(3));
        assert!(done.painted);
        assert!(done.tail.iter().any(|l| l == "oops"), "{:?}", done.tail);
        let seen = seen.lock().unwrap();
        assert!(
            seen.contains(&Event::Progress { step: 2, steps: 2 }),
            "{seen:?}"
        );
    }

    #[test]
    fn a_stopped_run_ends_at_once_though_its_output_stays_open() {
        // What it starts keeps the output open long after the runtime itself is ended, as a
        // runtime that takes its time to let go does.
        let slow_to_go = shell(
            "sleep 30 & sleep 30",
            "Start-Process -NoNewWindow -FilePath ping -ArgumentList '-n','30','127.0.0.1'; Start-Sleep -Seconds 30",
        );
        let (reporter, _) = collect();
        let cancel = CancelToken::new();
        let later = cancel.clone();
        let asked = Arc::new(Mutex::new(None));
        let at = asked.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            *at.lock().unwrap() = Some(std::time::Instant::now());
            later.cancel();
        });
        let done = run(slow_to_go, 4, &reporter, &cancel).unwrap();
        let took = asked.lock().unwrap().expect("stopped").elapsed();
        assert!(done.status.is_none());
        assert!(took < Duration::from_secs(2), "{took:?} after the stop");

        // The next run starts once the stopped one has ended, and runs to the end.
        let (reporter, _) = collect();
        let next = run(
            shell("echo next", "Write-Output next"),
            4,
            &reporter,
            &CancelToken::new(),
        )
        .unwrap();
        assert_eq!(next.status.and_then(|s| s.code()), Some(0));
        assert_eq!(*ending(), 0);
    }

    #[test]
    fn a_cancelled_run_is_ended() {
        let (reporter, _) = collect();
        let cancel = CancelToken::new();
        let later = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            later.cancel();
        });
        let started = std::time::Instant::now();
        let cmd = shell("sleep 30", "Start-Sleep -Seconds 30");
        let done = run(cmd, 4, &reporter, &cancel).unwrap();
        assert!(done.status.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
    }
}
