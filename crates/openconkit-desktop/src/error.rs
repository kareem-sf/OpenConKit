//! Error type for the desktop host, serializable across the IPC boundary.

use serde::Serialize;

/// Errors returned by Tauri commands.
#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
    /// `OPENCONKIT_HOME` was set but empty.
    #[error("OPENCONKIT_HOME is set but empty")]
    HomeOverrideEmpty,

    /// No home directory could be determined from the environment.
    #[error("could not determine the user home directory")]
    HomeNotFound,
}

// Tauri requires command errors to be serializable to the frontend.
impl Serialize for DesktopError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
