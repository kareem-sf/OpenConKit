//! Shell-free process configuration for the Codex app-server.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::CodexError;

/// Which official executable surface is being launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexBinaryKind {
    /// The dedicated `codex-app-server-*` release binary.
    StandaloneAppServer,
    /// A compatible `codex` CLI selected explicitly in Advanced settings.
    CodexCli,
}

/// Configuration for spawning and talking to the Codex app-server sidecar.
#[derive(Debug, Clone)]
pub struct CodexClientConfig {
    /// Absolute path to the staged sidecar or explicitly selected CLI.
    binary: PathBuf,
    /// Arguments passed to the process. Never interpolated through a shell.
    args: Vec<String>,
    /// Isolated OpenConKit-owned Codex profile.
    codex_home: PathBuf,
    /// Empty, run-specific working directory.
    working_directory: PathBuf,
    /// Default request/response timeout for short protocol calls.
    request_timeout: Duration,
    /// Optional local metadata-only protocol log.
    protocol_log: Option<PathBuf>,
}

impl CodexClientConfig {
    /// Configure the dedicated official app-server release binary.
    pub fn standalone(
        binary: PathBuf,
        codex_home: PathBuf,
        working_directory: PathBuf,
    ) -> Result<Self, CodexError> {
        Self::new(
            binary,
            CodexBinaryKind::StandaloneAppServer,
            codex_home,
            working_directory,
        )
    }

    /// Configure a compatible system `codex` CLI for development.
    pub fn system_cli(
        binary: PathBuf,
        codex_home: PathBuf,
        working_directory: PathBuf,
    ) -> Result<Self, CodexError> {
        Self::new(
            binary,
            CodexBinaryKind::CodexCli,
            codex_home,
            working_directory,
        )
    }

    fn new(
        binary: PathBuf,
        kind: CodexBinaryKind,
        codex_home: PathBuf,
        working_directory: PathBuf,
    ) -> Result<Self, CodexError> {
        for (label, path) in [
            ("binary", binary.as_path()),
            ("CODEX_HOME", codex_home.as_path()),
            ("working directory", working_directory.as_path()),
        ] {
            if !path.is_absolute() {
                return Err(CodexError::InvalidConfiguration(format!(
                    "{label} must be absolute"
                )));
            }
        }
        let args = match kind {
            CodexBinaryKind::StandaloneAppServer => vec!["--strict-config".to_string()],
            CodexBinaryKind::CodexCli => {
                vec!["app-server".to_string(), "--strict-config".to_string()]
            }
        };
        Ok(Self {
            binary,
            args,
            codex_home,
            working_directory,
            request_timeout: Duration::from_secs(30),
            protocol_log: None,
        })
    }

    /// Override the short protocol-call timeout.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Result<Self, CodexError> {
        if timeout.is_zero() {
            return Err(CodexError::InvalidConfiguration(
                "request timeout must be positive".to_string(),
            ));
        }
        self.request_timeout = timeout;
        Ok(self)
    }

    /// Enable metadata-only, size-rotated protocol diagnostics.
    pub fn with_protocol_log(mut self, path: PathBuf) -> Result<Self, CodexError> {
        if !path.is_absolute()
            || path.file_name().and_then(|name| name.to_str()) != Some("codex-protocol.jsonl")
        {
            return Err(CodexError::InvalidConfiguration(
                "protocol log must be an absolute codex-protocol.jsonl path".to_string(),
            ));
        }
        self.protocol_log = Some(path);
        Ok(self)
    }

    /// Path to the executable.
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// Shell-free argument list.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Isolated Codex profile directory.
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    /// Run-specific empty working directory.
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    /// Default timeout for short calls.
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Metadata-only protocol log path, when explicitly enabled.
    pub fn protocol_log(&self) -> Option<&Path> {
        self.protocol_log.as_deref()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn absolute(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn standalone_binary_is_invoked_directly() {
        let config = CodexClientConfig::standalone(
            absolute("codex-app-server"),
            absolute("codex-home"),
            absolute("sandbox"),
        )
        .expect("config");
        assert_eq!(config.args(), &["--strict-config".to_string()]);
    }

    #[test]
    fn system_cli_uses_app_server_subcommand() {
        let config = CodexClientConfig::system_cli(
            absolute("codex"),
            absolute("codex-home"),
            absolute("sandbox"),
        )
        .expect("config");
        assert_eq!(
            config.args(),
            &["app-server".to_string(), "--strict-config".to_string()]
        );
    }

    #[test]
    fn relative_paths_are_rejected() {
        let error = CodexClientConfig::standalone(
            PathBuf::from("codex"),
            absolute("codex-home"),
            absolute("sandbox"),
        )
        .expect_err("relative executable rejected");
        assert!(matches!(error, CodexError::InvalidConfiguration(_)));
    }

    #[test]
    fn protocol_log_path_is_fixed_and_absolute() {
        let config = CodexClientConfig::standalone(
            absolute("codex-app-server"),
            absolute("codex-home"),
            absolute("sandbox"),
        )
        .expect("config")
        .with_protocol_log(absolute("logs").join("codex-protocol.jsonl"))
        .expect("log");
        assert_eq!(
            config
                .protocol_log()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str()),
            Some("codex-protocol.jsonl")
        );
        assert!(config
            .clone()
            .with_protocol_log(absolute("logs").join("workbook.jsonl"))
            .is_err());
    }
}
