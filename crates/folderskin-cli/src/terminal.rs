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

// Echo goes off while a key is typed and comes back however the prompt ends: Enter, an error,
// or Ctrl+C, whose default action ends the process at once and would leave the terminal
// silent for whatever runs in it next.

#[cfg(windows)]
mod echo {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use windows_sys::core::BOOL;
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleCtrlHandler, SetConsoleMode, ENABLE_ECHO_INPUT,
        STD_INPUT_HANDLE,
    };

    /// The console's mode before echo went off, for the Ctrl+C handler to put back.
    static SAVED: AtomicU32 = AtomicU32::new(0);
    static ARMED: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn on_ctrl(_kind: u32) -> BOOL {
        if ARMED.swap(false, Ordering::SeqCst) {
            // SAFETY: the process's own standard input handle and a mode it had.
            unsafe {
                SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), SAVED.load(Ordering::SeqCst));
            }
        }
        0 // not handled: the process ends as it would have
    }

    pub fn off() -> Option<u32> {
        // SAFETY: the standard input handle is the process's own; the mode is plain data; the
        // handler is a plain function that lives as long as the process.
        unsafe {
            let input = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0u32;
            if GetConsoleMode(input, &mut mode) == 0 {
                return None;
            }
            SAVED.store(mode, Ordering::SeqCst);
            ARMED.store(true, Ordering::SeqCst);
            SetConsoleCtrlHandler(Some(on_ctrl), 1);
            SetConsoleMode(input, mode & !ENABLE_ECHO_INPUT);
            Some(mode)
        }
    }

    pub fn restore(mode: Option<u32>) {
        if let Some(mode) = mode {
            ARMED.store(false, Ordering::SeqCst);
            // SAFETY: as in `off`.
            unsafe {
                SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), mode);
                SetConsoleCtrlHandler(Some(on_ctrl), 0);
            }
        }
    }
}

#[cfg(unix)]
mod echo {
    use std::cell::UnsafeCell;
    use std::mem::MaybeUninit;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// The terminal's settings before echo went off, for the signal handler to put back.
    struct Saved(UnsafeCell<MaybeUninit<libc::termios>>);
    // SAFETY: written only while ARMED is false, and read, by the handler, only while it is true.
    unsafe impl Sync for Saved {}
    static SAVED: Saved = Saved(UnsafeCell::new(MaybeUninit::uninit()));
    static ARMED: AtomicBool = AtomicBool::new(false);

    /// What ends a prompt early: Ctrl+C, Ctrl+\, the terminal closing, and `kill`.
    const SIGNALS: [libc::c_int; 4] = [libc::SIGINT, libc::SIGQUIT, libc::SIGHUP, libc::SIGTERM];

    extern "C" fn on_signal(signal: libc::c_int) {
        // SAFETY: tcsetattr, signal and raise are async-signal-safe, and SAVED is complete
        // whenever ARMED is set. Raised again with the default action, the signal ends the
        // process as it would have.
        unsafe {
            if ARMED.swap(false, Ordering::SeqCst) {
                libc::tcsetattr(0, libc::TCSANOW, (*SAVED.0.get()).as_ptr());
            }
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
    }

    pub struct Guard {
        before: libc::termios,
        handlers: [libc::sighandler_t; 4],
    }

    pub fn off() -> Option<Guard> {
        // SAFETY: termios is plain data that tcgetattr fills in for the terminal on fd 0. SAVED
        // is written before ARMED is set and before the handler that reads it is installed.
        unsafe {
            let mut before: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut before) != 0 {
                return None;
            }
            (*SAVED.0.get()).write(before);
            ARMED.store(true, Ordering::SeqCst);
            let mut handlers = [libc::SIG_DFL; 4];
            for (slot, signal) in handlers.iter_mut().zip(SIGNALS) {
                *slot = libc::signal(
                    signal,
                    on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t,
                );
                if *slot == libc::SIG_IGN {
                    // Ignored (a background job): stay that way.
                    libc::signal(signal, libc::SIG_IGN);
                }
            }
            let mut quiet = before;
            quiet.c_lflag &= !libc::ECHO;
            libc::tcsetattr(0, libc::TCSANOW, &quiet);
            Some(Guard { before, handlers })
        }
    }

    pub fn restore(guard: Option<Guard>) {
        if let Some(guard) = guard {
            ARMED.store(false, Ordering::SeqCst);
            // SAFETY: puts back the settings and the handlers `off` found.
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &guard.before);
                for (handler, signal) in guard.handlers.into_iter().zip(SIGNALS) {
                    libc::signal(signal, handler);
                }
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
