//! Removing the model files an earlier setup left that the model doesn't use now, in a home of its
//! own. A test binary of its own because it sets FOLDERSKIN_LOCALGEN_HOME, which every other test
//! in a process would then share.

use folderskin_local::{
    kept_bytes, remove_unused, unused, unused_bytes, Backend, Reporter, Settings, Tier,
};
use std::path::Path;

fn write(path: &Path, bytes: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![0u8; bytes]).unwrap();
}

#[test]
fn what_an_earlier_setup_left_goes_and_the_model_stays() {
    let home = std::env::temp_dir().join(format!("fs-unused-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    // SAFETY: set before anything else in this binary reads the environment; one test, one thread.
    unsafe { std::env::set_var("FOLDERSKIN_LOCALGEN_HOME", &home) };
    let models = home.join("models");
    // An earlier build's klein at 8-bit and its Z-Image Turbo, and today's klein at 4-bit.
    write(&models.join("flux-2-klein-4b-Q8_0.gguf"), 4000);
    write(&models.join("Qwen3-4B-Q8_0.gguf"), 3000);
    write(&models.join("z_image_turbo-Q8_0.gguf"), 6000);
    write(&models.join("flux-2-klein-4b-Q4_0.gguf"), 2000);
    write(&models.join("flux-2-klein-4b-Q4_0.gguf.ok"), 64);
    write(&models.join("Qwen3-4B-Q4_K_M.gguf"), 1000);
    write(&models.join("full_encoder_small_decoder.safetensors"), 200);
    write(&home.join("bin/cuda/sd-cli.exe"), 300);

    let settings = Settings {
        backend: Backend::Cuda,
        tier: Tier::Q4,
        vram_gb: 4.0,
    };
    let names: Vec<String> = unused(&settings).into_iter().map(|u| u.name).collect();
    assert_eq!(
        names,
        [
            "z_image_turbo-Q8_0.gguf",
            "flux-2-klein-4b-Q8_0.gguf",
            "Qwen3-4B-Q8_0.gguf"
        ]
    );
    assert_eq!(unused_bytes(&settings), 13_000);
    assert_eq!(
        remove_unused(&settings, &Reporter::silent()).unwrap(),
        13_000
    );
    assert_eq!(unused_bytes(&settings), 0);
    // The model it runs with, and the runtime, are all still here.
    assert_eq!(kept_bytes(), 2000 + 64 + 1000 + 200 + 300);
    assert!(models.join("flux-2-klein-4b-Q4_0.gguf").is_file());
    // Nothing left over is nothing to remove.
    assert_eq!(remove_unused(&settings, &Reporter::silent()).unwrap(), 0);
    std::fs::remove_dir_all(&home).unwrap();
}
