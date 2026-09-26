//! How many folders are inside the chosen one, counted in the background.
//!
//! Picking a folder starts a count of everything inside it ([`Counts::start`]), abandoning the
//! count of the folder before. It reads depth first, so each folder's insides are finished before
//! the next folder's and their counts are known one by one: the webview asks for the folders it
//! needs numbers for ([`Count::counts`]), such as the ones ticked or cleared in "Choose
//! subfolders", and hears how far the whole count has got as it goes. Each folder keeps how many
//! folders have been found inside it and how many in its tree are still to read, so its count is
//! final when that reaches 0.

use super::{PathCountDto, SubfolderCountDto, Throttle};
use folderskin_core::apply::tree::{folders_in, Lookup, Order, Walk};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How often a count tells the webview how far it has got, at most.
const EVERY: Duration = Duration::from_millis(100);

/// The count of the folder on show. One at a time: picking another folder abandons it.
#[derive(Default)]
pub struct Counts(Mutex<Option<Arc<Count>>>);

impl Counts {
    /// Makes `count` the one going on, and stops the one before it.
    pub fn start(&self, count: Arc<Count>) {
        let mut current = lock(&self.0);
        if let Some(before) = current.replace(count) {
            before.stop.store(true, Ordering::SeqCst);
        }
    }

    /// The count going on (or finished) for `folder`, named as the webview named it.
    pub fn of(&self, folder: &str) -> Option<Arc<Count>> {
        lock(&self.0).clone().filter(|count| count.folder == folder)
    }
}

/// One folder's count.
pub struct Count {
    /// The folder as the webview named it, which is how it asks after it.
    pub folder: String,
    stop: AtomicBool,
    state: Mutex<Counted>,
    finished: Condvar,
}

struct Counted {
    walk: Walk,
    /// By folder number: how many folders have been found inside it so far.
    inside: Vec<u32>,
    /// By folder number: how many folders in its tree, itself included, are still to read.
    open: Vec<u32>,
}

impl Count {
    /// A count of `root` (canonical), not started.
    pub fn new(folder: String, root: PathBuf) -> Count {
        let walk = Walk::new(root, Order::Deepest);
        // A package has nothing to read, and so nothing open.
        let open = u32::from(!walk.is_done());
        Count {
            folder,
            stop: AtomicBool::new(false),
            state: Mutex::new(Counted {
                walk,
                inside: vec![0],
                open: vec![open],
            }),
            finished: Condvar::new(),
        }
    }

    /// Counts to the end, or until it's abandoned, telling `emit` how far it has got now and then
    /// and once more at the end.
    pub fn run(&self, emit: impl Fn(SubfolderCountDto)) {
        let mut throttle = Throttle::new(EVERY);
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return;
            }
            let next = lock(&self.state).walk.next_folder();
            let Some((folder, path)) = next else {
                break;
            };
            // Read without holding anything: a folder on a network disk can take a while.
            let inside = folders_in(&path);
            let snapshot = {
                let mut state = lock(&self.state);
                state.found(folder, inside);
                self.snapshot(&state)
            };
            if throttle.ready(Instant::now()) {
                emit(snapshot);
            }
        }
        let done = self.snapshot(&lock(&self.state));
        self.finished.notify_all();
        emit(done);
    }

    /// How far it has got, waiting up to `wait` for it to finish first, so a small folder's
    /// count is whole the first time the webview asks.
    pub fn wait(&self, wait: Duration) -> SubfolderCountDto {
        let state = lock(&self.state);
        let (state, _) = self
            .finished
            .wait_timeout_while(state, wait, |state| !state.walk.is_done())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.snapshot(&state)
    }

    /// How far it has got: the folders found inside so far, and whether that's all of them.
    pub fn now(&self) -> SubfolderCountDto {
        self.snapshot(&lock(&self.state))
    }

    /// Each of `paths` as far as the count has got: whether it has come to the folder, the
    /// folders found inside it so far, and whether that's final. A path the count hasn't come to
    /// is none of them, and one that isn't a folder of the tree (gone, or not one of the user's
    /// own) is found never, and final.
    pub fn counts(&self, paths: &[String]) -> Vec<PathCountDto> {
        let state = lock(&self.state);
        paths
            .iter()
            .map(|path| match state.walk.tree().find(Path::new(path)) {
                Lookup::At(k) => PathCountDto {
                    found: true,
                    inside: state.inside[k as usize].into(),
                    done: state.open[k as usize] == 0,
                },
                Lookup::Unread => PathCountDto::NOT_YET,
                Lookup::Missing => PathCountDto {
                    done: true,
                    ..PathCountDto::NOT_YET
                },
            })
            .collect()
    }

    fn snapshot(&self, state: &Counted) -> SubfolderCountDto {
        SubfolderCountDto {
            folder: self.folder.clone(),
            count: state.inside[0].into(),
            done: state.walk.is_done(),
        }
    }
}

impl Counted {
    /// `folder` has been read, and these are the folders inside it.
    fn found(&mut self, folder: u32, inside: Vec<std::ffi::OsString>) {
        let found = u32::try_from(inside.len()).unwrap_or(u32::MAX);
        let added = self.walk.found(
            folder,
            inside.into_iter().map(|name| (name, true)).collect(),
        );
        self.inside.resize(added.end as usize, 0);
        self.open.resize(added.end as usize, 1);
        // The folder is read (one fewer open) and has `found` more open inside it, and so does
        // every folder it's in.
        let mut at = Some(folder);
        while let Some(k) = at {
            let k_ = k as usize;
            self.inside[k_] += found;
            self.open[k_] = self.open[k_] + found - 1;
            at = self.walk.tree().parent(k);
        }
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

    fn count(scratch: &Scratch) -> Count {
        Count::new("the folder".into(), scratch.root())
    }

    #[test]
    fn a_count_finds_every_folder_inside_and_says_when_it_is_done() {
        let scratch = Scratch::with(&["a/a1/deep", "a/a2", "b", ".git/objects", "Thing.app/x"]);
        let count = count(&scratch);
        assert_eq!(
            count.now(),
            SubfolderCountDto {
                folder: "the folder".into(),
                count: 0,
                done: false
            }
        );
        let heard = Mutex::new(Vec::new());
        count.run(|dto| lock(&heard).push(dto));
        let heard = heard.into_inner().unwrap();
        assert_eq!(
            heard.last(),
            Some(&SubfolderCountDto {
                folder: "the folder".into(),
                count: 5,
                done: true
            })
        );
        assert!(
            heard.len() <= 2,
            "a small count is heard once or twice: {heard:?}"
        );
        assert_eq!(count.wait(Duration::ZERO), count.now());
    }

    #[test]
    fn each_folder_has_its_own_count_once_its_insides_are_read() {
        let scratch = Scratch::with(&["a/a1/deep", "a/a2", "b/b1", "c"]);
        let root = scratch.root();
        let count = count(&scratch);
        let paths: Vec<String> = ["", "a", "a/a1", "b", "c", "c/gone", "d/e"]
            .iter()
            .map(|p| root.join(p).to_string_lossy().into_owned())
            .collect();
        let at = |inside: u64, done: bool| PathCountDto {
            found: true,
            inside,
            done,
        };
        let never = PathCountDto {
            done: true,
            ..PathCountDto::NOT_YET
        };
        // Nothing read yet: nothing is known about any of them but the folder itself.
        let not_yet = [PathCountDto::NOT_YET; 6];
        assert_eq!(count.counts(&paths)[0], at(0, false));
        assert_eq!(count.counts(&paths)[1..], not_yet);

        // Read the root and all of a's tree (a, a1, deep and a2), the way a depth-first walk
        // comes to them.
        {
            let mut state = lock(&count.state);
            for _ in 0..5 {
                let (folder, path) = state.walk.next_folder().unwrap();
                let inside = folders_in(&path);
                state.found(folder, inside);
            }
        }
        assert_eq!(
            count.counts(&paths),
            [
                at(6, false),
                at(3, true),
                at(1, true),
                at(0, false),
                at(0, false),
                PathCountDto::NOT_YET,
                never,
            ],
            "a is finished, b and c aren't read, and d was never there"
        );

        count.run(|_| {});
        assert_eq!(
            count.counts(&paths),
            [
                at(7, true),
                at(3, true),
                at(1, true),
                at(1, true),
                at(0, true),
                never,
                never,
            ]
        );
    }

    #[test]
    fn a_count_that_is_abandoned_stops_where_it_is() {
        let scratch = Scratch::with(&["a", "b"]);
        let counts = Counts::default();
        let first = Arc::new(count(&scratch));
        counts.start(first.clone());
        assert!(counts.of("the folder").is_some());
        assert!(counts.of("another").is_none());
        let second = Arc::new(Count::new("another".into(), scratch.root()));
        counts.start(second.clone());
        assert!(
            counts.of("the folder").is_none(),
            "picking another folder abandons it"
        );
        let heard = Mutex::new(0);
        first.run(|_| *lock(&heard) += 1);
        assert_eq!(*lock(&heard), 0, "an abandoned count says nothing more");
        assert!(!first.now().done);
        second.run(|_| {});
        assert_eq!(second.now().count, 2);
    }

    #[test]
    fn a_package_has_nothing_inside_to_count() {
        let scratch = Scratch::with(&["Old.photoslibrary/originals"]);
        let count = Count::new("p".into(), scratch.root().join("Old.photoslibrary"));
        assert!(count.now().done, "known at once");
        count.run(|_| {});
        assert_eq!(count.now().count, 0);
    }
}
