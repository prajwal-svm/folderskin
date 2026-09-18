//! Apply / revert a folder icon on the current OS.

use std::path::{Path, PathBuf};

pub mod paths;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod windows;

/// User-facing failure reasons (the `Display` text is shown in the UI as-is).
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("{0} is not a folder")]
    NotADirectory(PathBuf),
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Platform(String),
}

pub use paths::validate_folder;

/// Applies the rendered icon set to `folder` using the current OS mechanism.
pub fn apply_icon(folder: &Path, icons: &crate::compositor::IconSet) -> Result<(), ApplyError> {
    let folder = validate_folder(folder)?;
    #[cfg(target_os = "macos")]
    {
        macos::apply(&folder, icons)
    }
    #[cfg(target_os = "windows")]
    {
        windows::apply(&folder, icons)
    }
    #[cfg(target_os = "linux")]
    {
        linux::apply(&folder, icons)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (folder, icons);
        Err(ApplyError::Platform("custom folder icons are not supported on this OS".into()))
    }
}

/// Restores the OS default icon for `folder`.
pub fn revert_icon(folder: &Path) -> Result<(), ApplyError> {
    let folder = validate_folder(folder)?;
    #[cfg(target_os = "macos")]
    {
        macos::revert(&folder)
    }
    #[cfg(target_os = "windows")]
    {
        windows::revert(&folder)
    }
    #[cfg(target_os = "linux")]
    {
        linux::revert(&folder)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = folder;
        Err(ApplyError::Platform("custom folder icons are not supported on this OS".into()))
    }
}
