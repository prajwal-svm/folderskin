//! Running git on a folderskin-community checkout: `packs index` and `packs catalog` date packs
//! from its history, and `packs rename` moves pack folders with it.

use std::path::Path;
use std::process::Command;

/// The variables that point git at a repository. Git sets them for the hooks it runs, so a
/// command started from a hook (tests run by a pre-commit hook, say) would act on that repository
/// instead of the folder it was given.
const REPOSITORY_VARS: [&str; 7] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_PREFIX",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// `git -C <dir>`, finding its repository from `dir` alone.
pub(crate) fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    for var in REPOSITORY_VARS {
        command.env_remove(var);
    }
    command
}

/// Runs `command` and returns what it printed. The error is a sentence ending in what git said.
pub(crate) fn run(mut command: Command, what: &str) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|e| format!("couldn't {what}, because git didn't run: {e}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!("couldn't {what}: git said {}", said.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether `dir` is inside a git checkout, with a work tree to move folders in.
pub(crate) fn is_checkout(dir: &Path) -> bool {
    let mut command = git(dir);
    command.args(["rev-parse", "--is-inside-work-tree"]);
    run(command, "look for a checkout").is_ok_and(|out| out.trim() == "true")
}
