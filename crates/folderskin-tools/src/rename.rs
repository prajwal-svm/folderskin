//! `packs rename`: generated ids for packs, such as `classic-art` becoming `classic-art-k7q2mx`.
//!
//! Each pack's folder moves with `git mv`, so its history follows it, and every place its old id
//! was written is brought along: `featured.json`, `official.json`, and `moved.json`, which records
//! the move so install links, install counts and the packs people added under the old id can
//! follow the pack to its new one. The rename is staged, ready to commit.
//!
//! Running it again changes nothing. A pack that has a generated id already is left alone, and
//! one whose old id is in `moved.json` is said to have moved already.

use crate::catalog::FEATURED_FILE;
use crate::git;
use crate::packs::{self, OFFICIAL_FILE, PACKS_DIR};
use folderskin_core::pack::{self, Moved, MANIFEST_FILE, MOVED_FILE};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Which packs get new ids.
#[derive(Debug, Clone, PartialEq)]
pub enum Which {
    /// Every pack whose id isn't a generated one.
    All,
    /// One pack, to a new generated id, or to `to` when it is a free generated id.
    One { id: String, to: Option<String> },
}

/// What [`rename`] did.
#[derive(Debug, Default)]
pub struct Renamed {
    /// Each pack that moved, as (old id, new id), in the order they moved.
    pub moved: Vec<(String, String)>,
    /// The packs left alone because their ids are generated ones already.
    pub kept: Vec<String>,
    /// Packs an earlier rename moved, as (old id, the id it has now).
    pub earlier: Vec<(String, String)>,
    /// The files it wrote beside `packs/`: `moved.json`, and the lists that named a pack that
    /// moved.
    pub written: Vec<PathBuf>,
}

/// Gives the packs in `<dir>/packs` that `which` names generated ids, in the git checkout `dir`
/// is in. Nothing moves when a list can't be read or an id asked for can't be had; when a move
/// fails partway, the packs moved before it are recorded as they should be and the error says so.
pub fn rename(dir: &Path, which: &Which) -> Result<Renamed, String> {
    let packs_dir = dir.join(PACKS_DIR);
    let folders = pack_folders(&packs_dir)?;
    let mut moved = packs::read_moved(dir)?;
    // Every id a new one can't be, in lower case: macOS and Windows take two names one capital
    // apart for one folder, and an old id in moved.json is never given out again.
    let mut taken: BTreeSet<String> = folders
        .iter()
        .map(|f| f.to_lowercase())
        .chain(moved.moved.keys().cloned())
        .collect();
    let mut report = Renamed::default();
    let mut plan: Vec<(String, String)> = Vec::new();
    match which {
        Which::All => {
            if let Some(bad) = folders.iter().find(|f| !pack::is_pack_id(f)) {
                return Err(format!(
                    "{} isn't named with a pack id, so it can't be recorded in {MOVED_FILE}; \
                     give it a pack id by hand first",
                    packs_dir.join(bad).display()
                ));
            }
            for id in &folders {
                if pack::is_generated_id(id) {
                    report.kept.push(id.clone());
                    continue;
                }
                let new = pack::new_id(&pack_name(&packs_dir.join(id), id), |c| taken.contains(c))?;
                taken.insert(new.clone());
                plan.push((id.clone(), new));
            }
        }
        Which::One { id, to } => {
            if !pack::is_pack_id(id) {
                return Err(format!("{id:?} isn't a pack id"));
            }
            if folders.contains(id) {
                let new = match to {
                    Some(to) => free_id(to, id, &folders, &moved)?,
                    None => {
                        pack::new_id(&pack_name(&packs_dir.join(id), id), |c| taken.contains(c))?
                    }
                };
                plan.push((id.clone(), new));
            } else if let Some(now) = moved.moved.get(id) {
                match to {
                    Some(to) if to != now => {
                        return Err(format!(
                            "{id} moved to {now} already; to give the pack another id, rename \
                             {now}"
                        ))
                    }
                    _ => report.earlier.push((id.clone(), now.clone())),
                }
            } else {
                return Err(format!("there's no pack {id} in {}", packs_dir.display()));
            }
        }
    }
    if plan.is_empty() {
        return Ok(report);
    }

    // Read before anything moves, so a list that can't be rewritten stops the rename.
    let mut lists = Vec::new();
    for file in [FEATURED_FILE, OFFICIAL_FILE] {
        if let Some(ids) = read_list(&dir.join(file))? {
            lists.push((file, ids.clone(), ids));
        }
    }
    if !git::is_checkout(dir) {
        return Err(format!(
            "{} isn't in a git checkout; packs rename moves packs with git mv, so that their \
             history goes with them",
            dir.display()
        ));
    }

    let mut failed = None;
    for (old, new) in plan {
        let mut command = git::git(dir);
        command.args(["mv", "--"]);
        command.arg(format!("{PACKS_DIR}/{old}"));
        command.arg(format!("{PACKS_DIR}/{new}"));
        if let Err(e) = git::run(command, &format!("move {PACKS_DIR}/{old} to {new}")) {
            failed = Some(e);
            break;
        }
        moved.record(&old, &new);
        for (_, _, ids) in &mut lists {
            for listed in ids.iter_mut().filter(|listed| **listed == old) {
                listed.clone_from(&new);
            }
        }
        report.moved.push((old, new));
    }

    if !report.moved.is_empty() {
        let json = serde_json::to_string_pretty(&moved).map_err(|e| e.to_string())? + "\n";
        write(&dir.join(MOVED_FILE), &json, &mut report.written)?;
        for (file, before, after) in &lists {
            if before != after {
                let json = serde_json::to_string_pretty(after).map_err(|e| e.to_string())? + "\n";
                write(&dir.join(file), &json, &mut report.written)?;
            }
        }
        // By name, since they all sit in `dir`: an absolute path can reach the checkout through
        // a link, which git reads as outside it.
        let mut command = git::git(dir);
        command
            .args(["add", "--"])
            .args(report.written.iter().filter_map(|p| p.file_name()));
        git::run(command, "stage the lists")?;
    }
    match failed {
        None => Ok(report),
        Some(e) if report.moved.is_empty() => Err(e),
        Some(e) => Err(format!(
            "{e}. The {} packs moved before it are recorded in {MOVED_FILE} and staged",
            report.moved.len()
        )),
    }
}

/// `to` when pack `id` can move there: a generated id that no folder has, in any capitals, and
/// that no pack had before.
fn free_id(
    to: &str,
    id: &str,
    folders: &BTreeSet<String>,
    moved: &Moved,
) -> Result<String, String> {
    if to == id {
        return Err(format!("{id} has that id already"));
    }
    if !pack::is_generated_id(to) {
        return Err(format!(
            "{to} isn't a generated id: a pack id that ends in a dash and six characters from a \
             to z and 2 to 7, such as classic-art-k7q2mx"
        ));
    }
    if folders.iter().any(|f| f.eq_ignore_ascii_case(to)) {
        return Err(format!("there's a pack {to} already"));
    }
    if let Some(now) = moved.moved.get(to) {
        return Err(format!(
            "{to} is an old id in {MOVED_FILE}, which sends it to {now}; an old id is never \
             given out again"
        ));
    }
    Ok(to.to_string())
}

/// The folders in `packs_dir`, leaving out dotfolders and anything that isn't a folder.
fn pack_folders(packs_dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries = std::fs::read_dir(packs_dir)
        .map_err(|e| format!("couldn't read {}: {e}", packs_dir.display()))?;
    let mut folders = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("couldn't read {}: {e}", packs_dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') && entry.file_type().is_ok_and(|t| t.is_dir()) {
            folders.insert(name);
        }
    }
    Ok(folders)
}

/// The name in a pack's `pack.json`, which its new id starts with; its old id when the file
/// can't be read or names nothing.
fn pack_name(folder: &Path, id: &str) -> String {
    std::fs::read(folder.join(MANIFEST_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|json| json["name"].as_str().map(|n| n.trim().to_string()))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| id.to_string())
}

/// A list of pack ids such as `featured.json`, as it is written, repeats and all; `None` when
/// there is no such file.
fn read_list(path: &Path) -> Result<Option<Vec<String>>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("couldn't read {}: {e}", path.display())),
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|_| {
        format!(
            "{} has to be a list of pack ids, such as [\"classic-art\", \"colours\"]",
            path.display()
        )
    })
}

fn write(path: &Path, text: &str, written: &mut Vec<PathBuf>) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("couldn't write {}: {e}", path.display()))?;
    written.push(path.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use folderskin_core::raster;
    use std::collections::{BTreeMap, HashMap};

    /// A folderskin-community checkout in the system temp folder, in a git repository of its own,
    /// removed when the test ends. Its commits don't read the user's git settings, so signing or
    /// hooks set up there can't get in the way.
    struct Repo(PathBuf);

    impl Repo {
        fn new(test: &str) -> Repo {
            let dir = std::env::temp_dir()
                .join(format!("folderskin-rename-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(PACKS_DIR)).unwrap();
            // An empty file in place of the user's settings.
            std::fs::write(dir.join(".gitconfig-empty"), "").unwrap();
            let repo = Repo(dir);
            repo.git(&["init", "-q"]);
            repo
        }

        fn git(&self, args: &[&str]) -> String {
            self.git_at(args, 1_700_000_000)
        }

        /// Runs git in the checkout, with commits dated `when` (Unix seconds).
        fn git_at(&self, args: &[&str], when: i64) -> String {
            // Git's own format for a date: seconds, then the time zone.
            let date = format!("{when} +0000");
            let mut command = git::git(&self.0);
            command
                .env("GIT_CONFIG_GLOBAL", self.0.join(".gitconfig-empty"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .env("GIT_AUTHOR_DATE", &date)
                .env("GIT_COMMITTER_DATE", &date)
                .args(args);
            git::run(command, "run git in the test").unwrap()
        }

        /// Commits everything, dated `when`.
        fn commit(&self, message: &str, when: i64) {
            self.git_at(&["add", "-A"], when);
            self.git_at(&["commit", "-q", "--no-verify", "-m", message], when);
        }

        /// Writes a pack called `name` with one picture.
        fn pack(&self, id: &str, name: &str) {
            let folder = self.0.join(PACKS_DIR).join(id);
            std::fs::create_dir_all(&folder).unwrap();
            let json = format!(
                r#"{{ "version": 1, "name": "{name}", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["test"], "skins": [{{ "file": "a.png", "name": "A" }}] }}"#
            );
            std::fs::write(folder.join(MANIFEST_FILE), json).unwrap();
            let art = image::RgbaImage::from_pixel(256, 256, image::Rgba([90, 120, 200, 255]));
            std::fs::write(folder.join("a.png"), raster::encode_png(&art)).unwrap();
        }

        fn write(&self, file: &str, text: &str) {
            std::fs::write(self.0.join(file), text).unwrap();
        }

        fn read(&self, file: &str) -> String {
            std::fs::read_to_string(self.0.join(file)).unwrap()
        }

        fn list(&self, file: &str) -> Vec<String> {
            serde_json::from_str(&self.read(file)).unwrap()
        }

        fn moved(&self) -> Moved {
            Moved::parse(self.read(MOVED_FILE).as_bytes()).unwrap()
        }

        fn folders(&self) -> BTreeSet<String> {
            pack_folders(&self.0.join(PACKS_DIR)).unwrap()
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Three packs of the old kind and one with a generated id, all committed.
    fn community(test: &str) -> Repo {
        let repo = Repo::new(test);
        repo.pack("classic-art", "Classic Art");
        repo.pack("colours", "Colours");
        repo.pack("greek-art", "Classic Art");
        repo.pack("travel-k7q2mx", "Travel");
        repo.write(FEATURED_FILE, "[\n  \"colours\",\n  \"classic-art\"\n]\n");
        repo.write(
            OFFICIAL_FILE,
            r#"["classic-art", "greek-art", "travel-k7q2mx", "colours"]"#,
        );
        repo.commit("The packs", 1_700_000_000);
        repo
    }

    #[test]
    fn every_pack_gets_a_generated_id_and_every_list_follows() {
        let repo = community("all");
        let renamed = rename(&repo.0, &Which::All).unwrap();
        assert_eq!(renamed.kept, ["travel-k7q2mx"]);
        let moves: HashMap<String, String> = renamed.moved.iter().cloned().collect();
        assert_eq!(moves.len(), 3);
        // The new id starts with the pack's name, not its old id, and names can repeat.
        assert!(moves["classic-art"].starts_with("classic-art-"));
        assert!(moves["greek-art"].starts_with("classic-art-"));
        assert_ne!(moves["classic-art"], moves["greek-art"]);
        assert!(moves.values().all(|id| pack::is_generated_id(id)));

        let mut expected: BTreeSet<String> = moves.values().cloned().collect();
        expected.insert("travel-k7q2mx".into());
        assert_eq!(repo.folders(), expected);
        let moved = repo.moved();
        assert_eq!(moved.version, 1);
        assert_eq!(
            moved.moved,
            moves.clone().into_iter().collect::<BTreeMap<_, _>>()
        );
        let new = |old: &str| moves[old].clone();
        assert_eq!(
            repo.list(FEATURED_FILE),
            [new("colours"), new("classic-art")]
        );
        assert_eq!(
            repo.list(OFFICIAL_FILE),
            [
                new("classic-art"),
                new("greek-art"),
                "travel-k7q2mx".to_string(),
                new("colours")
            ]
        );
        // Written as the builders write JSON, and ready to commit, moves and all.
        assert!(repo.read(FEATURED_FILE).starts_with("[\n  \""));
        assert!(repo.read(MOVED_FILE).ends_with("}\n"));
        // Without rename detection, which can pair up two packs whose files are alike.
        let staged = repo.git(&["diff", "--cached", "--name-status", "--no-renames"]);
        for (old, new) in &moves {
            for line in [
                format!("D\t{PACKS_DIR}/{old}/{MANIFEST_FILE}"),
                format!("A\t{PACKS_DIR}/{new}/{MANIFEST_FILE}"),
                format!("A\t{PACKS_DIR}/{new}/a.png"),
            ] {
                assert!(staged.lines().any(|l| l == line), "{line:?} in {staged}");
            }
        }
        for file in [MOVED_FILE, FEATURED_FILE, OFFICIAL_FILE] {
            assert!(staged.contains(file), "{file} in {staged}");
        }
        assert_eq!(
            repo.git(&["diff", "--name-only"]),
            "",
            "all of it is staged"
        );

        // The packs pass as they are, generated ids required, and the index carries the moves.
        let report = packs::check_with(
            &repo.0,
            &packs::CheckOptions {
                require_generated_ids: true,
                ..packs::CheckOptions::default()
            },
        )
        .unwrap();
        assert_eq!(report.problems, Vec::<String>::new());
        assert_eq!(report.moved, moved);
    }

    #[test]
    fn renaming_again_changes_nothing() {
        let repo = community("again");
        rename(&repo.0, &Which::All).unwrap();
        repo.commit("New ids", 1_700_000_100);
        let before: Vec<String> = [MOVED_FILE, FEATURED_FILE, OFFICIAL_FILE]
            .iter()
            .map(|f| repo.read(f))
            .collect();
        let folders = repo.folders();

        let again = rename(&repo.0, &Which::All).unwrap();
        assert!(again.moved.is_empty() && again.written.is_empty());
        assert_eq!(again.kept.len(), 4);
        // The same for one pack, by its old id: it moved already.
        let (old, now) = repo.moved().moved.into_iter().next().unwrap();
        for to in [None, Some(now.clone())] {
            let one = rename(
                &repo.0,
                &Which::One {
                    id: old.clone(),
                    to,
                },
            )
            .unwrap();
            assert_eq!(one.earlier, [(old.clone(), now.clone())]);
            assert!(one.moved.is_empty());
        }
        let after: Vec<String> = [MOVED_FILE, FEATURED_FILE, OFFICIAL_FILE]
            .iter()
            .map(|f| repo.read(f))
            .collect();
        assert_eq!(before, after);
        assert_eq!(repo.folders(), folders);
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        // Sent somewhere else, it says where the pack is now.
        let elsewhere = Which::One {
            id: old.clone(),
            to: Some("elsewhere-a2b3c4".into()),
        };
        let err = rename(&repo.0, &elsewhere).unwrap_err();
        assert!(err.contains(&format!("moved to {now} already")), "{err}");
    }

    #[test]
    fn one_pack_moves_to_the_id_asked_for_and_nothing_chains() {
        let repo = community("one");
        let first = rename(
            &repo.0,
            &Which::One {
                id: "classic-art".into(),
                to: Some("classic-art-a2b3c4".into()),
            },
        )
        .unwrap();
        assert_eq!(
            first.moved,
            [("classic-art".to_string(), "classic-art-a2b3c4".to_string())]
        );
        assert_eq!(
            first.written,
            [
                repo.0.join(MOVED_FILE),
                repo.0.join(FEATURED_FILE),
                repo.0.join(OFFICIAL_FILE)
            ]
        );
        // A pack with a generated id moves when it is named, and what led to it follows.
        let second = rename(
            &repo.0,
            &Which::One {
                id: "classic-art-a2b3c4".into(),
                to: None,
            },
        )
        .unwrap();
        let newest = &second.moved[0].1;
        assert!(newest.starts_with("classic-art-") && pack::is_generated_id(newest));
        let moved = repo.moved();
        assert_eq!(moved.current("classic-art"), newest.as_str());
        assert_eq!(moved.current("classic-art-a2b3c4"), newest.as_str());
        assert_eq!(moved.problems(&repo.folders()), Vec::<String>::new());
        assert!(repo.list(FEATURED_FILE).contains(newest));
        assert_eq!(second.written.len(), 3);
    }

    #[test]
    fn an_id_asked_for_has_to_be_generated_and_free() {
        let repo = community("refuse");
        repo.write(
            MOVED_FILE,
            r#"{ "version": 1, "moved": { "old-travel-q5r6s7": "travel-k7q2mx" } }"#,
        );
        repo.commit("An earlier move", 1_700_000_050);
        for (id, to, says) in [
            ("colours", "colours", "has that id already"),
            ("colours", "colours-2", "isn't a generated id"),
            ("colours", "Colours-A2B3C4", "isn't a generated id"),
            (
                "colours",
                "travel-k7q2mx",
                "there's a pack travel-k7q2mx already",
            ),
            (
                "colours",
                "old-travel-q5r6s7",
                "is an old id in moved.json, which sends it to travel-k7q2mx",
            ),
            ("travel-k7q2mx", "travel-k7q2mx", "has that id already"),
            ("nowhere", "nowhere-a2b3c4", "there's no pack nowhere"),
            ("Not An Id", "x-a2b3c4", "isn't a pack id"),
        ] {
            let which = Which::One {
                id: id.into(),
                to: Some(to.into()),
            };
            let err = rename(&repo.0, &which).unwrap_err();
            assert!(err.contains(says), "{id} to {to}: {err}");
        }
        // Nothing moved or changed on the way.
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
    }

    #[test]
    fn nothing_moves_outside_a_git_checkout_or_with_a_list_it_cant_read() {
        let repo = community("no-git");
        repo.write(OFFICIAL_FILE, r#"{"classic-art": true}"#);
        let err = rename(&repo.0, &Which::All).unwrap_err();
        assert!(err.contains("has to be a list of pack ids"), "{err}");
        repo.write(OFFICIAL_FILE, r#"["classic-art"]"#);

        std::fs::remove_dir_all(repo.0.join(".git")).unwrap();
        let err = rename(&repo.0, &Which::All).unwrap_err();
        assert!(err.contains("isn't in a git checkout"), "{err}");
        assert!(repo.folders().contains("classic-art"));
        assert!(!repo.0.join(MOVED_FILE).exists());
        // With nothing to rename, there's nothing that needs git.
        let generated = Repo::new("no-git-generated");
        generated.pack("travel-k7q2mx", "Travel");
        std::fs::remove_dir_all(generated.0.join(".git")).unwrap();
        assert_eq!(
            rename(&generated.0, &Which::All).unwrap().kept,
            ["travel-k7q2mx"]
        );
    }

    #[test]
    fn a_renamed_pack_keeps_the_date_it_was_first_added() {
        let repo = Repo::new("dates");
        repo.pack("classic-art", "Classic Art");
        repo.commit("Add the Classic Art pack", 1_700_000_000);
        repo.pack("colours", "Colours");
        repo.commit("Add the Colours pack", 1_710_000_000);
        let renamed = rename(&repo.0, &Which::All).unwrap();
        repo.commit("Give every pack a generated id", 1_720_000_000);
        let moves: HashMap<String, String> = renamed.moved.into_iter().collect();

        let report = packs::check(&repo.0).unwrap();
        assert_eq!(report.problems, Vec::<String>::new());
        let dates = catalog::git_dates(&repo.0, &report.moved);
        assert_eq!(dates[&moves["classic-art"]], 1_700_000_000);
        assert_eq!(dates[&moves["colours"]], 1_710_000_000);
        // Without moved.json the rename would look like two new packs.
        let unfollowed = catalog::git_dates(&repo.0, &Moved::default());
        assert_eq!(unfollowed[&moves["classic-art"]], 1_720_000_000);

        // index.json says the same, and carries the moves.
        packs::write_index(&repo.0, &report, &dates).unwrap();
        let index: pack::Index = serde_json::from_str(&repo.read(packs::INDEX_FILE)).unwrap();
        let added: HashMap<&str, Option<i64>> = index
            .packs
            .iter()
            .map(|p| (p.id.as_str(), p.added))
            .collect();
        assert_eq!(added[moves["classic-art"].as_str()], Some(1_700_000_000));
        assert_eq!(added[moves["colours"].as_str()], Some(1_710_000_000));
        assert_eq!(index.moved.len(), 2);
    }
}
