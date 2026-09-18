//! Folder path validation — implemented by the apply task.

use super::ApplyError;
use std::path::{Path, PathBuf};

/// Placeholder until the apply task lands; accepts any existing directory.
pub fn validate_folder(path: &Path) -> Result<PathBuf, ApplyError> {
    if path.is_dir() { Ok(path.to_path_buf()) } else { Err(ApplyError::NotADirectory(path.to_path_buf())) }
}
