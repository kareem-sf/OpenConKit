//! Error type for the desktop host, serializable across the IPC boundary.

use openconkit_application::IpcError;
use openconkit_domain::ErrorCode;
use serde::Serialize;

/// Errors returned by Tauri commands.
#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
    /// `OPENCONKIT_HOME` was set but empty.
    #[error("OPENCONKIT_HOME is set but empty")]
    HomeOverrideEmpty,

    /// `OPENCONKIT_HOME` was provided to a release build.
    #[error("OPENCONKIT_HOME is available only in development and tests")]
    HomeOverrideNotAllowed,

    /// No home directory could be determined from the environment.
    #[error("could not determine the user home directory")]
    HomeNotFound,

    /// App-home bootstrap failed.
    #[error("bootstrap failed: {0}")]
    Bootstrap(String),

    /// The native application window could not be configured or created.
    #[error("window startup failed: {0}")]
    WindowStartup(String),

    /// A repository / storage operation failed.
    #[error("storage error: {0}")]
    Storage(String),

    /// A domain invariant was violated.
    #[error("domain error: {0}")]
    Domain(String),

    /// Shared state mutex was poisoned.
    #[error("application state lock poisoned")]
    StatePoisoned,

    /// Tool registry composition failed.
    #[error("tool registry error: {0}")]
    Registry(String),

    /// IPC input was malformed or did not match authoritative state.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A tool engine failed or was cancelled.
    #[error("tool error: {0}")]
    Tool(String),

    /// A background task could not be joined.
    #[error("background task failed: {0}")]
    BackgroundTask(String),

    /// Error carrying a stable code from a lower architectural layer.
    #[error("{message}")]
    Coded {
        /// Stable code mapped to frontend localization.
        code: &'static str,
        /// Internal diagnostic retained in Rust and never serialized to IPC.
        message: String,
    },
}

impl DesktopError {
    /// Stable code safe to expose across IPC.
    pub fn code(&self) -> &'static str {
        match self {
            DesktopError::HomeOverrideEmpty => "HOME_OVERRIDE_EMPTY",
            DesktopError::HomeOverrideNotAllowed => "HOME_OVERRIDE_NOT_ALLOWED",
            DesktopError::HomeNotFound => "HOME_NOT_FOUND",
            DesktopError::Bootstrap(_) => "BOOTSTRAP_FAILED",
            DesktopError::WindowStartup(_) => "WINDOW_STARTUP_FAILED",
            DesktopError::Storage(_) => "STORAGE_FAILED",
            DesktopError::Domain(_) => "DOMAIN_INVALID",
            DesktopError::StatePoisoned => "STATE_POISONED",
            DesktopError::Registry(_) => "TOOL_REGISTRY",
            DesktopError::InvalidInput(_) => "INVALID_INPUT",
            DesktopError::Tool(_) => "TOOL_FAILED",
            DesktopError::BackgroundTask(_) => "BACKGROUND_TASK_FAILED",
            DesktopError::Coded { code, .. } => code,
        }
    }
}

// Tauri requires command errors to be serializable to the frontend.
impl Serialize for DesktopError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        IpcError::new(self.code()).serialize(serializer)
    }
}

impl From<openconkit_storage::StorageError> for DesktopError {
    fn from(err: openconkit_storage::StorageError) -> Self {
        DesktopError::Storage(err.to_string())
    }
}

impl From<openconkit_application::RepositoryError> for DesktopError {
    fn from(err: openconkit_application::RepositoryError) -> Self {
        DesktopError::Coded {
            code: err.code(),
            message: err.to_string(),
        }
    }
}

impl From<openconkit_application::RegisterProjectError> for DesktopError {
    fn from(err: openconkit_application::RegisterProjectError) -> Self {
        match err {
            openconkit_application::RegisterProjectError::Domain(e) => DesktopError::Coded {
                code: e.code(),
                message: e.to_string(),
            },
            openconkit_application::RegisterProjectError::Repository(e) => DesktopError::Coded {
                code: e.code(),
                message: e.to_string(),
            },
        }
    }
}

impl From<openconkit_application::ArchiveProjectError> for DesktopError {
    fn from(err: openconkit_application::ArchiveProjectError) -> Self {
        match err {
            openconkit_application::ArchiveProjectError::Domain(e) => DesktopError::Coded {
                code: e.code(),
                message: e.to_string(),
            },
            openconkit_application::ArchiveProjectError::Repository(e) => DesktopError::Coded {
                code: e.code(),
                message: e.to_string(),
            },
        }
    }
}

impl From<openconkit_application::ConfigError> for DesktopError {
    fn from(err: openconkit_application::ConfigError) -> Self {
        DesktopError::Coded {
            code: err.code(),
            message: err.to_string(),
        }
    }
}

impl From<openconkit_tool_sdk::RegistryError> for DesktopError {
    fn from(err: openconkit_tool_sdk::RegistryError) -> Self {
        DesktopError::Registry(err.to_string())
    }
}

impl From<openconkit_tool_sdk::ToolError> for DesktopError {
    fn from(err: openconkit_tool_sdk::ToolError) -> Self {
        DesktopError::Coded {
            code: err.code(),
            message: err.to_string(),
        }
    }
}

impl From<openconkit_ai_codex::CodexError> for DesktopError {
    fn from(err: openconkit_ai_codex::CodexError) -> Self {
        let code = match &err {
            openconkit_ai_codex::CodexError::Manifest => "AI_RUNTIME_INVALID",
            openconkit_ai_codex::CodexError::Json(_) => "AI_PROTOCOL_INCOMPATIBLE",
            openconkit_ai_codex::CodexError::SidecarUnavailable(_) => "AI_RUNTIME_UNAVAILABLE",
            openconkit_ai_codex::CodexError::InvalidConfiguration(_) => "AI_RUNTIME_INVALID",
            openconkit_ai_codex::CodexError::Io(_) => "AI_OFFLINE",
            openconkit_ai_codex::CodexError::Protocol => "AI_PROTOCOL_INCOMPATIBLE",
            openconkit_ai_codex::CodexError::Server { .. } => "AI_SERVICE_FAILED",
            openconkit_ai_codex::CodexError::Timeout => "AI_TIMEOUT",
            openconkit_ai_codex::CodexError::ProcessExited => "AI_OFFLINE",
            openconkit_ai_codex::CodexError::MessageTooLarge => "AI_SCOPE_TOO_LARGE",
            openconkit_ai_codex::CodexError::UnsupportedAuthentication => {
                "AI_AUTHENTICATION_UNSUPPORTED"
            }
            openconkit_ai_codex::CodexError::AnalysisFailed => "AI_ANALYSIS_FAILED",
            openconkit_ai_codex::CodexError::Cancelled => "AI_CANCELLED",
            openconkit_ai_codex::CodexError::RestartLimit => "AI_RESTART_LIMIT",
            openconkit_ai_codex::CodexError::UnsafeActivity => "AI_UNSAFE_RESPONSE",
        };
        DesktopError::Coded {
            code,
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn ipc_serialization_hides_internal_diagnostics() {
        let error = DesktopError::Storage(
            "could not open C:\\Users\\person\\sensitive\\boq.xlsx".to_string(),
        );
        let value = serde_json::to_value(error).expect("serialize");
        assert_eq!(value, serde_json::json!({"code": "STORAGE_FAILED"}));
    }

    #[test]
    fn lower_layer_codes_are_preserved() {
        let error = DesktopError::from(openconkit_tool_sdk::ToolError::Cancelled);
        assert_eq!(error.code(), "TOOL_CANCELLED");
        assert_eq!(
            serde_json::to_value(error).expect("serialize"),
            serde_json::json!({"code": "TOOL_CANCELLED"})
        );
    }
}
