//! Embeds every skin listed in `assets/skins/manifest.json` into the binary.
//!
//! Generates `$OUT_DIR/skins_gen.rs` with a `SKINS` table of `BuiltinSkin` values, each
//! carrying its image bytes via `include_bytes!`.

use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/skins");
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
        let file = root.join(s["file"].as_str().expect("skin.file"));
        let file = file
            .canonicalize()
            .unwrap_or_else(|e| panic!("skin file {} not found: {e}", file.display()));
        println!("cargo:rerun-if-changed={}", file.display());
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
        out += &format!(
            "    BuiltinSkin {{ id: {:?}, name: {:?}, collection: {:?}, focus: [{:?}, {:?}], bytes: include_bytes!({:?}) }},\n",
            s["id"].as_str().expect("skin.id"),
            s["name"].as_str().expect("skin.name"),
            s["collection"].as_str().expect("skin.collection"),
            focus[0],
            focus[1],
            file.to_str().expect("utf-8 path")
        );
    }
    out += "];\n";
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out_dir).join("skins_gen.rs"), out).expect("write skins_gen.rs");
    tauri_build::build()
}
