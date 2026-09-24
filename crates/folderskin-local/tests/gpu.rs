//! Real pictures painted on this computer, with the runtime and models `folderskin ai setup`
//! installed. They take from half a minute to a few minutes each and need a GPU, so they run only
//! when `FOLDERSKIN_GPU_TESTS=1`:
//!
//! ```text
//! FOLDERSKIN_GPU_TESTS=1 cargo test -p folderskin-local --test gpu -- --test-threads=1
//! ```

use folderskin_local::{
    detect, generate, CancelToken, Event, Job, ModelId, Reporter, Settings, Shape,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn wanted() -> bool {
    std::env::var("FOLDERSKIN_GPU_TESTS").is_ok_and(|v| v == "1")
}

fn out_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-gpu-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn collecting() -> (Reporter, Arc<Mutex<Vec<Event>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    (Reporter::new(move |e| sink.lock().unwrap().push(e)), seen)
}

#[test]
fn klein_paints_artwork_with_progress_and_provenance() {
    if !wanted() {
        return;
    }
    let settings = Settings::for_machine(&detect());
    let dir = out_dir("artwork");
    let mut job = Job::new("a lighthouse on a rock at dusk");
    job.style = "pop-art".into();
    job.model = Some(ModelId::Klein);
    job.seed = 42;
    let (reporter, seen) = collecting();
    let picture = runtime()
        .block_on(generate(
            &job,
            &settings,
            &dir,
            &reporter,
            &CancelToken::new(),
        ))
        .unwrap();
    let img = image::open(&picture.path).unwrap();
    assert_eq!((img.width(), img.height()), (1024, 960));
    assert!(!folderskin_core::painted::is_blank(&img.to_rgba8()));
    assert_eq!(picture.provenance.steps, 4);
    assert!(picture.path.with_extension("json").is_file());
    let seen = seen.lock().unwrap();
    assert!(
        seen.contains(&Event::Progress { step: 4, steps: 4 }),
        "every step is reported"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn klein_repaints_the_blank_folder_and_it_is_cut_out() {
    if !wanted() {
        return;
    }
    let settings = Settings::for_machine(&detect());
    let dir = out_dir("folder");
    let mut job = Job::new("a koi pond at night with paper lanterns");
    job.style = "woodblock".into();
    job.shape = Shape::Folder;
    job.seed = 7;
    let picture = runtime()
        .block_on(generate(
            &job,
            &settings,
            &dir,
            &Reporter::silent(),
            &CancelToken::new(),
        ))
        .unwrap();
    let fit = picture.provenance.silhouette_fit.unwrap();
    if fit >= folderskin_core::painted::MIN_SILHOUETTE_FIT {
        let img = image::open(&picture.path).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "cut out: {fit}");
        assert!(dir
            .join("raw")
            .join(picture.path.file_name().unwrap())
            .is_file());
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_cancelled_painting_ends_the_runtime() {
    if !wanted() {
        return;
    }
    let settings = Settings::for_machine(&detect());
    let dir = out_dir("cancel");
    let mut job = Job::new("a bicycle");
    job.model = Some(ModelId::Klein);
    let cancel = CancelToken::new();
    let later = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        later.cancel();
    });
    let started = std::time::Instant::now();
    let err = runtime()
        .block_on(generate(
            &job,
            &settings,
            &dir,
            &Reporter::silent(),
            &cancel,
        ))
        .unwrap_err();
    assert!(err.is_cancelled(), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(20));
    let _ = std::fs::remove_dir_all(&dir);
}
