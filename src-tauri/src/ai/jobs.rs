//! The runs that can be stopped, by the name the window gave each (`AiGenerateRequest.job`, or
//! [`LOCAL_SETUP`]), so `ai_cancel` can reach a run that is already under way.

use folderskin_local::CancelToken;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The name setting this computer up runs under, for `ai_cancel`.
pub const LOCAL_SETUP: &str = "local-setup";

/// How long a Stop that arrived before its run is kept for it. The window sends the request and
/// then the Stop, and the two can be handled in either order; a Stop older than this belongs to
/// a run that has already ended.
const EARLY_STOP: Duration = Duration::from_secs(5);

/// Shared by every command; clones are the same registry.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<Inner>>);

#[derive(Default)]
struct Inner {
    running: HashMap<String, (u64, CancelToken)>,
    /// Jobs stopped before they were started, and when.
    early: Vec<(String, Instant)>,
    serial: u64,
}

/// A run in the registry. Dropping it takes the run out again.
pub struct Running {
    jobs: Jobs,
    job: String,
    serial: u64,
    pub token: CancelToken,
}

impl Jobs {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic elsewhere can't leave the map half-changed: every change is one call.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Puts `job` in the registry. Its token is already stopped if a Stop for it came first.
    pub fn start(&self, job: &str) -> Running {
        let inner = self.lock();
        self.enter(inner, job)
    }

    /// Like [`Jobs::start`], but `None` while a run of that name is still going.
    pub fn try_start(&self, job: &str) -> Option<Running> {
        let inner = self.lock();
        if inner.running.contains_key(job) {
            return None;
        }
        Some(self.enter(inner, job))
    }

    fn enter(&self, mut inner: MutexGuard<'_, Inner>, job: &str) -> Running {
        let token = CancelToken::new();
        inner.early.retain(|(_, at)| at.elapsed() < EARLY_STOP);
        if let Some(i) = inner.early.iter().position(|(j, _)| j == job) {
            inner.early.remove(i);
            token.cancel();
        }
        inner.serial += 1;
        let serial = inner.serial;
        inner
            .running
            .insert(job.to_string(), (serial, token.clone()));
        Running {
            jobs: self.clone(),
            job: job.to_string(),
            serial,
            token,
        }
    }

    pub fn is_running(&self, job: &str) -> bool {
        self.lock().running.contains_key(job)
    }

    /// Stops `job`, or the run of that name as soon as it starts. True when it was running.
    pub fn cancel(&self, job: &str) -> bool {
        let mut inner = self.lock();
        if let Some((_, token)) = inner.running.get(job) {
            token.cancel();
            return true;
        }
        inner.early.retain(|(_, at)| at.elapsed() < EARLY_STOP);
        inner.early.push((job.to_string(), Instant::now()));
        false
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut inner = self.jobs.lock();
        // Only this run's own entry: a later run of the same name keeps its own.
        if inner
            .running
            .get(&self.job)
            .is_some_and(|(serial, _)| *serial == self.serial)
        {
            inner.running.remove(&self.job);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_job_is_stopped_through_its_token() {
        let jobs = Jobs::default();
        let run = jobs.start("c1-t1");
        assert!(jobs.is_running("c1-t1"));
        assert!(!run.token.is_cancelled());
        assert!(jobs.cancel("c1-t1"));
        assert!(run.token.is_cancelled());
        drop(run);
        assert!(!jobs.is_running("c1-t1"), "a finished run is forgotten");
    }

    #[test]
    fn a_stop_that_arrives_first_stops_the_run_when_it_starts() {
        let jobs = Jobs::default();
        assert!(!jobs.cancel("c1-t2"));
        let run = jobs.start("c1-t2");
        assert!(run.token.is_cancelled());
        drop(run);
        // It is used up: the next run of that name goes ahead.
        assert!(!jobs.start("c1-t2").token.is_cancelled());
    }

    #[test]
    fn stopping_one_run_leaves_the_others_going() {
        let jobs = Jobs::default();
        let (a, b) = (jobs.start("a"), jobs.start("b"));
        jobs.cancel("a");
        assert!(a.token.is_cancelled());
        assert!(!b.token.is_cancelled());
    }

    #[test]
    fn a_second_setup_waits_for_the_first_to_end() {
        let jobs = Jobs::default();
        let first = jobs.try_start(LOCAL_SETUP).expect("nothing is running");
        assert!(jobs.try_start(LOCAL_SETUP).is_none());
        drop(first);
        assert!(jobs.try_start(LOCAL_SETUP).is_some());
    }

    #[test]
    fn an_old_run_ending_leaves_a_newer_one_of_the_same_name_in_place() {
        let jobs = Jobs::default();
        let old = jobs.start(LOCAL_SETUP);
        let new = jobs.start(LOCAL_SETUP);
        drop(old);
        assert!(jobs.is_running(LOCAL_SETUP));
        assert!(jobs.cancel(LOCAL_SETUP));
        assert!(new.token.is_cancelled());
    }
}
