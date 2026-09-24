//! `run::end_all`, which a program calls on its way out. A test binary of its own: once it has
//! been called, every runtime this process starts is ended at once, which would stop any other
//! test's runtime.

#![cfg(unix)]

use folderskin_local::{run, CancelToken, Event, Reporter};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A runtime that says it has started and then runs for a minute, as a painting does.
fn a_long_painting() -> Command {
    let mut cmd = Command::new("sh");
    // exec: the process listed is the one that runs, with no shell left in between.
    cmd.args(["-c", "echo started; exec sleep 60"]);
    cmd
}

#[test]
fn quitting_ends_the_runtimes_still_running_and_any_started_after() {
    let (said, heard) = mpsc::channel();
    let reporter = Reporter::new(move |e| {
        if matches!(&e, Event::Log { message, .. } if message == "started") {
            let _ = said.send(());
        }
    });
    let started = Instant::now();
    let painting = std::thread::spawn(move || {
        run::run(a_long_painting(), 4, &reporter, &CancelToken::new()).unwrap()
    });
    heard
        .recv_timeout(Duration::from_secs(20))
        .expect("the runtime started");

    run::end_all();
    let done = painting.join().unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "ended at once, not after its minute: {:?}",
        started.elapsed()
    );
    let status = done
        .status
        .expect("it ended by itself, as far as run knows");
    assert!(!status.success(), "{status:?}");

    // A runtime started while the program is quitting doesn't outlive it either.
    let started = Instant::now();
    let late = run::run(
        a_long_painting(),
        4,
        &Reporter::silent(),
        &CancelToken::new(),
    )
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    assert!(!late.status.expect("ended").success());
}
