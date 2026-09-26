//! A run over a tree, in the background, and what it did.
//!
//! [`Runs`] keeps the latest run. Only one goes at a time: another can't start while one is
//! going, and a new one replaces the last once that has ended. A run is two threads over one
//! record. The walker reads the tree breadth first ([`Walk`]) and adds the folders the run takes,
//! as the run's [`Choice`] says, and the worker goes through them in the order they were found,
//! doing the same to each. The worker starts on the folder itself straight away, so a run is well
//! under way long before the walk is over, and waits for the walker whenever it catches up.
//!
//! The record keeps each folder's part in the run in a byte beside its place in the tree: not in
//! it, still to do, changed, failed (and why) or skipped. That is all carrying on, trying the
//! failures again and undoing need, by run id, so the folders themselves never cross to the
//! webview. It hears how far a run has got, a dozen times a second at most, with the first of the
//! folders that failed.

use super::{folder_name, path_string, Outcome, Throttle, TreeFailureDto, TreeRunDto};
use folderskin_core::apply::refresh_shell_icons;
use folderskin_core::apply::tree::{folders_in, Choice, Order, Walk};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// What the webview is told as a run goes: a [`TreeRunEventDto`].
pub const EVENT: &str = "tree-run";
/// How often a run tells the webview how far it has got, at most: about twelve times a second.
const EVERY: Duration = Duration::from_millis(80);
/// How many of the folders that failed the webview is shown, first to fail first.
pub const FAILURES_SHOWN: usize = 100;
/// How long the worker waits for the walker before it looks at Stop again.
const NAP: Duration = Duration::from_millis(100);

/// Numbers every message about runs, so the webview keeps the newest whatever order they come in.
static SEQ: AtomicU64 = AtomicU64::new(1);

/// The latest run as the webview hears of it: `{"seq": 42, "run": {...}}`, or `"run": null`
/// when there's none (it was never started, or it was put away).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct TreeRunEventDto {
    pub seq: u64,
    pub run: Option<TreeRunDto>,
}

/// Where a run's messages go: the webview, or a test.
pub type Emit = Arc<dyn Fn(TreeRunEventDto) + Send + Sync>;

/// What a run does to each folder it takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Apply {
        skin_id: String,
    },
    Revert {
        /// A folder with no icon of its own is left alone (`skipped`) rather than written to.
        skip_plain: bool,
        /// It takes off exactly what an apply put on.
        undoing: bool,
    },
}

// Each folder's part in a run.
/// Not in it: left out by the choice, or looked inside only for a folder ticked further down.
const OUT: u8 = 0;
const TO_DO: u8 = 1;
const CHANGED: u8 = 2;
const FAILED: u8 = 3;
const SKIPPED: u8 = 4;

/// The runs: the latest, whether it's going or has ended.
#[derive(Default)]
pub struct Runs {
    latest: Mutex<Option<Arc<Job>>>,
    ids: AtomicU64,
}

impl Runs {
    /// A number for a new run.
    pub fn next_id(&self) -> u64 {
        self.ids.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// What run `id` does, while it's the latest.
    pub fn kind_of(&self, id: u64) -> Option<Kind> {
        lock(&self.latest)
            .as_ref()
            .filter(|job| job.id == id)
            .map(|job| job.kind.clone())
    }

    /// The latest run as it is now.
    pub fn event(&self) -> TreeRunEventDto {
        match lock(&self.latest).clone() {
            Some(job) => job.event(&job.lock()),
            None => nothing(),
        }
    }

    /// Starts `job` on a thread of its own, handing it to `work`, unless a run is going. It's the
    /// latest run from now on, and the one before is forgotten.
    pub fn start(
        &self,
        job: Job,
        work: impl FnOnce(Arc<Job>) + Send + 'static,
    ) -> Result<TreeRunEventDto, String> {
        let mut latest = lock(&self.latest);
        if let Some(going) = latest.as_ref().filter(|job| job.is_running()) {
            return Err(busy(going));
        }
        let job = Arc::new(job);
        *latest = Some(job.clone());
        Ok(launch(job, work))
    }

    /// Carries the latest run on from where it stopped, or with `retry` tries the folders it
    /// couldn't change again, as run `id` (the webview's idea of the latest).
    pub fn resume(
        &self,
        id: u64,
        retry: bool,
        work: impl FnOnce(Arc<Job>) + Send + 'static,
    ) -> Result<TreeRunEventDto, String> {
        let latest = lock(&self.latest);
        let job = latest
            .as_ref()
            .filter(|job| job.id == id)
            .cloned()
            .ok_or_else(gone)?;
        if job.is_running() {
            return Err(busy(&job));
        }
        {
            let mut record = job.lock();
            if retry {
                record.retry();
            }
            record.stopped = false;
            record.error = None;
        }
        Ok(launch(job, work))
    }

    /// Takes off exactly what apply run `id` put on, as a run of its own that becomes the latest.
    pub fn undo(
        &self,
        id: u64,
        work: impl FnOnce(Arc<Job>) + Send + 'static,
    ) -> Result<TreeRunEventDto, String> {
        let mut latest = lock(&self.latest);
        let before = latest
            .as_ref()
            .filter(|job| job.id == id && matches!(job.kind, Kind::Apply { .. }))
            .cloned()
            .ok_or_else(gone)?;
        if before.is_running() {
            return Err(busy(&before));
        }
        let job = Arc::new(Job::undoing(&before, self.next_id()));
        *latest = Some(job.clone());
        Ok(launch(job, work))
    }

    /// Stops run `id` before its next folder, if it's the one going. The folder in hand is
    /// finished, and so is the folder the walker is reading.
    pub fn stop(&self, id: u64, emit: &Emit) {
        let Some(job) = lock(&self.latest).clone().filter(|job| job.id == id) else {
            return;
        };
        let mut record = job.lock();
        if !record.running {
            return;
        }
        job.stop.store(true, Ordering::SeqCst);
        record.stopping = true;
        job.wake.notify_all();
        job.tell(emit, record, true);
    }

    /// Forgets run `id` once it has ended, and says there's none.
    pub fn dismiss(&self, id: u64, emit: &Emit) {
        let mut latest = lock(&self.latest);
        if latest
            .as_ref()
            .is_some_and(|job| job.id == id && !job.is_running())
        {
            *latest = None;
            drop(latest);
            emit(nothing());
        }
    }
}

/// Marks `job` going and hands it to `work` on a thread of its own. Returns how it looks as it
/// starts.
fn launch(job: Arc<Job>, work: impl FnOnce(Arc<Job>) + Send + 'static) -> TreeRunEventDto {
    job.stop.store(false, Ordering::SeqCst);
    job.lock().running = true;
    let going = job.clone();
    let spawned = std::thread::Builder::new()
        .name("folderskin-tree-run".into())
        .spawn(move || work(going));
    let mut record = job.lock();
    if let Err(e) = spawned {
        // The system is out of threads: the run never starts, and says so.
        record.running = false;
        record.error = Some(e.to_string());
    }
    job.event(&record)
}

/// No run: none started, or the last one was put away.
fn nothing() -> TreeRunEventDto {
    TreeRunEventDto {
        seq: SEQ.fetch_add(1, Ordering::SeqCst),
        run: None,
    }
}

/// Why another run can't start while `going` is.
fn busy(going: &Job) -> String {
    format!(
        "wait for the run in {} to finish, or stop it, before starting another",
        going.name
    )
}

/// Why run `id` can't be carried on, tried again or undone: it isn't the latest any more.
fn gone() -> String {
    "that run isn't there any more".into()
}

/// One run: what it does, where, and its record.
pub struct Job {
    pub id: u64,
    pub kind: Kind,
    /// The folder as the webview named it.
    pub folder: String,
    root: PathBuf,
    name: String,
    /// The skin an apply puts on, or an undo takes off.
    skin_id: Option<String>,
    /// Stop was pressed.
    stop: AtomicBool,
    /// The worker is leaving, for whatever reason, and so the walker leaves too.
    leaving: AtomicBool,
    record: Mutex<Record>,
    /// Wakes the worker when the walker has found more, or has finished.
    wake: Condvar,
    throttle: Mutex<Throttle>,
}

/// What a run has found and done.
struct Record {
    walk: Walk,
    choice: Choice,
    /// Each folder's part in the run, by its number in the walk's tree.
    parts: Vec<u8>,
    /// Every different reason a folder failed, and which one each failed folder's is.
    reasons: Vec<String>,
    reason_of: HashMap<u32, u16>,
    /// The folders that failed, first to fail first.
    failures: Vec<u32>,
    /// Where the worker is: every folder before it is done or not in the run.
    next: usize,
    /// Folders the run takes that have been found so far, done or not.
    total: u64,
    done: u64,
    changed: u64,
    failed: u64,
    skipped: u64,
    /// The walk is over: `total` is every folder there is.
    walked: bool,
    running: bool,
    stopping: bool,
    /// It ended with Stop, with folders left to do.
    stopped: bool,
    /// The name of the last folder done, the folder itself's before any.
    current: String,
    /// Why the run couldn't go at all, before any folder was touched.
    error: Option<String>,
}

impl Job {
    /// A run over `root` (canonical) and the folders inside it that `choice` takes, the folder
    /// itself always first. Nothing is read or done until it's started.
    pub fn new(id: u64, kind: Kind, folder: String, root: PathBuf, choice: Choice) -> Job {
        let walk = Walk::new(root.clone(), Order::Nearest);
        let walked = walk.is_done();
        let name = folder_name(&root);
        let skin_id = match &kind {
            Kind::Apply { skin_id } => Some(skin_id.clone()),
            Kind::Revert { .. } => None,
        };
        Job::with(
            id,
            (kind, skin_id),
            folder,
            root,
            Record {
                walk,
                choice,
                parts: vec![TO_DO],
                total: 1,
                walked,
                current: name,
                ..Record::empty()
            },
        )
    }

    /// A revert of exactly the folders apply run `before` changed, taking its tree with it.
    fn undoing(before: &Job, id: u64) -> Job {
        let mut old = before.lock();
        let walk = std::mem::replace(
            &mut old.walk,
            Walk::new(before.root.clone(), Order::Nearest),
        );
        let parts: Vec<u8> = old
            .parts
            .iter()
            .map(|&part| if part == CHANGED { TO_DO } else { OUT })
            .collect();
        drop(old);
        let total = parts.iter().filter(|&&part| part == TO_DO).count() as u64;
        let kind = Kind::Revert {
            skip_plain: false,
            undoing: true,
        };
        let record = Record {
            walk,
            choice: Choice::everything(),
            parts,
            total,
            walked: true,
            current: before.name.clone(),
            ..Record::empty()
        };
        let what = (kind, before.skin_id.clone());
        Job::with(id, what, before.folder.clone(), before.root.clone(), record)
    }

    /// A run of `kind`, with the skin it puts on or takes off, over `root`.
    fn with(
        id: u64,
        (kind, skin_id): (Kind, Option<String>),
        folder: String,
        root: PathBuf,
        record: Record,
    ) -> Job {
        Job {
            id,
            kind,
            folder,
            name: folder_name(&root),
            skin_id,
            root,
            stop: AtomicBool::new(false),
            leaving: AtomicBool::new(false),
            record: Mutex::new(record),
            wake: Condvar::new(),
            throttle: Mutex::new(Throttle::new(EVERY)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.lock().running
    }

    /// Goes through the folders the run takes, in the order they're found, doing to each what
    /// `prepare` returns. Returns once it has done them all, or Stop was pressed.
    ///
    /// `prepare` is called once, just before the first folder, so its slow part (rendering the
    /// icon) starts after the webview knows the run is going, and never for a run with nothing to
    /// do. An error from it is the run's, and no folder is touched. A folder that fails doesn't
    /// end the run: it is kept with why, to try again.
    pub fn work<P, A>(self: &Arc<Self>, prepare: P, emit: &Emit)
    where
        P: FnOnce() -> Result<A, String>,
        A: FnMut(&Path) -> Result<Outcome, String>,
    {
        self.leaving.store(false, Ordering::SeqCst);
        let walked = self.lock().walked;
        let walker = (!walked).then(|| {
            let (job, emit) = (self.clone(), emit.clone());
            std::thread::Builder::new()
                .name("folderskin-tree-walk".into())
                .spawn(move || job.walk(&emit))
        });
        let mut prepare = Some(prepare);
        let mut act: Option<A> = None;
        let mut changed = false;
        while let Some((folder, path)) = self.next_folder() {
            let act = match &mut act {
                Some(act) => act,
                none => match prepare.take().expect("the action is made once")() {
                    Ok(made) => none.insert(made),
                    Err(e) => {
                        self.lock().error = Some(e);
                        break;
                    }
                },
            };
            let result = act(&path);
            changed |= matches!(result, Ok(Outcome::Changed));
            let mut record = self.lock();
            record.finished(folder, folder_name(&path), result);
            self.tell(emit, record, false);
        }
        self.leaving.store(true, Ordering::SeqCst);
        if let Some(Ok(walker)) = walker {
            // At most the folder it's reading is left, and it stops after that.
            let _ = walker.join();
        }
        if changed {
            // Once for the whole run, not once per folder: on Windows this refreshes every view.
            refresh_shell_icons();
        }
        let mut record = self.lock();
        record.stopped =
            self.stop.load(Ordering::SeqCst) && record.error.is_none() && record.has_left();
        record.running = false;
        record.stopping = false;
        self.tell(emit, record, true);
    }

    /// The next folder to do, waiting for the walker to find it if need be: `None` when Stop was
    /// pressed, or every folder there is has been done.
    fn next_folder(&self) -> Option<(u32, PathBuf)> {
        let mut record = self.lock();
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return None;
            }
            if let Some(folder) = record.next_to_do() {
                return Some((folder, record.walk.tree().path(folder)));
            }
            if record.walked {
                return None;
            }
            record = self
                .wake
                .wait_timeout(record, NAP)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    /// The walker: reads the tree, one folder at a time, until it's all read, or the run ends.
    fn walk(&self, emit: &Emit) {
        loop {
            if self.stop.load(Ordering::SeqCst) || self.leaving.load(Ordering::SeqCst) {
                break;
            }
            let next = self.lock().walk.next_folder();
            let Some((folder, path)) = next else {
                break;
            };
            // Read without holding the record: a folder on a network disk can take a while, and
            // the worker carries on meanwhile.
            let names = folders_in(&path);
            let mut record = self.lock();
            record.found(folder, &path, names);
            self.wake.notify_all();
            self.tell(emit, record, false);
        }
        let mut record = self.lock();
        record.walked = record.walk.is_done();
        self.wake.notify_all();
    }

    /// Tells the webview how the run looks now, unless it was told too recently (`force` tells
    /// it whatever), and lets go of the record before it does.
    fn tell(&self, emit: &Emit, record: MutexGuard<'_, Record>, force: bool) {
        let ready = lock(&self.throttle).ready(Instant::now());
        if !(force || ready) {
            return;
        }
        let event = self.event(&record);
        drop(record);
        emit(event);
    }

    /// The run as the webview sees it, numbered while the record is held, so a newer look always
    /// has the larger number.
    fn event(&self, record: &Record) -> TreeRunEventDto {
        TreeRunEventDto {
            seq: SEQ.fetch_add(1, Ordering::SeqCst),
            run: Some(self.snapshot(record)),
        }
    }

    fn snapshot(&self, record: &Record) -> TreeRunDto {
        let tree = record.walk.tree();
        let (kind, undoing, leaves_plain) = match &self.kind {
            Kind::Apply { .. } => ("apply", false, false),
            Kind::Revert {
                skip_plain,
                undoing,
            } => ("revert", *undoing, *skip_plain),
        };
        TreeRunDto {
            id: self.id,
            kind,
            undoing,
            leaves_plain,
            folder: self.folder.clone(),
            root: path_string(&self.root),
            name: self.name.clone(),
            skin_id: self.skin_id.clone(),
            running: record.running,
            stopping: record.stopping,
            stopped: record.stopped,
            done: record.done,
            total: record.total,
            counted: record.walked,
            current: record.current.clone(),
            changed: record.changed,
            failed: record.failed,
            skipped: record.skipped,
            failures: record
                .failures
                .iter()
                .take(FAILURES_SHOWN)
                .map(|&folder| {
                    let path = tree.path(folder);
                    let reason = record.reason_of[&folder] as usize;
                    TreeFailureDto {
                        name: folder_name(&path),
                        path: path_string(&path),
                        reason: record.reasons[reason].clone(),
                    }
                })
                .collect(),
            error: record.error.clone(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Record> {
        lock(&self.record)
    }
}

impl Record {
    fn empty() -> Record {
        Record {
            walk: Walk::new(PathBuf::new(), Order::Nearest),
            choice: Choice::everything(),
            parts: Vec::new(),
            reasons: Vec::new(),
            reason_of: HashMap::new(),
            failures: Vec::new(),
            next: 0,
            total: 0,
            done: 0,
            changed: 0,
            failed: 0,
            skipped: 0,
            walked: false,
            running: false,
            stopping: false,
            stopped: false,
            current: String::new(),
            error: None,
        }
    }

    /// The walker read `folder`, at `path`, and found `names` inside it: the ones the run takes
    /// are added to it, and so are the ones it has to look inside for a folder ticked further
    /// down. Anything else isn't kept at all.
    fn found(&mut self, folder: u32, path: &Path, names: Vec<OsString>) {
        let inside_taken = if folder == 0 {
            self.choice.all()
        } else {
            self.parts[folder as usize] != OUT
        };
        let mut inside = Vec::with_capacity(names.len());
        let mut taken = Vec::with_capacity(names.len());
        for name in names {
            let child = path.join(&name);
            let takes = self.choice.takes(&child, inside_taken);
            let looks = self.choice.looks_inside(&child, takes);
            if takes || looks {
                inside.push((name, looks));
                taken.push(takes);
            }
        }
        self.walk.found(folder, inside);
        self.total += taken.iter().filter(|&&takes| takes).count() as u64;
        self.parts.extend(
            taken
                .into_iter()
                .map(|takes| if takes { TO_DO } else { OUT }),
        );
    }

    /// The next folder still to do, in the order they were found, if the walker has found one.
    fn next_to_do(&mut self) -> Option<u32> {
        while self.next < self.parts.len() {
            let folder = self.next;
            self.next += 1;
            if self.parts[folder] == TO_DO {
                return Some(u32::try_from(folder).expect("a folder's number"));
            }
        }
        None
    }

    /// What became of `folder`, called `name`.
    fn finished(&mut self, folder: u32, name: String, result: Result<Outcome, String>) {
        let k = folder as usize;
        self.done += 1;
        self.current = name;
        match result {
            Ok(Outcome::Changed) => {
                self.parts[k] = CHANGED;
                self.changed += 1;
            }
            Ok(Outcome::Skipped) => {
                self.parts[k] = SKIPPED;
                self.skipped += 1;
            }
            Err(reason) => {
                self.parts[k] = FAILED;
                self.failed += 1;
                // Thousands of folders on a read-only disk fail for the same reason: kept once.
                let known = self.reasons.iter().position(|r| *r == reason);
                let at = known.unwrap_or_else(|| {
                    self.reasons.push(reason);
                    self.reasons.len() - 1
                });
                self.reason_of
                    .insert(folder, u16::try_from(at).unwrap_or(u16::MAX));
                self.failures.push(folder);
            }
        }
    }

    /// Puts the folders that failed back to do, for trying them again.
    fn retry(&mut self) {
        let Some(&first) = self.failures.iter().min() else {
            return;
        };
        for folder in self.failures.drain(..) {
            self.parts[folder as usize] = TO_DO;
        }
        self.done -= self.failed;
        self.failed = 0;
        self.reasons.clear();
        self.reason_of.clear();
        self.next = self.next.min(first as usize);
    }

    /// Whether there's anything left: folders found and not done yet, or the walk isn't over.
    fn has_left(&self) -> bool {
        !self.walked || self.parts[self.next..].contains(&TO_DO)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::tests::Scratch;
    use std::sync::atomic::AtomicUsize;

    /// A per-folder action that captures nothing.
    type Act = fn(&Path) -> Result<Outcome, String>;

    /// Every message a run sent.
    #[derive(Default)]
    struct Heard(Mutex<Vec<TreeRunEventDto>>);

    impl Heard {
        fn emit(self: &Arc<Self>) -> Emit {
            let heard = self.clone();
            Arc::new(move |event| lock(&heard.0).push(event))
        }

        fn take(&self) -> Vec<TreeRunEventDto> {
            std::mem::take(&mut *lock(&self.0))
        }
    }

    fn apply(skin: &str) -> Kind {
        Kind::Apply {
            skin_id: skin.into(),
        }
    }

    /// `/`-separated paths relative to `root`.
    fn relative(root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Runs `job` to the end on this thread with `act`, and returns how it ended.
    fn run(
        job: &Arc<Job>,
        emit: &Emit,
        act: impl FnMut(&Path) -> Result<Outcome, String>,
    ) -> TreeRunDto {
        job.lock().running = true;
        job.work(|| Ok(act), emit);
        job.event(&job.lock()).run.unwrap()
    }

    fn job(scratch: &Scratch, kind: Kind, choice: Choice) -> Arc<Job> {
        Arc::new(Job::new(
            1,
            kind,
            "the folder".into(),
            scratch.root(),
            choice,
        ))
    }

    #[test]
    fn a_run_does_the_folder_then_everything_inside_it_nearest_first() {
        let scratch = Scratch::with(&["b/z", "b/A", "a/y/deep", "C", ".git/objects"]);
        let root = scratch.root();
        let heard = Arc::new(Heard::default());
        let job = job(&scratch, apply("dune"), Choice::everything());
        let mut seen = Vec::new();
        let run = run(&job, &heard.emit(), |folder| {
            seen.push(relative(&root, folder));
            Ok(Outcome::Changed)
        });
        assert_eq!(seen, ["", "a", "b", "C", "a/y", "b/A", "b/z", "a/y/deep"]);
        assert_eq!(
            (run.done, run.total, run.changed, run.counted),
            (8, 8, 8, true)
        );
        assert!(!run.running && !run.stopped && run.error.is_none());
        assert_eq!(run.current, "deep");
        assert_eq!(run.kind, "apply");
        assert_eq!(run.skin_id.as_deref(), Some("dune"));
        assert_eq!(run.name, folder_name(&root));
        // The last message is the whole run, and every message is newer than the one before.
        let events = heard.take();
        assert_eq!(events.last().unwrap().run.as_ref(), Some(&run));
        assert!(events.windows(2).all(|w| w[0].seq < w[1].seq));
    }

    #[test]
    fn a_run_takes_only_the_folders_its_choice_says() {
        let scratch = Scratch::with(&[
            "Clients/Acme/Contracts",
            "Clients/Globex",
            "Photos/2020",
            "Photos/2021/Holiday",
            "Notes",
        ]);
        let root = scratch.root();
        let p = |s: &str| root.join(s);
        // Everything but Photos, except Holiday inside it; and nothing in Acme but Contracts.
        let choice = Choice::new(
            true,
            [
                (p("Photos"), false),
                (p("Photos/2021/Holiday"), true),
                (p("Clients/Acme"), false),
                (p("Clients/Acme/Contracts"), true),
            ],
        );
        let job = job(&scratch, apply("dune"), choice);
        let mut seen = Vec::new();
        let run = run(&job, &Arc::new(Heard::default()).emit(), |folder| {
            seen.push(relative(&root, folder));
            Ok(Outcome::Changed)
        });
        assert_eq!(
            seen,
            [
                "",
                "Clients",
                "Notes",
                "Clients/Globex",
                "Clients/Acme/Contracts",
                "Photos/2021/Holiday"
            ]
        );
        assert_eq!((run.total, run.changed), (6, 6));

        // With nothing inside taken, the folder goes on its own, and nothing inside is read.
        let alone = job_with(&scratch, Choice::new(false, []));
        let mut seen = Vec::new();
        run_on(&alone, |folder| seen.push(relative(&root, folder)));
        assert_eq!(seen, [""]);
        assert_eq!(alone.lock().walk.tree().len(), 1, "nothing inside was kept");
    }

    fn job_with(scratch: &Scratch, choice: Choice) -> Arc<Job> {
        job(scratch, apply("dune"), choice)
    }

    fn run_on(job: &Arc<Job>, mut each: impl FnMut(&Path)) -> TreeRunDto {
        run(job, &Arc::new(Heard::default()).emit(), move |folder| {
            each(folder);
            Ok(Outcome::Changed)
        })
    }

    #[test]
    fn folders_that_fail_are_kept_with_why_and_the_run_goes_on() {
        let scratch = Scratch::with(&["locked", "open", "gone", "also locked"]);
        let job = job(&scratch, apply("dune"), Choice::everything());
        let run = run(
            &job,
            &Arc::new(Heard::default()).emit(),
            |folder| match folder.file_name().and_then(|n| n.to_str()) {
                Some("locked" | "also locked") => {
                    Err("you don't have permission to change it".into())
                }
                Some("gone") => Err("it isn't there any more".into()),
                _ => Ok(Outcome::Changed),
            },
        );
        assert_eq!((run.done, run.changed, run.failed), (5, 2, 3));
        let failed: Vec<(&str, &str)> = run
            .failures
            .iter()
            .map(|f| (f.name.as_str(), f.reason.as_str()))
            .collect();
        assert_eq!(
            failed,
            [
                ("also locked", "you don't have permission to change it"),
                ("gone", "it isn't there any more"),
                ("locked", "you don't have permission to change it"),
            ]
        );
        assert_eq!(job.lock().reasons.len(), 2, "the same reason is kept once");
        assert!(run.failures[0].path.ends_with("also locked"));
    }

    #[test]
    fn only_the_first_failures_are_shown() {
        let many: Vec<String> = (0..FAILURES_SHOWN + 20)
            .map(|i| format!("f{i:03}"))
            .collect();
        let names: Vec<&str> = many.iter().map(String::as_str).collect();
        let scratch = Scratch::with(&names);
        let job = job(&scratch, apply("dune"), Choice::everything());
        let root = scratch.root();
        let run = run(&job, &Arc::new(Heard::default()).emit(), |folder| {
            if folder == root {
                Ok(Outcome::Changed)
            } else {
                Err("its disk is read-only".into())
            }
        });
        assert_eq!(run.failed, (FAILURES_SHOWN + 20) as u64);
        assert_eq!(run.failures.len(), FAILURES_SHOWN);
        assert_eq!(run.failures[0].name, "f000");
    }

    #[test]
    fn a_stopped_run_carries_on_from_where_it_stopped() {
        let scratch = Scratch::with(&["a/a1", "a/a2", "b/b1", "c"]);
        let root = scratch.root();
        let runs = Runs::default();
        let heard = Arc::new(Heard::default());
        let emit = heard.emit();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let job = Job::new(
            runs.next_id(),
            apply("dune"),
            "the folder".into(),
            root.clone(),
            Choice::everything(),
        );
        // Stop is pressed while the second folder is being changed: it's finished, and the run
        // stops there.
        let work = {
            let (seen, calls, emit, root) =
                (seen.clone(), calls.clone(), emit.clone(), root.clone());
            move |job: Arc<Job>| {
                let stopper = job.clone();
                let emit_ = emit.clone();
                job.work(
                    move || {
                        Ok(move |folder: &Path| {
                            lock(&seen).push(relative(&root, folder));
                            if calls.fetch_add(1, Ordering::SeqCst) + 1 == 2 {
                                let runs_emit = emit_.clone();
                                stopper.stop.store(true, Ordering::SeqCst);
                                stopper.lock().stopping = true;
                                runs_emit(stopper.event(&stopper.lock()));
                            }
                            Ok(Outcome::Changed)
                        })
                    },
                    &emit,
                )
            }
        };
        let started = runs.start(job, work).unwrap().run.unwrap();
        assert!(started.running && started.done == 0 && started.total == 1);
        let stopped = wait(&runs);
        assert!(stopped.stopped);
        assert_eq!((stopped.done, stopped.changed), (2, 2));
        assert_eq!(*lock(&seen), ["", "a"]);

        // Another run can't start meanwhile... but this one has ended, so a start would replace
        // it. Carrying on does the rest, and only the rest.
        let work = {
            let (seen, emit, root) = (seen.clone(), emit.clone(), root.clone());
            move |job: Arc<Job>| {
                job.work(
                    move || {
                        Ok(move |folder: &Path| {
                            lock(&seen).push(relative(&root, folder));
                            Ok(Outcome::Changed)
                        })
                    },
                    &emit,
                )
            }
        };
        runs.resume(stopped.id, false, work).unwrap();
        let done = wait(&runs);
        assert!(!done.stopped && done.counted);
        assert_eq!((done.done, done.total, done.changed), (7, 7, 7));
        assert_eq!(
            *lock(&seen),
            ["", "a", "b", "c", "a/a1", "a/a2", "b/b1"].map(String::from),
            "nothing twice, nothing missed"
        );
    }

    /// Waits for the latest run to end, and returns how it ended.
    fn wait(runs: &Runs) -> TreeRunDto {
        let started = Instant::now();
        loop {
            let run = runs.event().run.expect("a run");
            if !run.running {
                return run;
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "the run never ended"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn one_run_goes_at_a_time() {
        let scratch = Scratch::with(&["a"]);
        let runs = Runs::default();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let job = Job::new(
            runs.next_id(),
            apply("dune"),
            "the folder".into(),
            scratch.root(),
            Choice::everything(),
        );
        // The first folder waits until it's let go.
        let work = {
            let gate = gate.clone();
            move |job: Arc<Job>| {
                job.work(
                    move || {
                        Ok(move |_: &Path| {
                            let (open, turn) = &*gate;
                            let mut open = lock(open);
                            while !*open {
                                open = turn.wait(open).unwrap();
                            }
                            Ok(Outcome::Changed)
                        })
                    },
                    &Arc::new(Heard::default()).emit(),
                )
            }
        };
        let first = runs.start(job, work).unwrap().run.unwrap();
        let second = Job::new(
            runs.next_id(),
            apply("mint"),
            "another".into(),
            scratch.root(),
            Choice::everything(),
        );
        let name = folder_name(&scratch.root());
        assert_eq!(
            runs.start(second, |_| {}).unwrap_err(),
            format!("wait for the run in {name} to finish, or stop it, before starting another")
        );
        assert_eq!(
            runs.resume(first.id + 7, false, |_| {}).unwrap_err(),
            "that run isn't there any more"
        );
        *lock(&gate.0) = true;
        gate.1.notify_all();
        assert_eq!(wait(&runs).changed, 2);

        // Ended, it's replaced by the next one, and forgotten when put away.
        let emit = Arc::new(Heard::default());
        let third = Job::new(
            runs.next_id(),
            apply("mint"),
            "another".into(),
            scratch.root(),
            Choice::everything(),
        );
        let third = runs
            .start(third, |job| {
                job.work(
                    || Ok(|_: &Path| Ok(Outcome::Changed)),
                    &Arc::new(Heard::default()).emit(),
                )
            })
            .unwrap()
            .run
            .unwrap();
        assert_eq!(wait(&runs).id, third.id);
        runs.dismiss(third.id, &emit.emit());
        assert_eq!(runs.event().run, None);
        assert_eq!(emit.take().len(), 1, "told there's no run");
    }

    #[test]
    fn failures_are_tried_again_and_the_ones_that_work_count_as_changed() {
        let scratch = Scratch::with(&["a", "b", "c"]);
        let runs = Runs::default();
        let broken = Arc::new(Mutex::new(vec!["a".to_string(), "c".to_string()]));
        let work = |broken: Arc<Mutex<Vec<String>>>| {
            move |job: Arc<Job>| {
                job.work(
                    move || {
                        Ok(move |folder: &Path| {
                            let name = folder.file_name().unwrap().to_string_lossy().into_owned();
                            if lock(&broken).contains(&name) {
                                Err("its disk is full".to_string())
                            } else {
                                Ok(Outcome::Changed)
                            }
                        })
                    },
                    &Arc::new(Heard::default()).emit(),
                )
            }
        };
        let job = Job::new(
            runs.next_id(),
            apply("dune"),
            "the folder".into(),
            scratch.root(),
            Choice::everything(),
        );
        runs.start(job, work(broken.clone())).unwrap();
        let first = wait(&runs);
        assert_eq!((first.done, first.changed, first.failed), (4, 2, 2));

        // c is fixed; a still fails.
        lock(&broken).retain(|name| name != "c");
        runs.resume(first.id, true, work(broken.clone())).unwrap();
        let again = wait(&runs);
        assert_eq!(again.id, first.id, "the same run");
        assert_eq!(
            (again.done, again.total, again.changed, again.failed),
            (4, 4, 3, 1)
        );
        assert_eq!(again.failures.len(), 1);
        assert_eq!(again.failures[0].name, "a");
    }

    #[test]
    fn undoing_a_run_takes_off_exactly_what_it_put_on() {
        let scratch = Scratch::with(&["a/a1", "b", "c"]);
        let root = scratch.root();
        let runs = Runs::default();
        let job = Job::new(
            runs.next_id(),
            apply("dune"),
            "the folder".into(),
            root.clone(),
            Choice::everything(),
        );
        runs.start(job, |job| {
            job.work(
                || {
                    Ok(|folder: &Path| {
                        if folder.ends_with("b") {
                            Err("its disk is read-only".to_string())
                        } else {
                            Ok(Outcome::Changed)
                        }
                    })
                },
                &Arc::new(Heard::default()).emit(),
            )
        })
        .unwrap();
        let applied = wait(&runs);
        assert_eq!((applied.changed, applied.failed), (4, 1));

        let seen = Arc::new(Mutex::new(Vec::new()));
        let work = {
            let (seen, root) = (seen.clone(), root.clone());
            move |job: Arc<Job>| {
                job.work(
                    move || {
                        Ok(move |folder: &Path| {
                            lock(&seen).push(relative(&root, folder));
                            Ok(Outcome::Changed)
                        })
                    },
                    &Arc::new(Heard::default()).emit(),
                )
            }
        };
        let undo = runs.undo(applied.id, work).unwrap().run.unwrap();
        assert!(undo.id > applied.id && undo.undoing && undo.kind == "revert");
        assert_eq!(
            undo.skin_id.as_deref(),
            Some("dune"),
            "the skin it takes off"
        );
        assert!(
            !undo.leaves_plain,
            "an undo writes to every folder it changed"
        );
        let undone = wait(&runs);
        assert_eq!(*lock(&seen), ["", "a", "c", "a/a1"]);
        assert_eq!((undone.total, undone.changed, undone.counted), (4, 4, true));
        assert_eq!(
            runs.undo(undone.id, |_| {}).unwrap_err(),
            "that run isn't there any more",
            "only an apply is undone"
        );
    }

    #[test]
    fn a_run_that_cannot_prepare_touches_nothing() {
        let scratch = Scratch::with(&["a", "b"]);
        let job = job(&scratch, apply("gone"), Choice::everything());
        job.lock().running = true;
        let touched = Arc::new(AtomicUsize::new(0));
        let counter = touched.clone();
        job.work(
            move || -> Result<Act, String> {
                counter.fetch_add(1, Ordering::SeqCst);
                Err("that skin isn't available any more".into())
            },
            &Arc::new(Heard::default()).emit(),
        );
        let run = job.event(&job.lock()).run.unwrap();
        assert_eq!(
            run.error.as_deref(),
            Some("that skin isn't available any more")
        );
        assert_eq!((run.done, run.changed), (0, 0));
        assert!(!run.running && !run.stopped);
        assert_eq!(touched.load(Ordering::SeqCst), 1);

        // Stopped before it began, nothing is prepared at all.
        let job = job_with(&scratch, Choice::everything());
        job.stop.store(true, Ordering::SeqCst);
        job.lock().running = true;
        job.work(
            || -> Result<Act, String> { panic!("prepared for no folder") },
            &Arc::new(Heard::default()).emit(),
        );
        let run = job.event(&job.lock()).run.unwrap();
        assert!(run.stopped && run.done == 0);
    }

    #[test]
    fn a_big_run_tells_the_webview_now_and_then_not_for_every_folder() {
        let names: Vec<String> = (0..600).map(|i| format!("d{i}/inside")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let scratch = Scratch::with(&names);
        let heard = Arc::new(Heard::default());
        let job = job(&scratch, apply("dune"), Choice::everything());
        let started = Instant::now();
        let run = run(&job, &heard.emit(), |_| Ok(Outcome::Changed));
        let took = started.elapsed();
        assert_eq!(run.changed, 1201);
        let events = heard.take().len();
        // One now and then, and one at the end: never more than the time allows.
        let most = (took.as_millis() / EVERY.as_millis()) as usize + 2;
        assert!(events <= most, "{events} messages in {took:?}");
        assert!(events < 100, "{events} messages for 1,201 folders");
    }
}
