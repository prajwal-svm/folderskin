//! Community skin packs: the `pack.json` contract, its limits and the checks on it.
//!
//! A pack is a folder holding `pack.json` and the pictures it lists. The app runs these checks
//! before it saves anything from a pack, and `folderskin-tools packs check` runs the same ones on
//! every pull request, so a pack that passes CI is a pack the app accepts. docs/PACKS.md is this
//! contract in prose.

use crate::raster;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

/// The contract version a `pack.json` declares in `"version"`.
pub const PACK_VERSION: u32 = 1;
/// The version of `community/index.json` this build reads.
pub const INDEX_VERSION: u32 = 1;
/// The file every pack has.
pub const MANIFEST_FILE: &str = "pack.json";
/// `moved.json`, beside `packs/`: the packs whose ids changed.
pub const MOVED_FILE: &str = "moved.json";
pub const MOVED_VERSION: u32 = 1;
/// Most skins in one pack: a themed set that is quick to review and to download.
pub const MAX_SKINS: usize = 50;
/// Largest picture in a pack made or shared from FolderSkin 0.1.7 on, which has to be lossless
/// too ([`check_new_picture`]): 1.5 MB, which a lossless WebP of a 1024 px picture fits in.
pub const MAX_PICTURE_BYTES: usize = 1_572_864;
/// Largest picture FolderSkin reads in a pack: the 2 MB packs were held to before 0.1.7, so the
/// packs published then still open, until they're made again within [`MAX_PICTURE_BYTES`].
pub const MAX_READ_PICTURE_BYTES: usize = 2 * 1024 * 1024;
/// Most a pack's pictures may come to together: 64 MB, which fifty detailed lossless pictures of
/// about 1.2 MB each fit in.
pub const MAX_PACK_BYTES: usize = 64 * 1024 * 1024;
/// Largest side of a picture. 1024 px is the biggest icon any of the three systems draws.
pub const MAX_PICTURE_SIDE: u32 = 1024;
/// Smallest side of a picture, so no icon is blown up from a thumbnail.
pub const MIN_PICTURE_SIDE: u32 = 256;
/// Largest `pack.json`.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_PACK_NAME_CHARS: usize = 40;
pub const MAX_SKIN_NAME_CHARS: usize = 60;
/// Tags a pack gives all of its skins. The first one names the pack's collection.
pub const MAX_PACK_TAGS: usize = 5;
/// Tags one skin adds to its pack's.
pub const MAX_SKIN_TAGS: usize = 3;
/// Tags any one skin can carry, however it got them.
pub const MAX_TAGS: usize = 8;
pub const MAX_TAG_CHARS: usize = 24;
/// Licences a pack can use: Creative Commons, or MIT. All of them let anyone share the pictures;
/// CC BY and MIT keep the author's name with them.
pub const LICENSES: &[&str] = &["CC0-1.0", "CC-BY-4.0", "MIT"];
/// File name extensions a pack's pictures can have.
pub const PICTURE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
/// The sides a picture is tried at as it is made ready for a pack ([`encode_picture`]), largest
/// first: one whose lossless file is over the limit at 1024 px is made 896 px, then 768 px.
pub const PICTURE_SIDES: [u32; 3] = [MAX_PICTURE_SIDE, 896, 768];

/// `pack.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub version: u32,
    pub name: String,
    /// The author's GitHub user name.
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub skins: Vec<PackSkin>,
}

/// One skin in a pack.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PackSkin {
    /// A picture in the pack's folder.
    pub file: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Pack {
    /// Reads a `pack.json` and checks every rule in [`Pack::problems`]. The error lists each
    /// problem as a sentence.
    pub fn parse(bytes: &[u8]) -> Result<Pack, Vec<String>> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(vec![format!(
                "pack.json is over {} KB",
                MAX_MANIFEST_BYTES / 1024
            )]);
        }
        // The version first, so a pack from a newer FolderSkin says so instead of failing on a
        // field this version does not know.
        #[derive(Deserialize)]
        struct Head {
            version: Option<serde_json::Value>,
        }
        let head: Head = serde_json::from_slice(bytes)
            .map_err(|e| vec![format!("pack.json is not valid JSON: {e}")])?;
        match head.version.as_ref().and_then(serde_json::Value::as_u64) {
            Some(v) if v == u64::from(PACK_VERSION) => {}
            Some(v) if v > u64::from(PACK_VERSION) => {
                return Err(vec![format!(
                    "this pack is for a newer FolderSkin (pack format {v})"
                )])
            }
            _ => return Err(vec![format!("pack.json needs \"version\": {PACK_VERSION}")]),
        }
        let pack: Pack =
            serde_json::from_slice(bytes).map_err(|e| vec![format!("pack.json: {e}")])?;
        let problems = pack.problems();
        if problems.is_empty() {
            Ok(pack)
        } else {
            Err(problems)
        }
    }

    /// Everything wrong with the pack's own fields, one sentence each; empty when it is fine.
    /// The pictures are checked one at a time with [`check_picture`].
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !has_text(&self.name, MAX_PACK_NAME_CHARS) {
            problems.push(format!(
                "\"name\" must be 1 to {MAX_PACK_NAME_CHARS} characters"
            ));
        }
        if !is_github_user(&self.author) {
            problems.push(format!(
                "\"author\" must be your GitHub user name, not {:?}",
                self.author
            ));
        }
        if !LICENSES.contains(&self.license.as_str()) {
            problems.push(format!(
                "\"license\" must be one of {}",
                LICENSES.join(", ")
            ));
        }
        if self.tags.is_empty() {
            problems.push("\"tags\" needs at least one tag. The first names the pack".into());
        }
        check_tags(&self.tags, MAX_PACK_TAGS, "the pack", &mut problems);

        if self.skins.is_empty() || self.skins.len() > MAX_SKINS {
            problems.push(format!(
                "a pack has 1 to {MAX_SKINS} skins, and this one has {}",
                self.skins.len()
            ));
        }
        let mut files = std::collections::HashSet::new();
        for skin in &self.skins {
            let label = format!("skin {:?}", skin.file);
            if !is_picture_file_name(&skin.file) {
                problems.push(format!(
                    "{label}: a file name is letters, digits, dots, dashes and underscores, ending in .{}",
                    PICTURE_EXTENSIONS.join(", .")
                ));
            }
            if !files.insert(skin.file.to_ascii_lowercase()) {
                problems.push(format!("{label} is listed twice"));
            }
            if !has_text(&skin.name, MAX_SKIN_NAME_CHARS) {
                problems.push(format!(
                    "{label}: \"name\" must be 1 to {MAX_SKIN_NAME_CHARS} characters"
                ));
            }
            check_tags(&skin.tags, MAX_SKIN_TAGS, &label, &mut problems);
        }
        problems
    }

    /// The tags a skin from this pack gets: the pack's, then the skin's own.
    pub fn tags_for(&self, skin: &PackSkin) -> Vec<String> {
        clean_tags(self.tags.iter().chain(&skin.tags), MAX_TAGS)
    }
}

/// `community/index.json`: the packs the app lists, written by `folderskin-tools packs index`.
/// Each pack's preview strip sits beside it at `previews/<id>.png`.
///
/// Unlike `pack.json`, fields this version doesn't know are ignored here, which is what lets an
/// index gain fields without breaking the apps already installed: add new ones as optional, and
/// never rename or remove one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Index {
    pub version: u32,
    pub packs: Vec<IndexEntry>,
    /// Packs whose ids changed (`moved.json`), each old id to the one it has now. Left out when
    /// none has.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub moved: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IndexEntry {
    /// The pack's folder name.
    pub id: String,
    pub name: String,
    pub author: String,
    pub license: String,
    /// The pack's tags, then any its skins add.
    pub tags: Vec<String>,
    /// How many skins it has.
    pub count: usize,
    /// [`pack_hash`] of the pack when the index was written, so the app can tell that a pack it
    /// added has changed since. Empty in an index written before there was one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// When the pack was first published, in Unix seconds: the time of the commit that added its
    /// `pack.json`. Left out when nobody knows, and in an index written before there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<i64>,
    /// True for a pack the maintainer lists in `official.json`; left out for every other pack.
    #[serde(default, skip_serializing_if = "is_false")]
    pub official: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Index {
    /// Reads `index.json`. An index from a newer FolderSkin says so, instead of being called
    /// damaged because a field changed.
    pub fn parse(bytes: &[u8]) -> Result<Index, String> {
        let damaged = || "the list of community packs is damaged".to_string();
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| damaged())?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version.is_some_and(|v| v > u64::from(INDEX_VERSION)) {
            return Err("the community packs need a newer FolderSkin".into());
        }
        serde_json::from_value(value).map_err(|_| damaged())
    }
}

impl IndexEntry {
    /// The entry for `pack`, in folder `id`, whose contents hash to `hash` ([`pack_hash`]). It
    /// says nothing of when the pack was added or whether it is official: only the repository
    /// around the pack knows that.
    pub fn new(id: &str, pack: &Pack, hash: String) -> IndexEntry {
        let every: Vec<String> = pack.skins.iter().flat_map(|s| pack.tags_for(s)).collect();
        IndexEntry {
            id: id.to_string(),
            name: pack.name.trim().to_string(),
            author: pack.author.clone(),
            license: pack.license.clone(),
            tags: clean_tags(&every, usize::MAX),
            count: pack.skins.len(),
            hash,
            added: None,
            official: false,
        }
    }
}

/// A short fingerprint of a pack's exact contents: its `pack.json` bytes, then each picture's
/// file name and bytes in the order the pack lists them. Any change to a name, a tag or a
/// picture changes it. The index records it and the app keeps it with the skins it added, which
/// is how an update shows up. Sixteen hex digits of SHA-256.
pub fn pack_hash<'a>(
    manifest: &[u8],
    pictures: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    // Every part is length-prefixed, so moving bytes from one part to the next changes the hash.
    let mut part = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    part(manifest);
    for (file, bytes) in pictures {
        part(file.as_bytes());
        part(bytes);
    }
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A pack's folder name: lower-case letters and digits in words joined by single dashes, at most
/// 40 characters. It is part of every download URL, so nothing else gets through.
pub fn is_pack_id(id: &str) -> bool {
    !is_windows_device_name(id)
        && !id.is_empty()
        && id.len() <= 40
        && id.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// A pack id made from any name: `"Ukiyo-e Nights!"` becomes `"ukiyo-e-nights"`. Empty when
/// the name has no letters or digits it can keep.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out = out.trim_end_matches('-').to_string();
    out.truncate(40);
    let out = out.trim_end_matches('-').to_string();
    if is_windows_device_name(&out) {
        format!("{out}-1")
    } else {
        out
    }
}

/// The characters a generated id ends in: RFC 4648's base32 alphabet in lower case, which has
/// no `0`, `1`, `8` or `9` to mistake for a letter.
pub const ID_SUFFIX_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
/// How many of them end an id: 32⁶, about a billion ids for each name.
pub const ID_SUFFIX_LEN: usize = 6;
/// How much of a pack's name its id keeps, so the name, a dash and the suffix fit in 40.
pub const ID_BASE_MAX: usize = 40 - 1 - ID_SUFFIX_LEN;

/// A new id for a pack called `name`: the name as a slug, a dash and six random characters, such
/// as `classic-art-k7q2mx`. Names can repeat and ids never do, so an id `taken` says is in use
/// (another pack's folder, an old id in `moved.json`) is drawn again. The name part is only there
/// to make links readable: an id is fixed once given, whatever the pack is called later.
pub fn new_id(name: &str, taken: impl Fn(&str) -> bool) -> Result<String, String> {
    let mut base = slug(name);
    base.truncate(ID_BASE_MAX);
    let base = match base.trim_end_matches('-') {
        "" => "pack",
        kept => kept,
    };
    // A billion suffixes a name: a hundred draws that all collide mean `taken` says yes to
    // everything, not bad luck.
    for _ in 0..100 {
        let id = format!("{base}-{}", id_suffix()?);
        if is_generated_id(&id) && !taken(&id) {
            return Ok(id);
        }
    }
    Err(format!("no free id for {name:?}"))
}

/// Six characters of [`ID_SUFFIX_ALPHABET`], each from a byte of the system's secure random
/// source. 256 is a multiple of 32, so masking a byte picks every character equally often.
fn id_suffix() -> Result<String, String> {
    let mut bytes = [0u8; ID_SUFFIX_LEN];
    getrandom::fill(&mut bytes)
        .map_err(|e| format!("no random numbers to make an id with: {e}"))?;
    Ok(bytes
        .iter()
        .map(|b| char::from(ID_SUFFIX_ALPHABET[usize::from(b & 31)]))
        .collect())
}

/// Whether `id` has the shape [`new_id`] gives: a pack id of at least one word from a name, then
/// a last word of [`ID_SUFFIX_LEN`] characters from [`ID_SUFFIX_ALPHABET`].
pub fn is_generated_id(id: &str) -> bool {
    is_pack_id(id)
        && id.rsplit_once('-').is_some_and(|(base, suffix)| {
            !base.is_empty()
                && suffix.len() == ID_SUFFIX_LEN
                && suffix.bytes().all(|b| ID_SUFFIX_ALPHABET.contains(&b))
        })
}

/// `moved.json`: packs whose ids changed, each old id to the one it has now, so links, install
/// counts and the packs people added under an old id can follow the pack to its new one.
///
/// An old id always leads straight to a pack that exists: renaming a pack again updates every
/// entry that led to it, so nothing ever has to be followed twice.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Moved {
    pub version: u32,
    #[serde(default)]
    pub moved: BTreeMap<String, String>,
}

impl Default for Moved {
    fn default() -> Self {
        Moved {
            version: MOVED_VERSION,
            moved: BTreeMap::new(),
        }
    }
}

impl Moved {
    /// Reads `moved.json`.
    pub fn parse(bytes: &[u8]) -> Result<Moved, String> {
        let moved: Moved =
            serde_json::from_slice(bytes).map_err(|e| format!("{MOVED_FILE} isn't valid: {e}"))?;
        if moved.version != MOVED_VERSION {
            return Err(format!(
                "{MOVED_FILE} is version {}, and this FolderSkin reads version {MOVED_VERSION}",
                moved.version
            ));
        }
        Ok(moved)
    }

    /// Records that pack `old` is now `new`. Whatever led to `old` leads to `new` now, and `new`
    /// stops being an old id if it was one (a pack given back an id it had before).
    pub fn record(&mut self, old: &str, new: &str) {
        self.moved.remove(new);
        for to in self.moved.values_mut() {
            if to == old {
                *to = new.to_string();
            }
        }
        if old != new {
            self.moved.insert(old.to_string(), new.to_string());
        }
    }

    /// The id pack `id` has now: `id` itself unless it moved.
    pub fn current<'a>(&'a self, id: &'a str) -> &'a str {
        self.moved.get(id).map_or(id, String::as_str)
    }

    /// What's wrong with it, one sentence each, given the ids of the packs there are: every old
    /// id is a pack id no pack has, and every new one is a pack that exists and hasn't moved.
    pub fn problems(&self, packs: &BTreeSet<String>) -> Vec<String> {
        let mut problems = Vec::new();
        for (old, new) in &self.moved {
            if !is_pack_id(old) {
                problems.push(format!("{MOVED_FILE}: {old:?} isn't a pack id"));
            } else if packs.contains(old) {
                problems.push(format!(
                    "{MOVED_FILE}: {old} moved to {new}, but a pack still has the id {old}"
                ));
            }
            // An old id leading to another old id is its own mistake, whatever else is wrong.
            if self.moved.contains_key(new) {
                problems.push(format!(
                    "{MOVED_FILE}: {old} moved to {new}, which moved again. Point {old} at where {new} went"
                ));
            } else if !is_pack_id(new) || !packs.contains(new) {
                problems.push(format!(
                    "{MOVED_FILE}: {old} moved to {new}, and there's no pack {new}"
                ));
            }
        }
        problems
    }
}

/// Names Windows keeps for devices: `con`, `prn`, `aux`, `nul`, `com0`–`com9` and
/// `lpt0`–`lpt9`, with or without an extension and in any case. Git for Windows can't check out
/// a file or folder called that, so one in a pack would break every clone on Windows.
pub fn is_windows_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit())
}

/// A GitHub user name: letters, digits and single dashes, not at either end, at most 39.
pub fn is_github_user(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 39
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// A picture's file name in a pack: letters, digits, `.`, `-` and `_`, not starting with a dot,
/// ending in one of [`PICTURE_EXTENSIONS`]. It is part of a download URL too.
pub fn is_picture_file_name(file: &str) -> bool {
    let ext = file
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    !file.is_empty()
        && file.len() <= 64
        && !file.starts_with('.')
        && !is_windows_device_name(file)
        && file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        && PICTURE_EXTENSIONS.contains(&ext.as_str())
}

/// A tag as FolderSkin keeps it: lower case, letters, digits, spaces and dashes only, single
/// spaces, at most [`MAX_TAG_CHARS`]. `None` when nothing is left. The webview's `cleanTag`
/// follows the same rules.
pub fn clean_tag(tag: &str) -> Option<String> {
    let kept: String = tag
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    let words = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = words.chars().take(MAX_TAG_CHARS).collect();
    let cut = cut.trim_matches(|c: char| c == ' ' || c == '-');
    (!cut.is_empty()).then(|| cut.to_string())
}

/// Cleans every tag with [`clean_tag`], drops repeats and blanks, and keeps the first `max`.
pub fn clean_tags<'a>(tags: impl IntoIterator<Item = &'a String>, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        if out.len() == max {
            break;
        }
        if let Some(tag) = clean_tag(tag) {
            if !out.contains(&tag) {
                out.push(tag);
            }
        }
    }
    out
}

/// Checks one picture against the limits a pack is read with, without decoding all of it: its
/// size (within [`MAX_READ_PICTURE_BYTES`]), what it really is (PNG, JPEG or WebP, whatever its
/// name says) and its dimensions. Returns the dimensions. A picture going into a pack made or
/// shared now is held to [`check_new_picture`] as well.
pub fn check_picture(bytes: &[u8]) -> Result<(u32, u32), String> {
    within(bytes, MAX_READ_PICTURE_BYTES)?;
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "couldn't be read".to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP)
    ) {
        return Err("isn't a PNG, JPEG or WebP picture".into());
    }
    let (w, h) = reader
        .into_dimensions()
        .map_err(|_| "couldn't be read".to_string())?;
    if w > MAX_PICTURE_SIDE || h > MAX_PICTURE_SIDE {
        return Err(format!(
            "is {w}×{h} px, and the most is {MAX_PICTURE_SIDE}×{MAX_PICTURE_SIDE}"
        ));
    }
    if w.min(h) < MIN_PICTURE_SIDE {
        return Err(format!(
            "is {w}×{h} px, but each side needs at least {MIN_PICTURE_SIDE} px"
        ));
    }
    Ok((w, h))
}

/// Checks a picture going into a pack made or shared now: [`check_picture`], within
/// [`MAX_PICTURE_BYTES`], and lossless ([`is_lossless_picture`]), so a pack looks exactly as it was
/// made. Returns the dimensions.
pub fn check_new_picture(bytes: &[u8]) -> Result<(u32, u32), String> {
    within(bytes, MAX_PICTURE_BYTES)?;
    let dimensions = check_picture(bytes)?;
    if !is_lossless_picture(bytes) {
        return Err("isn't lossless: a pack's pictures are PNG or lossless WebP".into());
    }
    Ok(dimensions)
}

/// A picture made ready for a pack by [`encode_picture`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPicture {
    /// A lossless WebP, at most [`MAX_PICTURE_SIDE`] px on its longer side.
    pub webp: Vec<u8>,
    /// The side it was made smaller to, 896 or 768 px, when its file was over the size limit at
    /// 1024 px, or at its own size when that is smaller.
    pub scaled_to: Option<u32>,
}

/// `img` as a pack's picture: a lossless WebP ([`raster::encode_webp_lossless`]) at most
/// [`MAX_PICTURE_SIDE`] px on its longer side and within `max_bytes`. One whose file is over it is
/// made smaller, still lossless, to each of [`PICTURE_SIDES`] in turn, and never below
/// [`MIN_PICTURE_SIDE`]; [`EncodedPicture::scaled_to`] says when it was. The error finishes a
/// sentence that starts with the picture's name: it's too small for a pack, or too detailed to
/// fit even at the smallest side. `packs make` and the app share packs with this, so a pack
/// comes out the same whichever made it.
pub fn encode_picture(img: RgbaImage, max_bytes: usize) -> Result<EncodedPicture, String> {
    fit_picture(img, max_bytes, raster::encode_webp_lossless)
}

/// [`encode_picture`], encoding each side with `encode`. Tests pass a quicker encoder.
pub fn fit_picture(
    img: RgbaImage,
    max_bytes: usize,
    encode: impl Fn(&RgbaImage) -> Vec<u8>,
) -> Result<EncodedPicture, String> {
    let img = raster::shrink_to(img, MAX_PICTURE_SIDE);
    let big_enough = |img: &RgbaImage| check_picture_size(img.width(), img.height());
    big_enough(&img)?;
    let longest = img.width().max(img.height());
    let mut tried = longest;
    for side in PICTURE_SIDES {
        // A picture no bigger than this side was tried at its own size already.
        if side < MAX_PICTURE_SIDE && side >= longest {
            continue;
        }
        let smaller = raster::shrink_to(img.clone(), side);
        // Never made smaller than a pack allows, however big its file.
        if big_enough(&smaller).is_err() {
            break;
        }
        let webp = encode(&smaller);
        if webp.len() <= max_bytes {
            return Ok(EncodedPicture {
                webp,
                scaled_to: (side < longest).then_some(side),
            });
        }
        tried = side.min(longest);
    }
    Err(format!(
        "is too detailed to fit in {} without losing detail, even at {tried} px",
        size_label(max_bytes)
    ))
}

/// Refuses a `w`×`h` picture that is too small for a pack once [`encode_picture`] has made it
/// [`MAX_PICTURE_SIDE`] px at most: under [`MIN_PICTURE_SIDE`] px on a side. It needs only the
/// size, so a picture can be turned down before the slow part. The error finishes a sentence
/// that starts with the picture's name.
pub fn check_picture_size(w: u32, h: u32) -> Result<(), String> {
    let (w, h) = raster::shrunk_size(w, h, MAX_PICTURE_SIDE);
    if w.min(h) < MIN_PICTURE_SIDE {
        return Err(format!(
            "is {w}×{h} px, but a pack needs at least {MIN_PICTURE_SIDE} px on each side"
        ));
    }
    Ok(())
}

/// A size limit as people read one: "1.5 MB", "64 MB", "400 KB".
pub fn size_label(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    if bytes < MB {
        return format!("{} KB", bytes / 1024);
    }
    let tenths = bytes * 10 / MB;
    if tenths.is_multiple_of(10) {
        format!("{} MB", tenths / 10)
    } else {
        format!("{}.{} MB", tenths / 10, tenths % 10)
    }
}

/// Refuses a picture bigger than `max` bytes, in a sentence.
fn within(bytes: &[u8], max: usize) -> Result<(), String> {
    if bytes.len() > max {
        return Err(format!(
            "is {} KB, and the most is {} KB",
            bytes.len().div_ceil(1024),
            max / 1024
        ));
    }
    Ok(())
}

/// Whether `bytes` are a picture stored without loss: a PNG, or a WebP whose picture is lossless
/// (a `VP8L` chunk, on its own or in an extended file). A JPEG, a lossy WebP (`VP8 `, even one
/// with lossless alpha) and an animation aren't. Only the headers are read.
pub fn is_lossless_picture(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return true;
    }
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }
    // Chunks follow the header: a four-letter name, a little-endian length, the data, and a
    // padding byte after data of odd length. The extended header (VP8X), a colour profile and a
    // lossy picture's alpha (ALPH) come before the picture itself.
    let mut at = 12;
    while let Some(head) = bytes.get(at..at + 8) {
        let length = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        match &head[..4] {
            b"VP8L" => return true,
            b"VP8 " | b"ANIM" | b"ANMF" => return false,
            _ => {}
        }
        at = at.saturating_add(8 + length + (length & 1));
    }
    false
}

/// Decodes a picture that passed [`check_picture`]. The decoder is held to the same limits, so a
/// file that lies about its size in its header cannot make it allocate more.
pub fn decode_picture(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    check_picture(bytes)?;
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "couldn't be read".to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_PICTURE_SIDE);
    limits.max_image_height = Some(MAX_PICTURE_SIDE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader
        .decode()
        .map_err(|_| "couldn't be decoded".to_string())?
        .to_rgba8())
}

fn has_text(s: &str, max_chars: usize) -> bool {
    let s = s.trim();
    !s.is_empty() && s.chars().count() <= max_chars && !s.chars().any(is_hidden)
}

/// Characters that don't show but change what is shown: control characters, and the marks that
/// flip the direction of the text after them.
fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(c as u32, 0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// Adds a problem for each tag that is not already in the form [`clean_tag`] keeps, for repeats,
/// and for going over `max`.
fn check_tags(tags: &[String], max: usize, whose: &str, problems: &mut Vec<String>) {
    if tags.len() > max {
        problems.push(format!(
            "{whose} has {} tags, and the most is {max}",
            tags.len()
        ));
    }
    let mut seen = Vec::new();
    for tag in tags {
        match clean_tag(tag) {
            Some(clean) if clean == *tag => {
                if seen.contains(&clean) {
                    problems.push(format!("{whose}: the tag {tag:?} is listed twice"));
                }
                seen.push(clean);
            }
            Some(clean) => problems.push(format!(
                "{whose}: write the tag {tag:?} as {clean:?} (lower case, letters, digits, spaces and dashes)"
            )),
            None => problems.push(format!("{whose}: {tag:?} isn't a tag")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_id_is_the_name_then_six_random_characters() {
        let id = new_id("Classic Art", |_| false).unwrap();
        let (base, suffix) = id.rsplit_once('-').unwrap();
        assert_eq!(base, "classic-art");
        assert_eq!(suffix.len(), ID_SUFFIX_LEN);
        assert!(is_generated_id(&id), "{id}");
        // Two packs with one name still get two ids.
        assert_ne!(id, new_id("Classic Art", |_| false).unwrap());
    }

    #[test]
    fn a_new_id_fits_in_forty_characters_whatever_the_name() {
        let long = "The Complete Illustrated History of Every Folder Ever Made";
        let id = new_id(long, |_| false).unwrap();
        assert!(id.len() <= 40 && is_generated_id(&id), "{id}");
        assert!(id.starts_with("the-complete-illustrated-history"), "{id}");
        // A cut that lands on a dash doesn't leave two dashes in a row.
        let cut = format!("{}-b", "a".repeat(ID_BASE_MAX - 1));
        assert!(is_generated_id(&new_id(&cut, |_| false).unwrap()));
        for nameless in ["", "!!!", "東京"] {
            let id = new_id(nameless, |_| false).unwrap();
            assert!(id.starts_with("pack-") && is_generated_id(&id), "{id}");
        }
        let device = new_id("con", |_| false).unwrap();
        assert!(is_pack_id(&device) && is_generated_id(&device), "{device}");
    }

    #[test]
    fn a_new_id_is_drawn_again_while_taken_and_gives_up_on_a_rigged_check() {
        let seen = std::cell::Cell::new(0);
        let id = new_id("Colours", |_| {
            seen.set(seen.get() + 1);
            seen.get() <= 3
        })
        .unwrap();
        assert_eq!(seen.get(), 4);
        assert!(is_generated_id(&id));
        assert!(new_id("Colours", |_| true).is_err());
    }

    #[test]
    fn every_suffix_character_comes_from_the_alphabet() {
        let mut seen = BTreeSet::new();
        for _ in 0..200 {
            let id = new_id("x", |_| false).unwrap();
            seen.extend(id.rsplit_once('-').unwrap().1.bytes());
        }
        assert!(seen.iter().all(|b| ID_SUFFIX_ALPHABET.contains(b)));
        // 1,200 draws from 32 characters: all of them turn up (missing one is a ~1e-15 chance).
        assert_eq!(seen.len(), 32);
    }

    #[test]
    fn only_the_generated_shape_counts_as_a_generated_id() {
        for id in [
            "classic-art-k7q2mx",
            "pack-aaaaaa",
            "a-222222",
            "x1-y2-zzzzzz",
        ] {
            assert!(is_generated_id(id), "{id}");
        }
        for id in [
            "classic-art",
            "k7q2mx",
            "-k7q2mx",
            "classic-art-k7q2m",
            "classic-art-k7q2mxa",
            "classic-art-k7q2m1",
            "classic-art-k7q2m8",
            "classic-art-K7Q2MX",
            "classic--art-k7q2mx",
            &format!("{}-k7q2mx", "a".repeat(34)),
        ] {
            assert!(!is_generated_id(id), "{id}");
        }
    }

    #[test]
    fn moved_ids_lead_straight_to_the_pack_however_often_it_moves() {
        let mut moved = Moved::default();
        moved.record("classic-art", "classic-art-k7q2mx");
        moved.record("classic-art-k7q2mx", "old-masters-a2b3c4");
        assert_eq!(moved.current("classic-art"), "old-masters-a2b3c4");
        assert_eq!(moved.current("classic-art-k7q2mx"), "old-masters-a2b3c4");
        assert_eq!(moved.current("colours"), "colours");
        // Given back its first id, the pack stops being moved away from it.
        moved.record("old-masters-a2b3c4", "classic-art");
        assert_eq!(moved.current("classic-art"), "classic-art");
        assert_eq!(moved.current("classic-art-k7q2mx"), "classic-art");
        assert!(!moved.moved.contains_key("classic-art"));
        let packs = BTreeSet::from(["classic-art".to_string()]);
        assert_eq!(moved.problems(&packs), Vec::<String>::new());
    }

    #[test]
    fn moved_json_reads_back_and_says_what_is_wrong_with_it() {
        let moved = Moved::parse(br#"{ "version": 1, "moved": { "a": "b-k7q2mx" } }"#).unwrap();
        assert_eq!(moved.current("a"), "b-k7q2mx");
        assert!(Moved::parse(br#"{ "version": 2, "moved": {} }"#).is_err());
        assert!(Moved::parse(br#"{ "version": 1, "moved": {}, "note": "x" }"#).is_err());

        let packs = BTreeSet::from(["a".to_string(), "c-k7q2mx".to_string()]);
        let broken = Moved {
            version: 1,
            moved: BTreeMap::from([
                ("a".into(), "c-k7q2mx".into()),
                ("Not An Id".into(), "c-k7q2mx".into()),
                ("gone".into(), "nowhere-k7q2mx".into()),
                ("x".into(), "gone".into()),
            ]),
        };
        let problems = broken.problems(&packs).join("\n");
        assert!(problems.contains("a pack still has the id a"), "{problems}");
        assert!(
            problems.contains("\"Not An Id\" isn't a pack id"),
            "{problems}"
        );
        assert!(
            problems.contains("there's no pack nowhere-k7q2mx"),
            "{problems}"
        );
        assert!(problems.contains("which moved again"), "{problems}");
    }

    fn pack_json(extra_skins: usize) -> String {
        let mut skins = vec![
            r#"{ "file": "koi.png", "name": "Koi over the wave", "tags": ["animals"] }"#
                .to_string(),
        ];
        for i in 0..extra_skins {
            skins.push(format!(r#"{{ "file": "s{i}.jpg", "name": "Skin {i}" }}"#));
        }
        format!(
            r#"{{
  "version": 1,
  "name": "Ukiyo-e nights",
  "author": "prajwal-svm",
  "license": "CC-BY-4.0",
  "tags": ["woodblock", "japan"],
  "skins": [{}]
}}"#,
            skins.join(",")
        )
    }

    #[test]
    fn a_good_pack_parses_and_its_skins_get_the_pack_tags_first() {
        let pack = Pack::parse(pack_json(1).as_bytes()).unwrap();
        assert_eq!(pack.skins.len(), 2);
        assert_eq!(
            pack.tags_for(&pack.skins[0]),
            ["woodblock", "japan", "animals"]
        );
        assert_eq!(pack.tags_for(&pack.skins[1]), ["woodblock", "japan"]);
        let entry = IndexEntry::new("ukiyo-e-nights", &pack, "0123456789abcdef".into());
        assert_eq!(entry.tags, ["woodblock", "japan", "animals"]);
        assert_eq!(entry.count, 2);
        assert_eq!(entry.hash, "0123456789abcdef");
    }

    #[test]
    fn a_pack_hash_changes_with_any_part_of_the_pack() {
        let base = pack_hash(b"{}", [("a.png", &b"one"[..]), ("b.png", &b"two"[..])]);
        assert_eq!(base.len(), 16);
        assert!(base.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(
            base,
            pack_hash(b"{}", [("a.png", &b"one"[..]), ("b.png", &b"two"[..])]),
            "the same pack hashes the same"
        );
        for other in [
            pack_hash(b"{ }", [("a.png", &b"one"[..]), ("b.png", &b"two"[..])]),
            pack_hash(b"{}", [("a.png", &b"one"[..]), ("b.png", &b"tw0"[..])]),
            pack_hash(b"{}", [("a.png", &b"one"[..]), ("c.png", &b"two"[..])]),
            pack_hash(b"{}", [("b.png", &b"two"[..]), ("a.png", &b"one"[..])]),
            pack_hash(b"{}", [("a.png", &b"onet"[..]), ("b.png", &b"wo"[..])]),
        ] {
            assert_ne!(base, other);
        }
    }

    #[test]
    fn an_index_from_before_hashes_still_reads() {
        let old = r#"{ "version": 1, "packs": [ { "id": "colours", "name": "Colours",
  "author": "prajwal-svm", "license": "CC0-1.0", "tags": ["colour"], "count": 8 } ] }"#;
        let index = Index::parse(old.as_bytes()).unwrap();
        let entry = &index.packs[0];
        assert_eq!(
            (entry.hash.as_str(), entry.added, entry.official),
            ("", None, false)
        );
        let json = serde_json::to_string(entry).unwrap();
        for unsaid in ["hash", "added", "official"] {
            assert!(
                !json.contains(unsaid),
                "no {unsaid} is written when there is none: {json}"
            );
        }
    }

    #[test]
    fn an_index_says_when_a_pack_was_added_and_whether_it_is_official() {
        let index = r#"{ "version": 1, "packs": [ { "id": "classic-art", "name": "Classic Art",
  "author": "prajwal-svm", "license": "CC0-1.0", "tags": ["classic art"], "count": 16,
  "hash": "4edf8c11d48ab779", "added": 1790000000, "official": true } ] }"#;
        let entry = &Index::parse(index.as_bytes()).unwrap().packs[0];
        assert_eq!((entry.added, entry.official), (Some(1_790_000_000), true));
        let json = serde_json::to_string(entry).unwrap();
        assert!(
            json.ends_with(r#""hash":"4edf8c11d48ab779","added":1790000000,"official":true}"#),
            "{json}"
        );
        // Fields this version doesn't know are passed over, as 0.1.4 and 0.1.5 pass over these.
        let later = index.replace(
            r#""official": true"#,
            r#""official": true, "downloads": 12"#,
        );
        assert_eq!(&Index::parse(later.as_bytes()).unwrap().packs[0], entry);
    }

    #[test]
    fn a_pack_holds_one_to_fifty_skins() {
        assert!(Pack::parse(pack_json(MAX_SKINS - 1).as_bytes()).is_ok());
        let too_many = Pack::parse(pack_json(MAX_SKINS).as_bytes()).unwrap_err();
        assert!(too_many[0].contains("1 to 50 skins"), "{too_many:?}");
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let text = r#"{
  "version": 1, "name": "", "author": "not a user", "license": "GPL-3.0", "tags": ["Anime"],
  "skins": [ { "file": "../x.png", "name": "X" }, { "file": "a.gif", "name": "A" } ]
}"#;
        let problems = Pack::parse(text.as_bytes()).unwrap_err();
        let all = problems.join("\n");
        for needle in [
            "\"name\"",
            "GitHub user name",
            "\"license\"",
            "as \"anime\"",
            "\"../x.png\"",
            "\"a.gif\"",
        ] {
            assert!(all.contains(needle), "missing {needle:?} in:\n{all}");
        }
    }

    #[test]
    fn a_newer_index_says_so_instead_of_damaged() {
        let newer = r#"{ "version": 9, "packs": "something new" }"#;
        assert!(Index::parse(newer.as_bytes())
            .unwrap_err()
            .contains("newer FolderSkin"));
        assert!(Index::parse(b"not json").unwrap_err().contains("damaged"));
        let ok = r#"{ "version": 1, "packs": [] }"#;
        assert!(Index::parse(ok.as_bytes()).unwrap().packs.is_empty());
    }

    #[test]
    fn names_with_hidden_direction_marks_are_refused() {
        let flip = char::from_u32(0x202E).unwrap();
        let text = pack_json(0).replace("Ukiyo-e nights", &format!("Night{flip}gnp.exe"));
        assert!(Pack::parse(text.as_bytes()).unwrap_err()[0].contains("\"name\""));
    }

    #[test]
    fn versions_and_unknown_fields_are_explained() {
        let newer = r#"{ "version": 2, "whatever": true }"#;
        assert!(Pack::parse(newer.as_bytes()).unwrap_err()[0].contains("newer FolderSkin"));
        let missing = r#"{ "name": "x" }"#;
        assert!(Pack::parse(missing.as_bytes()).unwrap_err()[0].contains("\"version\": 1"));
        let typo = pack_json(0).replace("\"tags\": [\"woodblock\"", "\"tag\": [\"woodblock\"");
        assert!(Pack::parse(typo.as_bytes()).unwrap_err()[0].contains("unknown field"));
    }

    #[test]
    fn tags_are_cleaned_the_same_way_everywhere() {
        assert_eq!(clean_tag("  Art   Nouveau "), Some("art nouveau".into()));
        assert_eq!(clean_tag("#Anime!"), Some("anime".into()));
        assert_eq!(clean_tag("ukiyo-e"), Some("ukiyo-e".into()));
        assert_eq!(clean_tag("- -"), None);
        assert_eq!(clean_tag("桜 Sakura"), Some("桜 sakura".into()));
        assert_eq!(clean_tag(&"a".repeat(40)).unwrap().len(), MAX_TAG_CHARS);
        let tags: Vec<String> = ["Glow", "glow", "", "Night"].map(String::from).to_vec();
        assert_eq!(clean_tags(&tags, 8), ["glow", "night"]);
        assert_eq!(clean_tags(&tags, 1), ["glow"]);
    }

    #[test]
    fn ids_file_names_and_users_cannot_escape_their_url() {
        assert!(is_pack_id("ukiyo-e-nights"));
        for bad in ["", "-a", "a-", "a--b", "A", "a/b", "a.b", &"a".repeat(41)] {
            assert!(!is_pack_id(bad), "{bad:?}");
        }
        assert!(is_picture_file_name("Koi_01.webp"));
        for bad in [
            "../koi.png",
            ".koi.png",
            "koi.gif",
            "k oi.png",
            "koi",
            "a/b.png",
        ] {
            assert!(!is_picture_file_name(bad), "{bad:?}");
        }
        assert!(is_github_user("prajwal-svm"));
        for bad in ["", "-x", "x-", "a--b", "a b", &"a".repeat(40)] {
            assert!(!is_github_user(bad), "{bad:?}");
        }
        assert_eq!(slug("Ukiyo-e Nights!"), "ukiyo-e-nights");
        // Windows keeps these names for devices, so git there couldn't check them out.
        for bad in ["con", "aux", "nul", "com1", "lpt9"] {
            assert!(!is_pack_id(bad), "{bad:?}");
        }
        for bad in ["aux.png", "CON.jpg", "Com1.webp", "nul.jpeg"] {
            assert!(!is_picture_file_name(bad), "{bad:?}");
        }
        assert!(is_picture_file_name("console.png") && is_pack_id("auxiliary"));
        assert_eq!(slug("Con"), "con-1");
        assert_eq!(slug("  ***  "), "");
        assert!(is_pack_id(&slug(&"Long name ".repeat(10))));
    }

    #[test]
    fn pictures_are_held_to_the_limits() {
        let png = |w: u32, h: u32| {
            let mut bytes = Vec::new();
            image::RgbaImage::new(w, h)
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .unwrap();
            bytes
        };
        assert_eq!(check_picture(&png(1024, 958)), Ok((1024, 958)));
        assert!(check_picture(&png(1025, 900))
            .unwrap_err()
            .contains("the most is"));
        assert!(check_picture(&png(512, 200))
            .unwrap_err()
            .contains("at least"));
        assert!(check_picture(b"GIF89a....").is_err());
        assert_eq!(
            check_picture(&vec![0; MAX_READ_PICTURE_BYTES + 1]).unwrap_err(),
            "is 2049 KB, and the most is 2048 KB"
        );
        assert_eq!(
            decode_picture(&png(300, 300)).unwrap().dimensions(),
            (300, 300)
        );
    }

    /// A WebP file of `chunks`, each a name and its data, as RIFF lays them out.
    fn riff(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for (name, data) in chunks {
            body.extend_from_slice(*name);
            body.extend_from_slice(&(data.len() as u32).to_le_bytes());
            body.extend_from_slice(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        [
            b"RIFF".as_slice(),
            &(body.len() as u32).to_le_bytes(),
            &body,
        ]
        .concat()
    }

    #[test]
    fn only_png_and_lossless_webp_are_lossless() {
        let opaque = image::RgbaImage::from_fn(300, 260, |x, y| {
            image::Rgba([x as u8, y as u8, (x * y) as u8, 255])
        });
        let cutout = image::RgbaImage::from_fn(300, 260, |x, _| {
            image::Rgba([200, 90, 40, if x < 30 { 0 } else { 255 }])
        });
        let lossless = crate::raster::encode_webp_lossless(&opaque);
        assert!(is_lossless_picture(&crate::raster::encode_png(&opaque)));
        assert!(is_lossless_picture(&lossless));
        assert!(is_lossless_picture(&crate::raster::encode_webp_lossless(
            &cutout
        )));
        // A lossless picture in an extended file, after a colour profile.
        let vp8l = &lossless[20..];
        let extended = riff(&[
            (b"VP8X", &[0x20, 0, 0, 0, 43, 1, 0, 3, 1, 0]),
            (b"ICCP", b"odd"),
            (b"VP8L", vp8l),
        ]);
        assert!(is_lossless_picture(&extended));

        // Lossy, with or without lossless alpha beside it.
        assert!(!is_lossless_picture(&crate::raster::encode_webp_lossy(
            &opaque, 90.0
        )));
        let lossy_alpha = crate::raster::encode_webp_lossy(&cutout, 90.0);
        assert_eq!(&lossy_alpha[12..16], b"VP8X");
        assert!(!is_lossless_picture(&lossy_alpha));
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgba8(opaque)
            .into_rgb8()
            .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        assert!(!is_lossless_picture(&jpeg));
        // An animation, and files that aren't whole.
        assert!(!is_lossless_picture(&riff(&[
            (b"VP8X", &[2, 0, 0, 0, 43, 1, 0, 3, 1, 0]),
            (b"ANIM", &[0; 6]),
            (b"ANMF", vp8l),
        ])));
        assert!(!is_lossless_picture(&lossless[..16]));
        assert!(!is_lossless_picture(
            b"RIFF\xff\xff\xff\xffWEBPVP8X\xff\xff\xff\xff"
        ));
        assert!(!is_lossless_picture(b""));
    }

    #[test]
    fn a_new_picture_is_lossless_and_within_the_new_limit() {
        let img = image::RgbaImage::from_fn(512, 480, |x, y| {
            image::Rgba([(x / 2) as u8, (y / 2) as u8, 90, 255])
        });
        let webp = crate::raster::encode_webp_lossless(&img);
        assert_eq!(check_new_picture(&webp), Ok((512, 480)));
        assert_eq!(
            check_new_picture(&crate::raster::encode_png(&img)),
            Ok((512, 480))
        );
        assert_eq!(
            check_new_picture(&crate::raster::encode_webp_lossy(&img, 90.0)).unwrap_err(),
            "isn't lossless: a pack's pictures are PNG or lossless WebP"
        );
        // Bigger than 1.5 MB is too big for a new pack, though a published one is read up to 2 MB.
        let big = vec![0; MAX_PICTURE_BYTES + 1];
        assert_eq!(
            check_new_picture(&big).unwrap_err(),
            "is 1537 KB, and the most is 1536 KB"
        );
        assert_eq!(
            check_picture(&big).unwrap_err(),
            "isn't a PNG, JPEG or WebP picture",
            "within the size a published picture can be"
        );
        assert_eq!(MAX_PACK_BYTES, 67_108_864, "64 MB");
    }

    /// Grain, as a photo has, which a smaller picture averages away, so its file is smaller too.
    fn grain(w: u32, h: u32) -> RgbaImage {
        let mut state = 0x9e37_79b9_u32;
        RgbaImage::from_fn(w, h, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let [r, g, b, _] = state.to_le_bytes();
            image::Rgba([r, g, b, 255])
        })
    }

    #[test]
    fn a_picture_is_shrunk_to_1024_px_and_kept_lossless() {
        let img = RgbaImage::from_fn(1600, 1200, |x, y| {
            image::Rgba([(x / 7) as u8, (y / 5) as u8, ((x + y) / 11) as u8, 255])
        });
        let made = encode_picture(img.clone(), MAX_PICTURE_BYTES).unwrap();
        assert_eq!(
            made.scaled_to, None,
            "1024 px is where every picture starts"
        );
        assert!(is_lossless_picture(&made.webp));
        let back = image::load_from_memory(&made.webp).unwrap().to_rgba8();
        assert_eq!(
            back,
            raster::shrink_to(img, MAX_PICTURE_SIDE),
            "every pixel"
        );
        assert_eq!(back.dimensions(), (1024, 768));

        // Too small for a pack, before anything is encoded, and told from its size alone.
        assert_eq!(
            encode_picture(RgbaImage::new(1000, 200), MAX_PICTURE_BYTES).unwrap_err(),
            "is 1000×200 px, but a pack needs at least 256 px on each side"
        );
        assert_eq!(
            check_picture_size(4000, 900).unwrap_err(),
            "is 1024×230 px, but a pack needs at least 256 px on each side",
            "measured as it would be made"
        );
        assert_eq!(check_picture_size(4000, 1000), Ok(()));
    }

    #[test]
    fn a_picture_over_the_limit_is_made_smaller_still_lossless_or_refused() {
        // The quick encoder keeps this test quick; its files are lossless WebP all the same.
        let img = grain(1000, 700);
        let quick = raster::encode_webp_lossless_quick;
        let size_at = |side| quick(&raster::shrink_to(img.clone(), side)).len();
        let (at_1000, at_896, at_768) = (size_at(1000), size_at(896), size_at(768));
        assert!(at_768 < at_896 && at_896 < at_1000);

        // Over the limit at its own size, so it steps down, and says to what.
        let fit = |max| fit_picture(img.clone(), max, quick);
        let made = fit(at_896).unwrap();
        assert_eq!((made.webp.len(), made.scaled_to), (at_896, Some(896)));
        let made = fit(at_768).unwrap();
        assert_eq!((made.webp.len(), made.scaled_to), (at_768, Some(768)));
        let back = image::load_from_memory(&made.webp).unwrap().to_rgba8();
        assert_eq!(
            back,
            raster::shrink_to(img.clone(), 768),
            "still every pixel"
        );

        // Within the limit, it's left as it is.
        assert_eq!(fit(at_1000).unwrap().scaled_to, None);

        // Not even 768 px fits.
        assert_eq!(
            fit(at_768 - 1).unwrap_err(),
            format!(
                "is too detailed to fit in {} without losing detail, even at 768 px",
                size_label(at_768 - 1)
            )
        );
        // A wide picture stops at 896 px: at 768 it would be less than 256 px high.
        let wide = RgbaImage::from_fn(1000, 300, |x, y| *img.get_pixel(x, y));
        assert_eq!(
            fit_picture(wide, 1024, quick).unwrap_err(),
            "is too detailed to fit in 1 KB without losing detail, even at 896 px"
        );
    }

    #[test]
    fn size_limits_read_as_people_say_them() {
        assert_eq!(size_label(MAX_PICTURE_BYTES), "1.5 MB");
        assert_eq!(size_label(MAX_READ_PICTURE_BYTES), "2 MB");
        assert_eq!(size_label(MAX_PACK_BYTES), "64 MB");
        assert_eq!(size_label(400 * 1024), "400 KB");
    }
}
