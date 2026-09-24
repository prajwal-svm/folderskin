//! Everything generation downloads, pinned: a build, a revision, a size and a hash for every
//! file, so what was tested is what gets downloaded, and a run next month never needs GitHub's
//! API (whose anonymous limit is 60 calls an hour). `setup --runtime latest` moves the runtime on.

use crate::machine::{Arch, Backend, Os, Tier};
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

/// The release assets that make up each backend on `os` and `arch`, or `None` when there is no
/// build to download. The CUDA build needs its runtime DLLs beside it. The Linux builds are
/// x86_64 only; the macOS build is Apple Silicon only and wants macOS 26 (mflux, `--backend mlx`,
/// is the Mac's default). The Windows builds are x64, which Windows on Arm runs through its own
/// emulation.
pub fn sdcpp_assets(os: Os, arch: Arch, backend: Backend) -> Option<&'static [Asset]> {
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
    match (os, arch, backend) {
        (Os::Windows, _, Backend::Cuda) => Some(WINDOWS_CUDA),
        (Os::Windows, _, Backend::Vulkan) => Some(WINDOWS_VULKAN),
        (Os::Windows, _, Backend::Cpu) => Some(WINDOWS_CPU),
        (Os::Linux, Arch::X86_64, Backend::Vulkan) => Some(LINUX_VULKAN),
        (Os::Linux, Arch::X86_64, Backend::Cpu) => Some(LINUX_CPU),
        (Os::Macos, Arch::Arm64, Backend::Metal) => Some(MACOS_METAL),
        _ => None,
    }
}

/// The stable-diffusion.cpp backends there is a build of for `os` and `arch`.
pub fn sdcpp_backends(os: Os, arch: Arch) -> Vec<Backend> {
    Backend::ALL
        .into_iter()
        .filter(|b| sdcpp_assets(os, arch, *b).is_some())
        .collect()
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
pub fn sdcpp_patterns(os: Os, arch: Arch, backend: Backend) -> Option<&'static [AssetPattern]> {
    const WINDOWS_CUDA: &[AssetPattern] = &[
        pattern("", "", "bin-win-cuda12-x64.zip"),
        pattern("cudart-sd-bin-win-cu12-x64.zip", "", ""),
    ];
    const WINDOWS_VULKAN: &[AssetPattern] = &[pattern("", "", "bin-win-vulkan-x64.zip")];
    const WINDOWS_CPU: &[AssetPattern] = &[pattern("", "", "bin-win-cpu-x64.zip")];
    const LINUX_VULKAN: &[AssetPattern] = &[pattern("", "bin-Linux-", "-x86_64-vulkan.zip")];
    const LINUX_CPU: &[AssetPattern] = &[pattern("", "bin-Linux-", "-x86_64.zip")];
    const MACOS_METAL: &[AssetPattern] = &[pattern("", "bin-Darwin-", "-arm64.zip")];
    match (os, arch, backend) {
        (Os::Windows, _, Backend::Cuda) => Some(WINDOWS_CUDA),
        (Os::Windows, _, Backend::Vulkan) => Some(WINDOWS_VULKAN),
        (Os::Windows, _, Backend::Cpu) => Some(WINDOWS_CPU),
        (Os::Linux, Arch::X86_64, Backend::Vulkan) => Some(LINUX_VULKAN),
        (Os::Linux, Arch::X86_64, Backend::Cpu) => Some(LINUX_CPU),
        (Os::Macos, Arch::Arm64, Backend::Metal) => Some(MACOS_METAL),
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

/// The model that paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelId {
    Klein,
}

impl ModelId {
    pub fn id(self) -> &'static str {
        match self {
            ModelId::Klein => "klein",
        }
    }

    pub fn parse(s: &str) -> Option<ModelId> {
        [ModelId::Klein].into_iter().find(|m| m.id() == s)
    }

    pub fn info(self) -> &'static Model {
        match self {
            ModelId::Klein => &MODELS[0],
        }
    }
}

/// A model, the same on every platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Model {
    pub id: ModelId,
    pub label: &'static str,
    pub licence: &'static str,
    /// Whether it works from pictures: references, and the blank folder a whole-folder skin
    /// repaints.
    pub takes_pictures: bool,
    pub steps: u32,
}

/// The diffusion model, the text encoder and the VAE stable-diffusion.cpp runs a model with.
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

/// A file a model needs on this computer: where it is downloaded from, its size and hash, and
/// where it is kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModelFile {
    /// How it is named to people: the file's name, or for MLX weights its path in their folder,
    /// since their parts share file names (`transformer/0.safetensors`, `vae/0.safetensors`).
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: &'static str,
    pub local: PathBuf,
}

impl Model {
    /// The GGUF files stable-diffusion.cpp runs this model from at `tier`.
    pub fn files(&self, tier: Tier) -> ModelFiles {
        let llm = match tier {
            Tier::Q8 => QWEN3_4B_Q8,
            Tier::Q4 => QWEN3_4B_Q4,
        };
        match (self.id, tier) {
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

    /// The MLX weights mflux runs this model from at `tier`.
    pub fn mlx(&self, tier: Tier) -> &'static MlxWeights {
        match (self.id, tier) {
            (ModelId::Klein, Tier::Q8) => &KLEIN_MLX_Q8,
            (ModelId::Klein, Tier::Q4) => &KLEIN_MLX_Q4,
        }
    }

    /// Every file this model runs from with `backend` at `tier`: its MLX weights for mflux,
    /// otherwise the GGUF files for stable-diffusion.cpp.
    pub fn files_for(&self, backend: Backend, tier: Tier) -> Vec<ModelFile> {
        if backend == Backend::Mlx {
            let weights = self.mlx(tier);
            weights
                .files
                .iter()
                .map(|f| ModelFile {
                    name: f.path.to_string(),
                    url: weights.url(f),
                    size: f.size,
                    sha256: f.sha256,
                    local: weights.local(f),
                })
                .collect()
        } else {
            self.files(tier)
                .all()
                .iter()
                .map(|f| ModelFile {
                    name: f.name().to_string(),
                    url: f.url(),
                    size: f.size,
                    sha256: f.sha256,
                    local: f.local(),
                })
                .collect()
        }
    }
}

/// FLUX.2 [klein] 4B, Apache-2.0: four steps, and it paints from words alone or from pictures, so
/// one model does everything and there is one download: 4.6 GB on a Mac and 5.2 GB elsewhere at
/// q4. (Z-Image-Turbo was dropped: text to picture only, so a second download, and on a Mac 2.5
/// times slower for artwork klein paints as well.)
pub const MODELS: [Model; 1] = [Model {
    id: ModelId::Klein,
    label: "FLUX.2 [klein] 4B",
    licence: "Apache-2.0",
    takes_pictures: true,
    steps: 4,
}];

/// mflux, pinned like everything else: it shipped five releases in six weeks of 2026.
pub const MFLUX_VERSION: &str = "0.20.0";

/// One file of an MLX model, at `path` in its repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MlxFile {
    pub path: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// A model's weights for mflux: mflux-community's pre-quantised copy of the Apache-2.0 weights,
/// pinned to a revision and kept in the repository's own layout, which is how mflux loads a model
/// from a folder. Setup downloads them like every other model file, so they show their size and
/// their progress, carry on after a stop and are checked against their hashes, and painting never
/// touches the network. (Left to itself, mflux fetches weights the first time it paints, with
/// nothing to show for it, into Hugging Face's shared cache where FolderSkin can neither find nor
/// clear them; and asked for q8 it fetches the full-precision originals, 23.7 GB for klein, and
/// quantises them in memory on every run.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MlxWeights {
    pub repo: &'static str,
    pub rev: &'static str,
    /// The model mflux takes these weights to be (`--base-model`).
    pub base: &'static str,
    pub files: &'static [MlxFile],
}

impl MlxWeights {
    /// The folder they are kept in, named after their repository, so each tier has its own.
    pub fn dir(&self) -> PathBuf {
        crate::paths::models_dir()
            .join("mlx")
            .join(self.repo.rsplit('/').next().unwrap_or(self.repo))
    }

    pub fn local(&self, file: &MlxFile) -> PathBuf {
        file.path
            .split('/')
            .fold(self.dir(), |dir, part| dir.join(part))
    }

    pub fn url(&self, file: &MlxFile) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo, self.rev, file.path
        )
    }
}

const fn mlx(path: &'static str, size: u64, sha256: &'static str) -> MlxFile {
    MlxFile { path, size, sha256 }
}

/// klein at 4-bit for mflux: 4.62 GB.
pub const KLEIN_MLX_Q4: MlxWeights = MlxWeights {
    repo: "mflux-community/flux2-klein-4b-mflux-q4",
    rev: "77090341902cb5f9217f05c664ff604236ca18cc",
    base: "flux2-klein-4b",
    files: &[
        mlx(
            "text_encoder/0.safetensors",
            2135435237,
            "e59ed2b6825cb8ae3d0ba1e8a43575a565d465ce4518b7d79d28020b19b639cd",
        ),
        mlx(
            "text_encoder/1.safetensors",
            127582075,
            "edbdea3187bc4a0ad0e1ecb7152da8e6b344669413d9e45be5e7a2244f398ee3",
        ),
        mlx(
            "text_encoder/model.safetensors.index.json",
            51332,
            "7dc8598806b8bbdb0b87b9dc445ac7cf49d7acc42ae333bd0bc0252e46081ed9",
        ),
        mlx(
            "tokenizer/chat_template.jinja",
            4168,
            "a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8",
        ),
        mlx(
            "tokenizer/tokenizer.json",
            11422650,
            "be75606093db2094d7cd20f3c2f385c212750648bd6ea4fb2bf507a6a4c55506",
        ),
        mlx(
            "tokenizer/tokenizer_config.json",
            376,
            "779410a34a98fb2c6c925a1be6f24d632f1723473161b037f6676eb6c04886c6",
        ),
        mlx(
            "transformer/0.safetensors",
            2145323769,
            "36796395e3946496f3e118f26fe22594b1d7d871342a59dd7824349d91c08266",
        ),
        mlx(
            "transformer/1.safetensors",
            34727728,
            "9730ae148ca4f5ffc62860748b849a70987f0b7301588248bd98a2af9bc15070",
        ),
        mlx(
            "transformer/model.safetensors.index.json",
            26908,
            "dbdecbf3a957b37d15bd943b956dbb25aa648a01df363e7dd5ebbe5ce941358d",
        ),
        mlx(
            "vae/0.safetensors",
            165107888,
            "688ae0c3d2e93573ce8a6d3561d793cf6de5a4207b2e128cad6cd0d71e43b10f",
        ),
        mlx(
            "vae/model.safetensors.index.json",
            17547,
            "e13e551dabf8c088c8b3c6f52fca3f9e64ead5c42dd592586c7a1f868607afa4",
        ),
    ],
};

/// klein at 8-bit for mflux: 8.57 GB. The repository also keeps a second copy of every file under
/// a folder named " ", which isn't downloaded.
pub const KLEIN_MLX_Q8: MlxWeights = MlxWeights {
    repo: "mflux-community/flux2-klein-4b-mflux-q8",
    rev: "261787352bed056874d2b0ca01e57455a667b453",
    base: "flux2-klein-4b",
    files: &[
        mlx(
            "text_encoder/0.safetensors",
            2145931485,
            "c59037293e62b9142e06e602a03b734fdf7f0f6b80141759f875200bab5ff523",
        ),
        mlx(
            "text_encoder/1.safetensors",
            2128222096,
            "ccb1fd9fa0e19db71c44b3fdd9ed16e15dc14de3dcd240a0b162c4a10faba2cc",
        ),
        mlx(
            "text_encoder/model.safetensors.index.json",
            51332,
            "3273ab420ba8b5bfd991cc233b8d90afa794ffe856925299c892ee4c9d1614f2",
        ),
        mlx(
            "tokenizer/chat_template.jinja",
            4168,
            "a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8",
        ),
        mlx(
            "tokenizer/tokenizer.json",
            11422650,
            "be75606093db2094d7cd20f3c2f385c212750648bd6ea4fb2bf507a6a4c55506",
        ),
        mlx(
            "tokenizer/tokenizer_config.json",
            376,
            "779410a34a98fb2c6c925a1be6f24d632f1723473161b037f6676eb6c04886c6",
        ),
        mlx(
            "transformer/0.safetensors",
            2142058138,
            "442cb6770b834cb47043ef96f929b05b21e84bb1cca6daa806bf416aaa0b04f9",
        ),
        mlx(
            "transformer/1.safetensors",
            1975761716,
            "5cbf48740f8655185b3578a25188d2db72148ea2476385979961d89ee56d0dcc",
        ),
        mlx(
            "transformer/model.safetensors.index.json",
            26908,
            "fce32043337212c1fb02f5da46b0dde35cfecdb577d618e46bab6ce70392468f",
        ),
        mlx(
            "vae/0.safetensors",
            166156486,
            "fc2ff418c22e76c8ad93c43b4ffdee19d10a90898d3d10ef3c53dda9ad9cb4f2",
        ),
        mlx(
            "vae/model.safetensors.index.json",
            17547,
            "ae30aec3a3da440b700726376c89d1574cb4182e4ca534809243606df4d3be38",
        ),
    ],
};

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
            for arch in [Arch::X86_64, Arch::Arm64] {
                for backend in Backend::ALL {
                    let assets = sdcpp_assets(os, arch, backend);
                    let patterns = sdcpp_patterns(os, arch, backend);
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
        }
        for model in MODELS {
            for tier in [Tier::Q8, Tier::Q4] {
                for f in model.files(tier).all() {
                    assert_eq!(f.sha256.len(), 64, "{}", f.path);
                    assert!(f.size > 100_000_000);
                    assert!(f.url().starts_with("https://huggingface.co/"));
                    assert!(f.url().contains(f.rev));
                }
                let weights = model.mlx(tier);
                assert_eq!(weights.rev.len(), 40, "{}", weights.repo);
                let mut paths: Vec<&str> = weights.files.iter().map(|f| f.path).collect();
                paths.sort_unstable();
                paths.dedup();
                assert_eq!(paths.len(), weights.files.len(), "{}", weights.repo);
                for f in weights.files {
                    assert_eq!(f.sha256.len(), 64, "{}", f.path);
                    assert!(
                        f.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                        "{}",
                        f.path
                    );
                    assert!(f.size > 0, "{}", f.path);
                    // A relative path of plain parts: none empty, none with a space (the q8
                    // repository's " " copy), none that climbs out of the weights' folder.
                    assert!(
                        f.path
                            .split('/')
                            .all(|p| !p.is_empty() && p != ".." && !p.contains(' ')),
                        "{}",
                        f.path
                    );
                    assert!(weights.url(f).ends_with(f.path));
                    assert!(weights.url(f).contains(weights.rev));
                    assert!(weights.local(f).starts_with(weights.dir()));
                }
                // What mflux needs to load a model from a folder: each part's weights and index,
                // and the tokenizer.
                for part in ["text_encoder", "transformer", "vae"] {
                    let index = format!("{part}/model.safetensors.index.json");
                    assert!(paths.contains(&index.as_str()), "{} {index}", weights.repo);
                    assert!(
                        paths
                            .iter()
                            .any(|p| p.starts_with(part) && p.ends_with(".safetensors")),
                        "{} {part}",
                        weights.repo
                    );
                }
                assert!(paths.contains(&"tokenizer/tokenizer.json"));
            }
        }
        assert_ne!(
            KLEIN_MLX_Q4.dir(),
            KLEIN_MLX_Q8.dir(),
            "each tier has its own folder"
        );
    }

    #[test]
    fn the_download_sizes_match_what_setup_promises() {
        // The app, `ai setup --help` and SKILL.md tell people about these.
        let gb = |backend, tier| {
            MODELS
                .iter()
                .flat_map(|m| m.files_for(backend, tier))
                .map(|f| f.size)
                .sum::<u64>() as f64
                / 1e9
        };
        assert!(
            (gb(Backend::Mlx, Tier::Q4) - 4.6).abs() < 0.05,
            "{}",
            gb(Backend::Mlx, Tier::Q4)
        );
        assert!(
            (gb(Backend::Mlx, Tier::Q8) - 8.6).abs() < 0.05,
            "{}",
            gb(Backend::Mlx, Tier::Q8)
        );
        for backend in [Backend::Cuda, Backend::Vulkan, Backend::Metal, Backend::Cpu] {
            assert!(
                (gb(backend, Tier::Q4) - 5.2).abs() < 0.05,
                "{}",
                gb(backend, Tier::Q4)
            );
            assert!(
                (gb(backend, Tier::Q8) - 8.8).abs() < 0.05,
                "{}",
                gb(backend, Tier::Q8)
            );
        }
    }

    #[test]
    fn mflux_gets_its_weights_as_files_in_their_folders_and_sdcpp_by_name() {
        let klein = ModelId::Klein.info();
        let mlx = klein.files_for(Backend::Mlx, Tier::Q4);
        assert_eq!(mlx.len(), KLEIN_MLX_Q4.files.len());
        let transformer = mlx
            .iter()
            .find(|f| f.name == "transformer/0.safetensors")
            .unwrap();
        assert_eq!(
            transformer.local,
            crate::paths::models_dir()
                .join("mlx")
                .join("flux2-klein-4b-mflux-q4")
                .join("transformer")
                .join("0.safetensors")
        );
        assert_eq!(
            transformer.url,
            "https://huggingface.co/mflux-community/flux2-klein-4b-mflux-q4/resolve/\
             77090341902cb5f9217f05c664ff604236ca18cc/transformer/0.safetensors"
        );
        // Three parts share the name "0.safetensors": each is named by its path.
        let names: std::collections::HashSet<&str> = mlx.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names.len(), mlx.len());

        let gguf = klein.files_for(Backend::Vulkan, Tier::Q4);
        let names: Vec<&str> = gguf.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "flux-2-klein-4b-Q4_0.gguf",
                "Qwen3-4B-Q4_K_M.gguf",
                "full_encoder_small_decoder.safetensors"
            ]
        );
        assert!(gguf
            .iter()
            .all(|f| f.local.parent() == Some(&crate::paths::models_dir())));
    }

    #[test]
    fn a_newer_release_is_recognised_by_its_asset_names() {
        let cuda = sdcpp_patterns(Os::Windows, Arch::X86_64, Backend::Cuda).unwrap();
        assert!(cuda[0].matches("sd-master-912-abcdef0-bin-win-cuda12-x64.zip"));
        assert!(!cuda[0].matches("sd-master-912-abcdef0-bin-win-vulkan-x64.zip"));
        let linux = sdcpp_patterns(Os::Linux, Arch::X86_64, Backend::Cpu).unwrap();
        assert!(linux[0].matches("sd-master-912-abcdef0-bin-Linux-Ubuntu-24.04-x86_64.zip"));
        assert!(!linux[0].matches("sd-master-912-abcdef0-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip"));
        assert_eq!(ModelId::parse("klein"), Some(ModelId::Klein));
        assert_eq!(ModelId::Klein.info().steps, 4);
        assert_eq!(KLEIN_VAE.name(), "full_encoder_small_decoder.safetensors");
        assert_eq!(ModelId::parse("zimage"), None);
    }

    #[test]
    fn a_build_is_only_offered_for_the_processor_it_was_made_for() {
        // Every Linux build is x86_64: an ARM64 Linux computer gets nothing to install, rather
        // than an x86_64 program that can't start.
        assert_eq!(sdcpp_backends(Os::Linux, Arch::Arm64), []);
        assert_eq!(
            sdcpp_patterns(Os::Linux, Arch::Arm64, Backend::Vulkan),
            None
        );
        assert_eq!(
            sdcpp_backends(Os::Linux, Arch::X86_64),
            [Backend::Vulkan, Backend::Cpu]
        );
        // The Mac build is for Apple Silicon only.
        assert_eq!(sdcpp_backends(Os::Macos, Arch::X86_64), []);
        assert_eq!(sdcpp_backends(Os::Macos, Arch::Arm64), [Backend::Metal]);
        // Windows on Arm runs the x64 builds.
        assert_eq!(
            sdcpp_backends(Os::Windows, Arch::Arm64),
            [Backend::Cuda, Backend::Vulkan, Backend::Cpu]
        );
    }
}
