//! Stable DTOs crossing the privileged Rust-to-frontend IPC boundary.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Localizable command failure returned by the desktop host.
///
/// The privileged backend deliberately does not expose internal error
/// messages or paths. The frontend maps `code` to `errors.<code>` in the
/// locale catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct IpcError {
    /// Stable screaming-snake error code.
    pub code: String,
}

impl IpcError {
    /// Construct a payload from a stable backend error code.
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn serialization_contains_only_the_localizable_code() {
        let value = serde_json::to_value(IpcError::new("TOOL_CANCELLED")).expect("serialize");
        assert_eq!(value, serde_json::json!({"code": "TOOL_CANCELLED"}));
    }
}
