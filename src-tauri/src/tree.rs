//! "Include subfolders": a skin put on a folder and every folder inside it, and taken off
//! them again.
//!
//! The folders come from [`subfolders`], which leaves out anything that isn't the user's own
//! folder, nearest first. A run goes through them in order on one blocking thread, telling the
//! webview about each on a channel, and can be stopped between two folders. A folder that fails
//! doesn't end the run: it is reported with the reason, so the webview can offer to try it
//! again, and a run can be carried on, retried or undone by naming exactly its folders.

use crate::state::AppState;
use folderskin_core::apply::tree::{subfolders, MAX_TREE};
use folderskin_core::apply::{
    apply_prepared, bytes_per_folder, has_custom_icon, prepare_icon, refresh_shell_icons,
    revert_icon, validate_folder, ApplyError,
};
use folderskin_core::compositor::ICON_SIZES;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::ipc::Channel;
use tauri::State;

/// Set by [`stop_tree_run`]; cleared as every run starts and checked before each folder.
static STOP: AtomicBool = AtomicBool::new(false);

/// Shown for a folder that was deleted or moved while a run was on its way to it.
const GONE: &str = "it isn't there any more";
/// Shown for a folder on a disk that can't be written to.
const READ_ONLY: &str = "its disk is read-only";
/// `EROFS`, the same number on macOS and Linux.
#[cfg(unix)]
const EROFS: i32 = 30;

/// How many folders are inside one: `{"count": 12, "more": false}`. `more` is true when there
/// are more than [`MAX_TREE`], too many for a run, and `count` then stops at that.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct SubfolderCountDto {
    pub count: usize,
    pub more: bool,
}

/// How far a run has got: `{"done": 3, "total": 12, "name": "Invoices"}`, `name` being the folder
/// just finished (the folder the run started from while `done` is 0).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct TreeProgressDto {
    pub done: usize,
    pub total: usize,
    pub name: String,
}

/// A folder a run couldn't change, and why, as a sentence to show after its name.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TreeFailureDto {
    pub path: String,
    pub name: String,
    pub reason: String,
}

/// How a run went. Every folder it was given is in exactly one of `changed`, `failed`,
/// `skipped` (a count) or `remaining` (not reached because it was stopped), so they add up to
/// `total`. Paths are the canonical ones the run used, ready to hand back as `only`.
#[derive(Serialize, Debug, Default, PartialEq, Eq)]
pub struct TreeRunDto {
    pub total: usize,
    pub changed: Vec<String>,
    pub failed: Vec<TreeFailureDto>,
    /// Folders left alone: those with no icon to take off, when reverting a whole tree.
    pub skipped: usize,
    pub remaining: Vec<String>,
    pub stopped: bool,
}

/// What became of one folder that didn't fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Changed,
    Skipped,
}

// ---------- commands ----------

/// How many folders `folder` has inside it, not counting itself, as a run would find them.
#[tauri::command]
pub async fn subfolder_count(folder: String) -> Result<SubfolderCountDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        let found = subfolders(&root, MAX_TREE);
        Ok(SubfolderCountDto {
            count: found.folders.len(),
            more: found.more,
        })
    })
    .await
    .map_err(|e| e.to_string())?
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

/// Puts a skin on `folder` and every folder inside it, or with `only` on exactly those folders
/// (each the folder itself or inside it), in the order given: how a stopped run carries on and
/// failed folders are tried again. `on_progress` hears `done: 0` once the folders are known and
/// then each folder as it is finished.
#[tauri::command]
pub async fn apply_skin_tree(
    state: State<'_, AppState>,
    folder: String,
    skin_id: String,
    only: Option<Vec<String>>,
    on_progress: Channel<TreeProgressDto>,
) -> Result<TreeRunDto, String> {
    STOP.store(false, Ordering::SeqCst);
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        let skin = state.resolve(&skin_id)?;
        let folders = plan(&root, only.as_deref(), MAX_TREE)?;
        // The window may have gone, and the run goes on all the same.
        let progress = |p: TreeProgressDto| {
            let _ = on_progress.send(p);
        };
        run_tree(&root, &folders, &STOP, progress, || {
            // Rendered and encoded once for every folder, on this thread: on macOS the prepared
            // icon is an AppKit image, which can't move to another.
            let icon = prepare_icon(&skin.icon_set(&ICON_SIZES)).map_err(|e| e.to_string())?;
            Ok(move |folder: &Path| {
                apply_prepared(folder, &icon)
                    .map(|()| Outcome::Changed)
                    .map_err(|e| reason(&e))
            })
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Takes the custom icon off `folder` and every folder inside it that has one (the others are
/// `skipped`), or with `only` off exactly those folders, as undoing a run does. Planned,
/// reported and stopped like [`apply_skin_tree`].
#[tauri::command]
pub async fn revert_skin_tree(
    folder: String,
    only: Option<Vec<String>>,
    on_progress: Channel<TreeProgressDto>,
) -> Result<TreeRunDto, String> {
    STOP.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let root = validate_folder(Path::new(&folder)).map_err(|e| e.to_string())?;
        // Undoing a run takes off what it put on. A whole tree has folders that never had an
        // icon, and those are left alone rather than written to.
        let every_folder = only.is_none();
        let folders = plan(&root, only.as_deref(), MAX_TREE)?;
        let progress = |p: TreeProgressDto| {
            let _ = on_progress.send(p);
        };
        run_tree(&root, &folders, &STOP, progress, || {
            Ok(move |folder: &Path| {
                if every_folder && !has_custom_icon(folder) {
                    return Ok(Outcome::Skipped);
                }
                revert_icon(folder)
                    .map(|()| Outcome::Changed)
                    .map_err(|e| reason(&e))
            })
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stops the run in progress before its next folder. The folder being changed is finished.
#[tauri::command]
pub fn stop_tree_run() {
    STOP.store(true, Ordering::SeqCst);
}

// ---------- the run (unit-tested below) ----------

/// The folders a run in `root` goes through: without `only`, `root` and then every folder
/// inside it, nearest first, refused when there are more than `limit` of those; with `only`,
/// exactly those, in the order given, each of which must be `root` or inside it.
///
/// `root` is canonical, as [`validate_folder`] returns it.
pub(crate) fn plan(
    root: &Path,
    only: Option<&[String]>,
    limit: usize,
) -> Result<Vec<PathBuf>, String> {
    let Some(only) = only else {
        let found = subfolders(root, limit);
        if found.more {
            return Err(format!(
                "{} has more than {} folders inside it, too many to change at once. Pick a folder \
                 further down.",
                folder_name(root),
                thousands(limit)
            ));
        }
        return Ok(std::iter::once(root.to_path_buf())
            .chain(found.folders)
            .collect());
    };
    only.iter()
        .map(|path| within(root, Path::new(path)))
        .collect()
}

/// `path` as one of the folders of a run in `root`: canonical, and `root` or inside it.
///
/// A path that can't be used as it is, most likely a folder deleted since the run that named it,
/// is kept as given when it is still plainly under `root`, so that folder fails on its own with
/// a reason rather than the whole run being refused. Like every folder, it is checked again when
/// the run reaches it.
fn within(root: &Path, path: &Path) -> Result<PathBuf, String> {
    match validate_folder(path) {
        Ok(canonical) if canonical.starts_with(root) => Ok(canonical),
        Ok(_) => Err(outside(root, path)),
        Err(_) if is_plain_absolute(path) && path.starts_with(root) && leads_into(root, path) => {
            Ok(path.to_path_buf())
        }
        Err(_) => Err(outside(root, path)),
    }
}

/// True for an absolute path without `.` or `..` in it, which can't climb out of a folder it
/// starts with.
fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|part| {
            matches!(
                part,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            )
        })
}

/// True when the nearest folder along `path` that is still there is `root` or inside it once
/// resolved, so no link on the way leads out of `root`.
fn leads_into(root: &Path, path: &Path) -> bool {
    path.ancestors()
        .find_map(|dir| validate_folder(dir).ok())
        .is_some_and(|real| real.starts_with(root))
}

/// The refusal for a folder that isn't in the run's folder.
fn outside(root: &Path, path: &Path) -> String {
    format!(
        "{} isn't inside {}, so it can't be changed along with it",
        folder_name(path),
        folder_name(root)
    )
}

/// Goes through `folders` in order, doing to each what `prepare` returns, and says how it went.
///
/// `progress` hears `done: 0` with `root`'s name first, then `done: i + 1` with each folder's
/// name as it is finished, whether it changed, was skipped or failed. `prepare` is called once,
/// just before the first folder, so its slow part (rendering the icon) starts after the webview
/// knows the total, and never when the run is stopped first or has nothing to do; an error from
/// it is the run's, and no folder has been touched. `stop` is checked before each folder: once
/// it is set, the folders not reached are `remaining` and the run is `stopped`.
pub(crate) fn run_tree<P, A>(
    root: &Path,
    folders: &[PathBuf],
    stop: &AtomicBool,
    mut progress: impl FnMut(TreeProgressDto),
    prepare: P,
) -> Result<TreeRunDto, String>
where
    P: FnOnce() -> Result<A, String>,
    A: FnMut(&Path) -> Result<Outcome, String>,
{
    let total = folders.len();
    progress(TreeProgressDto {
        done: 0,
        total,
        name: folder_name(root),
    });
    let mut run = TreeRunDto {
        total,
        ..TreeRunDto::default()
    };
    let mut prepare = Some(prepare);
    let mut act: Option<A> = None;
    for (i, folder) in folders.iter().enumerate() {
        if stop.load(Ordering::SeqCst) {
            run.stopped = true;
            run.remaining = folders[i..].iter().map(|f| path_string(f)).collect();
            break;
        }
        let act = match &mut act {
            Some(act) => act,
            none => none.insert(prepare.take().expect("the action is made once")()?),
        };
        let name = folder_name(folder);
        match act(folder) {
            Ok(Outcome::Changed) => run.changed.push(path_string(folder)),
            Ok(Outcome::Skipped) => run.skipped += 1,
            Err(reason) => run.failed.push(TreeFailureDto {
                path: path_string(folder),
                name: name.clone(),
                reason,
            }),
        }
        progress(TreeProgressDto {
            done: i + 1,
            total,
            name,
        });
    }
    if !run.changed.is_empty() {
        // Once for the whole run, not once per folder: on Windows this refreshes every view.
        refresh_shell_icons();
    }
    Ok(run)
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

/// `n` with its thousands marked, as the sentences write numbers: 5,000.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;

    /// A scratch folder for one test, removed when it drops.
    struct Scratch(PathBuf);

    impl Scratch {
        /// `<temp>/folderskin-tree-<process>-<n>`, with `dirs` (`/`-separated) made inside.
        fn with(dirs: &[&str]) -> Scratch {
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
        fn root(&self) -> PathBuf {
            validate_folder(&self.0).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A per-folder action that captures nothing.
    type Act = fn(&Path) -> Result<Outcome, String>;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| Path::new("/work/Projects").join(n))
            .collect()
    }

    fn step(done: usize, total: usize, name: &str) -> TreeProgressDto {
        TreeProgressDto {
            done,
            total,
            name: name.into(),
        }
    }

    /// Runs `folders` in `/work/Projects` with `act`, returning the run and every progress
    /// message.
    fn run_with(
        folders: &[PathBuf],
        stop: &AtomicBool,
        act: impl FnMut(&Path) -> Result<Outcome, String>,
    ) -> (TreeRunDto, Vec<TreeProgressDto>) {
        let mut heard = Vec::new();
        let run = run_tree(
            Path::new("/work/Projects"),
            folders,
            stop,
            |p| heard.push(p),
            || Ok(act),
        )
        .unwrap();
        (run, heard)
    }

    #[test]
    fn a_run_goes_in_order_and_reports_each_folder_as_it_finishes() {
        let folders = paths(&["", "a", "b", "a/deep"]);
        let mut seen = Vec::new();
        let (run, heard) = run_with(&folders, &AtomicBool::new(false), |f| {
            seen.push(f.to_path_buf());
            Ok(Outcome::Changed)
        });
        assert_eq!(seen, folders);
        assert_eq!(
            heard,
            [
                step(0, 4, "Projects"),
                step(1, 4, "Projects"),
                step(2, 4, "a"),
                step(3, 4, "b"),
                step(4, 4, "deep"),
            ]
        );
        assert_eq!(
            run,
            TreeRunDto {
                total: 4,
                changed: folders.iter().map(|f| path_string(f)).collect(),
                ..TreeRunDto::default()
            }
        );
    }

    #[test]
    fn a_folder_that_fails_is_reported_and_the_run_goes_on() {
        let folders = paths(&["", "locked", "open", "gone"]);
        let (run, heard) = run_with(&folders, &AtomicBool::new(false), |f| {
            match f.file_name().and_then(|n| n.to_str()) {
                Some("locked") => Err("you don't have permission to change it".into()),
                Some("open") => Ok(Outcome::Skipped),
                Some("gone") => Err(GONE.into()),
                _ => Ok(Outcome::Changed),
            }
        });
        assert_eq!(run.changed, [path_string(&folders[0])]);
        assert_eq!(run.skipped, 1);
        assert_eq!(
            run.failed,
            [
                TreeFailureDto {
                    path: path_string(&folders[1]),
                    name: "locked".into(),
                    reason: "you don't have permission to change it".into(),
                },
                TreeFailureDto {
                    path: path_string(&folders[3]),
                    name: "gone".into(),
                    reason: GONE.into(),
                },
            ]
        );
        assert!(run.remaining.is_empty() && !run.stopped);
        assert_eq!(heard.len(), 5, "every folder is reported, failed or not");
        assert_eq!(heard[4], step(4, 4, "gone"));
    }

    #[test]
    fn a_stopped_run_says_which_folders_it_did_not_reach() {
        let folders = paths(&["", "a", "b", "c"]);
        let stop = AtomicBool::new(false);
        let mut calls = 0;
        let (run, heard) = run_with(&folders, &stop, |_| {
            calls += 1;
            if calls == 2 {
                // Stop is pressed while the second folder is being changed; it is finished.
                stop.store(true, Ordering::SeqCst);
            }
            Ok(Outcome::Changed)
        });
        assert_eq!(calls, 2);
        assert!(run.stopped);
        assert_eq!(run.changed.len(), 2);
        assert_eq!(
            run.remaining,
            [path_string(&folders[2]), path_string(&folders[3])]
        );
        assert_eq!(heard.last(), Some(&step(2, 4, "a")));
        assert_eq!(
            run.changed.len() + run.failed.len() + run.skipped + run.remaining.len(),
            run.total
        );
    }

    #[test]
    fn the_action_is_made_once_just_before_the_first_folder() {
        let folders = paths(&["", "a"]);
        let made = AtomicUsize::new(0);
        let heard = std::cell::Cell::new(0);
        let run = run_tree(
            Path::new("/work/Projects"),
            &folders,
            &AtomicBool::new(false),
            |_| heard.set(heard.get() + 1),
            || {
                made.fetch_add(1, Ordering::SeqCst);
                // The webview already knows the total when the slow part starts.
                assert_eq!(heard.get(), 1);
                Ok(|_: &Path| Ok(Outcome::Changed))
            },
        )
        .unwrap();
        assert_eq!(made.load(Ordering::SeqCst), 1);
        assert_eq!(heard.get(), 3);
        assert_eq!(run.changed.len(), 2);

        // Stopped before it began, or nothing to do: nothing is rendered at all.
        let never = || -> Result<Act, String> { panic!("prepared for no folder") };
        let stopped = run_tree(
            Path::new("/work/Projects"),
            &folders,
            &AtomicBool::new(true),
            |_| {},
            never,
        )
        .unwrap();
        assert!(stopped.stopped);
        assert_eq!(stopped.remaining.len(), 2);
        let empty = run_tree(
            Path::new("/work/Projects"),
            &[],
            &AtomicBool::new(false),
            |_| {},
            never,
        )
        .unwrap();
        assert_eq!(empty, TreeRunDto::default());

        // A failure to prepare is the run's, before any folder is touched.
        let failed = run_tree(
            Path::new("/work/Projects"),
            &folders,
            &AtomicBool::new(false),
            |_| {},
            || -> Result<Act, String> { Err("macOS could not read the 16 px icon".into()) },
        );
        assert_eq!(failed.unwrap_err(), "macOS could not read the 16 px icon");
    }

    #[test]
    fn a_whole_tree_is_the_folder_then_everything_inside_it() {
        let scratch = Scratch::with(&["b", "a/deep", ".git/objects"]);
        let root = scratch.root();
        let planned = plan(&root, None, MAX_TREE).unwrap();
        assert_eq!(
            planned,
            [
                root.clone(),
                root.join("a"),
                root.join("b"),
                root.join("a").join("deep")
            ]
        );

        let refused = plan(&root, None, 2).unwrap_err();
        let name = folder_name(&root);
        assert_eq!(
            refused,
            format!(
                "{name} has more than 2 folders inside it, too many to change at once. Pick a \
                 folder further down."
            )
        );
        assert!(plan(&root, None, 3).is_ok(), "exactly the limit is fine");
    }

    #[test]
    fn only_names_folders_of_the_run_and_nothing_outside_it() {
        let scratch = Scratch::with(&["a/deep", "b"]);
        let root = scratch.root();
        let elsewhere = Scratch::with(&["theirs"]);
        let s = |p: &Path| path_string(p);

        // Exactly those, in the order given, canonical.
        let given = [s(&root.join("b")), s(&root), s(&root.join("a/./deep"))];
        assert_eq!(
            plan(&root, Some(&given), MAX_TREE).unwrap(),
            [root.join("b"), root.clone(), root.join("a").join("deep")]
        );
        assert_eq!(
            plan(&root, Some(&[]), MAX_TREE).unwrap(),
            Vec::<PathBuf>::new()
        );

        // A folder deleted since is kept, to fail on its own.
        let gone = root.join("a").join("gone");
        assert_eq!(
            plan(&root, Some(&[s(&gone)]), MAX_TREE).unwrap(),
            std::slice::from_ref(&gone)
        );

        let root_name = folder_name(&root);
        for path in [
            elsewhere.root().join("theirs"),
            elsewhere.root(),
            root.join("..")
                .join(root.file_name().unwrap())
                .join("..")
                .join("x"),
            root.parent().unwrap().to_path_buf(),
            PathBuf::from("relative/a"),
        ] {
            let refused = plan(&root, Some(&[s(&root.join("b")), s(&path)]), MAX_TREE)
                .expect_err("outside the run's folder");
            assert!(
                refused.ends_with(&format!(
                    " isn't inside {root_name}, so it can't be changed along with it"
                )),
                "{refused}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_the_folder_to_one_outside_it_is_refused() {
        let scratch = Scratch::with(&["a"]);
        let root = scratch.root();
        let elsewhere = Scratch::with(&["theirs"]);
        std::os::unix::fs::symlink(elsewhere.root(), root.join("link")).unwrap();
        for past_the_link in ["theirs", "not there"] {
            let through = path_string(&root.join("link").join(past_the_link));
            assert!(
                plan(&root, Some(&[through]), MAX_TREE).is_err(),
                "{past_the_link}"
            );
        }
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
    fn numbers_in_sentences_have_their_thousands_marked() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(5_000), "5,000");
        assert_eq!(thousands(12_345), "12,345");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn the_run_serialises_the_shape_the_webview_expects() {
        let run = TreeRunDto {
            total: 3,
            changed: vec!["/a".into()],
            failed: vec![TreeFailureDto {
                path: "/a/b".into(),
                name: "b".into(),
                reason: GONE.into(),
            }],
            skipped: 0,
            remaining: vec!["/a/c".into()],
            stopped: true,
        };
        assert_eq!(
            serde_json::to_value(&run).unwrap(),
            json!({
                "total": 3,
                "changed": ["/a"],
                "failed": [{"path": "/a/b", "name": "b", "reason": "it isn't there any more"}],
                "skipped": 0,
                "remaining": ["/a/c"],
                "stopped": true,
            })
        );
        assert_eq!(
            serde_json::to_value(step(2, 9, "Invoices")).unwrap(),
            json!({"done": 2, "total": 9, "name": "Invoices"})
        );
        assert_eq!(
            serde_json::to_value(SubfolderCountDto {
                count: 12,
                more: false
            })
            .unwrap(),
            json!({"count": 12, "more": false})
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
        use std::sync::{Arc, Mutex};
        // ---------- through the IPC ----------

        /// Tests that start or stop runs take turns: the stop flag belongs to the whole process.
        static ONE_RUN_AT_A_TIME: Mutex<()> = Mutex::new(());

        /// What a mock app's progress channels were sent, and when to press Stop.
        #[derive(Default)]
        struct Heard {
            messages: Mutex<Vec<Value>>,
            /// Stop the run once the message with this `done` arrives; 0 never does.
            stop_at: AtomicUsize,
        }

        impl Heard {
            fn take(&self) -> Vec<Value> {
                std::mem::take(&mut *self.messages.lock().unwrap())
            }
        }

        /// A mock app with the tree commands, and what its channels hear.
        fn app() -> (
            tauri::App<tauri::test::MockRuntime>,
            tauri::WebviewWindow<tauri::test::MockRuntime>,
            Arc<Heard>,
        ) {
            let heard = Arc::new(Heard::default());
            let sink = heard.clone();
            let app = tauri::test::mock_builder()
                .channel_interceptor(move |_, _, _, body| {
                    if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                        let message: Value = serde_json::from_str(json).unwrap();
                        let stop_at = sink.stop_at.load(Ordering::SeqCst);
                        if stop_at > 0 && message["done"] == stop_at {
                            // As the user pressing Stop while that folder was being changed.
                            stop_tree_run();
                        }
                        sink.messages.lock().unwrap().push(message);
                    }
                    true
                })
                .manage(AppState::default())
                .invoke_handler(tauri::generate_handler![
                    subfolder_count,
                    tree_bytes,
                    apply_skin_tree,
                    revert_skin_tree,
                    stop_tree_run
                ])
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            (app, webview, heard)
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

        /// A skin kept for the session, as the library has it, and its id.
        fn keep_skin(app: &tauri::App<tauri::test::MockRuntime>) -> String {
            use tauri::Manager;
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

        const CHANNEL: &str = "__CHANNEL__:7";

        #[test]
        fn a_tree_is_counted_planned_reported_and_stopped_through_the_ipc() {
            let _turn = ONE_RUN_AT_A_TIME
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let scratch = Scratch::with(&["a/a1", "b", ".git/objects"]);
            std::fs::write(scratch.0.join("notes.txt"), b"x").unwrap();
            let root = scratch.root();
            let s = |p: &Path| path_string(p);
            let (app, webview, heard) = app();

            let count = invoke(&webview, "subfolder_count", json!({"folder": s(&root)})).unwrap();
            assert_eq!(count, json!({"count": 3, "more": false}));
            let refused = invoke(&webview, "subfolder_count", json!({"folder": "/"})).unwrap_err();
            assert_eq!(
                refused,
                "this is the root of a drive, not a folder FolderSkin can skin"
            );

            // Nothing in the tree wears an icon, so a whole-tree revert writes nothing anywhere.
            let args = json!({"folder": s(&root), "onProgress": CHANNEL});
            let run = invoke(&webview, "revert_skin_tree", args.clone()).unwrap();
            assert_eq!(
                run,
                json!({"total": 4, "changed": [], "failed": [], "skipped": 4, "remaining": [],
                   "stopped": false})
            );
            let root_name = folder_name(&root);
            assert_eq!(
                heard.take(),
                [
                    json!({"done": 0, "total": 4, "name": root_name}),
                    json!({"done": 1, "total": 4, "name": root_name}),
                    json!({"done": 2, "total": 4, "name": "a"}),
                    json!({"done": 3, "total": 4, "name": "b"}),
                    json!({"done": 4, "total": 4, "name": "a1"}),
                ]
            );

            // Stop pressed during the second folder: the rest are left for later.
            heard.stop_at.store(2, Ordering::SeqCst);
            let stopped = invoke(&webview, "revert_skin_tree", args).unwrap();
            heard.stop_at.store(0, Ordering::SeqCst);
            assert_eq!(stopped["stopped"], true);
            assert_eq!(stopped["skipped"], 2);
            assert_eq!(
                stopped["remaining"],
                json!([s(&root.join("b")), s(&root.join("a").join("a1"))])
            );
            assert_eq!(heard.take().len(), 3, "done 0, 1 and 2");

            // A stop between runs doesn't carry over into the next one.
            invoke(&webview, "stop_tree_run", json!({})).unwrap();
            assert!(STOP.load(Ordering::SeqCst));
            let none = json!({"folder": s(&root), "only": [], "onProgress": CHANNEL});
            let run = invoke(&webview, "revert_skin_tree", none).unwrap();
            assert_eq!(run["total"], 0);
            assert!(!STOP.load(Ordering::SeqCst));
            heard.take();

            // Applying: the skin must exist, and `only` must stay inside the folder.
            let skin_id = keep_skin(&app);
            let unknown = json!({"folder": s(&root), "skinId": "user:000000000000",
                             "onProgress": CHANNEL});
            assert_eq!(
                invoke(&webview, "apply_skin_tree", unknown).unwrap_err(),
                "that skin isn't available any more"
            );
            let elsewhere = Scratch::with(&[]);
            let outside = json!({"folder": s(&root.join("a")), "skinId": skin_id,
                             "only": [s(&root.join("b"))], "onProgress": CHANNEL});
            assert_eq!(
                invoke(&webview, "apply_skin_tree", outside).unwrap_err(),
                "b isn't inside a, so it can't be changed along with it"
            );
            let outside = json!({"folder": s(&root), "skinId": skin_id,
                             "only": [s(&elsewhere.root())], "onProgress": CHANNEL});
            assert!(invoke(&webview, "apply_skin_tree", outside).is_err());
            assert!(
                heard.take().is_empty(),
                "refused before anything was planned"
            );

            let nothing = json!({"folder": s(&root), "skinId": skin_id, "only": [],
                             "onProgress": CHANNEL});
            let run = invoke(&webview, "apply_skin_tree", nothing).unwrap();
            assert_eq!(
                run,
                json!({"total": 0, "changed": [], "failed": [], "skipped": 0, "remaining": [],
                   "stopped": false})
            );
            assert_eq!(
                heard.take(),
                [json!({"done": 0, "total": 0, "name": root_name})]
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
            let s = |p: &Path| path_string(p);
            let (app, webview, heard) = app();
            let skin_id = keep_skin(&app);
            let folders = [
                root.clone(),
                root.join("a"),
                root.join("b"),
                root.join("a").join("a1"),
            ];

            let bytes = invoke(&webview, "tree_bytes", json!({"skinId": skin_id})).unwrap();
            assert!(bytes.as_u64().unwrap() > 0, "{bytes}");

            let started = std::time::Instant::now();
            let args = json!({"folder": s(&root), "skinId": skin_id, "onProgress": CHANNEL});
            let run = invoke(&webview, "apply_skin_tree", args).unwrap();
            let took = started.elapsed();
            let changed: Vec<String> = folders.iter().map(|f| s(f)).collect();
            assert_eq!(
                run,
                json!({"total": 4, "changed": changed, "failed": [], "skipped": 0,
                   "remaining": [], "stopped": false})
            );
            assert_eq!(heard.take().len(), 5);
            assert!(folders.iter().all(|f| has_custom_icon(f)));

            // Undo: exactly the folders the run changed.
            let undo = json!({"folder": s(&root), "only": changed, "onProgress": CHANNEL});
            let run = invoke(&webview, "revert_skin_tree", undo).unwrap();
            assert_eq!(run["changed"].as_array().unwrap().len(), 4);
            assert!(folders.iter().all(|f| !has_custom_icon(f)));
            println!("4 folders in {took:?}; {bytes} bytes each");
        }
    }
}
