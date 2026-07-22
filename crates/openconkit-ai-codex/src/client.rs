//! Stdio client configuration for the Codex app server.
//!
//! The actual JSON-RPC-over-stdio transport is implemented in the Codex
//! integration phase; this module defines the configuration surface and the
//! safety invariants (no shell interpolation, explicit argument list).

use std::path::PathBuf;

/// Configuration for spawning and talking to the Codex app-server sidecar.
#[derive(Debug, Clone)]
pub struct CodexClientConfig {
    /// Absolute path to the staged sidecar binary.
    binary: PathBuf,
    /// Arguments passed to the sidecar. Never interpolated through a shell.
    args: Vec<String>,
}

impl CodexClientConfig {
    /// Create a configuration for the `app-server` stdio entry point.
    pub fn stdio(binary: PathBuf) -> Self {
        Self {
            binary,
            args: vec!["app-server".to_string()],
        }
    }

    /// Path to the sidecar binary.
    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    /// Argument list (shell-free).
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn stdio_config_uses_app_server_entry_point() {
        let config = CodexClientConfig::stdio(PathBuf::from("/tmp/codex"));
        assert_eq!(config.args(), &["app-server".to_string()]);
        assert_eq!(config.binary().to_string_lossy(), "/tmp/codex");
    }
}
