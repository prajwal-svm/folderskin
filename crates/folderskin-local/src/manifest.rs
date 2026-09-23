//! Everything generation downloads, pinned: a build, a revision, a size and a hash for every
//! file, so what was tested is what gets downloaded, and a run next month never needs GitHub's
//! API (whose anonymous limit is 60 calls an hour). `setup --runtime latest` moves the runtime on.

use crate::machine::{Backend, Os, Tier};
use serde::Serialize;
use std::path::PathBuf;

/// The stable-diffusion.cpp release everything here was tested with.
pub const SDCPP_TAG: &str = "master-899-28b454b";
pub const SDCPP_REPO: &str = "https://github.com/leejet/stable-diffusion.cpp";
/// Where `--runtime latest` asks which release is newest.
pub const SDCPP_LATEST_API: &str =
    "https://api.github.com/repos/leejet/stable-diffusion.cpp/releases/latest";

/// One file of a stable-diffusion.cpp release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

impl Asset {
    pub fn url(&self) -> String {
        format!("{SDCPP_REPO}/releases/download/{SDCPP_TAG}/{}", self.name)
    }
}

const fn asset(name: &'static str, size: u64, sha256: &'static str) -> Asset {
    Asset { name, size, sha256 }
}

/// The release assets that make up each backend. The CUDA build needs its runtime DLLs beside it.
/// The macOS build wants macOS 26; mflux (`--backend mlx`) is the Mac's default.
pub fn sdcpp_assets(os: Os, backend: Backend) -> Option<&'static [Asset]> {
    const WINDOWS_CUDA: &[Asset] = &[
        asset(
            "sd-master-28b454b-bin-win-cuda12-x64.zip",
            333408797,
            "85ba25c948b7a8e11e0d9bc2dddc977cad93f0690c31ed787ae089daf69a0588",
        ),
        asset(
            "cudart-sd-bin-win-cu12-x64.zip",
            563452046,
            "fe20366827d357c00797eebb58244dddab7fd9a348d70090c3871004c320f38d",
        ),
    ];
    const WINDOWS_VULKAN: &[Asset] = &[asset(
        "sd-master-28b454b-bin-win-vulkan-x64.zip",
        31955788,
        "3a4e5a75f022e4c0cad3e5a28c5921683adcb1808c0dd8e8a3536493497c793b",
    )];
    const WINDOWS_CPU: &[Asset] = &[asset(
        "sd-master-28b454b-bin-win-cpu-x64.zip",
        17205650,
        "cb55af2f5f112f5ef6a8b2785e25d44b5427714b89acae3f7c6fcf4ba7a72eb0",
    )];
    const LINUX_VULKAN: &[Asset] = &[asset(
        "sd-master-28b454b-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip",
        38579477,
        "28675635a82dd24970acd9600dc5f82a6eab1a54b66e962bb39dd51e3d2b7e47",
    )];
    const LINUX_CPU: &[Asset] = &[asset(
        "sd-master-28b454b-bin-Linux-Ubuntu-24.04-x86_64.zip",
        25456185,
        "378f5dcb7bdba87c2e50ea264eb17cb9e73c2bd6343294c1fc72279d78408b07",
    )];
    const MACOS_METAL: &[Asset] = &[asset(
        "sd-master-28b454b-bin-Darwin-macOS-26.6.2-arm64.zip",
        34355695,
        "2ef9041b3464dd4354748e52acb4c8a90904150231f131fccc3588b934e1f92d",
    )];
    match (os, backend) {
        (Os::Windows, Backend::Cuda) => Some(WINDOWS_CUDA),
        (Os::Windows, Backend::Vulkan) => Some(WINDOWS_VULKAN),
        (Os::Windows, Backend::Cpu) => Some(WINDOWS_CPU),
        (Os::Linux, Backend::Vulkan) => Some(LINUX_VULKAN),
        (Os::Linux, Backend::Cpu) => Some(LINUX_CPU),
        (Os::Macos, Backend::Metal) => Some(MACOS_METAL),
        _ => None,
    }
}

/// How to recognise an asset of a newer release, whose name carries its commit: it starts with
/// `prefix` (when there is one), contains `contains` and ends with `suffix`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetPattern {
    pub prefix: &'static str,
    pub contains: &'static str,
    pub suffix: &'static str,
}

impl AssetPattern {
    pub fn matches(&self, name: &str) -> bool {
        name.starts_with(self.prefix) && name.contains(self.contains) && name.ends_with(self.suffix)
    }
}

const fn pattern(
    prefix: &'static str,
    contains: &'static str,
    suffix: &'static str,
) -> AssetPattern {
    AssetPattern {
        prefix,
        contains,
        suffix,
    }
}

/// The same assets as [`sdcpp_assets`], by pattern, for `--runtime latest`.
pub fn sdcpp_patterns(os: Os, backend: Backend) -> Option<&'static [AssetPattern]> {
    const WINDOWS_CUDA: &[AssetPattern] = &[
        pattern("", "", "bin-win-cuda12-x64.zip"),
        pattern("cudart-sd-bin-win-cu12-x64.zip", "", ""),
    ];
    const WINDOWS_VULKAN: &[AssetPattern] = &[pattern("", "", "bin-win-vulkan-x64.zip")];
    const WINDOWS_CPU: &[AssetPattern] = &[pattern("", "", "bin-win-cpu-x64.zip")];
    const LINUX_VULKAN: &[AssetPattern] = &[pattern("", "bin-Linux-", "-x86_64-vulkan.zip")];
    const LINUX_CPU: &[AssetPattern] = &[pattern("", "bin-Linux-", "-x86_64.zip")];
    const MACOS_METAL: &[AssetPattern] = &[pattern("", "bin-Darwin-", "-arm64.zip")];
    match (os, backend) {
        (Os::Windows, Backend::Cuda) => Some(WINDOWS_CUDA),
        (Os::Windows, Backend::Vulkan) => Some(WINDOWS_VULKAN),
        (Os::Windows, Backend::Cpu) => Some(WINDOWS_CPU),
        (Os::Linux, Backend::Vulkan) => Some(LINUX_VULKAN),
        (Os::Linux, Backend::Cpu) => Some(LINUX_CPU),
        (Os::Macos, Backend::Metal) => Some(MACOS_METAL),
        _ => None,
    }
}

/// A model file on Hugging Face, pinned to a revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HfFile {
    pub repo: &'static str,
    pub rev: &'static str,
    pub path: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

impl HfFile {
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo, self.rev, self.path
        )
    }

    /// The file's name, without the folders it has in its repository.
    pub fn name(&self) -> &'static str {
        self.path.rsplit('/').next().unwrap_or(self.path)
    }

    /// Where it is kept on this computer.
    pub fn local(&self) -> PathBuf {
        crate::paths::models_dir().join(self.name())
    }
}

const fn hf(
    repo: &'static str,
    rev: &'static str,
    path: &'static str,
    size: u64,
    sha256: &'static str,
) -> HfFile {
    HfFile {
        repo,
        rev,
        path,
        size,
        sha256,
    }
}

pub const QWEN3_4B_Q8: HfFile = hf(
    "unsloth/Qwen3-4B-GGUF",
    "22c9fc8a8c7700b76a1789366280a6a5a1ad1120",
    "Qwen3-4B-Q8_0.gguf",
    4280405792,
    "eed555233267a33c7e8ee31682762cc7751b3f6d224039086e0e846f05fffa5d",
);
pub const QWEN3_4B_Q4: HfFile = hf(
    "unsloth/Qwen3-4B-GGUF",
    "22c9fc8a8c7700b76a1789366280a6a5a1ad1120",
    "Qwen3-4B-Q4_K_M.gguf",
    2497281312,
    "f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a",
);
pub const ZIMAGE_Q8: HfFile = hf(
    "leejet/Z-Image-Turbo-GGUF",
    "c61c0e422dc8b541b7548cf33a4ef8302b0f8085",
    "z_image_turbo-Q8_0.gguf",
    6577440704,
    "df1c5baa86d1398c979495a6072dbcee79444fdb884a2445582ba0769c44e9a1",
);
pub const ZIMAGE_Q4: HfFile = hf(
    "leejet/Z-Image-Turbo-GGUF",
    "c61c0e422dc8b541b7548cf33a4ef8302b0f8085",
    "z_image_turbo-Q4_K.gguf",
    3864250304,
    "14b375ab4f226bc5378f68f37e899ef3c2242b8541e61e2bc1aff40976086fbd",
);
pub const ZIMAGE_VAE: HfFile = hf(
    "Comfy-Org/z_image_turbo",
    "6fc90a3b1b653e935a0d175e260736de25b84df5",
    "split_files/vae/ae.safetensors",
    335304388,
    "afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38",
);
pub const KLEIN_Q8: HfFile = hf(
    "leejet/FLUX.2-klein-4B-GGUF",
    "3b1f5a9dc3abb32238b053aeb3d823c30afdacbd",
    "flux-2-klein-4b-Q8_0.gguf",
    4300629440,
    "0bba6951258ec8f92d51114a8fa13e66828297bfff58a738f52729b3ef66fa28",
);
pub const KLEIN_Q4: HfFile = hf(
    "leejet/FLUX.2-klein-4B-GGUF",
    "3b1f5a9dc3abb32238b053aeb3d823c30afdacbd",
    "flux-2-klein-4b-Q4_0.gguf",
    2460378560,
    "d1023499ef3f2f82ff7c50e6778495195c1b6cc34835741778868428111f9ff4",
);
/// The FLUX.2 VAE in the FLUX.2-dev repository is gated; this one isn't, and sd.cpp takes it.
pub const KLEIN_VAE: HfFile = hf(
    "black-forest-labs/FLUX.2-small-decoder",
    "a3efc24f613ef42d9428af62fdbd6f5fd8856c4a",
    "full_encoder_small_decoder.safetensors",
    249519092,
    "ea4273f02d1fafbf8e1d1c2cf6018ed8748652eb0bf34f2dd91171f16f15ab62",
);

/// Which of the two models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelId {
    Zimage,
    Klein,
}

impl ModelId {
    pub fn id(self) -> &'static str {
        match self {
            ModelId::Zimage => "zimage",
            ModelId::Klein => "klein",
        }
    }

    pub fn parse(s: &str) -> Option<ModelId> {
        [ModelId::Zimage, ModelId::Klein]
            .into_iter()
            .find(|m| m.id() == s)
    }

    pub fn info(self) -> &'static Model {
        match self {
            ModelId::Zimage => &MODELS[0],
            ModelId::Klein => &MODELS[1],
        }
    }
}

/// One of the two models, the same on every platform, both Apache-2.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Model {
    pub id: ModelId,
    pub label: &'static str,
    pub licence: &'static str,
    /// Whether it works from pictures: references, and the blank folder a whole-folder skin
    /// repaints.
    pub takes_pictures: bool,
    pub steps: u32,
    pub mlx_steps: u32,
}

/// The diffusion model, the text encoder and the VAE one model runs with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ModelFiles {
    pub diffusion: HfFile,
    pub llm: HfFile,
    pub vae: HfFile,
}

impl ModelFiles {
    pub fn all(&self) -> [HfFile; 3] {
        [self.diffusion, self.llm, self.vae]
    }
}

impl Model {
    pub fn files(&self, tier: Tier) -> ModelFiles {
        let llm = match tier {
            Tier::Q8 => QWEN3_4B_Q8,
            Tier::Q4 => QWEN3_4B_Q4,
        };
        match (self.id, tier) {
            (ModelId::Zimage, Tier::Q8) => ModelFiles {
                diffusion: ZIMAGE_Q8,
                llm,
                vae: ZIMAGE_VAE,
            },
            (ModelId::Zimage, Tier::Q4) => ModelFiles {
                diffusion: ZIMAGE_Q4,
                llm,
                vae: ZIMAGE_VAE,
            },
            (ModelId::Klein, Tier::Q8) => ModelFiles {
                diffusion: KLEIN_Q8,
                llm,
                vae: KLEIN_VAE,
            },
            (ModelId::Klein, Tier::Q4) => ModelFiles {
                diffusion: KLEIN_Q4,
                llm,
                vae: KLEIN_VAE,
            },
        }
    }
}

/// Z-Image-Turbo (6B, 8 steps), text to picture, the workhorse for artwork; and FLUX.2 [klein]
/// 4B (4 steps), which works from pictures.
pub const MODELS: [Model; 2] = [
    Model {
        id: ModelId::Zimage,
        label: "Z-Image-Turbo",
        licence: "Apache-2.0",
        takes_pictures: false,
        steps: 8,
        mlx_steps: 9,
    },
    Model {
        id: ModelId::Klein,
        label: "FLUX.2 [klein] 4B",
        licence: "Apache-2.0",
        takes_pictures: true,
        steps: 4,
        mlx_steps: 4,
    },
];

/// mflux, pinned like everything else: it shipped five releases in six weeks of 2026. It
/// downloads each model's weights itself the first time it runs it.
pub const MFLUX_VERSION: &str = "0.20.0";

/// mflux's name for a model, and the pre-quantised 4-bit copy it takes at `q4`. At `q8` mflux
/// quantises the upstream Apache weights as it loads them; at `q4` it takes these copies of the
/// same weights (the older filipstrand/ copy of Z-Image is tagged with another licence than its
/// upstream's, so not that one).
pub fn mlx_weights(model: ModelId) -> (&'static str, &'static str) {
    match model {
        ModelId::Zimage => ("mflux-community/z-image-turbo-mflux-q4", "z-image-turbo"),
        ModelId::Klein => ("mflux-community/flux2-klein-4b-mflux-q4", "flux2-klein-4b"),
    }
}

/// `packs make` saves a finished folder as WebP through cwebp, or as a PNG several times the
/// size, too big for a pack. Homebrew and Linux have a `webp` package; on Windows this is
/// Google's build.
pub const WEBP_WINDOWS_URL: &str = "https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-1.6.0-windows-x64.zip";
pub const WEBP_WINDOWS_SIZE: u64 = 4106264;
pub const WEBP_WINDOWS_SHA256: &str =
    "48886f506b21f62e4661f0f4cbfca19800897c385128e8902542d29a950c93f1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pin_is_complete() {
        for os in [Os::Windows, Os::Macos, Os::Linux] {
            for backend in Backend::ALL {
                let (assets, patterns) = (sdcpp_assets(os, backend), sdcpp_patterns(os, backend));
                assert_eq!(assets.is_some(), patterns.is_some(), "{os:?} {backend:?}");
                let (Some(assets), Some(patterns)) = (assets, patterns) else {
                    continue;
                };
                assert_eq!(assets.len(), patterns.len());
                for (asset, pattern) in assets.iter().zip(patterns) {
                    assert!(pattern.matches(asset.name), "{pattern:?} {}", asset.name);
                    assert_eq!(asset.sha256.len(), 64);
                    assert!(asset.url().ends_with(asset.name));
                }
            }
        }
        for model in MODELS {
            for tier in [Tier::Q8, Tier::Q4] {
                for f in model.files(tier).all() {
                    assert_eq!(f.sha256.len(), 64, "{}", f.path);
                    assert!(f.size > 100_000_000);
                    assert!(f.url().starts_with("https://huggingface.co/"));
                    assert!(f.url().contains(f.rev));
                }
            }
        }
    }

    #[test]
    fn the_download_sizes_match_what_setup_promises() {
        // SKILL.md tells people about 15.7 GB at q8 and 9.4 GB at q4.
        let total = |tier| {
            let mut files: Vec<HfFile> = MODELS.iter().flat_map(|m| m.files(tier).all()).collect();
            files.sort_by_key(|f| f.path);
            files.dedup();
            files.iter().map(|f| f.size).sum::<u64>() as f64 / 1e9
        };
        assert!((total(Tier::Q8) - 15.7).abs() < 0.1, "{}", total(Tier::Q8));
        assert!((total(Tier::Q4) - 9.4).abs() < 0.1, "{}", total(Tier::Q4));
    }

    #[test]
    fn a_newer_release_is_recognised_by_its_asset_names() {
        let cuda = sdcpp_patterns(Os::Windows, Backend::Cuda).unwrap();
        assert!(cuda[0].matches("sd-master-912-abcdef0-bin-win-cuda12-x64.zip"));
        assert!(!cuda[0].matches("sd-master-912-abcdef0-bin-win-vulkan-x64.zip"));
        let linux = sdcpp_patterns(Os::Linux, Backend::Cpu).unwrap();
        assert!(linux[0].matches("sd-master-912-abcdef0-bin-Linux-Ubuntu-24.04-x86_64.zip"));
        assert!(!linux[0].matches("sd-master-912-abcdef0-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip"));
        assert_eq!(ModelId::parse("klein"), Some(ModelId::Klein));
        assert_eq!(ModelId::Klein.info().steps, 4);
        assert_eq!(KLEIN_VAE.name(), "full_encoder_small_decoder.safetensors");
        assert_eq!(ZIMAGE_VAE.name(), "ae.safetensors");
    }
}
