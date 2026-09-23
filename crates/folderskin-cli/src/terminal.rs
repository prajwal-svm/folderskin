//! The two things the command line asks of the terminal itself: Ctrl+C that stops a job cleanly
//! (a download keeps what it has, a runtime is ended) rather than killing the process mid-write,
//! and typing an API key without it showing on screen.

use folderskin_local::CancelToken;
use std::io::{BufRead, IsTerminal, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static TOKEN: OnceLock<CancelToken> = OnceLock::new();
static PRESSED: AtomicBool = AtomicBool::new(false);

/// Makes the first Ctrl+C cancel the returned token; a second one ends the process as usual.
pub fn stop_on_ctrl_c() -> CancelToken {
    let token = TOKEN.get_or_init(CancelToken::new).clone();
    platform::install();
    token
}

/// Called from the signal handler: only atomics, nothing that allocates or locks.
fn first_press() -> bool {
    let first = !PRESSED.swap(true, Ordering::SeqCst);
    if first {
        if let Some(token) = TOKEN.get() {
            token.cancel();
        }
    }
    first
}

#[cfg(windows)]
mod platform {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::System::Console::{
        SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_C_EVENT,
    };

    unsafe extern "system" fn handler(kind: u32) -> BOOL {
        if (kind == CTRL_C_EVENT || kind == CTRL_BREAK_EVENT) && super::first_press() {
            return 1; // handled: the job stops itself
        }
        0 // the default: end the process
    }

    pub fn install() {
        // SAFETY: `handler` is a plain function that lives as long as the process.
        unsafe {
            SetConsoleCtrlHandler(Some(handler), 1);
        }
    }
}

#[cfg(unix)]
mod platform {
    extern "C" fn handler(_: libc::c_int) {
        if !super::first_press() {
            // SAFETY: signal and raise are async-signal-safe.
            unsafe {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::raise(libc::SIGINT);
            }
        }
    }

    pub fn install() {
        // SAFETY: `handler` only touches atomics, which is allowed in a signal handler.
        unsafe {
            libc::signal(
                libc::SIGINT,
                handler as extern "C" fn(libc::c_int) as libc::sighandler_t,
            );
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    pub fn install() {}
}

/// A secret from standard input: everything piped in, or one line typed at `prompt` without
/// being shown.
pub fn read_secret(prompt: &str) -> std::io::Result<String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        let mut all = String::new();
        stdin.lock().read_to_string(&mut all)?;
        return Ok(all.trim().to_string());
    }
    let mut stderr = std::io::stderr();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    let restore = echo::off();
    let mut line = String::new();
    let read = stdin.lock().read_line(&mut line);
    echo::restore(restore);
    writeln!(stderr)?;
    read?;
    Ok(line.trim().to_string())
}

#[cfg(windows)]
mod echo {
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_ECHO_INPUT, STD_INPUT_HANDLE,
    };

    pub fn off() -> Option<u32> {
        // SAFETY: the standard input handle is the process's own; the mode is plain data.
        unsafe {
            let input = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0u32;
            if GetConsoleMode(input, &mut mode) == 0 {
                return None;
            }
            SetConsoleMode(input, mode & !ENABLE_ECHO_INPUT);
            Some(mode)
        }
    }

    pub fn restore(mode: Option<u32>) {
        if let Some(mode) = mode {
            // SAFETY: as in `off`.
            unsafe {
                SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), mode);
            }
        }
    }
}

#[cfg(unix)]
mod echo {
    pub fn off() -> Option<libc::termios> {
        // SAFETY: termios is plain data that tcgetattr fills in for the terminal on fd 0.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) != 0 {
                return None;
            }
            let before = t;
            t.c_lflag &= !libc::ECHO;
            libc::tcsetattr(0, libc::TCSANOW, &t);
            Some(before)
        }
    }

    pub fn restore(before: Option<libc::termios>) {
        if let Some(t) = before {
            // SAFETY: puts back the settings read in `off`.
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &t);
            }
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod echo {
    pub fn off() -> Option<()> {
        None
    }
    pub fn restore(_: Option<()>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_press_cancels_and_the_second_does_not_claim_it() {
        let token = TOKEN.get_or_init(CancelToken::new).clone();
        assert!(first_press());
        assert!(token.is_cancelled());
        assert!(!first_press(), "a second Ctrl+C is left to end the process");
    }
}
