//! `folderskin ai remove --unused` as a person reads it, run as the command it is in a home of
//! its own: the model is what doesn't use the files, as `ai doctor` and the app say.

use std::path::Path;
use std::process::Command;

fn write(path: &Path, bytes: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![0u8; bytes]).unwrap();
}

/// What `ai remove --unused` says, with `more` after it, for the model on CUDA at 4-bit.
fn remove_unused(home: &Path, more: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_folderskin-cli"))
        .env("FOLDERSKIN_LOCALGEN_HOME", home)
        .args([
            "ai",
            "remove",
            "--unused",
            "--backend",
            "cuda",
            "--tier",
            "q4",
        ])
        .args(more)
        .output()
        .unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{said}");
    said
}

#[test]
fn the_files_are_the_ones_the_model_doesnt_use() {
    let home = std::env::temp_dir().join(format!("fs-cli-unused-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let models = home.join("models");
    // An earlier build's klein at 8-bit, beside today's at 4-bit.
    write(&models.join("flux-2-klein-4b-Q8_0.gguf"), 4000);
    write(&models.join("Qwen3-4B-Q8_0.gguf"), 3000);
    write(&models.join("flux-2-klein-4b-Q4_0.gguf"), 2000);

    let dry = remove_unused(&home, &["--dry-run"]);
    assert!(
        dry.contains(
            "dry run: 1 MB of model files the model doesn't use (cuda, q4); nothing is deleted"
        ),
        "{dry}"
    );
    let done = remove_unused(&home, &[]);
    assert!(
        done.contains("Deleted 2 model files the model doesn't use (cuda, q4): 1 MB back."),
        "{done}"
    );
    let again = remove_unused(&home, &[]);
    assert!(
        again.contains("Nothing to remove: the model uses every model file here (cuda, q4)."),
        "{again}"
    );
    assert!(models.join("flux-2-klein-4b-Q4_0.gguf").is_file());
    std::fs::remove_dir_all(&home).unwrap();
}
