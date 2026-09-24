//! The layers the composer draws a design between, as the files `composer-layers` writes and
//! `docs/images/composer/` keeps: FolderSkin's own folder at the top, Windows' in `windows/`.

use folderskin_core::compositor::{template_layers_in, Style};
use image::RgbaImage;

/// Where a style's layers go, under the folder `composer-layers` writes into.
pub fn style_dir(style: Style) -> &'static str {
    match style {
        Style::Mac => "",
        Style::Windows => "windows/",
    }
}

/// Each layer's file name and picture at `size` px, bottom to top the way the composer stacks
/// them around a design: the design masked by `back.png`, then `middle.png`, then the design
/// masked by `front.png`, then `top.png`, with `outline.png` last, for showing the folder's edges.
pub fn layer_files(size: u32, style: Style) -> [(&'static str, RgbaImage); 5] {
    let layers = template_layers_in(size, style);
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
        for style in [Style::Mac, Style::Windows] {
            let dir = format!(
                "{}/../../docs/images/composer/{}",
                env!("CARGO_MANIFEST_DIR"),
                style_dir(style)
            );
            for (file, want) in layer_files(1024, style) {
                let path = format!("{dir}{file}");
                let got = image::open(&path)
                    .unwrap_or_else(|e| panic!("couldn't read {path}: {e}"))
                    .to_rgba8();
                assert_eq!(got.dimensions(), want.dimensions(), "{path}");
                let differ = got
                    .pixels()
                    .zip(want.pixels())
                    .filter(|(a, b)| a != b)
                    .count();
                assert_eq!(
                    differ, 0,
                    "{differ} pixels of {path} differ from the compositor's; write them again with \
                     `cargo run -p folderskin-tools -- composer-layers --out docs/images/composer`"
                );
            }
        }
    }
}
