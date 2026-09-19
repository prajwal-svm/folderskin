//! `packs make`: a pack from a folder of pictures, such as renders saved from an image model.
//!
//! Each picture gets the split the app makes when you add one: a finished folder, painted on the
//! magenta key or on real transparency, is cut out and becomes the icon itself, and any other
//! picture is artwork for FolderSkin's folder. Then it is shrunk to the pack limit and compressed
//! until it fits the size budget: a folder as WebP when `cwebp` is installed (PNG otherwise, which
//! keeps the transparency but is several times bigger), artwork as JPEG. The folder that comes out
//! passes `packs check`, which runs on it before anything is reported.

use crate::packs;
use folderskin_core::pack::{
    self, Pack, PackSkin, MANIFEST_FILE, MAX_PACK_TAGS, MAX_PICTURE_SIDE, MAX_SKINS,
    MAX_SKIN_NAME_CHARS, MIN_PICTURE_SIDE, PACK_VERSION, PICTURE_EXTENSIONS,
};
use folderskin_core::{matte, raster};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use std::path::{Path, PathBuf};

/// Everything `make` needs besides the pictures.
#[derive(Debug, Clone)]
pub struct MakeOptions {
    /// The pack's id, which is also its folder's name.
    pub id: String,
    pub name: String,
    /// Tags every skin gets; the first names the pack.
    pub tags: Vec<String>,
    /// The GitHub name of whoever made the pictures.
    pub author: String,
    pub license: String,
    /// The folder holding `packs/`: `assets` for a built-in pack, `community` for a shared one.
    pub dir: PathBuf,
    /// The largest a picture may be once compressed.
    pub max_bytes: usize,
    /// The `cwebp` program, when there is one. Without it, folders are saved as PNG.
    pub cwebp: Option<PathBuf>,
}

/// One picture as it went into the pack.
#[derive(Debug, Clone)]
pub struct Made {
    pub source: PathBuf,
    /// Its file in the pack.
    pub file: String,
    pub name: String,
    /// True for a finished folder, false for artwork on FolderSkin's folder.
    pub folder: bool,
    /// Its size in the pack.
    pub bytes: usize,
}

/// Makes `<dir>/packs/<id>` from `pictures` (files, or folders whose pictures are taken in name
/// order) and returns the folder it wrote with what went into it. Nothing is left behind when a
/// picture can't be used.
pub fn make(pictures: &[PathBuf], opts: &MakeOptions) -> Result<(PathBuf, Vec<Made>), String> {
    if !pack::is_pack_id(&opts.id) {
        let suggestion = pack::slug(&opts.id);
        return Err(if pack::is_pack_id(&suggestion) {
            format!("\"{}\" can't be a pack id; try {suggestion}", opts.id)
        } else {
            format!(
                "\"{}\" can't be a pack id: use lower-case words joined by dashes",
                opts.id
            )
        });
    }
    let folder = opts.dir.join(packs::PACKS_DIR).join(&opts.id);
    if folder.exists() {
        return Err(format!(
            "{} exists already; pick another id or remove it",
            folder.display()
        ));
    }
    let sources = collect(pictures)?;
    if sources.len() > MAX_SKINS {
        return Err(format!(
            "that's {} pictures; a pack holds at most {MAX_SKINS}",
            sources.len()
        ));
    }

    let mut made = Vec::with_capacity(sources.len());
    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(sources.len());
    for (n, source) in sources.iter().enumerate() {
        let label = source.display();
        let (bytes, ext, folder_kind) =
            prepare(source, opts).map_err(|e| format!("{label} {e}"))?;
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = format!("{}.{ext}", unique_stem(&stem, n, &files));
        made.push(Made {
            source: source.clone(),
            file: file.clone(),
            name: display_name(&stem, n),
            folder: folder_kind,
            bytes: bytes.len(),
        });
        files.push((file, bytes));
    }

    let manifest = Pack {
        version: PACK_VERSION,
        name: opts.name.trim().to_string(),
        author: opts.author.trim().to_string(),
        license: opts.license.trim().to_string(),
        tags: pack::clean_tags(&opts.tags, MAX_PACK_TAGS),
        skins: made
            .iter()
            .map(|m| PackSkin {
                file: m.file.clone(),
                name: m.name.clone(),
                tags: Vec::new(),
            })
            .collect(),
    };
    let problems = manifest.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n";

    let written = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&folder)?;
        for (file, bytes) in &files {
            std::fs::write(folder.join(file), bytes)?;
        }
        std::fs::write(folder.join(MANIFEST_FILE), json)
    })();
    let checked = written
        .map_err(|e| format!("couldn't write {}: {e}", folder.display()))
        .and_then(|()| {
            packs::check_pack_within(&folder, &opts.id, opts.max_bytes)
                .map_err(|problems| problems.join("; "))
        });
    if let Err(e) = checked {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(e);
    }
    Ok((folder, made))
}

/// `cwebp` on the PATH, if it is installed (Homebrew's `webp`, Debian's `webp` package).
pub fn find_cwebp() -> Option<PathBuf> {
    let program = if cfg!(windows) { "cwebp.exe" } else { "cwebp" };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(program))
        .find(|p| p.is_file())
}

/// Every picture to use: files as given, and the pictures inside folders in name order.
fn collect(pictures: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for path in pictures {
        if path.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(path)
                .map_err(|e| format!("couldn't read {}: {e}", path.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_picture(p))
                .collect();
            found.sort();
            out.extend(found);
        } else if path.is_file() {
            out.push(path.clone());
        } else {
            return Err(format!("{} doesn't exist", path.display()));
        }
    }
    if out.is_empty() {
        return Err("there are no PNG, JPEG or WebP pictures there".into());
    }
    Ok(out)
}

fn is_picture(path: &Path) -> bool {
    let hidden = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'));
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    !hidden && PICTURE_EXTENSIONS.contains(&ext.as_str())
}

/// One picture, ready for the pack: its bytes, its extension, and whether it is a finished
/// folder. The error finishes a sentence that starts with the picture's path.
fn prepare(path: &Path, opts: &MakeOptions) -> Result<(Vec<u8>, &'static str, bool), String> {
    let rgba = image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("couldn't be read: {e}"))?
        .decode()
        .map_err(|e| format!("isn't a picture FolderSkin can read: {e}"))?
        .to_rgba8();
    if matte::alpha_bounds(&rgba, 8).is_none() {
        return Err("is completely transparent".into());
    }
    match matte::finished_cutout(&rgba, matte::MAGENTA) {
        Some(cut) => {
            let (bytes, ext) = encode_folder(&cut, opts)?;
            Ok((bytes, ext, true))
        }
        None => {
            let img = shrink(rgba, MAX_PICTURE_SIDE);
            big_enough(&img, "")?;
            Ok((encode_jpeg(&img, opts.max_bytes)?, "jpg", false))
        }
    }
}

/// A finished folder at the largest size and best quality that fit `opts.max_bytes`.
fn encode_folder(cut: &RgbaImage, opts: &MakeOptions) -> Result<(Vec<u8>, &'static str), String> {
    let img = shrink(cut.clone(), MAX_PICTURE_SIDE);
    big_enough(&img, " once cut out")?;
    let limit = opts.max_bytes / 1024;
    if let Some(cwebp) = &opts.cwebp {
        for quality in [90, 85, 80, 75, 70, 60] {
            let webp = run_cwebp(cwebp, &img, quality)?;
            if webp.len() <= opts.max_bytes {
                return Ok((webp, "webp"));
            }
        }
        return Err(format!("is still over {limit} KB as a WebP at quality 60"));
    }
    for side in [MAX_PICTURE_SIDE, 896, 768] {
        let smaller = shrink(img.clone(), side);
        if big_enough(&smaller, "").is_err() {
            break;
        }
        let png = raster::encode_png(&smaller);
        if png.len() <= opts.max_bytes {
            return Ok((png, "png"));
        }
    }
    Err(format!(
        "is over {limit} KB as a PNG. Install cwebp (the webp package) and run this again: \
         as a WebP it keeps its transparency at a fraction of the size"
    ))
}

/// `img` as a WebP from `cwebp`: lossy colour at `quality`, lossless alpha, so the folder's edge
/// stays clean.
fn run_cwebp(cwebp: &Path, img: &RgbaImage, quality: u8) -> Result<Vec<u8>, String> {
    let stem = format!(
        "folderskin-make-{}-{quality}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    );
    let input = std::env::temp_dir().join(format!("{stem}.png"));
    let output = std::env::temp_dir().join(format!("{stem}.webp"));
    let result = (|| {
        std::fs::write(&input, raster::encode_png(img))
            .map_err(|e| format!("couldn't be handed to cwebp: {e}"))?;
        let run = std::process::Command::new(cwebp)
            .args([
                "-quiet",
                "-q",
                &quality.to_string(),
                "-alpha_q",
                "100",
                "-m",
                "6",
            ])
            .args(["-sharp_yuv", "-metadata", "none"])
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .map_err(|e| format!("couldn't be compressed, because cwebp didn't run: {e}"))?;
        if !run.status.success() {
            return Err(format!(
                "couldn't be compressed: cwebp said {}",
                String::from_utf8_lossy(&run.stderr).trim()
            ));
        }
        std::fs::read(&output).map_err(|e| format!("couldn't be read back from cwebp: {e}"))
    })();
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
    result
}

/// `img` as a JPEG at the best quality that fits `max_bytes`.
fn encode_jpeg(img: &RgbaImage, max_bytes: usize) -> Result<Vec<u8>, String> {
    let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    for quality in [90, 85, 80, 75, 70] {
        let mut jpg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpg, quality)
            .write_image(&rgb, rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            .map_err(|e| format!("couldn't be encoded: {e}"))?;
        if jpg.len() <= max_bytes {
            return Ok(jpg);
        }
    }
    Err(format!(
        "is still over {} KB as a JPEG at quality 70",
        max_bytes / 1024
    ))
}

/// `img` with its longer side at most `max_side`, keeping its shape.
fn shrink(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w.max(h) <= max_side {
        return img;
    }
    let scale = max_side as f32 / w.max(h) as f32;
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    image::imageops::resize(&img, nw, nh, FilterType::Lanczos3)
}

/// Refuses a picture smaller than a pack allows. `when` says at what point it was measured.
fn big_enough(img: &RgbaImage, when: &str) -> Result<(), String> {
    let (w, h) = img.dimensions();
    if w.min(h) < MIN_PICTURE_SIDE {
        return Err(format!(
            "is {w}×{h} px{when}; a pack needs at least {MIN_PICTURE_SIDE} px on each side"
        ));
    }
    Ok(())
}

/// A skin's name from its file's: "glass_folder-2" is "Glass folder 2". `Skin <n>` when the
/// file's name has no letters or digits.
fn display_name(stem: &str, n: usize) -> String {
    let words = stem
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = words.chars();
    let name: String = match chars.next() {
        Some(first) if words.chars().any(char::is_alphanumeric) => {
            first.to_uppercase().chain(chars).collect()
        }
        _ => format!("Skin {}", n + 1),
    };
    name.chars().take(MAX_SKIN_NAME_CHARS).collect()
}

/// A file name stem for a picture: its name as a slug, or `skin-<n>`, never one already used.
fn unique_stem(stem: &str, n: usize, used: &[(String, Vec<u8>)]) -> String {
    let base = Some(pack::slug(stem))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("skin-{}", n + 1));
    let taken = |s: &str| {
        used.iter()
            .any(|(file, _)| file.rsplit_once('.').is_some_and(|(f, _)| f == s))
    };
    let mut candidate = base.clone();
    let mut i = 2;
    while taken(&candidate) {
        candidate = format!("{base}-{i}");
        i += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// A folder in the system temp folder, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("folderskin-make-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("in")).unwrap();
            Scratch(dir)
        }

        /// Saves `img` as `in/<file>`, in the format its extension names.
        fn picture(&self, file: &str, img: &RgbaImage) -> PathBuf {
            let path = self.0.join("in").join(file);
            if file.ends_with(".jpg") {
                image::DynamicImage::ImageRgba8(img.clone())
                    .to_rgb8()
                    .save(&path)
                    .unwrap();
            } else {
                img.save(&path).unwrap();
            }
            path
        }

        fn options(&self, id: &str) -> MakeOptions {
            MakeOptions {
                id: id.into(),
                name: "Test pack".into(),
                tags: vec!["3D".into(), "glossy".into()],
                author: "prajwal-svm".into(),
                license: "CC0-1.0".into(),
                dir: self.0.clone(),
                max_bytes: 400 * 1024,
                cwebp: None,
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// What an image model paints from FolderSkin's prompt: a folder-shaped subject on magenta.
    fn on_magenta() -> RgbaImage {
        RgbaImage::from_fn(640, 600, |x, y| {
            if (120..520).contains(&x) && (130..470).contains(&y) {
                Rgba([40 + (x % 60) as u8, 120, 220, 255])
            } else {
                Rgba([255, 0, 255, 255])
            }
        })
    }

    /// An ordinary picture, which goes on FolderSkin's folder.
    fn photo() -> RgbaImage {
        RgbaImage::from_fn(1400, 1100, |x, y| {
            Rgba([(x / 6) as u8, (y / 5) as u8, ((x + y) / 10) as u8, 255])
        })
    }

    #[test]
    fn cuts_out_a_folder_on_magenta_and_keeps_a_photo_as_artwork() {
        let scratch = Scratch::new("split");
        scratch.picture("glass_folder.png", &on_magenta());
        scratch.picture("sunset-photo.jpg", &photo());
        let (folder, made) = make(&[scratch.0.join("in")], &scratch.options("glossy")).unwrap();

        assert_eq!(folder, scratch.0.join("packs").join("glossy"));
        let names: Vec<(&str, &str, bool)> = made
            .iter()
            .map(|m| (m.file.as_str(), m.name.as_str(), m.folder))
            .collect();
        assert_eq!(
            names,
            [
                ("glass-folder.png", "Glass folder", true),
                ("sunset-photo.jpg", "Sunset photo", false),
            ]
        );
        let pack = Pack::parse(&std::fs::read(folder.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(pack.tags, ["3d", "glossy"]);
        assert_eq!(pack.skins.len(), 2);

        // The folder was cut out of the magenta and trimmed to the subject.
        let cut = image::open(folder.join("glass-folder.png"))
            .unwrap()
            .to_rgba8();
        assert_eq!(cut.dimensions(), (400, 340));
        assert_eq!(
            cut.get_pixel(0, 0).0[3],
            255,
            "the subject reaches the corner"
        );
        // The photo was shrunk to the pack limit and kept its shape.
        let art = image::open(folder.join("sunset-photo.jpg")).unwrap();
        assert_eq!((art.width(), art.height()), (1024, 805));
        assert!(made.iter().all(|m| m.bytes <= 400 * 1024));
    }

    #[test]
    fn refuses_a_bad_id_an_existing_pack_and_a_picture_too_small() {
        let scratch = Scratch::new("refuse");
        let photo = scratch.picture("a.jpg", &photo());
        let bad = make(std::slice::from_ref(&photo), &scratch.options("Not An Id")).unwrap_err();
        assert!(bad.contains("try not-an-id"), "{bad}");

        make(std::slice::from_ref(&photo), &scratch.options("twice")).unwrap();
        let again = make(&[photo], &scratch.options("twice")).unwrap_err();
        assert!(again.contains("exists already"), "{again}");

        let tiny = scratch.picture(
            "tiny.png",
            &RgbaImage::from_pixel(200, 200, Rgba([9, 9, 9, 255])),
        );
        let small = make(&[tiny], &scratch.options("tiny")).unwrap_err();
        assert!(small.contains("at least 256 px"), "{small}");
        assert!(
            !scratch.0.join("packs").join("tiny").exists(),
            "nothing is left behind"
        );
    }

    #[test]
    fn names_come_from_the_file_names() {
        assert_eq!(display_name("glass_folder-2", 0), "Glass folder 2");
        assert_eq!(display_name("grok image 17", 3), "Grok image 17");
        assert_eq!(display_name("___", 4), "Skin 5");
        let used = vec![("glass.webp".to_string(), Vec::new())];
        assert_eq!(unique_stem("Glass", 1, &used), "glass-2");
        assert_eq!(unique_stem("!!!", 1, &used), "skin-2");
    }

    #[test]
    fn a_folder_becomes_a_webp_with_its_transparency_when_cwebp_is_installed() {
        let Some(cwebp) = find_cwebp() else {
            eprintln!("cwebp isn't installed; skipping");
            return;
        };
        let scratch = Scratch::new("webp");
        scratch.picture("chrome.png", &on_magenta());
        let opts = MakeOptions {
            cwebp: Some(cwebp),
            ..scratch.options("chrome")
        };
        let (folder, made) = make(&[scratch.0.join("in")], &opts).unwrap();
        assert_eq!(made[0].file, "chrome.webp");
        let cut = image::open(folder.join("chrome.webp")).unwrap().to_rgba8();
        assert_eq!(cut.dimensions(), (400, 340));
        assert_eq!(cut.get_pixel(200, 170).0[3], 255);
    }
}
