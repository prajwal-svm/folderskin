//! The command line's own defaults, in `cli.json` beside the app's settings, and the keys it
//! shares with the app.

use crate::cli::ConfigKey;
use crate::error::CliError;
use crate::paint::cannot_run;
use folderskin_core::compositor::Style;
use folderskin_local::machine::{Arch, Os};
use folderskin_local::Backend;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The app's identifier, which names its config folder.
const APP_ID: &str = "app.folderskin.desktop";
const FILE: &str = "cli.json";

/// The app's config folder, where Tauri keeps it: `%APPDATA%\app.folderskin.desktop` on
/// Windows, `~/Library/Application Support/app.folderskin.desktop` on macOS and
/// `~/.config/app.folderskin.desktop` on Linux. `FOLDERSKIN_CONFIG_DIR` moves it.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("FOLDERSKIN_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    dirs::config_dir().map(|d| d.join(APP_ID))
}

pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join(FILE))
}

/// The app's data folder, where it keeps the skins and the folder it puts them on: the same as
/// [`config_dir`] on Windows and macOS, `~/.local/share/app.folderskin.desktop` on Linux.
/// `FOLDERSKIN_CONFIG_DIR` moves it with the settings.
pub fn app_data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("FOLDERSKIN_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    dirs::data_dir().map(|d| d.join(APP_ID))
}

/// The folder the app puts artwork on, as chosen in it (`folder-look.txt`, which the app's
/// `look.rs` keeps): the Mac's when nothing was chosen or the choice can't be read, as in the app.
pub fn saved_look() -> Style {
    app_data_dir().map_or(Style::Mac, |dir| look_saved_in(&dir))
}

/// The folder look kept in `dir`, the app's data folder.
fn look_saved_in(dir: &std::path::Path) -> Style {
    std::fs::read_to_string(dir.join(LOOK_FILE))
        .ok()
        .and_then(|s| Style::from_id(s.trim()))
        .unwrap_or(Style::Mac)
}

/// Where the app keeps the folder look, in its data folder.
const LOOK_FILE: &str = "folder-look.txt";

/// The defaults someone chose. Anything not set is decided from the computer each time.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
}

pub const LOCAL: &str = "local";
const LOCAL_MODELS: [&str; 2] = ["auto", "klein"];

impl Config {
    /// The saved defaults, or none when nothing was saved yet.
    pub fn load() -> Result<Config, CliError> {
        let Some(path) = config_file() else {
            return Ok(Config::default());
        };
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                CliError::fixable(
                    "config_unreadable",
                    "The command line's settings can't be read.",
                    format!("{} isn't valid: {e}.", path.display()),
                )
                .fix("Fix the file, or start again from the defaults: folderskin ai config unset provider")
                .fix(format!("Or delete {}", path.display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(CliError::io("read the settings", &path, &e)),
        }
    }

    pub fn save(&self) -> Result<PathBuf, CliError> {
        let path = config_file().ok_or_else(|| {
            CliError::environment(
                "no_config_folder",
                "There is nowhere to keep settings.",
                "This account has no configuration folder.",
            )
            .fix("Set FOLDERSKIN_CONFIG_DIR to a folder you can write to.")
        })?;
        let dir = path.parent().map(PathBuf::from).unwrap_or_default();
        std::fs::create_dir_all(&dir)
            .map_err(|e| CliError::io("make the settings folder", &dir, &e))?;
        let text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into()) + "\n";
        // Written beside and moved into place, so a crash never leaves half a file.
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                CliError::io("save the settings", &path, &e)
            })?;
        Ok(path)
    }

    pub fn get(&self, key: ConfigKey) -> Option<&str> {
        match key {
            ConfigKey::Provider => self.provider.as_deref(),
            ConfigKey::Model => self.model.as_deref(),
            ConfigKey::Tier => self.tier.as_deref(),
            ConfigKey::Backend => self.backend.as_deref(),
        }
    }

    /// Sets `key` after checking `value` makes sense for it.
    pub fn set(&mut self, key: ConfigKey, value: &str) -> Result<(), CliError> {
        // Model ids are kept as the provider spells them (Ideogram's is `V_3`); everything else
        // is lower case.
        let value = match key {
            ConfigKey::Model => value.trim().to_string(),
            _ => value.trim().to_lowercase(),
        };
        let bad = |why: String| {
            CliError::fixable(
                "config_value",
                format!("{value:?} isn't a {} FolderSkin knows.", key.id()),
                why,
            )
        };
        match key {
            ConfigKey::Provider => {
                if value != LOCAL && folderskin_ai::provider(&value).is_none() {
                    return Err(bad(format!(
                        "The provider can be {LOCAL} or {}.",
                        or_list(&provider_ids())
                    ))
                    .fix("See them all: folderskin ai models"));
                }
                if self.provider.as_deref() != Some(value.as_str()) && self.model.is_some() {
                    // A model belongs to a provider; the old one's model means nothing here.
                    self.model = None;
                }
                self.provider = Some(value);
            }
            ConfigKey::Model => {
                let provider = self.provider.clone().unwrap_or_else(|| LOCAL.into());
                let known = model_ids(&provider);
                let Some(model) = known.iter().find(|m| m.eq_ignore_ascii_case(&value)) else {
                    return Err(bad(format!(
                        "For {provider}, the model can be {}.",
                        or_list(&known)
                    ))
                    .fix("Set the provider first if you meant another one's: folderskin ai config set provider <id>"));
                };
                self.model = Some(model.to_string());
            }
            ConfigKey::Tier => {
                let known = ["auto", "q8", "q4"];
                if !known.contains(&value.as_str()) {
                    return Err(bad(format!("The tier can be {}.", or_list(&known))));
                }
                self.tier = Some(value);
            }
            ConfigKey::Backend => {
                let known = ["auto", "cuda", "vulkan", "metal", "cpu", "mlx"];
                if !known.contains(&value.as_str()) {
                    return Err(bad(format!("The backend can be {}.", or_list(&known))));
                }
                // Kept, it would stop every command that paints until it was taken out again.
                let (os, arch) = (Os::this(), Arch::this());
                if let Some(what) = Backend::parse(&value).and_then(|b| cannot_run(b, os, arch)) {
                    return Err(CliError::fixable(
                        "backend_unavailable",
                        what,
                        format!("This is {} {}.", os.id(), arch.id()),
                    )
                    .fix("Leave it at auto to use what suits this computer: folderskin ai config set backend auto"));
                }
                self.backend = Some(value);
            }
        }
        Ok(())
    }

    /// What `key` is when nothing is set, for `ai config get`: a provider's first model, and
    /// `auto` (decided from the computer) for the local model, the tier and the backend.
    pub fn default_for(&self, key: ConfigKey) -> String {
        match key {
            ConfigKey::Provider => LOCAL.to_string(),
            ConfigKey::Model => {
                let provider = self.provider.as_deref().unwrap_or(LOCAL);
                match folderskin_ai::provider(provider) {
                    Some(p) if provider != LOCAL => p.models[0].id.to_string(),
                    _ => "auto".to_string(),
                }
            }
            ConfigKey::Tier | ConfigKey::Backend => "auto".to_string(),
        }
    }

    pub fn unset(&mut self, key: ConfigKey) {
        match key {
            ConfigKey::Provider => {
                self.provider = None;
                self.model = None;
            }
            ConfigKey::Model => self.model = None,
            ConfigKey::Tier => self.tier = None,
            ConfigKey::Backend => self.backend = None,
        }
    }

    /// The model configured for `provider`, if the configured model belongs to it.
    pub fn model_for(&self, provider: &str) -> Option<&str> {
        let configured = self.provider.as_deref().unwrap_or(LOCAL);
        (configured == provider)
            .then_some(self.model.as_deref())
            .flatten()
    }
}

/// "a, b or c".
fn or_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// Every provider a key works with.
pub fn provider_ids() -> Vec<&'static str> {
    folderskin_ai::providers().iter().map(|p| p.id).collect()
}

/// The models `provider` offers.
pub fn model_ids(provider: &str) -> Vec<&'static str> {
    if provider == LOCAL {
        return LOCAL_MODELS.to_vec();
    }
    folderskin_ai::provider(provider)
        .map(|p| p.models.iter().map(|m| m.id).collect())
        .unwrap_or_default()
}

/// The key store the app uses, opened on its folder.
pub fn keys() -> folderskin_keys::Keys {
    let keys = folderskin_keys::Keys::default();
    if let Some(dir) = config_dir() {
        keys.open(&dir);
    }
    keys
}

/// The environment variable that holds a key for `provider`: `FOLDERSKIN_OPENAI_KEY`.
pub fn key_variable(provider: &str) -> String {
    let id: String = provider
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("FOLDERSKIN_{id}_KEY")
}

/// Where a key came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Environment,
    Saved,
}

/// The key for `provider`: from its environment variable first, then the saved keys.
pub fn key_for(provider: &str, keys: &folderskin_keys::Keys) -> Option<(String, KeySource)> {
    if let Ok(key) = std::env::var(key_variable(provider)) {
        if !key.trim().is_empty() {
            return Some((key.trim().to_string(), KeySource::Environment));
        }
    }
    keys.get(provider).map(|k| (k, KeySource::Saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked_before_they_are_kept() {
        let mut c = Config::default();
        c.set(ConfigKey::Tier, "Q4").unwrap();
        assert_eq!(c.tier.as_deref(), Some("q4"));
        assert!(c.set(ConfigKey::Tier, "q5").is_err());
        assert!(c.set(ConfigKey::Backend, "rocm").is_err());
        c.set(ConfigKey::Backend, "vulkan").unwrap();
        // A backend this computer can never run isn't kept.
        for backend in ["mlx", "metal"] {
            let parsed = Backend::parse(backend).unwrap();
            match cannot_run(parsed, Os::this(), Arch::this()) {
                Some(_) => {
                    let e = c.set(ConfigKey::Backend, backend).unwrap_err();
                    assert_eq!(e.code, "backend_unavailable");
                    assert_eq!(c.backend.as_deref(), Some("vulkan"));
                }
                None => {
                    c.set(ConfigKey::Backend, backend).unwrap();
                    c.set(ConfigKey::Backend, "vulkan").unwrap();
                }
            }
        }
        let err = c.set(ConfigKey::Provider, "midjourney").unwrap_err();
        assert!(err.why.contains("openai"), "{err:?}");
        c.set(ConfigKey::Model, "klein").unwrap();
        let err = c.set(ConfigKey::Model, "gpt-image-1").unwrap_err();
        assert_eq!(
            err.why, "For local, the model can be auto or klein.",
            "not a local model"
        );
    }

    #[test]
    fn a_model_keeps_its_providers_spelling() {
        let mut c = Config::default();
        c.set(ConfigKey::Provider, "Ideogram").unwrap();
        assert_eq!(c.provider.as_deref(), Some("ideogram"));
        for typed in ["V_3", "v_3"] {
            c.set(ConfigKey::Model, typed).unwrap();
            assert_eq!(c.model.as_deref(), Some("V_3"), "{typed}");
        }
        c.set(ConfigKey::Provider, "local").unwrap();
        c.set(ConfigKey::Model, "Klein").unwrap();
        assert_eq!(c.model.as_deref(), Some("klein"));
    }

    #[test]
    fn unset_settings_show_what_they_come_to() {
        let mut c = Config::default();
        assert_eq!(c.default_for(ConfigKey::Provider), "local");
        assert_eq!(c.default_for(ConfigKey::Model), "auto");
        c.set(ConfigKey::Provider, "openai").unwrap();
        assert_eq!(
            c.default_for(ConfigKey::Model),
            folderskin_ai::provider("openai").unwrap().models[0].id
        );
        assert_eq!(c.default_for(ConfigKey::Tier), "auto");
    }

    #[test]
    fn a_model_belongs_to_its_provider() {
        let mut c = Config::default();
        c.set(ConfigKey::Provider, "openai").unwrap();
        c.set(ConfigKey::Model, "gpt-image-1").unwrap();
        assert_eq!(c.model_for("openai"), Some("gpt-image-1"));
        assert_eq!(
            c.model_for(LOCAL),
            None,
            "another provider doesn't inherit it"
        );
        c.set(ConfigKey::Provider, "xai").unwrap();
        assert_eq!(
            c.model, None,
            "changing provider forgets the old one's model"
        );
        c.unset(ConfigKey::Provider);
        assert_eq!(c, Config::default());
    }

    #[test]
    fn it_round_trips_as_json_leaving_out_what_isnt_set() {
        let mut c = Config::default();
        c.set(ConfigKey::Provider, "local").unwrap();
        c.set(ConfigKey::Model, "klein").unwrap();
        let text = serde_json::to_string(&c).unwrap();
        assert_eq!(text, r#"{"provider":"local","model":"klein"}"#);
        assert_eq!(serde_json::from_str::<Config>(&text).unwrap(), c);
        assert_eq!(
            serde_json::from_str::<Config>("{}").unwrap(),
            Config::default()
        );
    }

    #[test]
    fn the_folder_look_is_the_one_the_app_kept() {
        let dir = std::env::temp_dir().join(format!("fs-cli-look-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(look_saved_in(&dir), Style::Mac, "nothing chosen yet");
        for (kept, look) in [
            ("windows", Style::Windows),
            (
                "mac
",
                Style::Mac,
            ),
            ("?", Style::Mac),
        ] {
            std::fs::write(dir.join(LOOK_FILE), kept).unwrap();
            assert_eq!(look_saved_in(&dir), look, "{kept:?}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn key_variables_are_named_after_the_provider() {
        assert_eq!(key_variable("openai"), "FOLDERSKIN_OPENAI_KEY");
        assert_eq!(key_variable("x-ai"), "FOLDERSKIN_X_AI_KEY");
    }
}
