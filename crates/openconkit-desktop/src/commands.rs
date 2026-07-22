//! Tauri commands exposed to the frontend.
//!
//! IPC surface is kept minimal and explicit; every command is listed in
//! `invoke_handler!` and validated by the capabilities file
//! (`capabilities/default.json`). See `docs/threat-model.md`.

use crate::error::DesktopError;

/// Application version, matching the root `VERSION` file.
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Resolved application home directory.
///
/// Precedence: `OPENCONKIT_HOME` (dev/test override only), then
/// `%USERPROFILE%\.openconkit` on Windows or `$HOME/.openconkit` elsewhere.
#[tauri::command]
pub fn openconkit_home() -> Result<String, DesktopError> {
    resolve_home().map(|path| path.to_string_lossy().into_owned())
}

/// Home resolution logic, separated from the command for testability.
pub(crate) fn resolve_home() -> Result<std::path::PathBuf, DesktopError> {
    use std::env::var_os;

    if let Some(override_dir) = var_os("OPENCONKIT_HOME") {
        if override_dir.is_empty() {
            return Err(DesktopError::HomeOverrideEmpty);
        }
        return Ok(std::path::PathBuf::from(override_dir));
    }
    // Windows first, then Unix.
    var_os("USERPROFILE")
        .or_else(|| var_os("HOME"))
        .map(|home| std::path::PathBuf::from(home).join(".openconkit"))
        .ok_or(DesktopError::HomeNotFound)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn app_version_matches_package_version() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn home_override_wins() {
        // SAFETY-FREE NOTE: tests run in-process; env mutation here is
        // serialized by cargo's default test threads sharing this binary,
        // so we restore the previous value immediately after.
        let previous = std::env::var_os("OPENCONKIT_HOME");
        std::env::set_var("OPENCONKIT_HOME", "Z:\\openconkit-test-home");
        let resolved = resolve_home().expect("override resolves");
        std::env::remove_var("OPENCONKIT_HOME");
        if let Some(value) = previous {
            std::env::set_var("OPENCONKIT_HOME", value);
        }
        assert_eq!(
            resolved,
            std::path::PathBuf::from("Z:\\openconkit-test-home")
        );
    }
}
