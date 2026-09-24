//! Removing what setup downloaded, in a home of its own. A test binary of its own because it sets
//! FOLDERSKIN_LOCALGEN_HOME, which every other test in a process would then share.

use folderskin_local::{kept_bytes, remove, Reporter};
use std::path::Path;

fn write(path: &Path, bytes: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![0u8; bytes]).unwrap();
}

#[test]
fn removing_takes_back_setups_files_and_nothing_else() {
    let home = std::env::temp_dir().join(format!("fs-remove-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    // SAFETY: set before anything else in this binary reads the environment; one test, one thread.
    unsafe { std::env::set_var("FOLDERSKIN_LOCALGEN_HOME", &home) };
    assert_eq!(folderskin_local::home(), home);

    let weights = home.join("models/mlx/flux2-klein-4b-mflux-q4");
    write(&weights.join("transformer/0.safetensors"), 4000);
    write(&weights.join("text_encoder/0.safetensors.part"), 1000); // a download stopped part-way
    write(&home.join("models/flux-2-klein-4b-Q4_0.gguf"), 2000);
    write(&home.join("downloads/sd-bin.zip"), 500);
    write(&home.join("bin/cuda/sd-cli"), 300);
    write(&home.join("app/painting-1/picture.png"), 200);
    // What isn't setup's stays: a note someone kept here, and how long a picture took.
    write(&home.join("notes.txt"), 7);
    write(&home.join("app-timing.json"), 40);

    assert_eq!(kept_bytes(), 8000);
    let freed = remove(&Reporter::silent()).unwrap();
    assert_eq!(freed, 8000);
    for gone in ["models", "downloads", "bin", "app"] {
        assert!(!home.join(gone).exists(), "{gone} is still there");
    }
    assert!(home.join("notes.txt").is_file());
    assert!(home.join("app-timing.json").is_file());
    assert_eq!(kept_bytes(), 0);
    // Nothing left to remove is not an error.
    assert_eq!(remove(&Reporter::silent()).unwrap(), 0);

    std::fs::remove_dir_all(&home).unwrap();
}
