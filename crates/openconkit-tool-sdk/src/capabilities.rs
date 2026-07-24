//! Input capabilities and declared tool permissions.
//!
//! These declarations are reviewed at code-review time and surfaced to the
//! user before a run, so a tool can never quietly exceed what it announced.
//! They are part of the local-first trust model (see `AGENTS.md` and
//! `docs/privacy.md`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What kind of source files a tool can ingest.
///
/// Extensions are stored lowercase with a leading dot, e.g. `[".xls",
/// ".xlsx"]`. [`InputCapabilities::accepts`] normalizes its argument, so
/// callers can probe with `"XLSX"`, `".Xlsx"`, or `"xlsx"` alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct InputCapabilities {
    /// Accepted file extensions, lowercase with leading dot.
    pub accepted_extensions: Vec<String>,
    /// Maximum source file size the tool will ingest, in bytes.
    #[ts(type = "number")]
    pub max_file_size_bytes: u64,
    /// Whether the tool accepts more than one source file per run.
    pub accepts_multiple: bool,
}

impl InputCapabilities {
    /// Whether `extension` is among the accepted extensions.
    ///
    /// Case-insensitive and tolerant of a missing leading dot: `"xlsx"`,
    /// `".xlsx"`, and `".XLSX"` all match a declared `".xlsx"`.
    pub fn accepts(&self, extension: &str) -> bool {
        let probe = extension.trim_start_matches('.');
        self.accepted_extensions
            .iter()
            .any(|accepted| accepted.trim_start_matches('.').eq_ignore_ascii_case(probe))
    }
}

/// Permissions a tool declares, reviewed, and the shell surfaces to the user.
///
/// `network` must stay `false` unless the tool implements an explicitly
/// user-invoked AI feature — the app is local-first with no telemetry
/// (see `AGENTS.md`, product invariant 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ToolPermissions {
    /// The tool reads source workbook files (the immutable stored copies in
    /// app home — never the user's originals).
    pub reads_source_files: bool,
    /// The tool writes export artifacts (always as new files, never in place).
    pub writes_exports: bool,
    /// The tool accesses the network. Must be `false` unless the tool ships
    /// an AI feature the user explicitly invokes.
    pub network: bool,
    /// The tool uses AI features (shown as suggestions, never silently
    /// applied to data).
    pub ai: bool,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn boq_capabilities() -> InputCapabilities {
        InputCapabilities {
            accepted_extensions: vec![".xls".to_string(), ".xlsx".to_string()],
            max_file_size_bytes: 50 * 1024 * 1024,
            accepts_multiple: false,
        }
    }

    #[test]
    fn accepts_matches_case_insensitively_with_or_without_dot() {
        let caps = boq_capabilities();
        for probe in [".xls", "xls", ".XLS", "XLSX", ".xlsx", "xlsx"] {
            assert!(caps.accepts(probe), "expected {probe} to be accepted");
        }
    }

    #[test]
    fn accepts_rejects_unlisted_extensions() {
        let caps = boq_capabilities();
        for probe in [".csv", ".pdf", "", ".", "xlsm"] {
            assert!(!caps.accepts(probe), "expected {probe} to be rejected");
        }
    }

    #[test]
    fn capabilities_and_permissions_serde_round_trip() {
        let caps = boq_capabilities();
        let json = serde_json::to_string(&caps).expect("serializes");
        let back: InputCapabilities = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(caps, back);

        let permissions = ToolPermissions {
            reads_source_files: true,
            writes_exports: true,
            network: false,
            ai: false,
        };
        let json = serde_json::to_string(&permissions).expect("serializes");
        let back: ToolPermissions = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(permissions, back);
    }
}
