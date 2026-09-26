//! "Include subfolders": a skin put on a folder and every folder inside it, and taken off
//! them again, however many there are.
//!
//! Three parts, all reading the tree through [`folderskin_core::apply::tree`], which leaves out
//! anything that isn't the user's own folder and never crosses into another volume:
//! - Counting ([`count`]): picking a folder starts a count of everything inside it in the
//!   background, which the webview hears grow ([`COUNT_EVENT`]) and asks for the counts of the
//!   folders it needs ([`subfolder_counts`]). Picking another folder abandons it.
//! - Choosing ([`subfolder_list`]): "Choose subfolders" reads one folder's own folders at a time,
//!   as each column opens, so a folder with a million folders inside opens at once. What's chosen
//!   comes back as rules ([`ChoiceDto`]), never a list of folders.
//! - The run ([`job`]): one at a time, in the background, going on while the webview is
//!   elsewhere, and kept after it ends so it can be carried on, tried again or undone by its id.

mod count;
mod job;

pub use count::Counts;
pub use job::{Runs, TreeRunEventDto};

use crate::state::AppState;
use count::Count;
use folderskin_core::apply::tree::{folders_in, has_folders, is_in_package, Choice};
use folderskin_core::apply::{
    apply_prepared, bytes_per_folder, has_custom_icon, prepare_icon, revert_icon, validate_folder,
    ApplyError,
};
use folderskin_core::compositor::ICON_SIZES;
use job::{Emit, Job, Kind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf, MAIN_SEPARATOR_STR};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// What the webview is told as a count grows: a [`SubfolderCountDto`].
pub const COUNT_EVENT: &str = "subfolder-count";
/// How long `subfolder_count` waits for a count to finish, so a folder with a few hundred folders
/// inside is counted by the time it answers, and "Counting" never flashes up for it.
const COUNT_WAIT: Duration = Duration::from_millis(80);
/// How long listing a column may spend looking inside its folders for folders of their own. The
/// rest are shown as maybe having some, and are read when they're opened.
const PEEK_FOR: Duration = Duration::from_millis(250);

/// Shown for a folder that was deleted or moved while a run was on its way to it.
const GONE: &str = "it isn't there any more";
/// Shown for a folder on a disk that can't be written to.
const READ_ONLY: &str = "its disk is read-only";
/// `EROFS`, the same number on macOS and Linux.
#[cfg(unix)]
const EROFS: i32 = 30;

/// How far a count of the folders inside one has got: `{"folder": "/Users/me/Projects",
/// "count": 12400, "done": false}`. `folder` is as the webview named it; `count` is the folders
/// found inside so far, and all of them once `done`.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct SubfolderCountDto {
    pub folder: String,
    pub count: u64,
    pub done: bool,
}

/// The count of the folders inside one, and of the folders inside each of some folders in it,
/// numbered alike: `paths[i]` is for the `i`th path asked about.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct SubfolderCountsDto {
    pub folder: String,
    pub count: u64,
    pub done: bool,
    pub paths: Vec<PathCountDto>,
}

/// One folder as far as a count has got: whether it has come to the folder, the folders found
/// inside it so far, and whether that's all of them.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathCountDto {
    pub found: bool,
    pub inside: u64,
    pub done: bool,
}

impl PathCountDto {
    /// A folder the count hasn't come to.
    pub const NOT_YET: PathCountDto = PathCountDto {
        found: false,
        inside: 0,
        done: false,
    };
}

/// One folder's own folders, for a column of "Choose subfolders":
///
/// ```json
/// {"path": "/Users/me/Projects", "separator": "/", "names": ["Clients", "Notes"],
///  "nested": [true, null]}
/// ```
///
/// `path` is the folder as a run names it (canonical), and a folder inside it is `path`,
/// `separator` and its name. `names` are exactly the folders a run finds there, and `nested[i]`
/// says whether `names[i]` has folders of its own: null when there wasn't time to look.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct SubfolderListDto {
    pub path: String,
    pub separator: String,
    pub names: Vec<String>,
    pub nested: Vec<Option<bool>>,
}

/// Which folders inside one a run takes, as "Choose subfolders" left them: every folder inside
/// is taken or not as `all` says, unless a rule on it or a folder it's in says otherwise, the
/// nearest rule winning. `{"all": true, "rules": [{"path": "/Users/me/Projects/Photos", "on":
/// false}]}` is everything but Photos and whatever is inside it.
#[derive(Deserialize, Debug, Clone)]
pub struct ChoiceDto {
    pub all: bool,
    pub rules: Vec<RuleDto>,
}

/// A folder ticked (`on`) or cleared, with everything inside it.
#[derive(Deserialize, Debug, Clone)]
pub struct RuleDto {
    pub path: String,
    pub on: bool,
}

/// The run's [`Choice`] from what the webview sent: every folder inside without one.
fn choice_of(choice: Option<ChoiceDto>) -> Choice {
    match choice {
        None => Choice::everything(),
        Some(choice) => Choice::new(
            choice.all,
            choice
                .rules
                .into_iter()
                .map(|rule| (PathBuf::from(rule.path), rule.on)),
        ),
    }
}

/// A run over a tree, as the webview sees it. `done` of `total` folders are finished, `total`
/// being the folders the run takes found so far, and every one of them once `counted`. `changed`,
/// `failed` and `skipped` (a revert leaving alone a folder with no icon of its own) add up to
/// `done`. `current` is the folder just finished, or the folder itself's name before any. A run
/// that ended with Stop and folders left is `stopped`. `failures` are the first of the folders
/// that failed, and `error` why a run couldn't go at all.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct TreeRunDto {
    pub id: u64,
    /// "apply" or "revert".
    pub kind: &'static str,
    /// A revert that takes off exactly what an apply put on.
    pub undoing: bool,
    /// A revert that leaves alone the folders with no icon of their own.
    pub leaves_plain: bool,
    /// The folder as the webview named it, and as the run names it (canonical).
    pub folder: String,
    pub root: String,
    pub name: String,
    pub skin_id: Option<String>,
    pub running: bool,
    pub stopping: bool,
    pub stopped: bool,
    pub done: u64,
    pub total: u64,
    pub counted: bool,
    pub current: String,
    pub changed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub failures: Vec<TreeFailureDto>,
    pub error: Option<String>,
}

/// A folder a run couldn't change, and why, as a sentence to show after its name.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct TreeFailureDto {
    pub path: String,
    pub name: String,
    pub reason: String,
}

/// What became of one folder that didn't fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Changed,
    Skipped,
}

// ---------- counting ----------

/// Starts counting the folders inside `folder` in the background, and stops counting whatever
/// was picked before. Answers with how far it has got once it's done, or after a moment, and the
/// rest comes as [`COUNT_EVENT`]s.
#[tauri::command]
pub async fn subfolder_count<R: Runtime>(
    app: AppHandle<R>,
    folder: String,
) -> Result<SubfolderCountDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        let count = Arc::new(Count::new(folder, root));
        app.state::<Counts>().start(count.clone());
        let counting = count.clone();
        std::thread::Builder::new()
            .name("folderskin-tree-count".into())
            .spawn(move || {
                counting.run(|dto| {
                    let _ = app.emit(COUNT_EVENT, dto);
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(count.wait(COUNT_WAIT))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The count of the folders inside `folder` so far, and inside each of `paths` (folders in it),
/// from the count `subfolder_count` started. Nothing is read from disk: a folder picked since,
/// or not counted at all, is 0 and not done.
#[tauri::command]
pub fn subfolder_counts(
    counts: State<'_, Counts>,
    folder: String,
    paths: Vec<String>,
) -> SubfolderCountsDto {
    match counts.of(&folder) {
        Some(count) => {
            let now = count.now();
            SubfolderCountsDto {
                folder,
                count: now.count,
                done: now.done,
                paths: count.counts(&paths),
            }
        }
        None => SubfolderCountsDto {
            folder,
            count: 0,
            done: false,
            paths: vec![PathCountDto::NOT_YET; paths.len()],
        },
    }
}

// ---------- choosing ----------

/// The folders directly inside `folder` a run would take, for one column of "Choose
/// subfolders", and whether each has folders inside it (see [`SubfolderListDto`]).
#[tauri::command]
pub async fn subfolder_list(folder: String) -> Result<SubfolderListDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        Ok(list(&dir, PEEK_FOR))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// [`subfolder_list`] for `dir` (canonical), spending at most `peek_for` looking inside the
/// folders it finds.
pub(crate) fn list(dir: &Path, peek_for: Duration) -> SubfolderListDto {
    let names = if is_in_package(dir) {
        Vec::new()
    } else {
        folders_in(dir)
    };
    let until = Instant::now() + peek_for;
    let nested = names
        .iter()
        .map(|name| (Instant::now() < until).then(|| has_folders(&dir.join(name))))
        .collect();
    SubfolderListDto {
        path: path_string(dir),
        separator: MAIN_SEPARATOR_STR.into(),
        names: names
            .iter()
            .map(|name| name.to_string_lossy().into_owned())
            .collect(),
        nested,
    }
}

/// How many bytes of disk each folder's copy of a skin's icon takes (see
/// [`bytes_per_folder`]), so the webview can say what a whole tree will use.
#[tauri::command]
pub async fn tree_bytes(state: State<'_, AppState>, skin_id: String) -> Result<u64, String> {
    let state = state.inner().clone();
    // One thread for all of it: on macOS the prepared icon is an AppKit object that stays on
    // the thread that made it.
    tauri::async_runtime::spawn_blocking(move || {
        let skin = state.resolve(&skin_id)?;
        let icon = prepare_icon(&skin.icon_set(&ICON_SIZES)).map_err(|e| e.to_string())?;
        bytes_per_folder(&icon).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- the run ----------

/// Starts putting a skin on `folder` and the folders inside it `choice` takes (all of them
/// without one), in the background. Refused while another run is going. Answers as it starts, and
/// the rest comes as [`job::EVENT`]s.
#[tauri::command]
pub async fn start_tree_apply<R: Runtime>(
    app: AppHandle<R>,
    folder: String,
    skin_id: String,
    choice: Option<ChoiceDto>,
) -> Result<TreeRunEventDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        // Refused before the run is anything: the skin has to be there.
        app.state::<AppState>().resolve(&skin_id)?;
        let runs = app.state::<Runs>();
        let kind = Kind::Apply {
            skin_id: skin_id.clone(),
        };
        let job = Job::new(runs.next_id(), kind, folder, root, choice_of(choice));
        runs.start(job, applying(&app, skin_id))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Starts taking the custom icons off `folder` and the folders inside it `choice` takes, in the
/// background, as [`start_tree_apply`] starts a run. `skip_plain` (yes unless it says otherwise)
/// leaves the folders with no icon of their own alone rather than writing to them.
#[tauri::command]
pub async fn start_tree_revert<R: Runtime>(
    app: AppHandle<R>,
    folder: String,
    choice: Option<ChoiceDto>,
    skip_plain: Option<bool>,
) -> Result<TreeRunEventDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        let runs = app.state::<Runs>();
        let kind = Kind::Revert {
            skip_plain: skip_plain.unwrap_or(true),
            undoing: false,
        };
        let job = Job::new(
            runs.next_id(),
            kind.clone(),
            folder,
            root,
            choice_of(choice),
        );
        runs.start(job, working(&app, &kind))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stops run `id` before its next folder. The folder being changed is finished.
#[tauri::command]
pub fn stop_tree_run<R: Runtime>(app: AppHandle<R>, id: u64) {
    app.state::<Runs>().stop(id, &emitter(&app));
}

/// Carries run `id` on from where it stopped, with the folders it didn't reach.
#[tauri::command]
pub fn carry_on_tree_run<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
) -> Result<TreeRunEventDto, String> {
    resume(&app, id, false)
}

/// Tries the folders run `id` couldn't change once more.
#[tauri::command]
pub fn retry_tree_run<R: Runtime>(app: AppHandle<R>, id: u64) -> Result<TreeRunEventDto, String> {
    resume(&app, id, true)
}

/// Takes off exactly what apply run `id` put on, as a run of its own.
#[tauri::command]
pub fn undo_tree_run<R: Runtime>(app: AppHandle<R>, id: u64) -> Result<TreeRunEventDto, String> {
    let kind = Kind::Revert {
        skip_plain: false,
        undoing: true,
    };
    app.state::<Runs>().undo(id, working(&app, &kind))
}

/// Forgets run `id` once it has ended: its summary was put away.
#[tauri::command]
pub fn dismiss_tree_run<R: Runtime>(app: AppHandle<R>, id: u64) {
    app.state::<Runs>().dismiss(id, &emitter(&app));
}

/// The latest run, going or ended, for a webview that has just loaded.
#[tauri::command]
pub fn tree_run(runs: State<'_, Runs>) -> TreeRunEventDto {
    runs.event()
}

/// Carries run `id` on, or with `retry` tries its failures again, doing what it did before.
fn resume<R: Runtime>(app: &AppHandle<R>, id: u64, retry: bool) -> Result<TreeRunEventDto, String> {
    let runs = app.state::<Runs>();
    let Some(kind) = runs.kind_of(id) else {
        return Err("that run isn't there any more".into());
    };
    runs.resume(id, retry, working(app, &kind))
}

/// What a run of `kind` does, as the work a run's thread is given.
fn working<R: Runtime>(
    app: &AppHandle<R>,
    kind: &Kind,
) -> Box<dyn FnOnce(Arc<Job>) + Send + 'static> {
    match kind {
        Kind::Apply { skin_id } => Box::new(applying(app, skin_id.clone())),
        Kind::Revert { skip_plain, .. } => {
            let (emit, skip_plain) = (emitter(app), *skip_plain);
            Box::new(move |job: Arc<Job>| {
                job.work(
                    || {
                        Ok(move |folder: &Path| {
                            if skip_plain && !has_custom_icon(folder) {
                                return Ok(Outcome::Skipped);
                            }
                            revert_icon(folder)
                                .map(|()| Outcome::Changed)
                                .map_err(|e| reason(&e))
                        })
                    },
                    &emit,
                )
            })
        }
    }
}

/// The work of a run that puts skin `skin_id` on its folders.
fn applying<R: Runtime>(
    app: &AppHandle<R>,
    skin_id: String,
) -> impl FnOnce(Arc<Job>) + Send + 'static {
    let (emit, state) = (emitter(app), app.state::<AppState>().inner().clone());
    move |job: Arc<Job>| {
        job.work(
            || {
                // Rendered and encoded once for every folder, on this thread: on macOS the
                // prepared icon is an AppKit image, which can't move to another.
                let skin = state.resolve(&skin_id)?;
                let icon = prepare_icon(&skin.icon_set(&ICON_SIZES)).map_err(|e| e.to_string())?;
                Ok(move |folder: &Path| {
                    apply_prepared(folder, &icon)
                        .map(|()| Outcome::Changed)
                        .map_err(|e| reason(&e))
                })
            },
            &emit,
        )
    }
}

/// Where a run's messages go: every webview, as [`job::EVENT`]s. The window may have gone, and
/// the run goes on all the same.
fn emitter<R: Runtime>(app: &AppHandle<R>) -> Emit {
    let app = app.clone();
    Arc::new(move |event| {
        let _ = app.emit(job::EVENT, event);
    })
}

// ---------- shared ----------

/// Lets a message through now and then: the first straight away, then none sooner than `every`
/// after the last. A revert gets through about a thousand folders a second, far more than anyone
/// can read, and every message is a render in the webview.
pub(crate) struct Throttle {
    every: Duration,
    last: Option<Instant>,
}

impl Throttle {
    pub fn new(every: Duration) -> Throttle {
        Throttle { every, last: None }
    }

    /// Whether a message may go at `now`. When it may, it's counted as sent.
    pub fn ready(&mut self, now: Instant) -> bool {
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < self.every)
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// Why one folder of a run couldn't be changed, as the sentence the webview shows after its
/// name.
pub(crate) fn reason(error: &ApplyError) -> String {
    match error {
        ApplyError::Io(e) => io_reason(e),
        // The folder was checked as it was reached, and wasn't a folder by then.
        ApplyError::NotADirectory(path) if path.symlink_metadata().is_err() => GONE.into(),
        ApplyError::NotADirectory(_) => "it isn't a folder any more".into(),
        other => other.to_string(),
    }
}

/// [`reason`] for an I/O error: the common ones in words, anything else as the OS put it.
fn io_reason(e: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::PermissionDenied => "you don't have permission to change it".into(),
        ErrorKind::NotFound => GONE.into(),
        ErrorKind::ReadOnlyFilesystem => READ_ONLY.into(),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => "its disk is full".into(),
        #[cfg(unix)]
        _ if e.raw_os_error() == Some(EROFS) => READ_ONLY.into(),
        _ => e.to_string(),
    }
}

/// A folder's name as the user knows it: the last part of its path, or the whole path for a
/// folder that has none.
fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A scratch folder for one test, removed when it drops.
    pub(crate) struct Scratch(pub PathBuf);

    impl Scratch {
        /// `<temp>/folderskin-tree-<process>-<n>`, with `dirs` (`/`-separated) made inside.
        pub(crate) fn with(dirs: &[&str]) -> Scratch {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "folderskin-tree-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            for dir in dirs {
                std::fs::create_dir_all(path.join(dir)).unwrap();
            }
            std::fs::create_dir_all(&path).unwrap();
            Scratch(path)
        }

        /// The folder as a run sees it: canonical.
        pub(crate) fn root(&self) -> PathBuf {
            validate_folder(&self.0).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_throttle_lets_one_message_through_in_each_stretch_of_time() {
        let mut throttle = Throttle::new(Duration::from_millis(80));
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        assert!(throttle.ready(at(0)), "the first goes at once");
        assert!(!throttle.ready(at(1)));
        assert!(!throttle.ready(at(79)));
        assert!(throttle.ready(at(80)));
        assert!(!throttle.ready(at(150)));
        assert!(
            throttle.ready(at(400)),
            "after a quiet stretch, straight away"
        );
        // A clock that seems to go back never lets a burst through.
        assert!(!throttle.ready(at(300)));
        // A thousand folders a second for two seconds: 25 messages, not 2,000.
        let mut throttle = Throttle::new(Duration::from_millis(80));
        let sent = (0..2000).filter(|&ms| throttle.ready(at(ms))).count();
        assert_eq!(sent, 25);
    }

    #[test]
    fn a_column_lists_one_folders_own_folders_and_which_have_more_inside() {
        let scratch = Scratch::with(&["b/inner", "a", ".git/objects", "Thing.app/Contents", "C"]);
        std::fs::write(scratch.0.join("a").join("notes.txt"), b"x").unwrap();
        let root = scratch.root();
        let listed = list(&root, PEEK_FOR);
        assert_eq!(listed.path, path_string(&root));
        assert_eq!(listed.separator, MAIN_SEPARATOR_STR);
        assert_eq!(listed.names, ["a", "b", "C"]);
        assert_eq!(listed.nested, [Some(false), Some(true), Some(false)]);
        // With no time to look, the folders are there all the same, maybe with folders inside.
        let quick = list(&root, Duration::ZERO);
        assert_eq!(quick.names, ["a", "b", "C"]);
        assert_eq!(quick.nested, [None, None, None]);
        // A package has nothing inside, as far as a run goes.
        assert!(list(&root.join("Thing.app"), PEEK_FOR).names.is_empty());
    }

    #[test]
    fn a_choice_from_the_webview_is_rules_by_path() {
        let dto: ChoiceDto = serde_json::from_value(json!({
            "all": false,
            "rules": [{"path": "/r/Photos", "on": true}, {"path": "/r/Photos/Raw", "on": false}]
        }))
        .unwrap();
        let choice = choice_of(Some(dto));
        assert!(!choice.all());
        assert!(choice.takes(Path::new("/r/Photos"), false));
        assert!(!choice.takes(Path::new("/r/Photos/Raw"), true));
        assert!(!choice.takes(Path::new("/r/Notes"), false));
        assert!(choice_of(None).takes(Path::new("/r/Notes"), true));
    }

    #[test]
    fn reasons_are_sentences_about_the_folder() {
        use std::io::{Error, ErrorKind};
        let io = |kind: ErrorKind| reason(&ApplyError::Io(Error::from(kind)));
        assert_eq!(
            io(ErrorKind::PermissionDenied),
            "you don't have permission to change it"
        );
        assert_eq!(io(ErrorKind::NotFound), "it isn't there any more");
        assert_eq!(io(ErrorKind::ReadOnlyFilesystem), "its disk is read-only");
        assert_eq!(io(ErrorKind::StorageFull), "its disk is full");
        #[cfg(unix)]
        assert_eq!(
            reason(&ApplyError::Io(Error::from_raw_os_error(EROFS))),
            "its disk is read-only"
        );
        let other = Error::other("the disk made a noise");
        assert_eq!(reason(&ApplyError::Io(other)), "the disk made a noise");
        let refused = "this is your home folder. Pick a folder inside it instead";
        assert_eq!(reason(&ApplyError::Refused(refused.into())), refused);
        let platform = "macOS refused to change this folder's icon (is it writable?)";
        assert_eq!(reason(&ApplyError::Platform(platform.into())), platform);

        let scratch = Scratch::with(&[]);
        let file = scratch.0.join("Report");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(
            reason(&ApplyError::NotADirectory(scratch.0.join("gone"))),
            "it isn't there any more"
        );
        assert_eq!(
            reason(&ApplyError::NotADirectory(file)),
            "it isn't a folder any more"
        );
    }

    #[test]
    fn what_the_webview_hears_has_the_shape_it_expects() {
        let run = TreeRunDto {
            id: 3,
            kind: "apply",
            undoing: false,
            leaves_plain: false,
            folder: "/a".into(),
            root: "/a".into(),
            name: "a".into(),
            skin_id: Some("user:1".into()),
            running: true,
            stopping: false,
            stopped: false,
            done: 2,
            total: 9,
            counted: false,
            current: "b".into(),
            changed: 1,
            failed: 1,
            skipped: 0,
            failures: vec![TreeFailureDto {
                path: "/a/b".into(),
                name: "b".into(),
                reason: GONE.into(),
            }],
            error: None,
        };
        assert_eq!(
            serde_json::to_value(TreeRunEventDto {
                seq: 7,
                run: Some(run)
            })
            .unwrap(),
            json!({"seq": 7, "run": {
                "id": 3, "kind": "apply", "undoing": false, "leaves_plain": false,
                "folder": "/a", "root": "/a", "name": "a", "skin_id": "user:1",
                "running": true, "stopping": false, "stopped": false,
                "done": 2, "total": 9, "counted": false, "current": "b",
                "changed": 1, "failed": 1, "skipped": 0,
                "failures": [{"path": "/a/b", "name": "b", "reason": "it isn't there any more"}],
                "error": null,
            }})
        );
        assert_eq!(
            serde_json::to_value(SubfolderCountsDto {
                folder: "/a".into(),
                count: 12,
                done: false,
                paths: vec![PathCountDto {
                    found: true,
                    inside: 3,
                    done: true
                }],
            })
            .unwrap(),
            json!({"folder": "/a", "count": 12, "done": false,
                   "paths": [{"found": true, "inside": 3, "done": true}]})
        );
        assert_eq!(
            serde_json::to_value(SubfolderListDto {
                path: "/a".into(),
                separator: "/".into(),
                names: vec!["b".into()],
                nested: vec![None],
            })
            .unwrap(),
            json!({"path": "/a", "separator": "/", "names": ["b"], "nested": [null]})
        );
    }

    // Not on Windows: with tauri's `test` feature the lib's test binary imports a WebView2 entry
    // point the runner's loader can't resolve, so it dies with STATUS_ENTRYPOINT_NOT_FOUND before
    // a single test runs. The commands themselves are checked on macOS and Linux, and the Windows
    // writer has its own tests in folderskin-core.
    #[cfg(not(windows))]
    mod ipc {
        use super::*;
        use crate::store::{self, NewSkin, SkinImage, SkinSource};
        use serde_json::Value;
        use std::sync::Mutex;

        /// Tests that start runs take turns: a mock app's runs are its own, but the icons they
        /// write aren't.
        static ONE_RUN_AT_A_TIME: Mutex<()> = Mutex::new(());

        /// A mock app with the subfolder commands.
        fn app() -> (
            tauri::App<tauri::test::MockRuntime>,
            tauri::WebviewWindow<tauri::test::MockRuntime>,
        ) {
            let app = tauri::test::mock_builder()
                .manage(AppState::default())
                .manage(Counts::default())
                .manage(Runs::default())
                .invoke_handler(tauri::generate_handler![
                    subfolder_count,
                    subfolder_counts,
                    subfolder_list,
                    tree_bytes,
                    start_tree_apply,
                    start_tree_revert,
                    stop_tree_run,
                    carry_on_tree_run,
                    retry_tree_run,
                    undo_tree_run,
                    dismiss_tree_run,
                    tree_run
                ])
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            (app, webview)
        }

        /// The command as the webview calls it, with its arguments named the webview's way.
        fn invoke(
            webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
            cmd: &str,
            args: Value,
        ) -> Result<Value, Value> {
            tauri::test::get_ipc_response(
                webview,
                tauri::webview::InvokeRequest {
                    cmd: cmd.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "tauri://localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(args),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .map(|b| b.deserialize::<Value>().unwrap())
        }

        /// Asks for the latest run until it has ended, and returns it.
        fn ended(webview: &tauri::WebviewWindow<tauri::test::MockRuntime>) -> Value {
            let started = Instant::now();
            loop {
                let run = invoke(webview, "tree_run", json!({})).unwrap()["run"].clone();
                if run["running"] == false {
                    return run;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(60),
                    "the run never ended"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        /// A skin kept for the session, as the library has it, and its id.
        fn keep_skin(app: &tauri::App<tauri::test::MockRuntime>) -> String {
            let id = store::skin_id(b"tree");
            let skin = SkinImage::Folder(Arc::new(image::RgbaImage::from_pixel(
                96,
                80,
                image::Rgba([0x2E, 0x7D, 0x5B, 0xFF]),
            )));
            let new = NewSkin {
                id: id.clone(),
                name: "Tree".into(),
                source: SkinSource::Import,
                provider: None,
                model: None,
                idea: None,
                tags: Vec::new(),
                pack: None,
                pack_name: None,
                author: None,
                license: None,
                pack_hash: None,
            };
            app.state::<AppState>().keep_unsaved(new, skin);
            id
        }

        #[test]
        fn a_tree_is_counted_listed_and_counted_by_folder_through_the_ipc() {
            let scratch = Scratch::with(&["a/a1", "b", ".git/objects"]);
            std::fs::write(scratch.0.join("notes.txt"), b"x").unwrap();
            let root = scratch.root();
            let s = |p: &Path| path_string(p);
            let (_app, webview) = app();
            let folder = scratch.0.to_string_lossy().into_owned();

            // A small tree is counted by the time the command answers.
            let count = invoke(&webview, "subfolder_count", json!({"folder": folder})).unwrap();
            assert_eq!(count, json!({"folder": folder, "count": 3, "done": true}));
            let refused = invoke(&webview, "subfolder_count", json!({"folder": "/"})).unwrap_err();
            assert_eq!(
                refused,
                "this is the root of a drive, not a folder FolderSkin can skin"
            );

            let paths = [
                s(&root.join("a")),
                s(&root.join("b")),
                s(&root.join("gone")),
            ];
            let counts = invoke(
                &webview,
                "subfolder_counts",
                json!({"folder": folder, "paths": paths}),
            )
            .unwrap();
            assert_eq!(
                counts,
                json!({"folder": folder, "count": 3, "done": true, "paths": [
                    {"found": true, "inside": 1, "done": true},
                    {"found": true, "inside": 0, "done": true},
                    {"found": false, "inside": 0, "done": true}
                ]})
            );
            let elsewhere = invoke(
                &webview,
                "subfolder_counts",
                json!({"folder": "/not/counted", "paths": [s(&root)]}),
            )
            .unwrap();
            assert_eq!(
                elsewhere["paths"],
                json!([{"found": false, "inside": 0, "done": false}])
            );

            let listed = invoke(&webview, "subfolder_list", json!({"folder": folder})).unwrap();
            assert_eq!(
                listed,
                json!({"path": s(&root), "separator": MAIN_SEPARATOR_STR, "names": ["a", "b"],
                       "nested": [true, false]})
            );
        }

        #[test]
        fn a_revert_runs_in_the_background_and_is_put_away_after() {
            let _turn = ONE_RUN_AT_A_TIME
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let scratch = Scratch::with(&["a/a1", "b", ".git/objects"]);
            let root = scratch.root();
            let (_app, webview) = app();
            let folder = scratch.0.to_string_lossy().into_owned();

            assert_eq!(
                invoke(&webview, "tree_run", json!({})).unwrap()["run"],
                Value::Null,
                "no run yet"
            );
            // Nothing in the tree wears an icon, so taking them off writes nothing anywhere.
            let started = invoke(&webview, "start_tree_revert", json!({"folder": folder})).unwrap();
            let id = started["run"]["id"].clone();
            assert_eq!(started["run"]["kind"], "revert");
            assert_eq!(started["run"]["leaves_plain"], true);
            let run = ended(&webview);
            assert_eq!(run["id"], id);
            assert_eq!(
                (
                    &run["done"],
                    &run["total"],
                    &run["skipped"],
                    &run["changed"]
                ),
                (&json!(4), &json!(4), &json!(4), &json!(0))
            );
            assert_eq!(run["counted"], true);
            assert_eq!(run["root"], path_string(&root));
            assert_eq!(run["folder"], folder);

            // Carrying on a run that finished does nothing more, and an unknown one is refused.
            assert_eq!(
                invoke(&webview, "carry_on_tree_run", json!({"id": 999})).unwrap_err(),
                "that run isn't there any more"
            );

            // The chosen ones only: nothing inside but a1.
            let chosen = json!({"all": false, "rules": [{"path": path_string(&root.join("a").join("a1")), "on": true}]});
            invoke(
                &webview,
                "start_tree_revert",
                json!({"folder": folder, "choice": chosen, "skipPlain": true}),
            )
            .unwrap();
            let run = ended(&webview);
            assert_eq!((&run["total"], &run["skipped"]), (&json!(2), &json!(2)));

            // Applying needs a skin that's there.
            let unknown = json!({"folder": folder, "skinId": "user:000000000000"});
            assert_eq!(
                invoke(&webview, "start_tree_apply", unknown).unwrap_err(),
                "that skin isn't available any more"
            );

            // Put away once it has ended, there's no run to show.
            let id = run["id"].as_u64().unwrap();
            invoke(&webview, "dismiss_tree_run", json!({"id": id})).unwrap();
            assert_eq!(
                invoke(&webview, "tree_run", json!({})).unwrap()["run"],
                Value::Null
            );
        }

        #[test]
        #[cfg_attr(
            target_os = "macos",
            ignore = "writes real Finder icons: cargo test -p folderskin -- --ignored whole_tree"
        )]
        fn a_skin_goes_on_a_whole_tree_and_comes_off_again_through_the_ipc() {
            let _turn = ONE_RUN_AT_A_TIME
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let scratch = Scratch::with(&["a/a1", "b"]);
            let root = scratch.root();
            let (app, webview) = app();
            let skin_id = keep_skin(&app);
            let folders = [
                root.clone(),
                root.join("a"),
                root.join("b"),
                root.join("a").join("a1"),
            ];
            let folder = path_string(&root);

            let bytes = invoke(&webview, "tree_bytes", json!({"skinId": skin_id})).unwrap();
            assert!(bytes.as_u64().unwrap() > 0, "{bytes}");

            let started = Instant::now();
            let args = json!({"folder": folder, "skinId": skin_id});
            invoke(&webview, "start_tree_apply", args).unwrap();
            let run = ended(&webview);
            let took = started.elapsed();
            assert_eq!(
                (&run["changed"], &run["failed"], &run["total"]),
                (&json!(4), &json!(0), &json!(4))
            );
            assert!(folders.iter().all(|f| has_custom_icon(f)));

            // Undo: exactly the folders the run changed.
            let id = run["id"].clone();
            invoke(&webview, "undo_tree_run", json!({"id": id})).unwrap();
            let undone = ended(&webview);
            assert_eq!(undone["undoing"], true);
            assert_eq!(undone["changed"], 4);
            assert!(folders.iter().all(|f| !has_custom_icon(f)));
            println!("4 folders in {took:?}; {bytes} bytes each");
        }
    }
}
