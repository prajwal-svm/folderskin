//! The prompts people save from the AI chat, which its "/" menu lists as "Your prompts", kept
//! between runs as skills ([`folderskin_ai::skill`]).
//!
//! ```text
//! <app data>/skills.json   {"version": 1, "skills": [{"format": "folderskin.skill/1", …}]},
//!                          newest first
//! ```
//!
//! A saved prompt is the words in the box and the look that goes with them: a built-in style, or
//! the look of another saved prompt, under a name of its own. Saving under a name that's taken,
//! whatever its capitals, replaces that prompt and keeps its place and its id. Every skill is
//! checked by [`folderskin_ai::skill::check`] before it is written, the same rules the command
//! line and the harness hold it to. The file is written atomically, so a crash leaves the old
//! list or the new one; a file that can't be read is set aside under another name rather than
//! overwritten, and the list starts empty.

use folderskin_ai::skill::{self, Skill};
use folderskin_core::apply::paths::write_atomic;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::State;

const FILE: &str = "skills.json";
const VERSION: u32 = 1;
/// How many prompts can be kept.
pub const MAX_PROMPTS: usize = 500;

/// Where the prompts are kept; `None` until setup opens it (or when there's no app data folder).
#[derive(Clone, Default)]
pub struct Prompts(Arc<Mutex<Option<PathBuf>>>);

impl Prompts {
    pub fn open(&self, dir: PathBuf) {
        *lock(&self.0) = Some(dir.join(FILE));
    }

    fn file(&self) -> Result<PathBuf, String> {
        lock(&self.0).clone().ok_or_else(|| {
            "prompts can't be kept on this computer: FolderSkin has no data folder here".into()
        })
    }

    /// The saved prompt `id`, as it is now. Reads the disk: call it off the async threads.
    pub fn find(&self, id: &str) -> Option<Skill> {
        let file = self.file().ok()?;
        let _one = lock(&WRITING);
        read(&file).into_iter().find(|s| s.id == id)
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    #[serde(default)]
    skills: Vec<Skill>,
}

/// The prompts in `file`, newest first. A file that isn't there is none yet; one that can't be read
/// is renamed out of the way, so the next save doesn't overwrite what may be recoverable.
pub fn read(file: &Path) -> Vec<Skill> {
    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    match serde_json::from_slice::<File>(&bytes) {
        Ok(f) if f.version <= VERSION => f.skills,
        _ => {
            let aside = file.with_file_name(format!("skills-unreadable-{}.json", now_ms()));
            if let Err(e) = std::fs::rename(file, &aside) {
                eprintln!("folderskin: couldn't set aside {}: {e}", file.display());
            }
            Vec::new()
        }
    }
}

fn write(file: &Path, skills: &[Skill]) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(&File {
        version: VERSION,
        skills: skills.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("couldn't save your prompts: {e}"))?;
    }
    write_atomic(file, &bytes).map_err(|e| format!("couldn't save your prompts: {e}"))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Saves `text` under `name` in `file` with its look: `look` is a built-in style's id, or the id
/// of another saved prompt whose look it takes. A prompt already called `name` is replaced in its
/// place, keeping its id; a new one goes first. Returns it as kept.
pub fn save(file: &Path, name: &str, text: &str, look: Option<&str>) -> Result<Skill, String> {
    let now = skill::utc_now();
    let mut skills = read(file);
    let look = look.map(str::trim).filter(|l| !l.is_empty());
    let from = look.and_then(|id| skills.iter().find(|s| s.id == id).cloned());
    let mut new = Skill::saved(name, text, None, &now);
    match (from, look) {
        // Another saved prompt's look, whole: its style and anything it adds to it.
        (Some(from), _) => {
            new.base_style = from.base_style;
            new.treatment = from.treatment;
            new.palette = from.palette;
            new.light = from.light;
            new.keep_out = from.keep_out;
            new.lettering = from.lettering;
            new.providers = from.providers;
        }
        (None, Some(style)) => new.base_style = Some(style.to_string()),
        (None, None) => {}
    }
    skill::check(&mut new)?;
    let same = |s: &Skill| s.name.to_lowercase() == new.name.to_lowercase();
    let kept = match skills.iter_mut().find(|s| same(s)) {
        Some(old) => {
            new.id = old.id.clone();
            new.created = old.created.clone();
            *old = new.clone();
            new
        }
        None => {
            if skills.len() >= MAX_PROMPTS {
                return Err(format!(
                    "you have {MAX_PROMPTS} prompts already. Remove one to save another"
                ));
            }
            skills.insert(0, new.clone());
            new
        }
    };
    write(file, &skills)?;
    Ok(kept)
}

/// Puts back `removed` at `at` (or first), as it was: the Undo after a prompt is removed. One
/// with its id or its name already kept isn't put back twice.
pub fn restore(file: &Path, mut removed: Skill, at: Option<usize>) -> Result<Skill, String> {
    skill::check(&mut removed)?;
    let mut skills = read(file);
    if let Some(kept) = skills
        .iter()
        .find(|s| s.id == removed.id || s.name.to_lowercase() == removed.name.to_lowercase())
    {
        return Ok(kept.clone());
    }
    let at = at.unwrap_or(0).min(skills.len());
    skills.insert(at, removed.clone());
    write(file, &skills)?;
    Ok(removed)
}

/// Removes the prompt `id` from `file`. One that isn't there is gone already.
pub fn delete(file: &Path, id: &str) -> Result<(), String> {
    let mut skills = read(file);
    let before = skills.len();
    skills.retain(|s| s.id != id);
    if skills.len() == before {
        return Ok(());
    }
    write(file, &skills)
}

// ---------- commands ----------
//
// On blocking threads, like the chats': a save waits for the disk.

/// Held while the file is read and written: two saves at once would each write a list without
/// the other's prompt.
static WRITING: Mutex<()> = Mutex::new(());

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn prompts_list(prompts: State<'_, Prompts>) -> Result<Vec<Skill>, String> {
    let file = prompts.file()?;
    blocking(move || {
        let _one = lock(&WRITING);
        Ok(read(&file))
    })
    .await
}

#[tauri::command]
pub async fn prompt_save(
    prompts: State<'_, Prompts>,
    name: String,
    text: String,
    style: Option<String>,
) -> Result<Skill, String> {
    let file = prompts.file()?;
    blocking(move || {
        let _one = lock(&WRITING);
        save(&file, &name, &text, style.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn prompt_restore(
    prompts: State<'_, Prompts>,
    skill: Skill,
    at: Option<usize>,
) -> Result<Skill, String> {
    let file = prompts.file()?;
    blocking(move || {
        let _one = lock(&WRITING);
        restore(&file, skill, at)
    })
    .await
}

#[tauri::command]
pub async fn prompt_delete(prompts: State<'_, Prompts>, id: String) -> Result<(), String> {
    let file = prompts.file()?;
    blocking(move || {
        let _one = lock(&WRITING);
        delete(&file, &id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "folderskin-prompts-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn prompts_are_saved_as_skills_newest_first_read_back_and_removed() {
        let dir = temp("roundtrip");
        let file = dir.join(FILE);
        assert!(read(&file).is_empty(), "none yet");
        let koi = save(&file, " Moody  koi ", "a koi pond at night", Some("ukiyoe")).unwrap();
        assert_eq!(koi.name, "Moody koi");
        assert_eq!(koi.format, skill::FORMAT);
        assert_eq!(koi.command, "moody-koi");
        assert_eq!(
            koi.base_style.as_deref(),
            Some("woodblock"),
            "by the style's id today"
        );
        assert_eq!(koi.idea.as_deref(), Some("a koi pond at night"));
        let fox = save(&file, "Fox", "  a fox in the snow\n", Some(" ")).unwrap();
        assert_eq!(
            (fox.idea.as_deref(), fox.base_style.as_deref()),
            (Some("a fox in the snow"), None)
        );
        let names: Vec<String> = read(&file).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Fox", "Moody koi"], "newest first");
        assert_ne!(koi.id, fox.id);
        // The file is the skill format, a list of them.
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["skills"][1]["format"], "folderskin.skill/1");
        delete(&file, &koi.id).unwrap();
        assert_eq!(read(&file), std::slice::from_ref(&fox));
        // Removing one that's gone already is nothing to report.
        delete(&file, &koi.id).unwrap();
        // Undo puts it back where it was, as it was.
        assert_eq!(restore(&file, koi.clone(), Some(1)).unwrap(), koi);
        assert_eq!(read(&file), [fox, koi.clone()]);
        assert_eq!(restore(&file, koi.clone(), None).unwrap(), koi, "not twice");
        assert_eq!(read(&file).len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_under_a_name_thats_taken_replaces_that_prompt_in_its_place() {
        let dir = temp("replace");
        let file = dir.join(FILE);
        let first = save(&file, "Koi", "a koi pond", None).unwrap();
        save(&file, "Fox", "a fox", None).unwrap();
        let again = save(&file, "KOI", "a koi pond at dawn", Some("oil")).unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(again.created, first.created);
        let list = read(&file);
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[1].idea.as_deref(),
            Some("a koi pond at dawn"),
            "kept its place"
        );
        assert_eq!(list[1].name, "KOI");
        assert_eq!(list[1].base_style.as_deref(), Some("oil"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_prompt_saved_with_another_prompts_look_takes_all_of_it() {
        let dir = temp("look");
        let file = dir.join(FILE);
        let mut night = Skill::saved("Night prints", "", Some("woodblock"), "now");
        night.light = Some("cool moonlight from the upper left".into());
        night.keep_out = vec!["Mount Fuji".into()];
        write(&file, std::slice::from_ref(&night)).unwrap();
        let fox = save(&file, "Night fox", "a fox", Some(&night.id)).unwrap();
        assert_eq!(fox.base_style.as_deref(), Some("woodblock"));
        assert_eq!(fox.light, night.light);
        assert_eq!(fox.keep_out, night.keep_out);
        assert_ne!(fox.id, night.id);
        let found = Prompts::default();
        found.open(dir.clone());
        assert_eq!(found.find(&fox.id), Some(fox));
        assert_eq!(found.find("nope"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_prompt_needs_a_name_and_words_of_a_sane_length() {
        let dir = temp("refused");
        let file = dir.join(FILE);
        assert!(save(&file, "  ", "a fox", None)
            .unwrap_err()
            .contains("name"));
        assert!(save(&file, "Fox", " \n ", None)
            .unwrap_err()
            .contains("nothing to save"));
        let long_name = "n".repeat(skill::MAX_NAME_CHARS + 1);
        assert!(save(&file, &long_name, "a fox", None)
            .unwrap_err()
            .contains("too long"));
        let long_text = "w ".repeat(3000);
        assert!(save(&file, "Fox", &long_text, None)
            .unwrap_err()
            .contains("too long"));
        assert!(save(&file, "Fox", "a fox", Some("no-such-style"))
            .unwrap_err()
            .contains("no style called"));
        assert!(!file.exists(), "nothing was written");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = temp("damaged");
        let file = dir.join(FILE);
        std::fs::write(&file, b"{not json").unwrap();
        assert!(read(&file).is_empty());
        let aside: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("skills-unreadable-")
            })
            .collect();
        assert_eq!(aside.len(), 1);
        save(&file, "Fox", "a fox", None).unwrap();
        assert_eq!(read(&file).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
