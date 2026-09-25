//! `packs make`: a pack from a folder of pictures, such as renders saved from an image model.
//!
//! Each picture gets the split the app makes when you add one: a finished folder, painted on the
//! magenta key or on real transparency, is cut out and becomes the icon itself, and any other
//! picture is artwork for FolderSkin's folder. Then it is shrunk to 1024 px and saved as a
//! lossless WebP (`raster::encode_webp_lossless`, the encoder the app shares packs with), so the
//! pack looks exactly as the pictures did, a cut-out's transparency included. One still over the
//! size limit is made 896 px, then 768 px, and says so. Nothing needs `cwebp` installed. The
//! folder that comes out passes `packs check` with `--require-lossless`, which runs on it before
//! anything is reported.
//!
//! Two finished folders or more are given one shape, as `packs normalize` gives them
//! ([`folderskin_core::shape`]): each is redrawn at the pack's shape, as wide as FolderSkin's own
//! folder and on its baseline, in a 1024 px square. One more than 8% off that shape is left out
//! and reported, unless [`MakeOptions::keep_outliers`] keeps it as it is. A pack with no folder
//! kept as it is that way passes `packs check --require-one-shape` too.
//!
//! A new pack gets an id of its own, its name and six random characters ([`pack::new_id`]), so
//! two packs can share a name. The id is its folder's name, and it never changes after.

use crate::{normalize, packs, parallel};
use folderskin_core::pack::{
    self, Pack, PackSkin, MANIFEST_FILE, MAX_PACK_BYTES, MAX_PACK_TAGS, MAX_PICTURE_BYTES,
    MAX_PICTURE_SIDE, MAX_SKINS, MAX_SKIN_NAME_CHARS, MIN_PICTURE_SIDE, PACK_VERSION,
    PICTURE_EXTENSIONS,
};
use folderskin_core::{matte, raster, shape};
use image::RgbaImage;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Everything `make` needs besides the pictures.
#[derive(Debug, Clone)]
pub struct MakeOptions {
    /// A pack to make again, which has to be in `packs/` already: its folder is replaced, and it
    /// keeps its id, so everyone who added it gets the new pictures as an update. `None` makes a
    /// new pack, with an id of its own.
    pub id: Option<String>,
    pub name: String,
    /// Tags every skin gets; the first names the pack.
    pub tags: Vec<String>,
    /// The GitHub name of whoever made the pictures.
    pub author: String,
    pub license: String,
    /// The community folder, holding `packs/`.
    pub dir: PathBuf,
    /// The largest a picture may be once compressed: the pack limit ([`MAX_PICTURE_BYTES`]) or
    /// less.
    pub max_bytes: usize,
    /// Also cut away a flat backdrop of any colour, for renders whose #FF00FF drifted to pink or
    /// raspberry. Only the backdrop that reaches the edge goes, so the same colour inside the
    /// folder stays.
    pub flat_backdrop: bool,
    /// Keep a finished folder more than [`shape::TOLERANCE`] off the pack's shape, as it is,
    /// instead of leaving it out.
    pub keep_outliers: bool,
}

impl MakeOptions {
    /// The most a picture may come to: `max_bytes`, and never more than a pack's limit.
    pub fn picture_limit(&self) -> usize {
        self.max_bytes.min(MAX_PICTURE_BYTES)
    }
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
    /// The side it was made smaller to, 896 or 768 px, when it was over `max_bytes` at 1024.
    pub scaled_to: Option<u32>,
    /// True for a finished folder redrawn at the pack's shape.
    pub redrawn: bool,
    /// For a finished folder kept as it is though it is more than the tolerance off the pack's
    /// shape ([`MakeOptions::keep_outliers`]), how much it would have been reshaped.
    pub outlier: Option<f32>,
}

impl Made {
    /// What happened to its shape, to end its line in a report: ", redrawn at the pack's shape",
    /// ", kept as it is: it is 43% wider for its height than the pack's shape", or nothing.
    pub fn shape_note(&self) -> String {
        match (self.redrawn, self.outlier) {
            (true, _) => ", redrawn at the pack's shape".into(),
            (false, Some(reshaping)) => {
                format!(", kept as it is: it {}", normalize::off_by(reshaping))
            }
            (false, None) => String::new(),
        }
    }
}

/// A picture `make` left out: a finished folder more than the tolerance off the pack's shape.
#[derive(Debug, Clone)]
pub struct LeftOut {
    pub source: PathBuf,
    /// How much it would have been reshaped ([`shape::reshaping`]).
    pub reshaping: f32,
}

impl LeftOut {
    /// "left out: in/odd.png, a finished folder that is 43% wider for its height than the pack's
    /// shape, more than 8%; --keep-outliers keeps it as it is".
    pub fn describe(&self) -> String {
        format!(
            "left out: {}, a finished folder that {}, more than {}; --keep-outliers keeps it as \
             it is",
            self.source.display(),
            normalize::off_by(self.reshaping),
            packs::percent(shape::TOLERANCE)
        )
    }
}

/// The pack `make` wrote.
#[derive(Debug, Clone)]
pub struct MadePack {
    /// Its folder, `<dir>/packs/<id>`.
    pub folder: PathBuf,
    /// Its skins, in order.
    pub made: Vec<Made>,
    /// The shape its finished folders were given, when it has two or more.
    pub shape: Option<f32>,
    /// The pictures left out for being too far off that shape.
    pub left_out: Vec<LeftOut>,
}

/// What happens to a picture on its way into the pack.
#[derive(Debug, Clone, Copy)]
enum Treat {
    /// A finished folder, redrawn at the pack's shape.
    Redraw(f32),
    /// Shrunk to 1024 px as it is. For a finished folder kept though it is more than the
    /// tolerance off the pack's shape, how far off.
    AsItIs(Option<f32>),
}

/// Makes `<dir>/packs/<id>` from `pictures` (files, or folders whose pictures are taken in name
/// order) and returns the folder it wrote with what went into it: a new pack under a new id, or
/// `opts.id` made again. Nothing is left behind when a picture can't be used, and a pack made
/// again stays as it was.
pub fn make(pictures: &[PathBuf], opts: &MakeOptions) -> Result<MadePack, String> {
    let packs_dir = opts.dir.join(packs::PACKS_DIR);
    let id = match &opts.id {
        Some(id) => to_make_again(&packs_dir, id)?,
        None => pack::new_id(&opts.name, {
            let taken = taken_ids(&opts.dir)?;
            move |id| taken.contains(id)
        })?,
    };
    let folder = packs_dir.join(&id);
    let sources = collect(pictures)?;
    if sources.len() > MAX_SKINS {
        return Err(format!(
            "that's {} pictures; a pack holds at most {MAX_SKINS}",
            sources.len()
        ));
    }

    // Every picture is looked at first, for its kind and a finished folder's shape, and turned
    // down now if it can't go in; all on every core at once.
    let looked = parallel::map(&sources, |source| {
        look(source, opts).map_err(|e| format!("{} {e}", source.display()))
    })
    .into_iter()
    .collect::<Result<Vec<_>, String>>()?;
    let aspects: Vec<f32> = looked.iter().flatten().copied().collect();
    let plan = shape::plan(&aspects, shape::TOLERANCE);
    let mut jobs: Vec<(&PathBuf, Treat)> = Vec::with_capacity(sources.len());
    let mut left_out = Vec::new();
    // Which finished folder this is, in the order `plan` has them.
    let mut k = 0;
    for (source, aspect) in sources.iter().zip(&looked) {
        let (Some(plan), Some(_)) = (&plan, aspect) else {
            jobs.push((source, Treat::AsItIs(None)));
            continue;
        };
        let reshaping = plan.reshaping[k];
        let outlier = plan.is_outlier(k);
        k += 1;
        if !outlier {
            jobs.push((source, Treat::Redraw(plan.shape)));
        } else if opts.keep_outliers {
            jobs.push((source, Treat::AsItIs(Some(reshaping))));
        } else {
            left_out.push(LeftOut {
                source: source.clone(),
                reshaping,
            });
        }
    }
    if jobs.is_empty() {
        return Err(format!(
            "every picture is a finished folder more than {} off the others' shape, so none is \
             left; --keep-outliers keeps them as they are",
            packs::percent(shape::TOLERANCE)
        ));
    }

    // Then made, on every core at once: libwebp's smallest lossless file takes most of a second
    // a picture.
    let prepared = parallel::map(&jobs, |(source, treat)| {
        prepare(source, opts, *treat).map_err(|e| format!("{} {e}", source.display()))
    })
    .into_iter()
    .collect::<Result<Vec<_>, String>>()?;
    let total: usize = prepared.iter().map(|p| p.bytes.len()).sum();
    if total > MAX_PACK_BYTES {
        return Err(format!(
            "the pictures come to {} MB, and a pack's come to {} MB at most; split them into two \
             packs",
            total.div_ceil(1024 * 1024),
            MAX_PACK_BYTES / (1024 * 1024)
        ));
    }
    let mut made = Vec::with_capacity(jobs.len());
    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(jobs.len());
    for (n, ((source, treat), picture)) in jobs.iter().zip(prepared).enumerate() {
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = format!("{}.webp", unique_stem(&stem, n, &files));
        made.push(Made {
            source: (*source).clone(),
            file: file.clone(),
            name: display_name(&stem, n),
            folder: picture.folder,
            bytes: picture.bytes.len(),
            scaled_to: picture.scaled_to,
            redrawn: matches!(treat, Treat::Redraw(_)),
            outlier: match treat {
                Treat::AsItIs(outlier) => *outlier,
                Treat::Redraw(_) => None,
            },
        });
        files.push((file, picture.bytes));
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

    // Written and checked beside the packs first, in a folder whose leading dot every check
    // passes over: a pack that fails leaves nothing behind, and one made again replaces the old
    // folder only once the new one has passed.
    let staging = packs_dir.join(format!(".make-{id}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let written = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&staging)?;
        for (file, bytes) in &files {
            std::fs::write(staging.join(file), bytes)?;
        }
        std::fs::write(staging.join(MANIFEST_FILE), json)
    })();
    let placed = written
        .map_err(|e| format!("couldn't write {}: {e}", folder.display()))
        .and_then(|()| {
            let rules = packs::CheckOptions {
                max_bytes: opts.max_bytes,
                require_lossless: true,
                // Its folders are one shape unless one was kept as it is on purpose.
                require_one_shape: made.iter().all(|m| m.outlier.is_none()),
                ..packs::CheckOptions::default()
            };
            packs::check_pack_with(&staging, &id, &rules).map_err(|problems| problems.join("; "))
        })
        .and_then(|_| put_in_place(&staging, &folder));
    if let Err(e) = placed {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    Ok(MadePack {
        folder,
        made,
        shape: plan.map(|p| p.shape),
        left_out,
    })
}

/// `id` when it is a pack in `packs_dir` to make again.
fn to_make_again(packs_dir: &Path, id: &str) -> Result<String, String> {
    if !pack::is_pack_id(id) {
        let suggestion = pack::slug(id);
        return Err(if pack::is_pack_id(&suggestion) {
            format!("\"{id}\" can't be a pack id; try {suggestion}")
        } else {
            format!("\"{id}\" can't be a pack id: use lower-case words joined by dashes")
        });
    }
    if !packs_dir.join(id).is_dir() {
        return Err(format!(
            "there's no pack {id} in {} to make again; leave out --id and the new pack gets an \
             id of its own",
            packs_dir.display()
        ));
    }
    Ok(id.to_string())
}

/// The ids a new pack in `dir` can't have: every folder in `packs/`, in lower case since macOS and
/// Windows take two names one capital apart for one folder, and every old id in `moved.json`,
/// which is never given out again.
fn taken_ids(dir: &Path) -> Result<HashSet<String>, String> {
    let packs_dir = dir.join(packs::PACKS_DIR);
    let mut taken = HashSet::new();
    match std::fs::read_dir(&packs_dir) {
        Ok(entries) => {
            for entry in entries {
                let entry =
                    entry.map_err(|e| format!("couldn't read {}: {e}", packs_dir.display()))?;
                taken.insert(entry.file_name().to_string_lossy().to_lowercase());
            }
        }
        // The first pack in a new checkout makes the folder.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("couldn't read {}: {e}", packs_dir.display())),
    }
    taken.extend(packs::read_moved(dir)?.moved.into_keys());
    Ok(taken)
}

/// Moves the checked pack in `staging` to `folder`. A pack already there is moved aside first
/// and put back if the new one can't take its place, so a failure never loses it.
fn put_in_place(staging: &Path, folder: &Path) -> Result<(), String> {
    let failed = |e: std::io::Error| format!("couldn't write {}: {e}", folder.display());
    if !folder.exists() {
        return std::fs::rename(staging, folder).map_err(failed);
    }
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let old = folder.with_file_name(format!(".make-old-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(folder, &old).map_err(failed)?;
    if let Err(e) = std::fs::rename(staging, folder) {
        let _ = std::fs::rename(&old, folder);
        return Err(failed(e));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
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

/// One picture, ready for the pack.
struct Prepared {
    /// A lossless WebP.
    bytes: Vec<u8>,
    /// True for a finished folder, false for artwork on FolderSkin's folder.
    folder: bool,
    /// The side it was made smaller to, when it was over the size limit at 1024 px.
    scaled_to: Option<u32>,
}

/// A picture as it goes into the pack, at its own size: the finished folder cut out of it, or the
/// whole picture as artwork, and which it is. The error finishes a sentence that starts with the
/// picture's path.
fn load(path: &Path, opts: &MakeOptions) -> Result<(RgbaImage, bool), String> {
    let rgba = image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("couldn't be read: {e}"))?
        .decode()
        .map_err(|e| format!("isn't a picture FolderSkin can read: {e}"))?
        .to_rgba8();
    if matte::alpha_bounds(&rgba, 8).is_none() {
        return Err("is completely transparent".into());
    }
    let cut = matte::finished_cutout(&rgba, matte::MAGENTA).or_else(|| {
        let key = matte::flat_backdrop(&rgba).filter(|_| opts.flat_backdrop)?;
        let cut = matte::cutout_connected(&rgba, key);
        // Something substantial has to be left, as with a magenta key.
        let solid = cut.pixels().filter(|p| p.0[3] >= 128).count() as f32;
        (solid >= 0.02 * rgba.width() as f32 * rgba.height() as f32).then_some(cut)
    });
    Ok(match cut {
        Some(cut) => (cut, true),
        None => (rgba, false),
    })
}

/// A finished folder's shape, `None` for artwork, once it has been made sure the picture can go
/// into a pack. The error finishes a sentence that starts with the picture's path.
fn look(path: &Path, opts: &MakeOptions) -> Result<Option<f32>, String> {
    let (img, folder) = load(path, opts)?;
    let (w, h) = raster::shrunk_size(img.width(), img.height(), MAX_PICTURE_SIDE);
    big_enough(w, h, if folder { " once cut out" } else { "" })?;
    Ok(if folder { shape::aspect(&img) } else { None })
}

/// One picture, ready for the pack: a lossless WebP of the finished folder cut out of it, redrawn
/// at the pack's shape or as it is, or of the whole picture as artwork. The error finishes a
/// sentence that starts with the picture's path.
fn prepare(path: &Path, opts: &MakeOptions, treat: Treat) -> Result<Prepared, String> {
    let (img, folder) = load(path, opts)?;
    let img = match treat {
        Treat::Redraw(shape) => {
            shape::redraw(&img, shape).ok_or("has nothing more than half opaque to redraw")?
        }
        Treat::AsItIs(_) => raster::shrink_to(img, MAX_PICTURE_SIDE),
    };
    let made = pack::encode_picture(img, opts.picture_limit())?;
    Ok(Prepared {
        bytes: made.webp,
        folder,
        scaled_to: made.scaled_to,
    })
}

/// `img` as a WebP from `cwebp`: lossy colour at `quality`, lossless alpha, so the folder's edge
/// stays clean.
pub(crate) fn run_cwebp(cwebp: &Path, img: &RgbaImage, quality: u8) -> Result<Vec<u8>, String> {
    // The catalog encodes on several threads at once, so the clock alone could name two files
    // the same.
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stem = format!(
        "folderskin-make-{}-{quality}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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

/// Refuses a picture `w`×`h` px once shrunk to 1024 px, smaller than a pack allows. `when` says
/// at what point it was measured.
fn big_enough(w: u32, h: u32, when: &str) -> Result<(), String> {
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

        /// A new pack called "Test pack".
        fn options(&self) -> MakeOptions {
            MakeOptions {
                id: None,
                name: "Test pack".into(),
                tags: vec!["3D".into(), "glossy".into()],
                author: "prajwal-svm".into(),
                license: "CC0-1.0".into(),
                dir: self.0.clone(),
                max_bytes: MAX_PICTURE_BYTES,
                flat_backdrop: false,
                keep_outliers: false,
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
        let jpeg = scratch.picture("sunset-photo.jpg", &photo());
        let MadePack {
            folder,
            made,
            shape,
            left_out,
        } = make(&[scratch.0.join("in")], &scratch.options()).unwrap();
        assert_eq!(shape, None, "one finished folder keeps its own shape");
        assert!(left_out.is_empty() && made.iter().all(|m| !m.redrawn));

        assert_eq!(folder.parent(), Some(scratch.0.join("packs").as_path()));
        let id = folder.file_name().unwrap().to_string_lossy();
        assert!(
            id.starts_with("test-pack-") && pack::is_generated_id(&id),
            "{id}"
        );
        let names: Vec<(&str, &str, bool)> = made
            .iter()
            .map(|m| (m.file.as_str(), m.name.as_str(), m.folder))
            .collect();
        assert_eq!(
            names,
            [
                ("glass-folder.webp", "Glass folder", true),
                ("sunset-photo.webp", "Sunset photo", false),
            ]
        );
        let pack = Pack::parse(&std::fs::read(folder.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(pack.tags, ["3d", "glossy"]);
        assert_eq!(pack.skins.len(), 2);

        // The folder was cut out of the magenta and trimmed to the subject.
        let cut_bytes = std::fs::read(folder.join("glass-folder.webp")).unwrap();
        assert!(pack::is_lossless_picture(&cut_bytes));
        let cut = image::load_from_memory(&cut_bytes).unwrap().to_rgba8();
        assert_eq!(cut.dimensions(), (400, 340));
        assert_eq!(
            cut.get_pixel(0, 0).0[3],
            255,
            "the subject reaches the corner"
        );
        // The photo was shrunk to the pack limit, kept its shape, and every pixel of it.
        let art_bytes = std::fs::read(folder.join("sunset-photo.webp")).unwrap();
        assert!(pack::is_lossless_picture(&art_bytes));
        let art = image::load_from_memory(&art_bytes).unwrap().to_rgba8();
        let source = image::open(&jpeg).unwrap().to_rgba8();
        assert_eq!(art, raster::shrink_to(source, MAX_PICTURE_SIDE), "lossless");
        assert_eq!(art.dimensions(), (1024, 805));
        assert!(made
            .iter()
            .all(|m| m.bytes <= MAX_PICTURE_BYTES && m.scaled_to.is_none()));
    }

    /// The names in `<scratch>/packs`, dotfolders too, sorted.
    fn pack_folders(scratch: &Scratch) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(scratch.0.join("packs"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn packs_with_one_name_get_ids_of_their_own() {
        let scratch = Scratch::new("same-name");
        let photo = scratch.picture("a.jpg", &photo());
        let first = make(std::slice::from_ref(&photo), &scratch.options())
            .unwrap()
            .folder;
        let second = make(&[photo], &scratch.options()).unwrap().folder;
        assert_ne!(first, second);
        let folders = pack_folders(&scratch);
        assert_eq!(folders.len(), 2, "{folders:?}");
        for id in &folders {
            assert!(
                id.starts_with("test-pack-") && pack::is_generated_id(id),
                "{id}"
            );
        }
    }

    #[test]
    fn a_new_id_is_never_a_folder_or_an_old_id_in_moved_json() {
        let scratch = Scratch::new("taken");
        assert!(taken_ids(&scratch.0).unwrap().is_empty(), "no packs/ yet");
        std::fs::create_dir_all(scratch.0.join("packs").join("Reds-K7Q2MX")).unwrap();
        std::fs::write(
            scratch.0.join(pack::MOVED_FILE),
            r#"{ "version": 1, "moved": { "blues": "blues-a2b3c4" } }"#,
        )
        .unwrap();
        let taken = taken_ids(&scratch.0).unwrap();
        assert_eq!(
            taken,
            HashSet::from(["reds-k7q2mx".to_string(), "blues".to_string()])
        );
        // A moved.json it can't read stops the pack rather than risk an old id.
        std::fs::write(scratch.0.join(pack::MOVED_FILE), "{").unwrap();
        let photo = scratch.picture("a.jpg", &photo());
        let err = make(&[photo], &scratch.options()).unwrap_err();
        assert!(err.contains("moved.json isn't valid"), "{err}");
    }

    #[test]
    fn id_makes_a_pack_that_is_there_again_and_keeps_its_id() {
        let scratch = Scratch::new("again");
        let first = scratch.picture("first.jpg", &photo());
        let folder = make(&[first], &scratch.options()).unwrap().folder;
        let id = folder.file_name().unwrap().to_string_lossy().into_owned();

        let second = scratch.picture("second_one.png", &on_magenta());
        let again = MakeOptions {
            id: Some(id.clone()),
            name: "Test pack, redone".into(),
            ..scratch.options()
        };
        let MadePack {
            folder: same, made, ..
        } = make(&[second], &again).unwrap();
        assert_eq!(same, folder);
        assert_eq!(made[0].file, "second-one.webp");
        let mut files: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(
            files,
            ["pack.json", "second-one.webp"],
            "the old pictures go"
        );
        let pack = Pack::parse(&std::fs::read(folder.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(pack.name, "Test pack, redone");
        assert_eq!(
            pack_folders(&scratch),
            std::slice::from_ref(&id),
            "nothing else is left"
        );

        // A picture that fails leaves the pack as it was.
        let tiny = scratch.picture(
            "tiny.png",
            &RgbaImage::from_pixel(200, 200, Rgba([9, 9, 9, 255])),
        );
        let small = make(&[tiny], &again).unwrap_err();
        assert!(small.contains("at least 256 px"), "{small}");
        assert!(folder.join("second-one.webp").is_file());
        assert_eq!(pack_folders(&scratch), [id]);
    }

    #[test]
    fn refuses_a_bad_id_a_pack_that_isnt_there_and_a_picture_too_small() {
        let scratch = Scratch::new("refuse");
        let photo = scratch.picture("a.jpg", &photo());
        let with = |id: &str| MakeOptions {
            id: Some(id.into()),
            ..scratch.options()
        };
        let bad = make(std::slice::from_ref(&photo), &with("Not An Id")).unwrap_err();
        assert!(bad.contains("try not-an-id"), "{bad}");
        let missing = make(std::slice::from_ref(&photo), &with("nope-k7q2mx")).unwrap_err();
        assert!(
            missing.contains("there's no pack nope-k7q2mx") && missing.contains("leave out --id"),
            "{missing}"
        );

        let tiny = scratch.picture(
            "tiny.png",
            &RgbaImage::from_pixel(200, 200, Rgba([9, 9, 9, 255])),
        );
        let small = make(&[tiny], &scratch.options()).unwrap_err();
        assert!(small.contains("at least 256 px"), "{small}");
        assert!(
            !scratch.0.join("packs").exists() || pack_folders(&scratch).is_empty(),
            "nothing is left behind: {:?}",
            pack_folders(&scratch)
        );
    }

    /// A render whose magenta drifted to raspberry, with a crimson patch in the folder.
    fn on_raspberry() -> RgbaImage {
        RgbaImage::from_fn(640, 600, |x, y| {
            if !(120..520).contains(&x) || !(130..470).contains(&y) {
                Rgba([189, 0, 103, 255])
            } else if (300..340).contains(&x) && (280..320).contains(&y) {
                Rgba([180, 12, 70, 255])
            } else {
                Rgba([40 + (x % 60) as u8, 120, 220, 255])
            }
        })
    }

    #[test]
    fn a_drifted_backdrop_is_cut_away_only_when_asked() {
        let scratch = Scratch::new("drift");
        scratch.picture("drifted.png", &on_raspberry());
        let pictures = [scratch.0.join("in")];

        let made = make(&pictures, &scratch.options()).unwrap().made;
        assert!(
            !made[0].folder,
            "the app's own split: not magenta, so artwork"
        );

        let opts = MakeOptions {
            flat_backdrop: true,
            ..scratch.options()
        };
        let MadePack { folder, made, .. } = make(&pictures, &opts).unwrap();
        assert!(made[0].folder);
        let cut = image::open(folder.join("drifted.webp")).unwrap().to_rgba8();
        assert_eq!(cut.dimensions(), (400, 340));
        assert_eq!(cut.get_pixel(200, 170).0[3], 255, "the crimson patch stays");
    }

    /// A folder-shaped subject `w`×`h` px in `rgb` on magenta, as an image model paints one.
    fn folder_on_magenta(w: u32, h: u32, rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(w + 80, h + 80, |x, y| {
            if (40..40 + w).contains(&x) && (40..40 + h).contains(&y) {
                Rgba([rgb[0], rgb[1], rgb[2], 255])
            } else {
                Rgba([255, 0, 255, 255])
            }
        })
    }

    /// Four folders 1.25, 1.175, 1.2 and 1.75 times as wide as tall, which makes the pack's shape
    /// 1.225 and the last 43% off it, and a photo.
    fn a_batch_of_renders(scratch: &Scratch) -> [PathBuf; 1] {
        scratch.picture("a.png", &folder_on_magenta(500, 400, [40, 120, 220]));
        scratch.picture("b.png", &folder_on_magenta(470, 400, [200, 40, 40]));
        scratch.picture("c.png", &folder_on_magenta(480, 400, [40, 160, 60]));
        scratch.picture("odd.png", &folder_on_magenta(700, 400, [220, 180, 40]));
        scratch.picture("photo.jpg", &photo());
        [scratch.0.join("in")]
    }

    #[test]
    fn a_packs_finished_folders_are_made_one_shape_and_an_outlier_is_left_out() {
        let scratch = Scratch::new("one-shape");
        let pictures = a_batch_of_renders(&scratch);
        let MadePack {
            folder,
            made,
            shape,
            left_out,
        } = make(&pictures, &scratch.options()).unwrap();

        let shape = shape.unwrap();
        assert!((shape - 1.225).abs() < 0.001, "{shape}");
        assert_eq!(left_out.len(), 1);
        assert!(left_out[0].source.ends_with("odd.png"));
        assert!((left_out[0].reshaping - 0.4286).abs() < 0.001);
        let files: Vec<(&str, bool, bool)> = made
            .iter()
            .map(|m| (m.file.as_str(), m.folder, m.redrawn))
            .collect();
        assert_eq!(
            files,
            [
                ("a.webp", true, true),
                ("b.webp", true, true),
                ("c.webp", true, true),
                ("photo.webp", false, false),
            ]
        );
        // Every folder is FolderSkin's width on its baseline, at the pack's shape.
        let want = folderskin_core::shape::target(shape);
        for file in ["a.webp", "b.webp", "c.webp"] {
            let img = image::open(folder.join(file)).unwrap().to_rgba8();
            let placed = folderskin_core::shape::Placed::of(&img).unwrap();
            assert_eq!((img.dimensions(), placed.visible), ((1024, 1024), want));
        }
        let pack = Pack::parse(&std::fs::read(folder.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(pack.skins.len(), 4, "the outlier isn't in it");
        let one_shape = packs::CheckOptions {
            require_one_shape: true,
            require_lossless: true,
            ..packs::CheckOptions::default()
        };
        assert!(packs::check_with(&scratch.0, &one_shape)
            .unwrap()
            .problems
            .is_empty());
    }

    #[test]
    fn keep_outliers_keeps_one_as_it_is() {
        let scratch = Scratch::new("keep");
        let pictures = a_batch_of_renders(&scratch);
        let opts = MakeOptions {
            keep_outliers: true,
            ..scratch.options()
        };
        let MadePack {
            folder,
            made,
            left_out,
            ..
        } = make(&pictures, &opts).unwrap();
        assert!(left_out.is_empty());
        assert_eq!(made.len(), 5);
        let odd = &made[3];
        assert_eq!(odd.file, "odd.webp");
        assert!(!odd.redrawn && odd.outlier.is_some_and(|r| r > 0.4));
        // Cut out and trimmed, as a lone folder would be.
        let img = image::open(folder.join("odd.webp")).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (700, 400));
        assert!(made[..3].iter().all(|m| m.redrawn && m.outlier.is_none()));
    }

    #[test]
    fn folders_too_far_apart_to_share_a_shape_make_no_pack() {
        let scratch = Scratch::new("apart");
        // 1.0 and 1.3: each is 13% off the shape between them.
        scratch.picture("a.png", &folder_on_magenta(400, 400, [40, 120, 220]));
        scratch.picture("b.png", &folder_on_magenta(520, 400, [200, 40, 40]));
        let err = make(&[scratch.0.join("in")], &scratch.options()).unwrap_err();
        assert!(err.contains("--keep-outliers keeps them"), "{err}");
        assert!(!scratch.0.join("packs").exists() || pack_folders(&scratch).is_empty());
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
}
