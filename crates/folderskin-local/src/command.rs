//! The command lines that run the models, exactly as the local-generation skill ran them.

use crate::machine::Backend;
use crate::manifest::{MlxWeights, Model, ModelFiles, ModelId};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Artwork is 1024 x 958 (`compositor::SKIN_WIDTH`/`SKIN_HEIGHT`); models want sides divisible by
/// 16, and the compositor cover-fits any size, so 1024 x 960 loses nothing. A whole folder is
/// painted in the same frame.
pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 960;

/// Below this much VRAM, a reference picture is encoded at half size (see [`sdcpp`]).
const SMALL_VRAM_GB: f64 = 6.0;

/// The arguments for stable-diffusion.cpp's `sd-cli`, after the program itself. The model files
/// are named in a form sd-cli can open ([`crate::paths::for_sdcpp`]); `pictures` and `out` are
/// passed as they are given.
#[allow(clippy::too_many_arguments)]
pub fn sdcpp(
    files: &ModelFiles,
    model: &Model,
    prompt: &str,
    seed: u64,
    pictures: &[PathBuf],
    out: &Path,
    backend: Backend,
    vram_gb: f64,
) -> Vec<OsString> {
    let local = |f: &crate::manifest::HfFile| {
        let path = f.local();
        crate::paths::for_sdcpp(&path).unwrap_or(path)
    };
    let mut args: Vec<OsString> = vec!["--diffusion-model".into()];
    args.push(local(&files.diffusion).into());
    args.push("--llm".into());
    args.push(local(&files.llm).into());
    args.push("--vae".into());
    args.push(local(&files.vae).into());
    for a in [
        "-p",
        prompt,
        "--cfg-scale",
        "1.0",
        "--steps",
        &model.steps.to_string(),
        "--sampling-method",
        "euler",
        "-W",
        &WIDTH.to_string(),
        "-H",
        &HEIGHT.to_string(),
        "-s",
        &seed.to_string(),
        // The prompt stays out of the PNG: the .json beside it keeps it, and a picture shared on
        // its own shouldn't carry it.
        "--disable-image-metadata",
        // Flash attention on every backend. sd.cpp's docs say it slows the non-CUDA ones, but
        // without it klein at 1024 x 960 ran out of memory on a 4 GB card under Vulkan.
        "--diffusion-fa",
        "-o",
    ] {
        args.push(a.into());
    }
    args.push(out.into());
    for p in pictures {
        args.push("-r".into());
        args.push(p.into());
    }
    if !pictures.is_empty() && vram_gb > 0.0 && vram_gb < SMALL_VRAM_GB {
        // Encoding a 1024 px reference wants ~3.8 GB of VRAM, more than a 4 GB card has free. At
        // 512 px it takes a second on the GPU (and it is still a 1024 px picture that comes out);
        // the other way out, the VAE on the CPU, costs a minute a picture.
        args.push("--ref-image-args".into());
        args.push("vae_input_max_pixels=262144".into());
    }
    if backend == Backend::Cpu {
        args.push("--backend".into());
        args.push("cpu".into());
    }
    args
}

/// The size FolderSkin's blank folder is handed to the model at when a whole folder is
/// repainted; what comes out is the full [`WIDTH`] x [`HEIGHT`] frame either way. mflux works from
/// a reference at the reference's own size, so a half-size folder is a quarter of the reference's
/// tokens: on an M3 Pro a whole folder took 67 s instead of 139 s, and fitted FolderSkin's
/// silhouette as closely (0.989 against 0.987). stable-diffusion.cpp keeps the full frame, since
/// how it scales a reference is its own (see `--ref-image-args` in [`sdcpp`]).
pub fn template_size(backend: Backend) -> (u32, u32) {
    if backend == Backend::Mlx {
        (WIDTH / 2, HEIGHT / 2)
    } else {
        (WIDTH, HEIGHT)
    }
}

/// mflux's program for `model` and its arguments: the weights from their folder, never from the
/// network, and the pictures to work from, if there are any.
pub fn mflux(
    model: &Model,
    weights: &MlxWeights,
    prompt: &str,
    seed: u64,
    pictures: &[PathBuf],
    out: &Path,
) -> (&'static str, Vec<OsString>) {
    let mut args: Vec<OsString> = vec![
        "--model".into(),
        weights.dir().into(),
        "--base-model".into(),
        weights.base.into(),
    ];
    let program = match model.id {
        ModelId::Klein if pictures.is_empty() => "mflux-generate-flux2",
        ModelId::Klein => {
            args.push("--image-paths".into());
            args.extend(pictures.iter().map(OsString::from));
            "mflux-generate-flux2-edit"
        }
    };
    for a in [
        "--prompt",
        prompt,
        "--width",
        &WIDTH.to_string(),
        "--height",
        &HEIGHT.to_string(),
        "--seed",
        &seed.to_string(),
        "--steps",
        &model.steps.to_string(),
        // As with sd-cli's --disable-image-metadata: mflux otherwise writes the prompt into the
        // PNG (EXIF, IPTC and XMP), and a picture shared on its own shouldn't carry it.
        "--no-metadata",
        "--output",
    ] {
        args.push(a.into());
    }
    args.push(out.into());
    (program, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Tier;
    use crate::manifest::ModelId;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn stable_diffusion_cpp_gets_the_tested_flags() {
        let klein = ModelId::Klein.info();
        let files = klein.files(Tier::Q8);
        let args = strings(&sdcpp(
            &files,
            klein,
            "a lighthouse",
            42,
            &[],
            Path::new("out/a.png"),
            Backend::Cuda,
            4.0,
        ));
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].as_str())
        };
        assert!(args[1].ends_with("flux-2-klein-4b-Q8_0.gguf"), "{args:?}");
        assert!(at("--llm").unwrap().ends_with("Qwen3-4B-Q8_0.gguf"));
        assert!(at("--vae")
            .unwrap()
            .ends_with("full_encoder_small_decoder.safetensors"));
        assert_eq!(at("-p"), Some("a lighthouse"));
        assert_eq!(at("--cfg-scale"), Some("1.0"));
        assert_eq!(at("--steps"), Some("4"));
        assert_eq!(at("--sampling-method"), Some("euler"));
        assert_eq!((at("-W"), at("-H")), (Some("1024"), Some("960")));
        assert_eq!(at("-s"), Some("42"));
        assert!(args.contains(&"--disable-image-metadata".to_string()));
        assert!(args.contains(&"--diffusion-fa".to_string()));
        assert_eq!(at("-o"), Some("out/a.png"));
        assert!(!args.contains(&"-r".to_string()));
        assert!(
            !args.contains(&"--ref-image-args".to_string()),
            "no pictures, no limit"
        );
        assert!(!args.contains(&"--backend".to_string()));
    }

    #[test]
    fn references_on_a_small_card_are_encoded_at_half_size() {
        let klein = ModelId::Klein.info();
        let files = klein.files(Tier::Q4);
        let pictures = [PathBuf::from("t.png"), PathBuf::from("dog.jpg")];
        let run = |vram, backend| {
            strings(&sdcpp(
                &files,
                klein,
                "p",
                1,
                &pictures,
                Path::new("o.png"),
                backend,
                vram,
            ))
        };
        let small = run(4.0, Backend::Cuda);
        let refs: Vec<&String> = small
            .iter()
            .zip(small.iter().skip(1))
            .filter(|(flag, _)| *flag == "-r")
            .map(|(_, v)| v)
            .collect();
        assert_eq!(refs, ["t.png", "dog.jpg"], "the template first, in order");
        assert!(small.ends_with(&[
            "--ref-image-args".into(),
            "vae_input_max_pixels=262144".into()
        ]));
        assert!(!run(8.0, Backend::Cuda).contains(&"--ref-image-args".to_string()));
        // Unknown VRAM (Vulkan on another GPU): --auto-fit decides.
        assert!(!run(0.0, Backend::Vulkan).contains(&"--ref-image-args".to_string()));
        assert!(run(0.0, Backend::Cpu).ends_with(&["--backend".into(), "cpu".into()]));
    }

    #[test]
    fn mflux_paints_from_the_weights_folder_setup_filled() {
        let klein = ModelId::Klein.info();
        let weights = klein.mlx(Tier::Q4);
        let (program, args) = mflux(klein, weights, "p", 7, &[], Path::new("o.png"));
        assert_eq!(program, "mflux-generate-flux2");
        let args = strings(&args);
        assert_eq!(
            &args[..4],
            [
                "--model",
                &weights.dir().to_string_lossy(),
                "--base-model",
                "flux2-klein-4b"
            ]
        );
        assert!(
            !args.iter().any(|a| a == "-q" || a == "--quantize"),
            "{args:?}"
        );
        assert!(args.windows(2).any(|w| w == ["--steps", "4"]));
        assert!(args.windows(2).any(|w| w == ["--width", "1024"]));
        assert!(args.windows(2).any(|w| w == ["--height", "960"]));
        assert!(args.contains(&"--no-metadata".to_string()));
        assert!(!args.contains(&"--image-paths".to_string()));
        assert!(args.ends_with(&["--output".into(), "o.png".into()]));

        let q8 = klein.mlx(Tier::Q8);
        let (program, args) = mflux(
            klein,
            q8,
            "p",
            7,
            &[PathBuf::from("t.png"), PathBuf::from("dog.jpg")],
            Path::new("o.png"),
        );
        assert_eq!(program, "mflux-generate-flux2-edit");
        let args = strings(&args);
        assert_eq!(args[1], q8.dir().to_string_lossy());
        let at = args.iter().position(|a| a == "--image-paths").unwrap();
        assert_eq!(
            &args[at + 1..at + 3],
            ["t.png", "dog.jpg"],
            "the template first"
        );
    }

    #[test]
    fn mlx_repaints_a_half_size_folder_and_sdcpp_the_full_frame() {
        assert_eq!(template_size(Backend::Mlx), (512, 480));
        for b in [Backend::Cuda, Backend::Vulkan, Backend::Metal, Backend::Cpu] {
            assert_eq!(template_size(b), (WIDTH, HEIGHT));
        }
        // Sides the model takes: multiples of 16.
        let (w, h) = template_size(Backend::Mlx);
        assert_eq!((w % 16, h % 16), (0, 0));
    }
}
