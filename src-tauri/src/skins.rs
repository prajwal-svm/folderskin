//! Built-in skins embedded at compile time (see `build.rs`).

/// One shipped skin: metadata from the manifest plus the encoded image bytes.
pub struct BuiltinSkin {
    pub id: &'static str,
    pub name: &'static str,
    pub collection: &'static str,
    pub focus: [f32; 2],
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/skins_gen.rs"));
