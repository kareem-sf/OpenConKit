//! The single error type crossing the tool/shell boundary.
//!
//! Every fallible operation in the contract — engine runs and exports —
//! fails with [`ToolError`]. It is serializable to the frontend, where the
//! stable [`openconkit_domain::ErrorCode::code`] maps to the i18n key
//! `errors.<code>` (convention documented in `docs/architecture.md`).

use openconkit_domain::ErrorCode;
use serde::Serialize;

/// Errors produced by tool engines and export providers.
///
/// Serialized across IPC with a stable `kind` tag; the JSON shape is locked
/// by tests and must not change without bumping
/// [`crate::TOOL_CONTRACT_VERSION`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ToolError {
    /// The run was cancelled via the [`crate::CancellationToken`].
    #[error("tool run was cancelled")]
    Cancelled,

    /// The serialized input payload failed the tool's validation or
    /// deserialization.
    #[error("invalid tool input: {message}")]
    InvalidInput {
        /// What was wrong with the input.
        message: String,
    },

    /// The serialized settings payload failed the tool's validation or
    /// deserialization.
    #[error("invalid tool settings: {message}")]
    InvalidSettings {
        /// What was wrong with the settings.
        message: String,
    },

    /// A declared capability is not implemented yet; structured placeholder
    /// for phased implementation.
    #[error("tool capability not ready: {capability}")]
    NotReady {
        /// Which capability is missing, e.g. `"export:xlsx"`.
        capability: &'static str,
    },

    /// The engine itself failed (IO, parsing, internal invariant).
    #[error("tool engine error: {message}")]
    Engine {
        /// What went wrong.
        message: String,
    },
}

impl ErrorCode for ToolError {
    fn code(&self) -> &'static str {
        match self {
            ToolError::Cancelled => "TOOL_CANCELLED",
            ToolError::InvalidInput { .. } => "TOOL_INVALID_INPUT",
            ToolError::InvalidSettings { .. } => "TOOL_INVALID_SETTINGS",
            ToolError::NotReady { .. } => "TOOL_NOT_READY",
            ToolError::Engine { .. } => "TOOL_ENGINE",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn error_codes_are_stable_screaming_snake() {
        let cases: [(ToolError, &str); 5] = [
            (ToolError::Cancelled, "TOOL_CANCELLED"),
            (
                ToolError::InvalidInput {
                    message: "x".into(),
                },
                "TOOL_INVALID_INPUT",
            ),
            (
                ToolError::InvalidSettings {
                    message: "x".into(),
                },
                "TOOL_INVALID_SETTINGS",
            ),
            (
                ToolError::NotReady {
                    capability: "export:pdf",
                },
                "TOOL_NOT_READY",
            ),
            (
                ToolError::Engine {
                    message: "x".into(),
                },
                "TOOL_ENGINE",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected);
        }
    }

    #[test]
    fn serialization_shape_is_stable_snake_case_tagged() {
        assert_eq!(
            serde_json::to_value(ToolError::Cancelled).expect("serializes"),
            json!({ "kind": "cancelled" })
        );
        assert_eq!(
            serde_json::to_value(ToolError::InvalidInput {
                message: "bad value".into(),
            })
            .expect("serializes"),
            json!({ "kind": "invalid_input", "message": "bad value" })
        );
        assert_eq!(
            serde_json::to_value(ToolError::InvalidSettings {
                message: "bad setting".into(),
            })
            .expect("serializes"),
            json!({ "kind": "invalid_settings", "message": "bad setting" })
        );
        assert_eq!(
            serde_json::to_value(ToolError::NotReady {
                capability: "export:pdf",
            })
            .expect("serializes"),
            json!({ "kind": "not_ready", "capability": "export:pdf" })
        );
        assert_eq!(
            serde_json::to_value(ToolError::Engine {
                message: "boom".into(),
            })
            .expect("serializes"),
            json!({ "kind": "engine", "message": "boom" })
        );
    }
}
