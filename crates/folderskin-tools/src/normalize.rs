//! One shape for the finished folders in a pack: `packs normalize`, and the same step `community
//! pull` runs on every pack it pulls. [`folderskin_core::shape`] has the rule and why.
//!
//! In a pack with two finished folders or more, each is redrawn at the pack's shape into a
//! 1024 px square and saved as a lossless WebP over its picture. A picture that was a PNG becomes
//! a `.webp` of the same name, and `pack.json` follows. A folder more than the tolerance off the
//! pack's shape is an outlier: it is left as it is and reported, or with [`Outliers::Drop`] taken
//! out of `pack.json` and its file deleted. Artwork, which the app draws onto its own folder, is
//! never touched, and neither is a pack with one finished folder.
//!
//! Running it again changes nothing: a folder already redrawn at its pack's shape is left as it
//! is ([`shape::Placed::is_redrawn`]), and a file that holds the bytes it would get isn't written.

use crate::{packs, parallel, rename};
use folderskin_core::pack::{
    self, Pack, MANIFEST_FILE, MAX_PACK_BYTES, MAX_PICTURE_BYTES, MAX_READ_PICTURE_BYTES,
};
use folderskin_core::{matte, shape};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// What happens to a folder more than the tolerance off its pack's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outliers {
    /// It is left as it is, and reported.
    Keep,
    /// It is taken out of `pack.json` and its file deleted. For the maintainer, and never done
    /// unless asked.
    Drop,
}

/// How [`normalize_folder`] goes about a pack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// How much a folder may be reshaped to take the pack's shape ([`shape::TOLERANCE`]).
    pub tolerance: f32,
    pub outliers: Outliers,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            tolerance: shape::TOLERANCE,
            outliers: Outliers::Keep,
        }
    }
}

/// A finished folder more than the tolerance off its pack's shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outlier {
    /// The skin's name.
    pub name: String,
    /// Its file in the pack.
    pub file: String,
    /// How much it would have to be reshaped ([`shape::reshaping`]): above 0 when it is wider for
    /// its height than the pack's shape.
    pub reshaping: f32,
}

/// A skin redrawn at its pack's shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Redrawn {
    pub name: String,
    /// The file it is in now.
    pub file: String,
    /// The side it was made smaller to, 896 or 768 px, to fit in a pack's 1.5 MB.
    pub scaled_to: Option<u32>,
}

/// What [`normalize_folder`] did to a pack.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct Normalized {
    /// How many of its skins are finished folders.
    pub folders: usize,
    /// Its shape, when it has two finished folders or more.
    pub shape: Option<f32>,
    /// How far apart its finished folders are as it stands now ([`shape::spread`]), outliers
    /// kept as they are included.
    pub spread: f32,
    /// The folders redrawn at the pack's shape.
    pub redrawn: Vec<Redrawn>,
    /// Its outliers, dropped when [`Normalized::dropped`] says so and kept as they are otherwise.
    pub outliers: Vec<Outlier>,
    pub dropped: bool,
    /// The files it wrote, `pack.json` included, and the ones it removed, by name in the pack's
    /// folder.
    pub written: Vec<String>,
    pub removed: Vec<String>,
}

impl Normalized {
    /// Whether anything in the pack's folder changed.
    pub fn changed(&self) -> bool {
        !self.written.is_empty() || !self.removed.is_empty()
    }

    /// How many of its finished folders were at its shape already.
    pub fn already(&self) -> usize {
        self.folders
            .saturating_sub(self.redrawn.len() + self.outliers.len())
    }

    /// What it did, a few words each: "all at it already, the furthest two 0.3% apart", or "9
    /// redrawn", "2 at it already", "1 too far off it, left as it is".
    pub fn summary(&self) -> Vec<String> {
        if self.redrawn.is_empty() && self.outliers.is_empty() {
            let apart = if self.spread >= 0.0005 {
                format!(", the furthest two {} apart", packs::percent(self.spread))
            } else {
                String::new()
            };
            return vec![format!("all at it already{apart}")];
        }
        let mut parts = Vec::new();
        if !self.redrawn.is_empty() {
            parts.push(format!("{} redrawn", self.redrawn.len()));
        }
        if self.already() > 0 {
            parts.push(format!("{} at it already", self.already()));
        }
        if !self.outliers.is_empty() {
            let fate = if self.dropped {
                "dropped"
            } else {
                "left as it is"
            };
            parts.push(format!("{} too far off it, {fate}", self.outliers.len()));
        }
        parts
    }
}

impl Outlier {
    /// "\"Dusk\" (dusk.png) is 12% wider for its height than the pack's shape".
    pub fn describe(&self) -> String {
        format!(
            "\"{}\" ({}) {}",
            self.name,
            self.file,
            off_by(self.reshaping)
        )
    }
}

/// "1.170 times as wide as tall": a shape as people read one.
pub fn times_as_wide(shape: f32) -> String {
    format!("{shape:.3} times as wide as tall")
}

/// "is 12% wider for its height than the pack's shape", for a folder `reshaping` off it
/// ([`shape::reshaping`]).
pub fn off_by(reshaping: f32) -> String {
    let way = if reshaping > 0.0 { "wider" } else { "narrower" };
    format!(
        "is {} {way} for its height than the pack's shape",
        packs::percent(reshaping)
    )
}

/// A skin's picture, measured.
enum Measured {
    /// Artwork, which the app draws onto its own folder, or a picture with nothing more than half
    /// opaque to measure: left as it is.
    Other { bytes: usize },
    /// A finished folder: its shape, measured on its cut-out, and where it sits in its picture.
    Folder {
        bytes: usize,
        aspect: f32,
        placed: Option<shape::Placed>,
    },
}

impl Measured {
    fn bytes(&self) -> usize {
        match self {
            Measured::Other { bytes } | Measured::Folder { bytes, .. } => *bytes,
        }
    }
}

/// Reads the picture `file` in `folder` and tells a finished folder from artwork the way the app
/// does ([`matte::finished_cutout`]). The error starts with the file's name.
fn measure(folder: &Path, file: &str) -> Result<Measured, String> {
    let path = folder.join(file);
    let bytes = std::fs::metadata(&path)
        .map_err(|e| format!("{file} couldn't be read: {e}"))?
        .len() as usize;
    let rgba =
        packs::read_picture(&path, MAX_READ_PICTURE_BYTES).map_err(|e| format!("{file} {e}"))?;
    let Some(cut) = matte::finished_cutout(&rgba, matte::MAGENTA) else {
        return Ok(Measured::Other { bytes });
    };
    Ok(match shape::aspect(&cut) {
        Some(aspect) => Measured::Folder {
            bytes,
            aspect,
            placed: shape::Placed::of(&rgba),
        },
        None => Measured::Other { bytes },
    })
}

/// A folder redrawn at its pack's shape, ready to be saved.
struct Drawn {
    webp: Vec<u8>,
    /// Its shape as it measures once redrawn.
    aspect: f32,
    scaled_to: Option<u32>,
}

/// The finished folder in `file` redrawn at `shape` as a pack's picture: a lossless WebP of at
/// most 1.5 MB, made 896 px and then 768 px if it has to be ([`pack::encode_picture`]). The
/// error finishes a sentence that starts with the file's name.
fn redraw(folder: &Path, file: &str, shape: f32) -> Result<Drawn, String> {
    let rgba = packs::read_picture(&folder.join(file), MAX_READ_PICTURE_BYTES)?;
    let canvas = matte::finished_cutout(&rgba, matte::MAGENTA)
        .and_then(|cut| shape::redraw(&cut, shape))
        .ok_or("isn't a finished folder with anything more than half opaque")?;
    let aspect = shape::aspect(&canvas).unwrap_or(shape);
    let made = pack::encode_picture(canvas, MAX_PICTURE_BYTES)?;
    Ok(Drawn {
        webp: made.webp,
        aspect,
        scaled_to: made.scaled_to,
    })
}

/// Gives the finished folders of the pack in `folder` one shape, as `opts` says, and returns what
/// it did. Nothing is changed when a picture can't be read or redrawn, when the pictures would
/// come to more than a pack may, or when dropping outliers would leave the pack empty.
///
/// Every picture is written under a name of its own first, then `pack.json`, and only then are the
/// files it no longer lists removed, so a run cut short leaves every picture `pack.json` names in
/// place.
pub fn normalize_folder(folder: &Path, opts: &Options) -> Result<Normalized, String> {
    let bytes = std::fs::read(folder.join(MANIFEST_FILE))
        .map_err(|e| format!("{MANIFEST_FILE} couldn't be read: {e}"))?;
    let mut pack = Pack::parse(&bytes).map_err(|problems| problems.join("; "))?;
    let measured = parallel::map(&pack.skins, |skin| measure(folder, &skin.file))
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?;

    let folders: Vec<(usize, f32, Option<shape::Placed>)> = measured
        .iter()
        .enumerate()
        .filter_map(|(i, m)| match m {
            Measured::Folder { aspect, placed, .. } => Some((i, *aspect, *placed)),
            Measured::Other { .. } => None,
        })
        .collect();
    let aspects: Vec<f32> = folders.iter().map(|(_, a, _)| *a).collect();
    let mut done = Normalized {
        folders: folders.len(),
        spread: shape::spread(&aspects),
        ..Normalized::default()
    };
    let Some(plan) = shape::plan(&aspects, opts.tolerance) else {
        return Ok(done);
    };
    done.shape = Some(plan.shape);

    // Which folders are dropped, which are redrawn, and what each measures once it's done.
    let mut drop = BTreeSet::new();
    let mut to_draw = Vec::new();
    let mut after: Vec<(usize, f32)> = Vec::new();
    for (k, &(i, aspect, placed)) in folders.iter().enumerate() {
        let skin = &pack.skins[i];
        if plan.is_outlier(k) {
            done.outliers.push(Outlier {
                name: skin.name.clone(),
                file: skin.file.clone(),
                reshaping: plan.reshaping[k],
            });
            if opts.outliers == Outliers::Drop {
                drop.insert(i);
            } else {
                after.push((i, aspect));
            }
        } else if placed.is_some_and(|p| p.is_redrawn(plan.shape)) {
            after.push((i, aspect));
        } else {
            to_draw.push(i);
        }
    }
    if drop.len() == pack.skins.len() {
        return Err(format!(
            "every skin in it is a folder more than {} off the pack's shape, and a pack keeps one at \
             least; nothing was changed",
            packs::percent(opts.tolerance)
        ));
    }

    // Redrawn on every core at once: libwebp takes most of a second a picture.
    let drawn = parallel::map(&to_draw, |&i| {
        let file = &pack.skins[i].file;
        redraw(folder, file, plan.shape).map_err(|e| format!("{file} {e}; nothing was changed"))
    })
    .into_iter()
    .collect::<Result<Vec<_>, String>>()?;

    // A redrawn picture is a WebP, so a PNG's name changes to say so, to one no file has.
    let mut taken: BTreeSet<String> = pack
        .skins
        .iter()
        .map(|s| s.file.to_ascii_lowercase())
        .chain(files_in(folder)?)
        .collect();
    let mut names = Vec::with_capacity(drawn.len());
    for &i in &to_draw {
        let file = &pack.skins[i].file;
        let name = if has_extension(file, "webp") {
            file.clone()
        } else {
            let name = webp_name(file, &taken);
            taken.insert(name.to_ascii_lowercase());
            name
        };
        names.push(name);
    }

    let mut sizes: Vec<usize> = measured.iter().map(Measured::bytes).collect();
    for (&i, d) in to_draw.iter().zip(&drawn) {
        sizes[i] = d.webp.len();
    }
    let total: usize = (0..sizes.len())
        .filter(|i| !drop.contains(i))
        .map(|i| sizes[i])
        .sum();
    if total > MAX_PACK_BYTES {
        return Err(format!(
            "its pictures would come to {} MB redrawn, and a pack's come to {} MB at most; split \
             it into two packs; nothing was changed",
            total.div_ceil(1024 * 1024),
            MAX_PACK_BYTES / (1024 * 1024)
        ));
    }

    // The pictures first, then pack.json, then the files it no longer lists.
    let mut gone = Vec::new();
    for ((&i, d), name) in to_draw.iter().zip(&drawn).zip(&names) {
        let skin = &mut pack.skins[i];
        after.push((i, d.aspect));
        let same =
            *name == skin.file && std::fs::read(folder.join(name)).is_ok_and(|old| old == d.webp);
        if same {
            continue;
        }
        write_file(folder, name, &d.webp)?;
        done.written.push(name.clone());
        done.redrawn.push(Redrawn {
            name: skin.name.clone(),
            file: name.clone(),
            scaled_to: d.scaled_to,
        });
        if *name != skin.file {
            gone.push(std::mem::replace(&mut skin.file, name.clone()));
        }
    }
    gone.extend(drop.iter().map(|&i| pack.skins[i].file.clone()));
    if !gone.is_empty() {
        pack.skins = std::mem::take(&mut pack.skins)
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !drop.contains(i))
            .map(|(_, skin)| skin)
            .collect();
        let json = serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())? + "\n";
        write_file(folder, MANIFEST_FILE, json.as_bytes())?;
        done.written.push(MANIFEST_FILE.to_string());
        for file in gone {
            let path = folder.join(&file);
            std::fs::remove_file(&path)
                .map_err(|e| format!("couldn't remove {}: {e}", path.display()))?;
            done.removed.push(file);
        }
    }
    done.dropped = !drop.is_empty();
    after.sort_by_key(|(i, _)| *i);
    done.spread = shape::spread(&after.iter().map(|(_, a)| *a).collect::<Vec<_>>());
    Ok(done)
}

/// The names in `folder`, in lower case, dotfiles included: a new name is never one of them.
fn files_in(folder: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(folder)
        .map_err(|e| format!("couldn't read {}: {e}", folder.display()))?;
    Ok(entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .collect())
}

/// Whether `file` ends in `.<ext>`, in any capitals.
fn has_extension(file: &str, ext: &str) -> bool {
    file.rsplit_once('.')
        .is_some_and(|(_, e)| e.eq_ignore_ascii_case(ext))
}

/// A `.webp` name for the picture in `file`: its name with that extension, or with `-2`, `-3` and
/// so on when a file has that name already in any capitals, within the 64 characters a pack's
/// file name may have.
fn webp_name(file: &str, taken: &BTreeSet<String>) -> String {
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    (1..)
        .map(|n: u32| {
            let suffix = if n == 1 {
                String::new()
            } else {
                format!("-{n}")
            };
            // File names are ASCII, so bytes are characters.
            let room = 64 - ".webp".len() - suffix.len();
            format!("{}{suffix}.webp", &stem[..stem.len().min(room)])
        })
        .find(|name| {
            pack::is_picture_file_name(name) && !taken.contains(&name.to_ascii_lowercase())
        })
        .expect("some number makes a name nothing has")
}

/// Writes `bytes` to `name` in `folder` in one step: to a dotfile beside it first, which every
/// check passes over, then moved over it.
fn write_file(folder: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let path = folder.join(name);
    let partial = folder.join(format!(".{name}.normalizing-{}", std::process::id()));
    std::fs::write(&partial, bytes)
        .and_then(|()| std::fs::rename(&partial, &path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&partial);
            format!("couldn't write {}: {e}", path.display())
        })
}

/// What `packs normalize` says about `results`: a line for each pack, an indented line for each
/// folder it redrew and each outlier, and a line for the whole run.
pub fn report(results: &[PackResult], opts: &Options) -> Vec<String> {
    let mut lines = Vec::new();
    let (mut shaped, mut changed, mut redrawn, mut alone, mut failed) = (0, 0, 0, 0, 0);
    let (mut kept, mut dropped) = (0, 0);
    let tolerance = packs::percent(opts.tolerance);
    for PackResult { id, result } in results {
        let n = match result {
            Ok(n) => n,
            Err(e) => {
                failed += 1;
                lines.push(format!("{id}: {e}"));
                continue;
            }
        };
        let Some(shape) = n.shape else {
            alone += 1;
            let what = match n.folders {
                0 => "artwork, which FolderSkin draws on its own folder",
                _ => "one finished folder",
            };
            lines.push(format!("{id}: {what}, left as it is"));
            continue;
        };
        shaped += 1;
        changed += usize::from(n.changed());
        redrawn += n.redrawn.len();
        if n.dropped {
            dropped += n.outliers.len();
        } else {
            kept += n.outliers.len();
        }
        lines.push(format!(
            "{id}: {} finished folders, {}: {}",
            n.folders,
            times_as_wide(shape),
            n.summary().join(", ")
        ));
        for r in &n.redrawn {
            let scaled = r.scaled_to.map_or(String::new(), |side| {
                format!(", made {side} px to fit 1.5 MB")
            });
            lines.push(format!("  redrew \"{}\" into {}{scaled}", r.name, r.file));
        }
        for o in &n.outliers {
            let fate = if n.dropped {
                format!("dropped, and {} deleted", o.file)
            } else {
                "left as it is".to_string()
            };
            lines.push(format!("  {}, more than {tolerance}: {fate}", o.describe()));
        }
    }
    let mut parts = Vec::new();
    if shaped > 0 {
        parts.push(match changed {
            0 => format!("{shaped} of finished folders, all one shape already"),
            _ => format!(
                "{shaped} of finished folders, {changed} of them changed ({} redrawn)",
                packs::count(redrawn, "picture")
            ),
        });
    }
    if alone > 0 {
        parts.push(format!("{alone} left alone"));
    }
    match kept {
        0 => {}
        1 => parts.push("1 outlier left as it is".into()),
        _ => parts.push(format!("{kept} outliers left as they are")),
    }
    if dropped > 0 {
        parts.push(format!("{} dropped", packs::count(dropped, "outlier")));
    }
    if failed > 0 {
        parts.push(format!("{failed} couldn't be done"));
    }
    let packs = packs::count(results.len(), "pack");
    lines.push(if parts.is_empty() {
        packs
    } else {
        format!("{packs}: {}", parts.join("; "))
    });
    lines
}

/// One pack [`normalize`] went to, with what it did or why it couldn't.
#[derive(Debug)]
pub struct PackResult {
    pub id: String,
    pub result: Result<Normalized, String>,
}

/// `packs normalize`: gives the finished folders of every pack in `<dir>/packs`, or of the packs
/// `ids` names, one shape each, as `opts` says. A pack is checked as `packs check` checks it
/// first, and one with a problem is left alone; so is an id with no pack. Fails only when the
/// packs folder can't be read.
pub fn normalize(dir: &Path, ids: &[String], opts: &Options) -> Result<Vec<PackResult>, String> {
    let packs_dir = dir.join(packs::PACKS_DIR);
    let folders = rename::pack_folders(&packs_dir)?;
    let mut chosen: Vec<String> = Vec::new();
    if ids.is_empty() {
        chosen.extend(folders.iter().cloned());
    } else {
        for id in ids {
            if !chosen.contains(id) {
                chosen.push(id.clone());
            }
        }
    }
    Ok(chosen
        .into_iter()
        .map(|id| {
            let folder = packs_dir.join(&id);
            let result = if !folders.contains(&id) {
                Err(format!("there's no pack {id} in {}", packs_dir.display()))
            } else {
                match packs::check_pack(&folder, &id) {
                    Ok(_) => normalize_folder(&folder, opts),
                    Err(problems) => Err(format!(
                        "left alone until `packs check` passes it: {}",
                        problems.join("; ")
                    )),
                }
            };
            PackResult { id, result }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_core::raster;
    use image::{Rgba, RgbaImage};
    use std::path::PathBuf;
    use std::time::SystemTime;

    /// A community folder in the system temp folder, removed when the test ends.
    struct Community(PathBuf);

    impl Community {
        fn new(test: &str) -> Community {
            let dir = std::env::temp_dir().join(format!(
                "folderskin-normalize-{test}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(packs::PACKS_DIR)).unwrap();
            Community(dir)
        }

        fn pack_dir(&self, id: &str) -> PathBuf {
            self.0.join(packs::PACKS_DIR).join(id)
        }

        /// Writes a pack of `skins`, each (file, name, bytes).
        fn pack(&self, id: &str, skins: &[(&str, &str, Vec<u8>)]) {
            let dir = self.pack_dir(id);
            std::fs::create_dir_all(&dir).unwrap();
            let listed: Vec<String> = skins
                .iter()
                .map(|(file, name, _)| format!(r#"{{ "file": "{file}", "name": "{name}" }}"#))
                .collect();
            let json = format!(
                r#"{{ "version": 1, "name": "Test", "author": "prajwal-svm", "license": "CC0-1.0",
  "tags": ["test"], "skins": [{}] }}"#,
                listed.join(", ")
            );
            std::fs::write(dir.join(MANIFEST_FILE), json).unwrap();
            for (file, _, bytes) in skins {
                std::fs::write(dir.join(file), bytes).unwrap();
            }
        }

        fn read(&self, id: &str, file: &str) -> Vec<u8> {
            std::fs::read(self.pack_dir(id).join(file)).unwrap()
        }

        fn manifest(&self, id: &str) -> Pack {
            Pack::parse(&self.read(id, MANIFEST_FILE)).unwrap()
        }

        /// Every file in the pack's folder, dotfiles too, with its bytes and when it was written.
        fn snapshot(&self, id: &str) -> Vec<(String, Vec<u8>, SystemTime)> {
            let mut files: Vec<_> = std::fs::read_dir(self.pack_dir(id))
                .unwrap()
                .flatten()
                .map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let modified = e.metadata().unwrap().modified().unwrap();
                    (name, std::fs::read(e.path()).unwrap(), modified)
                })
                .collect();
            files.sort_by(|a, b| a.0.cmp(&b.0));
            files
        }
    }

    impl Drop for Community {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A finished folder cut out on transparency, `w`×`h` px of solid folder with a soft rim,
    /// tinted `rgb` and darker on its right half, in a picture 24 px bigger all round.
    fn folder(w: u32, h: u32, rgb: [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(w + 48, h + 48, |x, y| {
            let inside = (24..24 + w).contains(&x) && (24..24 + h).contains(&y);
            let rim = (23..25 + w).contains(&x) && (23..25 + h).contains(&y);
            let shade = if x < 24 + w / 2 { 1.0 } else { 0.6 };
            let [r, g, b] = rgb.map(|c| (f32::from(c) * shade) as u8);
            match (inside, rim) {
                (true, _) => Rgba([r, g, b, 255]),
                (false, true) => Rgba([r, g, b, 90]),
                _ => Rgba([0, 0, 0, 0]),
            }
        })
    }

    /// An opaque picture, which the app wraps onto its own folder.
    fn artwork() -> Vec<u8> {
        raster::encode_png(&RgbaImage::from_fn(320, 300, |x, y| {
            Rgba([x as u8, y as u8, 90, 255])
        }))
    }

    fn png(img: &RgbaImage) -> Vec<u8> {
        raster::encode_png(img)
    }

    /// The shape of the folder in `bytes`.
    fn aspect_of(bytes: &[u8]) -> f32 {
        let img = image::load_from_memory(bytes).unwrap().to_rgba8();
        shape::aspect(&img).unwrap()
    }

    #[test]
    fn a_packs_folders_are_redrawn_at_one_shape_and_its_outlier_is_kept_as_it_is() {
        let c = Community::new("shape");
        // Shapes 1.25, 1.17, 1.125 and, far off, 1.60. The median is 1.21, which the first three
        // are within 8% of.
        c.pack(
            "desk-k7q2mx",
            &[
                ("wide.png", "Wide", png(&folder(500, 400, [200, 40, 40]))),
                ("mid.webp", "Mid", {
                    raster::encode_webp_lossless(&folder(468, 400, [40, 160, 40]))
                }),
                ("art.png", "Art", artwork()),
                ("tall.png", "Tall", png(&folder(450, 400, [40, 40, 200]))),
                ("odd.png", "Odd", png(&folder(640, 400, [120, 120, 20]))),
            ],
        );
        let before_odd = c.read("desk-k7q2mx", "odd.png");
        let before_art = c.read("desk-k7q2mx", "art.png");
        let done = normalize_folder(&c.pack_dir("desk-k7q2mx"), &Options::default()).unwrap();

        assert_eq!(done.folders, 4);
        let shape = done.shape.unwrap();
        assert!((shape - (1.25 + 1.17) / 2.0).abs() < 0.001, "{shape}");
        let redrawn: Vec<&str> = done.redrawn.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(redrawn, ["wide.webp", "mid.webp", "tall.webp"]);
        assert_eq!(done.outliers.len(), 1);
        assert_eq!(done.outliers[0].name, "Odd");
        assert!(done.outliers[0].reshaping > 0.3, "{:?}", done.outliers);
        assert!(!done.dropped);
        assert_eq!(done.removed, ["wide.png", "tall.png"]);
        assert!(done.written.contains(&MANIFEST_FILE.to_string()));

        // pack.json names the WebPs, in the same order, and still lists the outlier.
        let files: Vec<String> = c
            .manifest("desk-k7q2mx")
            .skins
            .iter()
            .map(|s| s.file.clone())
            .collect();
        assert_eq!(
            files,
            ["wide.webp", "mid.webp", "art.png", "tall.webp", "odd.png"]
        );
        // Every redrawn folder is FolderSkin's width, on its baseline, at the pack's shape.
        let want = shape::target(shape);
        for file in ["wide.webp", "mid.webp", "tall.webp"] {
            let bytes = c.read("desk-k7q2mx", file);
            assert!(pack::is_lossless_picture(&bytes));
            let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
            assert_eq!(img.dimensions(), (1024, 1024), "{file}");
            let placed = shape::Placed::of(&img).unwrap();
            assert_eq!(placed.visible, want, "{file}");
            assert!(placed.is_redrawn(shape), "{file}");
        }
        // The artwork and the outlier are exactly as they were.
        assert_eq!(c.read("desk-k7q2mx", "art.png"), before_art);
        assert_eq!(c.read("desk-k7q2mx", "odd.png"), before_odd);
        assert!((aspect_of(&before_odd) - 1.6).abs() < 0.01);
        assert!(
            done.spread > 0.3,
            "the outlier is still apart: {}",
            done.spread
        );
        // And the pack passes the check, though not the one for one shape: the outlier is apart.
        assert!(packs::check(&c.0).unwrap().problems.is_empty());
        let one_shape = packs::CheckOptions {
            require_one_shape: true,
            ..packs::CheckOptions::default()
        };
        let problems = packs::check_with(&c.0, &one_shape).unwrap().problems;
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("\"Odd\" (odd.png) is 1.60 times as wide as it is tall"),
            "{problems:?}"
        );
    }

    #[test]
    fn running_it_again_changes_nothing() {
        let c = Community::new("again");
        c.pack(
            "desk-k7q2mx",
            &[
                ("a.png", "A", png(&folder(500, 400, [200, 40, 40]))),
                ("b.png", "B", png(&folder(470, 400, [40, 160, 40]))),
                ("c.png", "C", png(&folder(700, 400, [40, 40, 200]))),
            ],
        );
        let first = normalize_folder(&c.pack_dir("desk-k7q2mx"), &Options::default()).unwrap();
        assert!(first.changed());
        assert_eq!(first.redrawn.len(), 2);
        let before = c.snapshot("desk-k7q2mx");

        let again = normalize_folder(&c.pack_dir("desk-k7q2mx"), &Options::default()).unwrap();
        assert!(!again.changed(), "{again:?}");
        assert!(again.redrawn.is_empty());
        assert_eq!(again.already(), 2);
        // Measured on the redrawn pixels, the shape is the one they were drawn at.
        assert!((again.shape.unwrap() - first.shape.unwrap()).abs() < 0.002);
        assert_eq!(again.outliers.len(), 1, "the outlier is still reported");
        assert!(
            c.snapshot("desk-k7q2mx") == before,
            "not a byte or a timestamp changed"
        );
    }

    #[test]
    fn only_a_folder_out_of_its_place_is_redrawn() {
        let c = Community::new("moved");
        c.pack(
            "desk-k7q2mx",
            &[
                (
                    "a.webp",
                    "A",
                    raster::encode_webp_lossless(&folder(500, 400, [200, 40, 40])),
                ),
                (
                    "b.webp",
                    "B",
                    raster::encode_webp_lossless(&folder(480, 400, [40, 160, 40])),
                ),
            ],
        );
        let dir = c.pack_dir("desk-k7q2mx");
        normalize_folder(&dir, &Options::default()).unwrap();
        let a = image::load_from_memory(&c.read("desk-k7q2mx", "a.webp"))
            .unwrap()
            .to_rgba8();
        let moved_by = |dx: i64| {
            let mut moved = RgbaImage::new(1024, 1024);
            image::imageops::replace(&mut moved, &a, dx, 0);
            raster::encode_webp_lossless(&moved)
        };
        // A pixel to the left is within the pixel an edge rounds to: it is left as it is.
        std::fs::write(dir.join("a.webp"), moved_by(-1)).unwrap();
        let before = c.snapshot("desk-k7q2mx");
        assert!(!normalize_folder(&dir, &Options::default())
            .unwrap()
            .changed());
        assert!(c.snapshot("desk-k7q2mx") == before);
        // Four pixels, and it is redrawn back into its place.
        std::fs::write(dir.join("a.webp"), moved_by(-4)).unwrap();
        let done = normalize_folder(&dir, &Options::default()).unwrap();
        assert_eq!(done.written, ["a.webp"], "only the one that moved");
        let back = image::load_from_memory(&c.read("desk-k7q2mx", "a.webp"))
            .unwrap()
            .to_rgba8();
        assert!(shape::Placed::of(&back)
            .unwrap()
            .is_redrawn(done.shape.unwrap()));
        let before = c.snapshot("desk-k7q2mx");
        assert!(!normalize_folder(&dir, &Options::default())
            .unwrap()
            .changed());
        assert!(c.snapshot("desk-k7q2mx") == before);
    }

    #[test]
    fn outliers_are_dropped_only_when_asked() {
        let c = Community::new("drop");
        c.pack(
            "desk-k7q2mx",
            &[
                ("a.png", "A", png(&folder(500, 400, [200, 40, 40]))),
                ("odd.png", "Odd", png(&folder(700, 400, [120, 120, 20]))),
                ("b.png", "B", png(&folder(480, 400, [40, 160, 40]))),
            ],
        );
        let dir = c.pack_dir("desk-k7q2mx");
        let kept = normalize_folder(&dir, &Options::default()).unwrap();
        assert!(dir.join("odd.png").is_file(), "never dropped by default");
        assert!(!kept.dropped && kept.outliers.len() == 1);

        let drop = Options {
            outliers: Outliers::Drop,
            ..Options::default()
        };
        let dropped = normalize_folder(&dir, &drop).unwrap();
        assert!(dropped.dropped);
        assert_eq!(dropped.removed, ["odd.png"]);
        assert!(!dir.join("odd.png").exists());
        let names: Vec<String> = c
            .manifest("desk-k7q2mx")
            .skins
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(names, ["A", "B"]);
        assert!(
            dropped.redrawn.is_empty(),
            "the others were in place already"
        );
        assert!(dropped.spread <= shape::ONE_SHAPE, "{}", dropped.spread);
        assert!(packs::check_with(
            &c.0,
            &packs::CheckOptions {
                require_one_shape: true,
                ..packs::CheckOptions::default()
            }
        )
        .unwrap()
        .problems
        .is_empty());

        // A wider tolerance takes what 8% wouldn't.
        c.pack(
            "wide-k7q2mx",
            &[
                ("a.png", "A", png(&folder(500, 400, [200, 40, 40]))),
                ("odd.png", "Odd", png(&folder(560, 400, [120, 120, 20]))),
                ("b.png", "B", png(&folder(480, 400, [40, 160, 40]))),
            ],
        );
        let loose = Options {
            tolerance: 0.2,
            ..Options::default()
        };
        let done = normalize_folder(&c.pack_dir("wide-k7q2mx"), &loose).unwrap();
        assert!(done.outliers.is_empty());
        assert_eq!(done.redrawn.len(), 3);
    }

    #[test]
    fn a_pack_is_never_dropped_to_nothing() {
        let c = Community::new("empty");
        // Two folders 30% apart: each is 13% off the shape between them.
        c.pack(
            "pair-k7q2mx",
            &[
                ("a.png", "A", png(&folder(400, 400, [200, 40, 40]))),
                ("b.png", "B", png(&folder(520, 400, [40, 160, 40]))),
            ],
        );
        let drop = Options {
            outliers: Outliers::Drop,
            ..Options::default()
        };
        let before = c.snapshot("pair-k7q2mx");
        let err = normalize_folder(&c.pack_dir("pair-k7q2mx"), &drop).unwrap_err();
        assert!(err.contains("a pack keeps one at least"), "{err}");
        assert!(c.snapshot("pair-k7q2mx") == before, "nothing was changed");
    }

    #[test]
    fn artwork_and_a_single_folder_are_left_alone() {
        let c = Community::new("alone");
        c.pack(
            "art-k7q2mx",
            &[("a.png", "A", artwork()), ("b.png", "B", artwork())],
        );
        c.pack(
            "one-k7q2mx",
            &[
                ("a.png", "A", png(&folder(500, 400, [200, 40, 40]))),
                ("b.png", "B", artwork()),
            ],
        );
        for (id, folders) in [("art-k7q2mx", 0), ("one-k7q2mx", 1)] {
            let before = c.snapshot(id);
            let done = normalize_folder(&c.pack_dir(id), &Options::default()).unwrap();
            assert_eq!((done.folders, done.shape), (folders, None), "{id}");
            assert!(!done.changed(), "{id}");
            assert!(c.snapshot(id) == before, "{id}");
        }
    }

    #[test]
    fn what_it_did_reads_as_a_sentence() {
        assert_eq!(
            off_by(0.12),
            "is 12% wider for its height than the pack's shape"
        );
        assert_eq!(
            off_by(-0.093),
            "is 9.3% narrower for its height than the pack's shape"
        );
        assert_eq!(times_as_wide(1.106), "1.106 times as wide as tall");
        let dusk = Outlier {
            name: "Dusk".into(),
            file: "dusk.png".into(),
            reshaping: 0.12,
        };
        let redrawn = Redrawn {
            name: "Dawn".into(),
            file: "dawn.webp".into(),
            scaled_to: None,
        };
        let mixed = Normalized {
            folders: 12,
            shape: Some(1.17),
            spread: 0.14,
            redrawn: vec![redrawn.clone(); 9],
            outliers: vec![dusk],
            written: vec!["dawn.webp".into()],
            ..Normalized::default()
        };
        assert_eq!(
            mixed.summary(),
            [
                "9 redrawn",
                "2 at it already",
                "1 too far off it, left as it is"
            ]
        );
        let done = Normalized {
            folders: 16,
            shape: Some(1.106),
            spread: 0.0031,
            ..Normalized::default()
        };
        assert_eq!(
            done.summary(),
            ["all at it already, the furthest two 0.3% apart"]
        );

        let results = vec![
            PackResult {
                id: "sky-k7q2mx".into(),
                result: Ok(Normalized {
                    redrawn: vec![Redrawn {
                        scaled_to: Some(896),
                        ..redrawn
                    }],
                    ..mixed
                }),
            },
            PackResult {
                id: "classic-art-5rxas2".into(),
                result: Ok(done),
            },
            PackResult {
                id: "colours-67qg7j".into(),
                result: Ok(Normalized::default()),
            },
            PackResult {
                id: "nope-k7q2mx".into(),
                result: Err("there's no pack nope-k7q2mx in packs".into()),
            },
        ];
        assert_eq!(
            report(&results, &Options::default()),
            [
                "sky-k7q2mx: 12 finished folders, 1.170 times as wide as tall: 1 redrawn, 10 at \
                 it already, 1 too far off it, left as it is",
                "  redrew \"Dawn\" into dawn.webp, made 896 px to fit 1.5 MB",
                "  \"Dusk\" (dusk.png) is 12% wider for its height than the pack's shape, more \
                 than 8%: left as it is",
                "classic-art-5rxas2: 16 finished folders, 1.106 times as wide as tall: all at it \
                 already, the furthest two 0.3% apart",
                "colours-67qg7j: artwork, which FolderSkin draws on its own folder, left as it is",
                "nope-k7q2mx: there's no pack nope-k7q2mx in packs",
                "4 packs: 2 of finished folders, 1 of them changed (1 picture redrawn); 1 left \
                 alone; 1 outlier left as it is; 1 couldn't be done",
            ]
        );
        assert_eq!(report(&[], &Options::default()), ["0 packs"]);
    }

    #[test]
    fn a_new_name_never_takes_another_files() {
        let taken: BTreeSet<String> = ["a.png", "a.webp", "a-2.webp", "b.png"]
            .map(String::from)
            .into();
        assert_eq!(webp_name("a.png", &taken), "a-3.webp");
        assert_eq!(webp_name("b.png", &taken), "b.webp");
        let capitals: BTreeSet<String> = ["c.webp".to_string()].into();
        assert_eq!(
            webp_name("C.png", &capitals),
            "C-2.webp",
            "one file on macOS"
        );
        let long = format!("{}.png", "x".repeat(60));
        let name = webp_name(&long, &BTreeSet::new());
        assert!(
            name.len() <= 64 && pack::is_picture_file_name(&name),
            "{name}"
        );
        assert!(has_extension("A.WEBP", "webp") && !has_extension("a.png", "webp"));
    }

    #[test]
    fn packs_normalize_goes_through_every_pack_and_leaves_a_broken_one_alone() {
        let c = Community::new("command");
        c.pack(
            "desk-k7q2mx",
            &[
                ("a.png", "A", png(&folder(500, 400, [200, 40, 40]))),
                ("b.png", "B", png(&folder(470, 400, [40, 160, 40]))),
            ],
        );
        c.pack("art-k7q2mx", &[("a.png", "A", artwork())]);
        c.pack("broken-k7q2mx", &[("a.png", "A", artwork())]);
        std::fs::write(c.pack_dir("broken-k7q2mx").join("stray.txt"), "hi").unwrap();

        let all = normalize(&c.0, &[], &Options::default()).unwrap();
        let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["art-k7q2mx", "broken-k7q2mx", "desk-k7q2mx"]);
        assert!(all[0].result.as_ref().is_ok_and(|n| !n.changed()));
        let broken = all[1].result.as_ref().unwrap_err();
        assert!(broken.contains("stray.txt isn't listed"), "{broken}");
        assert_eq!(all[2].result.as_ref().unwrap().redrawn.len(), 2);

        // By id, once each; an id with no pack says so.
        let some = normalize(
            &c.0,
            &[
                "desk-k7q2mx".into(),
                "nope-k7q2mx".into(),
                "desk-k7q2mx".into(),
            ],
            &Options::default(),
        )
        .unwrap();
        assert_eq!(some.len(), 2);
        assert!(some[0].result.as_ref().is_ok_and(|n| !n.changed()));
        assert!(some[1]
            .result
            .as_ref()
            .unwrap_err()
            .starts_with("there's no pack nope-k7q2mx"));
        assert!(normalize(&c.0.join("nowhere"), &[], &Options::default()).is_err());
    }
}
