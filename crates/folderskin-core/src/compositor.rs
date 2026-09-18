//! Compositor: renders the folder template with cover-fitted artwork.
//!
//! The types below are the interface other modules and crates rely on; keep their names and
//! signatures stable.

/// Source artwork plus the focus point (0..1, 0..1) that cover-fit crops keep centred.
pub struct Artwork {
    pub rgba: image::RgbaImage,
    pub focus: (f32, f32),
}

/// Rendered icon at several sizes (straight alpha RGBA).
pub struct IconSet {
    pub sizes: Vec<(u32, image::RgbaImage)>,
}

impl IconSet {
    /// PNG bytes for one size, if that size was rendered.
    pub fn png(&self, size: u32) -> Option<Vec<u8>> {
        self.sizes
            .iter()
            .find(|(s, _)| *s == size)
            .map(|(_, img)| crate::raster::encode_png(img))
    }
}

/// Master render size; every smaller size is a Lanczos3 downsample of this.
pub const RENDER_SIZE: u32 = 2048;
/// Sizes the app renders for an applied icon.
pub const ICON_SIZES: [u32; 10] = [2048, 1024, 512, 256, 128, 64, 48, 32, 24, 16];
