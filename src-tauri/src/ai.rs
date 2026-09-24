//! The AI assistant's commands: provider catalogue, key storage and one generation.
//!
//! Every request uses the user's own key, read from FolderSkin's private key file (keys.rs) at the
//! moment of the call. Nothing here runs unless the user presses Generate, and no key is ever
//! returned to the webview or included in an error message.

use crate::commands::SkinDto;
use crate::keys::Keys;
use crate::state::AppState;
use crate::store::{self, NewSkin, SkinImage, SkinSource};
use folderskin_ai::prompts::{self, Shape};
use folderskin_ai::Finished;
use folderskin_core::compositor::Artwork;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[derive(Serialize)]
pub struct AiModelDto {
    pub id: String,
    pub label: String,
    pub native_alpha: bool,
    pub accepts_reference: bool,
    pub sizes: Vec<String>,
    pub price_hint: String,
}

#[derive(Serialize)]
pub struct AiProviderDto {
    pub id: String,
    pub label: String,
    pub models: Vec<AiModelDto>,
    pub keys_url: String,
    pub docs_url: String,
    pub key_hint: String,
    pub has_key: bool,
}

#[derive(Serialize)]
pub struct AiPresetDto {
    pub id: String,
    pub label: String,
    pub idea: String,
}

#[derive(Serialize)]
pub struct AiCatalogueDto {
    pub providers: Vec<AiProviderDto>,
    pub presets: Vec<AiPresetDto>,
}

#[derive(Deserialize)]
pub struct AiGenerateRequest {
    pub provider: String,
    pub model: String,
    pub idea: String,
    /// "skin" (flat artwork for our compositor) or "folder" (the model draws the whole folder).
    pub shape: String,
    pub size: Option<String>,
    pub reference_path: Option<String>,
    /// Tags for the result, such as the style the idea asks for. Cleaned before saving.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Providers, their models, whether a key is already saved, and the prompt presets.
#[tauri::command]
pub fn ai_catalogue(keys: State<'_, Keys>) -> AiCatalogueDto {
    let providers = folderskin_ai::providers()
        .iter()
        .map(|p| AiProviderDto {
            id: p.id.to_string(),
            label: p.label.to_string(),
            models: p
                .models
                .iter()
                .map(|m| AiModelDto {
                    id: m.id.to_string(),
                    label: m.label.to_string(),
                    native_alpha: m.native_alpha,
                    accepts_reference: m.accepts_reference,
                    sizes: m.sizes.iter().map(|s| s.to_string()).collect(),
                    price_hint: m.price_hint.to_string(),
                })
                .collect(),
            keys_url: p.keys_url.to_string(),
            docs_url: p.docs_url.to_string(),
            key_hint: p.key_hint.to_string(),
            has_key: keys.has(p.id),
        })
        .collect();
    let presets = prompts::PRESETS
        .iter()
        .map(|p| AiPresetDto {
            id: p.id.to_string(),
            label: p.label.to_string(),
            idea: p.idea.to_string(),
        })
        .collect();
    AiCatalogueDto { providers, presets }
}

/// Saves a key to the private key file. The key never comes back out to the webview.
#[tauri::command]
pub fn ai_set_key(keys: State<'_, Keys>, provider: String, key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("that key is empty".into());
    }
    if folderskin_ai::catalogue::provider(&provider).is_none() {
        return Err(format!(
            "FolderSkin doesn't know a provider called {provider:?}"
        ));
    }
    keys.set(&provider, key)
}

#[tauri::command]
pub fn ai_clear_key(keys: State<'_, Keys>, provider: String) -> Result<(), String> {
    keys.clear(&provider)
}

/// Confirms the stored key is accepted by the provider.
#[tauri::command]
pub async fn ai_test_key(keys: State<'_, Keys>, provider: String) -> Result<(), String> {
    let key = stored_key(&keys, &provider)?;
    folderskin_ai::test_key(&provider, &key)
        .await
        .map_err(|e| e.to_string())
}

/// Generates one image and saves it as a skin, like an imported picture.
#[tauri::command]
pub async fn ai_generate(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    req: AiGenerateRequest,
) -> Result<SkinDto, String> {
    let state = state.inner().clone();
    let shape =
        Shape::from_id(&req.shape).ok_or_else(|| format!("unknown shape {:?}", req.shape))?;
    let model = folderskin_ai::model(&req.provider, &req.model).ok_or_else(|| {
        format!(
            "{} does not offer a model called {}",
            req.provider, req.model
        )
    })?;
    if req.idea.trim().is_empty() {
        return Err("describe what the skin should look like first".into());
    }
    let key = stored_key(&keys, &req.provider)?;

    let user_reference = req
        .reference_path
        .clone()
        .filter(|p| !p.trim().is_empty() && model.accepts_reference);
    let reference_png = match user_reference {
        Some(path) => Some(
            tauri::async_runtime::spawn_blocking(move || load_reference(PathBuf::from(path)))
                .await
                .map_err(|e| e.to_string())??,
        ),
        None => None,
    };
    // The prompt, the size, and our blank template when a whole folder should repaint it; shared
    // with the command line (folderskin_ai::finish).
    let (provider, idea, size) = (req.provider.clone(), req.idea.clone(), req.size.clone());
    let request = tauri::async_runtime::spawn_blocking(move || {
        folderskin_ai::plan(&provider, model, shape, &idea, size, reference_png)
    })
    .await
    .map_err(|e| e.to_string())?;

    let result = folderskin_ai::generate(&request, &key)
        .await
        .map_err(|e| e.to_string())?;

    let new = NewSkin {
        id: store::skin_id(&result.image),
        name: short_name(&req.idea),
        source: SkinSource::Ai,
        provider: Some(req.provider.clone()),
        model: Some(model.id.to_string()),
        idea: Some(req.idea.trim().to_string()),
        tags: req.tags.clone(),
        pack: None,
        pack_name: None,
        author: None,
        license: None,
        pack_hash: None,
    };

    tauri::async_runtime::spawn_blocking(move || {
        let image = match folderskin_ai::finish(&result, shape).map_err(|e| e.to_string())? {
            Finished::Folder(cut) => SkinImage::Folder(Arc::new(cut)),
            Finished::Artwork(rgba) => SkinImage::Artwork(Arc::new(Artwork {
                rgba,
                focus: (0.5, 0.5),
            })),
        };
        // The user has paid for this image, so a failed write keeps it for the session instead
        // of throwing it away.
        let (entry, thumb) = state.save(new.clone(), image.clone()).unwrap_or_else(|e| {
            eprintln!("folderskin: keeping {} for this session only: {e}", new.id);
            state.keep_unsaved(new, image)
        });
        Ok(SkinDto::saved(&entry, &thumb))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- helpers ----------

fn stored_key(keys: &Keys, provider: &str) -> Result<String, String> {
    keys.get(provider)
        .ok_or_else(|| format!("add your {provider} API key first"))
}

/// Reads a reference picture and re-encodes it as a modest PNG for upload.
fn load_reference(path: PathBuf) -> Result<Vec<u8>, String> {
    let img = image::open(&path)
        .map_err(|_| "couldn't read that reference picture".to_string())?
        .to_rgba8();
    Ok(folderskin_ai::finish::reference_png(img))
}

/// A short, human label for a generated skin, taken from the first few words of the idea.
pub fn short_name(idea: &str) -> String {
    const MAX_BYTES: usize = 28;
    let words: Vec<&str> = idea.split_whitespace().take(3).collect();
    let mut name = words.join(" ");
    if name.is_empty() {
        return "Generated".into();
    }
    // Cut on a character boundary: `truncate` panics inside a multi-byte character.
    if name.len() > MAX_BYTES {
        let cut = (0..=MAX_BYTES)
            .rev()
            .find(|&i| name.is_char_boundary(i))
            .unwrap_or(0);
        name.truncate(cut);
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Generated".into(),
    }
}

/// First 12 hex characters of the image's SHA-256, so the same bytes always map to one id.
pub fn hash12(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_name_uses_the_first_words_and_capitalises() {
        assert_eq!(short_name("a night sky with aurora ribbons"), "A night sky");
        assert_eq!(short_name(""), "Generated");
        assert_eq!(short_name("   "), "Generated");
        assert!(short_name(&"verylongword".repeat(10)).len() <= 28);
    }

    #[test]
    fn short_name_never_cuts_a_character_in_half() {
        // Ten three-byte characters: byte 28 falls inside the tenth.
        let name = short_name("桜桜桜桜桜桜桜桜桜桜 at night");
        assert_eq!(name, "桜".repeat(9));
        assert_eq!(short_name("🦊🦊🦊🦊🦊🦊🦊🦊"), "🦊".repeat(7));
    }

    #[test]
    fn hash12_is_stable_and_short() {
        assert_eq!(hash12(b"abc"), hash12(b"abc"));
        assert_ne!(hash12(b"abc"), hash12(b"abd"));
        assert_eq!(hash12(b"abc").len(), 12);
    }
}
