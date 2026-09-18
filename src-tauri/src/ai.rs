//! The AI assistant's commands: provider catalogue, key storage and one generation.
//!
//! Every request uses the user's own key, read from the OS keychain at the moment of the call.
//! Nothing here runs unless the user presses Generate, and no key is ever returned to the
//! webview, written to a file, or included in an error message.

use crate::commands::{data_url, SkinDto};
use crate::state::AppState;
use folderskin_ai::prompts::{self, Shape};
use folderskin_core::compositor::{self, Artwork};
use folderskin_core::manifest::{SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte::{self, KeyOptions, MAGENTA};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Size the folder shape is rendered at before it is cut out (the folder's own aspect).
const FOLDER_W: u32 = 1166;
const FOLDER_H: u32 = 1091;
const THUMB_SIZE: u32 = 512;
/// Reference pictures are downscaled before upload; models do not need more and it keeps the
/// request small.
const REFERENCE_MAX_SIDE: u32 = 1024;

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
}

/// Providers, their models, whether a key is already saved, and the prompt presets.
#[tauri::command]
pub fn ai_catalogue() -> AiCatalogueDto {
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
            has_key: folderskin_ai::keys::has(p.id),
        })
        .collect();
    let presets = prompts::PRESETS
        .iter()
        .map(|p| AiPresetDto { id: p.id.to_string(), label: p.label.to_string(), idea: p.idea.to_string() })
        .collect();
    AiCatalogueDto { providers, presets }
}

/// Stores a key in the OS keychain. The key never comes back out to the webview.
#[tauri::command]
pub fn ai_set_key(provider: String, key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("that key is empty".into());
    }
    folderskin_ai::keys::set(&provider, key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_clear_key(provider: String) -> Result<(), String> {
    folderskin_ai::keys::clear(&provider).map_err(|e| e.to_string())
}

/// Confirms the stored key is accepted by the provider.
#[tauri::command]
pub async fn ai_test_key(provider: String) -> Result<(), String> {
    let key = stored_key(&provider)?;
    folderskin_ai::test_key(&provider, &key).await.map_err(|e| e.to_string())
}

/// Generates one image and adds it to the session's skins.
#[tauri::command]
pub async fn ai_generate(
    app: AppHandle,
    state: State<'_, AppState>,
    req: AiGenerateRequest,
) -> Result<SkinDto, String> {
    let state = state.inner().clone();
    let shape = Shape::from_id(&req.shape).ok_or_else(|| format!("unknown shape {:?}", req.shape))?;
    let model = folderskin_ai::model(&req.provider, &req.model)
        .ok_or_else(|| format!("{} does not offer a model called {}", req.provider, req.model))?;
    if req.idea.trim().is_empty() {
        return Err("describe what the skin should look like first".into());
    }
    let key = stored_key(&req.provider)?;

    // A folder render needs transparency. Use the model's own alpha when it has one, otherwise
    // ask for a magenta backdrop and cut it out ourselves.
    let wants_cutout = shape == Shape::Folder;
    let want_alpha = wants_cutout && model.native_alpha;
    let key_hex = (wants_cutout && !model.native_alpha).then_some("#FF00FF");
    let (width, height) = match shape {
        Shape::Skin => (SKIN_WIDTH, SKIN_HEIGHT),
        Shape::Folder => (FOLDER_W, FOLDER_H),
    };

    let reference_png = match req.reference_path.as_deref().filter(|_| model.accepts_reference) {
        Some(path) => Some(load_reference(PathBuf::from(path))?),
        None => None,
    };
    let prompt = if reference_png.is_some() {
        prompts::compose_with_reference(shape, &req.idea, width, height, key_hex)
    } else {
        prompts::compose(shape, &req.idea, width, height, key_hex)
    };

    let result = folderskin_ai::generate(
        &folderskin_ai::GenerateRequest {
            provider: req.provider.clone(),
            model: model.id.to_string(),
            prompt,
            reference_png,
            size: req.size.clone(),
            want_alpha,
        },
        &key,
    )
    .await
    .map_err(|e| e.to_string())?;

    let name = short_name(&req.idea);
    let id = format!("ai:{}", &hash12(&result.image));

    tauri::async_runtime::spawn_blocking(move || {
        let img = image::load_from_memory(&result.image)
            .map_err(|_| "the provider returned something that is not an image".to_string())?
            .to_rgba8();

        let (thumbnail, skin_id) = if wants_cutout {
            let cut = if result.native_alpha {
                matte::autocrop(&img, 0)
            } else {
                if !matte::has_key_background(&img, MAGENTA, KeyOptions::default()) {
                    return Err(
                        "the model drew a scene instead of a folder on a plain backdrop. Try again, or \
                         switch to Artwork, which does not need one."
                            .to_string(),
                    );
                }
                matte::cutout(&img, MAGENTA, KeyOptions::default())
            };
            let png = compositor::preview_png_from_image(&cut, THUMB_SIZE);
            state.remember_prerendered(id.clone(), Arc::new(cut));
            (data_url(&png), id)
        } else {
            let art = Arc::new(Artwork { rgba: matte::crop_to_aspect(&img, SKIN_WIDTH, SKIN_HEIGHT, (0.5, 0.5)), focus: (0.5, 0.5) });
            let png = compositor::render_preview_png(&art, THUMB_SIZE);
            state.remember_custom(id.clone(), art);
            (data_url(&png), id)
        };
        state.remember_thumb(skin_id.clone(), thumbnail.clone());
        let _ = &app;
        Ok(SkinDto { id: skin_id, name, collection: "yours".into(), thumbnail, custom: true })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- helpers ----------

fn stored_key(provider: &str) -> Result<String, String> {
    folderskin_ai::keys::get(provider)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("add your {provider} API key first"))
}

/// Reads a reference picture and re-encodes it as a modest PNG for upload.
fn load_reference(path: PathBuf) -> Result<Vec<u8>, String> {
    let img = image::open(&path).map_err(|_| "couldn't read that reference picture".to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    let img = if longest > REFERENCE_MAX_SIDE {
        let s = REFERENCE_MAX_SIDE as f32 / longest as f32;
        image::imageops::resize(
            &img,
            ((w as f32 * s).round() as u32).max(1),
            ((h as f32 * s).round() as u32).max(1),
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };
    Ok(folderskin_core::raster::encode_png(&img))
}

/// A short, human label for a generated skin, taken from the first few words of the idea.
pub fn short_name(idea: &str) -> String {
    let words: Vec<&str> = idea.split_whitespace().take(3).collect();
    let mut name = words.join(" ");
    if name.is_empty() {
        return "Generated".into();
    }
    name.truncate(28);
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
    fn hash12_is_stable_and_short() {
        assert_eq!(hash12(b"abc"), hash12(b"abc"));
        assert_ne!(hash12(b"abc"), hash12(b"abd"));
        assert_eq!(hash12(b"abc").len(), 12);
    }
}
