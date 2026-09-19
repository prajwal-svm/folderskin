//! The layers the composer draws a design between, as the files `composer-layers` writes and
//! `docs/images/composer/` keeps.

use folderskin_core::compositor::template_layers;
use image::RgbaImage;

/// Each layer's file name and picture at `size` px, bottom to top the way the composer stacks
/// them around a design: the design masked by `back.png`, then `middle.png`, then the design
/// masked by `front.png`, then `top.png`, with `outline.png` last, for showing the folder's edges.
pub fn layer_files(size: u32) -> [(&'static str, RgbaImage); 5] {
    let layers = template_layers(size);
    [
        ("back.png", layers.back),
        ("middle.png", layers.middle),
        ("front.png", layers.front),
        ("top.png", layers.top),
        ("outline.png", layers.outline),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pictures in the docs are the layers the compositor draws now, pixel for pixel, so they
    /// can't go stale without this failing.
    #[test]
    fn the_layers_in_the_docs_are_the_ones_the_compositor_draws() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/images/composer/");
        for (file, want) in layer_files(1024) {
            let path = format!("{dir}{file}");
            let got = image::open(&path)
                .unwrap_or_else(|e| panic!("couldn't read {path}: {e}"))
                .to_rgba8();
            assert_eq!(got.dimensions(), want.dimensions(), "{file}");
            let differ = got
                .pixels()
                .zip(want.pixels())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                differ, 0,
                "{differ} pixels of {file} differ from the compositor's; write them again with \
                 `cargo run -p folderskin-tools -- composer-layers --out docs/images/composer`"
            );
        }
    }
}
