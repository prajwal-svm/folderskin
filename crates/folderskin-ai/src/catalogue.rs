//! Which providers and models the assistant offers.
//!
//! `native_alpha` is the field that changes FolderSkin's behaviour: models that return a real
//! alpha channel are asked for a transparent background, and everything else is asked for a flat
//! backdrop in the provider's key colour, which FolderSkin keys out itself.
//!
//! Each provider's first model is its default, and each list holds only models the provider has
//! announced no shutdown for. A model taken out stays in [`RETIRED`] with the one that took its
//! place, so a choice saved while it was on offer goes on working.
//!
//! Prices are each provider's own, per picture at about one megapixel, as FolderSkin asks for it
//! (the quality it sends, the reference picture a whole folder carries). They were read from the
//! providers' pricing pages on 26 September 2026.

use folderskin_core::matte::MAGENTA;

/// One model a provider offers.
#[derive(Clone, Copy, Debug)]
pub struct ModelInfo {
    /// Exactly the id the provider's API expects.
    pub id: &'static str,
    pub label: &'static str,
    /// True when the API can return a genuine transparent background.
    pub native_alpha: bool,
    /// True when the model accepts an input picture to work from.
    pub accepts_reference: bool,
    /// How many pictures one request can carry, the template included.
    pub max_references: usize,
    /// Sizes the API accepts; the first is FolderSkin's default. A model that takes any size, or
    /// only aspect ratios, lists the size it is asked for when nothing else is said.
    pub sizes: &'static [&'static str],
    pub price_hint: &'static str,
}

/// One provider, its models, and where the user gets a key.
#[derive(Clone, Copy, Debug)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub models: &'static [ModelInfo],
    pub keys_url: &'static str,
    pub docs_url: &'static str,
    pub key_hint: &'static str,
    /// The backdrop colour its pictures are cut out of: magenta, or green for a provider whose
    /// pictures come back with a dark rim on magenta.
    pub key_colour: [u8; 3],
}

/// A model FolderSkin offered once and doesn't now, and the model that took its place.
#[derive(Clone, Copy, Debug)]
pub struct Retired {
    pub provider: &'static str,
    pub id: &'static str,
    pub label: &'static str,
    /// The model a saved choice of this one moves to.
    pub successor: &'static str,
}

/// GPT Image 2 and 2.5 take any size on a 16-pixel grid. FolderSkin asks Flare for `high`,
/// Sunburst for `max` and GPT Image 2 for `medium`, and those are the prices at about a
/// megapixel, from OpenAI's per-token prices and its own calculator.
const OPENAI_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gpt-image-2.5-flare",
        label: "GPT Image 2.5 Flare",
        native_alpha: true,
        accepts_reference: true,
        max_references: 16,
        sizes: &["1024x1024"],
        price_hint: "~$0.05 / image",
    },
    ModelInfo {
        id: "gpt-image-2.5-sunburst",
        label: "GPT Image 2.5 Sunburst",
        native_alpha: true,
        accepts_reference: true,
        max_references: 16,
        sizes: &["1024x1024"],
        price_hint: "~$0.20 / image",
    },
    // Its transparent background is still a preview, so it is keyed like the models without.
    ModelInfo {
        id: "gpt-image-2",
        label: "GPT Image 2",
        native_alpha: false,
        accepts_reference: true,
        max_references: 16,
        sizes: &["1024x1024"],
        price_hint: "~$0.05 / image",
    },
];

/// Grok Imagine 2.0 is xAI's image model now; the first Grok Imagine stays as the cheaper one.
/// An edit bills the picture sent as well as the one made, so a whole folder costs a little more.
const XAI_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "grok-imagine-image-2.0",
        label: "Grok Imagine 2.0",
        native_alpha: false,
        accepts_reference: true,
        max_references: 5,
        sizes: &["1024x1024"],
        price_hint: "~$0.04 / image",
    },
    ModelInfo {
        id: "grok-imagine-image",
        label: "Grok Imagine",
        native_alpha: false,
        accepts_reference: true,
        max_references: 3,
        sizes: &["1024x1024"],
        price_hint: "~$0.02 / image",
    },
];

/// Recraft V4.1 is Recraft's default. Its 1K sizes are a fixed list. V3's prompts stop at 1,000
/// characters, fewer than FolderSkin's own instructions take, so it is no longer offered.
const RECRAFT_SIZES: &[&str] = &[
    "1024x1024",
    "1536x768",
    "768x1536",
    "1280x832",
    "832x1280",
    "1216x896",
    "896x1216",
    "1152x896",
    "896x1152",
    "832x1344",
    "1280x896",
    "896x1280",
    "1344x768",
    "768x1344",
];

const RECRAFT_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "recraftv4_1",
        label: "Recraft V4.1",
        native_alpha: false,
        accepts_reference: false,
        max_references: 0,
        sizes: RECRAFT_SIZES,
        price_hint: "~$0.035 / image",
    },
    ModelInfo {
        id: "recraftv4_1_flash",
        label: "Recraft V4.1 Flash",
        native_alpha: false,
        accepts_reference: false,
        max_references: 0,
        sizes: RECRAFT_SIZES,
        price_hint: "~$0.007 / image",
    },
];

/// Gemini 2.5 Flash Image shuts down on 2 October 2026, and 3.1 Flash Image is Google's
/// replacement. 3 Pro Image costs twice as much and letters best; 3.1 Flash Lite Image is the
/// cheapest. The Pro price includes the thinking it always does.
const GOOGLE_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gemini-3.1-flash-image",
        label: "Gemini 3.1 Flash Image",
        native_alpha: false,
        accepts_reference: true,
        max_references: 14,
        sizes: &["1024x1024"],
        price_hint: "~$0.07 / image",
    },
    ModelInfo {
        id: "gemini-3-pro-image",
        label: "Gemini 3 Pro Image",
        native_alpha: false,
        accepts_reference: true,
        max_references: 14,
        sizes: &["1024x1024"],
        price_hint: "~$0.14 / image",
    },
    ModelInfo {
        id: "gemini-3.1-flash-lite-image",
        label: "Gemini 3.1 Flash Lite Image",
        native_alpha: false,
        accepts_reference: true,
        max_references: 14,
        sizes: &["1024x1024"],
        price_hint: "~$0.034 / image",
    },
];

/// FLUX.2 is billed by the megapixel, for the picture made and for each picture sent, so a whole
/// folder (which sends the template) costs the upper figure. FLUX 1.1 Pro is the previous
/// generation and took no pictures, so it made way for FLUX.2 pro.
const BFL_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "flux-2-pro",
        label: "FLUX.2 pro",
        native_alpha: false,
        accepts_reference: true,
        max_references: 8,
        sizes: &["1024x1024"],
        price_hint: "~$0.03–0.05 / image",
    },
    ModelInfo {
        id: "flux-2-max",
        label: "FLUX.2 max",
        native_alpha: false,
        accepts_reference: true,
        max_references: 8,
        sizes: &["1024x1024"],
        price_hint: "~$0.07–0.10 / image",
    },
    ModelInfo {
        id: "flux-2-flex",
        label: "FLUX.2 flex",
        native_alpha: false,
        accepts_reference: true,
        max_references: 8,
        sizes: &["1024x1024"],
        price_hint: "~$0.05–0.10 / image",
    },
    ModelInfo {
        id: "flux-2-klein-4b",
        label: "FLUX.2 klein 4B",
        native_alpha: false,
        accepts_reference: true,
        max_references: 4,
        sizes: &["1024x1024"],
        price_hint: "~$0.015 / image",
    },
];

/// A credit is a cent: Ultra takes 8 a picture and Core 3.
const STABILITY_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "ultra",
        label: "Stable Image Ultra",
        native_alpha: false,
        accepts_reference: false,
        max_references: 0,
        sizes: &["1024x1024"],
        price_hint: "~$0.08 / image",
    },
    ModelInfo {
        id: "core",
        label: "Stable Image Core",
        native_alpha: false,
        accepts_reference: false,
        max_references: 0,
        sizes: &["1024x1024"],
        price_hint: "~$0.03 / image",
    },
];

/// Ideogram 3.0 at its default speed. 4.0 rewrites every prompt given as words, which FolderSkin
/// keeps out of the way of the idea, so it isn't offered until its prompts are written for it.
const IDEOGRAM_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "V_3",
    label: "Ideogram 3.0",
    native_alpha: false,
    accepts_reference: false,
    max_references: 0,
    sizes: &["1024x1024"],
    price_hint: "~$0.06 / image",
}];

const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "openai",
        label: "OpenAI",
        models: OPENAI_MODELS,
        keys_url: "https://platform.openai.com/api-keys",
        docs_url: "https://developers.openai.com/api/docs/guides/image-generation",
        key_hint: "starts with sk-",
        key_colour: MAGENTA,
    },
    ProviderInfo {
        id: "xai",
        label: "xAI Grok",
        models: XAI_MODELS,
        keys_url: "https://console.x.ai",
        docs_url: "https://docs.x.ai/developers/model-capabilities/imagine",
        key_hint: "starts with xai-",
        key_colour: MAGENTA,
    },
    ProviderInfo {
        id: "recraft",
        label: "Recraft",
        models: RECRAFT_MODELS,
        keys_url: "https://app.recraft.ai/profile/api",
        docs_url: "https://www.recraft.ai/docs",
        key_hint: "from your Recraft profile",
        key_colour: MAGENTA,
    },
    ProviderInfo {
        id: "google",
        label: "Google Gemini",
        models: GOOGLE_MODELS,
        keys_url: "https://aistudio.google.com/apikey",
        docs_url: "https://ai.google.dev/gemini-api/docs/generate-content/image-generation",
        key_hint: "from Google AI Studio",
        // Gemini leaves a dark reddish rim around a subject on magenta, and none on green.
        key_colour: crate::finish::GREEN,
    },
    ProviderInfo {
        id: "bfl",
        label: "Black Forest Labs",
        models: BFL_MODELS,
        keys_url: "https://dashboard.bfl.ai",
        docs_url: "https://docs.bfl.ai",
        key_hint: "from the BFL dashboard",
        key_colour: MAGENTA,
    },
    ProviderInfo {
        id: "stability",
        label: "Stability AI",
        models: STABILITY_MODELS,
        keys_url: "https://platform.stability.ai/account/keys",
        docs_url: "https://platform.stability.ai/docs/api-reference",
        key_hint: "starts with sk-",
        key_colour: MAGENTA,
    },
    ProviderInfo {
        id: "ideogram",
        label: "Ideogram",
        models: IDEOGRAM_MODELS,
        keys_url: "https://ideogram.ai/manage-api",
        docs_url: "https://developer.ideogram.ai",
        key_hint: "from your Ideogram account",
        key_colour: MAGENTA,
    },
];

/// Models taken out of the catalogue, each with the one a saved choice of it moves to.
pub const RETIRED: &[Retired] = &[
    // OpenAI shuts GPT Image 1 down on 23 October 2026 and names GPT Image 2 as its replacement.
    Retired {
        provider: "openai",
        id: "gpt-image-1",
        label: "GPT Image 1",
        successor: "gpt-image-2",
    },
    Retired {
        provider: "google",
        id: "gemini-2.5-flash-image",
        label: "Gemini 2.5 Flash Image",
        successor: "gemini-3.1-flash-image",
    },
    Retired {
        provider: "bfl",
        id: "flux-pro-1.1",
        label: "FLUX 1.1 Pro",
        successor: "flux-2-pro",
    },
    Retired {
        provider: "recraft",
        id: "recraftv3",
        label: "Recraft V3",
        successor: "recraftv4_1",
    },
];

pub fn providers() -> &'static [ProviderInfo] {
    PROVIDERS
}

pub fn provider(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// The model `model_id` names at `provider_id`: one on offer, or the one that took the place of a
/// model FolderSkin no longer offers, so a choice saved while it was on offer goes on working.
pub fn model(provider_id: &str, model_id: &str) -> Option<&'static ModelInfo> {
    let offered = |id: &str| provider(provider_id)?.models.iter().find(|m| m.id == id);
    offered(model_id).or_else(|| offered(retired(provider_id, model_id)?.successor))
}

/// The name of the model `model_id` names, whether FolderSkin offers it now or did once: what
/// made a skin stays what it was.
pub fn model_label(provider_id: &str, model_id: &str) -> Option<&'static str> {
    provider(provider_id)?
        .models
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.label)
        .or_else(|| retired(provider_id, model_id).map(|r| r.label))
}

fn retired(provider_id: &str, model_id: &str) -> Option<&'static Retired> {
    RETIRED
        .iter()
        .find(|r| r.provider == provider_id && r.id == model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_are_unique_and_lookups_work() {
        let mut ids: Vec<&str> = providers().iter().map(|p| p.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(provider("openai").unwrap().label, "OpenAI");
        assert!(provider("nope").is_none());
    }

    #[test]
    fn every_model_is_usable() {
        for p in providers() {
            assert!(!p.models.is_empty(), "{} has no models", p.id);
            assert!(p.keys_url.starts_with("https://"), "{}", p.id);
            assert!(p.docs_url.starts_with("https://"), "{}", p.id);
            let mut ids: Vec<&str> = p.models.iter().map(|m| m.id).collect();
            let count = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), count, "{} repeats a model id", p.id);
            for m in p.models {
                assert!(!m.sizes.is_empty(), "{}/{} has no sizes", p.id, m.id);
                // The app says it again in the language on show: "~{{price}} / image".
                assert!(
                    m.price_hint.starts_with("~$") && m.price_hint.ends_with(" / image"),
                    "{}/{}: {}",
                    p.id,
                    m.id,
                    m.price_hint
                );
                assert_eq!(
                    m.accepts_reference,
                    m.max_references > 0,
                    "{}/{} says two things about pictures",
                    p.id,
                    m.id
                );
                for s in m.sizes {
                    let (w, h) = s.split_once('x').expect("sizes look like 1024x1024");
                    assert!(w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok(), "{s}");
                }
            }
        }
    }

    #[test]
    fn model_lookup_is_scoped_to_its_provider() {
        assert!(model("openai", "gpt-image-2.5-flare").is_some());
        assert!(
            model("xai", "gpt-image-2.5-flare").is_none(),
            "models must not leak across providers"
        );
        assert!(model("nope", "whatever").is_none());
        assert!(model("openai", "no-such-model").is_none());
    }

    #[test]
    fn a_saved_choice_of_a_retired_model_moves_to_its_successor() {
        for r in RETIRED {
            let p = provider(r.provider).expect("a retired model's provider is still offered");
            assert!(
                p.models.iter().all(|m| m.id != r.id),
                "{} is retired and offered at once",
                r.id
            );
            assert_eq!(model(r.provider, r.id).map(|m| m.id), Some(r.successor));
            // What made a skin keeps its own name.
            assert_eq!(model_label(r.provider, r.id), Some(r.label));
            // Only within its own provider.
            assert!(model("stability", r.id).is_none());
        }
        assert_eq!(
            model("openai", "gpt-image-1").map(|m| m.label),
            Some("GPT Image 2")
        );
        assert_eq!(
            model_label("openai", "gpt-image-2.5-flare"),
            Some("GPT Image 2.5 Flare")
        );
        assert_eq!(model_label("openai", "dall-e-9"), None);
    }

    #[test]
    fn only_the_models_documented_as_alpha_capable_claim_it() {
        for p in providers() {
            for m in p.models {
                let expected = m.id.starts_with("gpt-image-2.5-");
                assert_eq!(m.native_alpha, expected, "{}/{} alpha flag", p.id, m.id);
            }
        }
    }

    #[test]
    fn google_is_cut_out_of_green_and_everyone_else_of_magenta() {
        for p in providers() {
            let expected = if p.id == "google" {
                crate::finish::GREEN
            } else {
                MAGENTA
            };
            assert_eq!(p.key_colour, expected, "{}", p.id);
        }
    }
}
