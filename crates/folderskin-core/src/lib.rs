//! FolderSkin core: folder geometry, compositor, icon containers, and per-OS icon writers.
//!
//! Every icon the app shows or applies is produced by [`compositor::render_icon_set`], so the
//! gallery preview, the drop-zone preview and the icon written to disk are the same pixels.

pub mod adjust;
pub mod apply;
pub mod compositor;
pub mod fit;
pub mod geometry;
pub mod ico;
pub mod matte;
pub mod pack;
pub mod painted;
pub mod raster;
