//! Embeds the built-in skins into the binary: every skin listed in `assets/skins/manifest.json`,
//! and every pack under `assets/packs/`.
//!
//! Generates `$OUT_DIR/skins_gen.rs` with a `SKINS` table of `BuiltinSkin` values and a `PACKS`
//! table of `BuiltinPack` values, each carrying its image bytes via `include_bytes!`. A built-in
//! pack follows the community pack contract (`folderskin_core::pack`), which
//! `folderskin-tools packs check --dir assets` enforces; this script only reads it.

use std::path::{Path, PathBuf};

fn main() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    let mut out = skins(&assets.join("skins"));
    out += &packs(&assets.join("packs"));
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out_dir).join("skins_gen.rs"), out).expect("write skins_gen.rs");
    tauri_build::build()
}

/// The `SKINS` table, from `assets/skins/manifest.json`.
fn skins(root: &Path) -> String {
    let manifest_path = root.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let text = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));
    let manifest: serde_json::Value =
        serde_json::from_str(&text).expect("manifest.json is valid JSON");
    let mut out = String::from("pub static SKINS: &[BuiltinSkin] = &[\n");
    for s in manifest["skins"]
        .as_array()
        .expect("manifest.skins is an array")
    {
        let file = embedded(&root.join(s["file"].as_str().expect("skin.file")));
        let focus = s
            .get("focus")
            .and_then(|f| f.as_array())
            .map(|f| {
                [
                    f[0].as_f64().unwrap_or(0.5) as f32,
                    f[1].as_f64().unwrap_or(0.5) as f32,
                ]
            })
            .unwrap_or([0.5, 0.5]);
        let collection = s["collection"].as_str().expect("skin.collection");
        // A built-in skin's tags are its manifest "tags", or else its collection.
        let tags: Vec<&str> = s
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|t| t.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| vec![collection]);
        out += &format!(
            "    BuiltinSkin {{ id: {:?}, name: {:?}, collection: {:?}, tags: &{:?}, focus: [{:?}, {:?}], bytes: include_bytes!({:?}) }},\n",
            s["id"].as_str().expect("skin.id"),
            s["name"].as_str().expect("skin.name"),
            collection,
            tags,
            focus[0],
            focus[1],
            file
        );
    }
    out + "];\n"
}

/// The `PACKS` table: one entry per folder of `assets/packs`, in name order. Each skin's id is
/// `<pack>/<file name without its extension>`, which no manifest skin can have.
fn packs(root: &Path) -> String {
    // A directory here makes Cargo rerun this script when anything inside it changes, so a new
    // pack, picture or pack.json is picked up.
    println!("cargo:rerun-if-changed={}", root.display());
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    let mut out = String::from("pub static PACKS: &[BuiltinPack] = &[\n");
    for dir in dirs {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a pack folder has a UTF-8 name");
        let manifest_path = dir.join("pack.json");
        let text = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));
        let pack: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{} isn't valid JSON: {e}", manifest_path.display()));
        let text_of = |v: &serde_json::Value, key: &str| -> String {
            v[key]
                .as_str()
                .unwrap_or_else(|| panic!("{}: \"{key}\" is missing", manifest_path.display()))
                .to_string()
        };
        let tags_of = |v: &serde_json::Value| -> Vec<String> {
            v["tags"]
                .as_array()
                .map(|t| {
                    t.iter()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        out += &format!(
            "    BuiltinPack {{ id: {id:?}, name: {:?}, author: {:?}, license: {:?}, tags: &{:?}, skins: &[\n",
            text_of(&pack, "name"),
            text_of(&pack, "author"),
            text_of(&pack, "license"),
            tags_of(&pack),
        );
        for skin in pack["skins"].as_array().expect("pack.skins is an array") {
            let file = text_of(skin, "file");
            let stem = Path::new(&file)
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("a picture has a name");
            out += &format!(
                "        BuiltinPackSkin {{ id: {:?}, name: {:?}, tags: &{:?}, bytes: include_bytes!({:?}) }},\n",
                format!("{id}/{stem}"),
                text_of(skin, "name"),
                tags_of(skin),
                embedded(&dir.join(&file)),
            );
        }
        out += "    ] },\n";
    }
    out + "];\n"
}

/// The absolute path `include_bytes!` needs, after making Cargo watch the file.
fn embedded(file: &Path) -> String {
    let file = file
        .canonicalize()
        .unwrap_or_else(|e| panic!("skin file {} not found: {e}", file.display()));
    println!("cargo:rerun-if-changed={}", file.display());
    file.to_str().expect("utf-8 path").to_string()
}
