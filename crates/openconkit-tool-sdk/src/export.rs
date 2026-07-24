//! Export providers: turning a finished run's output into report artifacts.
//!
//! A tool may ship zero or more [`ExportProvider`]s (one per
//! [`openconkit_domain::ExportKind`]). Providers write **new files only**,
//! never in place (product invariant: reports are new files, sources are
//! never modified).

use std::path::Path;

use jiff::Timestamp;
use openconkit_domain::{AnalysisRun, ExportKind, SourceRevision};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ToolError;

/// A report artifact produced by an [`ExportProvider`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ExportedArtifact {
    /// The file format of the export.
    pub kind: ExportKind,
    /// BCP 47 language tag the artifact was generated in, e.g. `"en"`.
    pub language: String,
    /// Path of the artifact relative to the destination directory the
    /// provider was given.
    pub relative_path: String,
    /// Lowercase hex SHA-256 of the artifact's bytes, for integrity checks.
    pub sha256: String,
}

/// Authoritative provenance supplied by the host for report generation.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportContext {
    pub run: AnalysisRun,
    pub source_revision: SourceRevision,
    pub report_timestamp: Timestamp,
    /// Optional tool-validated AI output selected by the host for the report
    /// language. Providers must parse their own strict type and label it as
    /// generated commentary.
    pub validated_ai_output: Option<serde_json::Value>,
}

/// Generates a report artifact from a run's serialized output.
///
/// `run_output` is the same tool-typed output the engine returned,
/// serialized; the provider validates and deserializes it, mapping failures
/// to [`ToolError::InvalidInput`].
pub trait ExportProvider: Send + Sync {
    /// The file format this provider generates.
    fn kind(&self) -> ExportKind;

    /// Language tags this provider can generate, e.g. `["en", "ar"]`.
    fn languages(&self) -> Vec<String>;

    /// Generate the artifact into `dest_dir` for `language`.
    ///
    /// Implementations must create new files only and return an
    /// [`ExportedArtifact`] describing what was written.
    fn export(
        &self,
        context: &ExportContext,
        run_output: &serde_json::Value,
        dest_dir: &Path,
        language: &str,
    ) -> Result<ExportedArtifact, ToolError>;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn exported_artifact_serde_round_trip() {
        let artifact = ExportedArtifact {
            kind: ExportKind::Xlsx,
            language: "en".to_string(),
            relative_path: "reports/run-1.xlsx".to_string(),
            sha256: "a".repeat(64),
        };
        let json = serde_json::to_string(&artifact).expect("serializes");
        let back: ExportedArtifact = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(artifact, back);
    }
}
