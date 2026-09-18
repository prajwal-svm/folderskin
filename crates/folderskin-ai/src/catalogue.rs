//! Which providers and models the assistant offers.
//!
//! `native_alpha` is the field that changes FolderSkin's behaviour: models that return a real
//! alpha channel are asked for a transparent background, and everything else is asked for a flat
//! magenta backdrop that FolderSkin keys out itself.

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
    /// Sizes the API accepts; the first is FolderSkin's default.
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
}

const OPENAI_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gpt-image-2.5-flare",
        label: "GPT Image 2.5 Flare",
        native_alpha: true,
        accepts_reference: true,
        sizes: &["1024x1024", "1536x1024", "1024x1536"],
        price_hint: "~$0.04 / image",
    },
    ModelInfo {
        id: "gpt-image-2.5-sunburst",
        label: "GPT Image 2.5 Sunburst",
        native_alpha: true,
        accepts_reference: true,
        sizes: &["1024x1024", "1536x1024", "1024x1536"],
        price_hint: "~$0.19 / image",
    },
    ModelInfo {
        id: "gpt-image-1",
        label: "GPT Image 1",
        native_alpha: true,
        accepts_reference: true,
        sizes: &["1024x1024", "1536x1024", "1024x1536"],
        price_hint: "~$0.02–0.17 / image",
    },
];

const XAI_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "grok-imagine-image",
    label: "Grok Imagine",
    native_alpha: false,
    accepts_reference: true,
    sizes: &["1024x1024"],
    price_hint: "~$0.02 / image",
}];

const RECRAFT_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "recraftv3",
    label: "Recraft V3",
    native_alpha: true,
    accepts_reference: false,
    sizes: &["1024x1024", "1365x1024", "1024x1365"],
    price_hint: "~$0.04 / image",
}];

const GOOGLE_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "gemini-2.5-flash-image",
    label: "Gemini 2.5 Flash Image",
    native_alpha: false,
    accepts_reference: true,
    sizes: &["1024x1024"],
    price_hint: "~$0.04 / image",
}];

const BFL_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "flux-pro-1.1",
    label: "FLUX 1.1 Pro",
    native_alpha: false,
    accepts_reference: false,
    sizes: &["1024x1024"],
    price_hint: "~$0.04 / image",
}];

const STABILITY_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "core",
    label: "Stable Image Core",
    native_alpha: false,
    accepts_reference: false,
    sizes: &["1024x1024"],
    price_hint: "~3 credits / image",
}];

const IDEOGRAM_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "V_3",
    label: "Ideogram v3",
    native_alpha: false,
    accepts_reference: false,
    sizes: &["1024x1024"],
    price_hint: "~$0.03–0.09 / image",
}];

const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "openai",
        label: "OpenAI",
        models: OPENAI_MODELS,
        keys_url: "https://platform.openai.com/api-keys",
        docs_url: "https://platform.openai.com/docs/guides/image-generation",
        key_hint: "starts with sk-",
    },
    ProviderInfo {
        id: "xai",
        label: "xAI Grok",
        models: XAI_MODELS,
        keys_url: "https://console.x.ai",
        docs_url: "https://docs.x.ai/developers/model-capabilities/imagine",
        key_hint: "starts with xai-",
    },
    ProviderInfo {
        id: "recraft",
        label: "Recraft",
        models: RECRAFT_MODELS,
        keys_url: "https://www.recraft.ai/profile/api",
        docs_url: "https://www.recraft.ai/docs",
        key_hint: "from your Recraft profile",
    },
    ProviderInfo {
        id: "google",
        label: "Google Gemini",
        models: GOOGLE_MODELS,
        keys_url: "https://aistudio.google.com/apikey",
        docs_url: "https://ai.google.dev/gemini-api/docs/image-generation",
        key_hint: "from Google AI Studio",
    },
    ProviderInfo {
        id: "bfl",
        label: "Black Forest Labs",
        models: BFL_MODELS,
        keys_url: "https://dashboard.bfl.ai",
        docs_url: "https://docs.bfl.ai",
        key_hint: "from the BFL dashboard",
    },
    ProviderInfo {
        id: "stability",
        label: "Stability AI",
        models: STABILITY_MODELS,
        keys_url: "https://platform.stability.ai/account/keys",
        docs_url: "https://platform.stability.ai/docs/api-reference",
        key_hint: "starts with sk-",
    },
    ProviderInfo {
        id: "ideogram",
        label: "Ideogram",
        models: IDEOGRAM_MODELS,
        keys_url: "https://ideogram.ai/manage-api",
        docs_url: "https://developer.ideogram.ai",
        key_hint: "from your Ideogram account",
    },
];

pub fn providers() -> &'static [ProviderInfo] {
    PROVIDERS
}

pub fn provider(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

pub fn model(provider_id: &str, model_id: &str) -> Option<&'static ModelInfo> {
    provider(provider_id)?
        .models
        .iter()
        .find(|m| m.id == model_id)
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
                assert!(!m.price_hint.is_empty(), "{}/{}", p.id, m.id);
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
    }

    #[test]
    fn only_the_providers_documented_as_alpha_capable_claim_it() {
        for p in providers() {
            for m in p.models {
                let expected = matches!(p.id, "openai" | "recraft");
                assert_eq!(m.native_alpha, expected, "{}/{} alpha flag", p.id, m.id);
            }
        }
    }
}
