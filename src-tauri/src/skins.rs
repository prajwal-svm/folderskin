//! Built-in skins embedded at compile time (see `build.rs`): the manifest skins in
//! `assets/skins`, and the packs in `assets/packs`.

/// One shipped skin: metadata from the manifest plus the encoded image bytes.
pub struct BuiltinSkin {
    pub id: &'static str,
    pub name: &'static str,
    pub collection: &'static str,
    /// What the gallery filters it by: the manifest's tags, or its collection.
    pub tags: &'static [&'static str],
    pub focus: [f32; 2],
    pub bytes: &'static [u8],
}

/// A shipped pack: a folder of `assets/packs` with its `pack.json`, the same contract as a
/// community pack.
pub struct BuiltinPack {
    pub id: &'static str,
    pub name: &'static str,
    /// The GitHub name of whoever made it.
    pub author: &'static str,
    pub license: &'static str,
    /// The pack's tags; the first one names it.
    pub tags: &'static [&'static str],
    pub skins: &'static [BuiltinPackSkin],
}

/// One skin of a shipped pack. Whether it is a finished folder or artwork for the template is
/// decided when it is decoded, the way a community pack's pictures are.
pub struct BuiltinPackSkin {
    /// `<pack>/<file name without its extension>`.
    pub id: &'static str,
    pub name: &'static str,
    /// The skin's own tags, which come after the pack's.
    pub tags: &'static [&'static str],
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/skins_gen.rs"));

/// True for the id of any skin that ships with the app.
pub fn is_builtin(id: &str) -> bool {
    SKINS.iter().any(|s| s.id == id) || PACKS.iter().any(|p| p.skins.iter().any(|s| s.id == id))
}
