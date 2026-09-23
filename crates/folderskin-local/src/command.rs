//! The command lines that run the models, exactly as the local-generation skill ran them.

use crate::machine::{Backend, Tier};
use crate::manifest::{mlx_weights, Model, ModelFiles, ModelId};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Artwork is 1024 x 958 (`compositor::SKIN_WIDTH`/`SKIN_HEIGHT`); models want sides divisible by
/// 16, and the compositor cover-fits any size, so 1024 x 960 loses nothing. A whole folder is
/// painted in the same frame.
pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 960;

/// Below this much VRAM, a reference picture is encoded at half size (see [`sdcpp`]).
const SMALL_VRAM_GB: f64 = 6.0;

/// The arguments for stable-diffusion.cpp's `sd-cli`, after the program itself.
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
    let mut args: Vec<OsString> = vec!["--diffusion-model".into()];
    args.push(files.diffusion.local().into());
    args.push("--llm".into());
    args.push(files.llm.local().into());
    args.push("--vae".into());
    args.push(files.vae.local().into());
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

/// mflux's program for `model` and its arguments.
pub fn mflux(
    model: &Model,
    tier: Tier,
    prompt: &str,
    seed: u64,
    pictures: &[PathBuf],
    out: &Path,
) -> (&'static str, Vec<OsString>) {
    let (repo, base) = mlx_weights(model.id);
    let mut args: Vec<OsString> = match tier {
        Tier::Q8 => vec!["--model".into(), base.into(), "-q".into(), "8".into()],
        Tier::Q4 => vec![
            "--model".into(),
            repo.into(),
            "--base-model".into(),
            base.into(),
        ],
    };
    let program = match model.id {
        ModelId::Zimage => "mflux-generate-z-image-turbo",
        ModelId::Klein if !pictures.is_empty() => {
            args.push("--image-paths".into());
            args.extend(pictures.iter().map(OsString::from));
            "mflux-generate-flux2-edit"
        }
        ModelId::Klein => "mflux-generate-flux2",
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
        &model.mlx_steps.to_string(),
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
    fn mflux_takes_upstream_weights_at_q8_and_the_community_copy_at_q4() {
        let zimage = ModelId::Zimage.info();
        let (program, args) = mflux(zimage, Tier::Q8, "p", 7, &[], Path::new("o.png"));
        assert_eq!(program, "mflux-generate-z-image-turbo");
        let args = strings(&args);
        assert_eq!(&args[..4], ["--model", "z-image-turbo", "-q", "8"]);
        assert!(args.windows(2).any(|w| w == ["--steps", "9"]));

        let klein = ModelId::Klein.info();
        let (program, args) = mflux(klein, Tier::Q4, "p", 7, &[], Path::new("o.png"));
        assert_eq!(program, "mflux-generate-flux2");
        assert_eq!(
            &strings(&args)[..4],
            [
                "--model",
                "mflux-community/flux2-klein-4b-mflux-q4",
                "--base-model",
                "flux2-klein-4b"
            ]
        );
        let (program, args) = mflux(
            klein,
            Tier::Q8,
            "p",
            7,
            &[PathBuf::from("a.png")],
            Path::new("o.png"),
        );
        assert_eq!(program, "mflux-generate-flux2-edit");
        assert!(strings(&args)
            .windows(2)
            .any(|w| w == ["--image-paths", "a.png"]));
        assert!(strings(&args).ends_with(&["--output".into(), "o.png".into()]));
    }
}
