//! Export records: report artifacts generated from analysis runs.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AnalysisRunId, ExportId, Sha256Hash};
use crate::paths::validate_relative_path;
use crate::DomainError;

/// The file format of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ExportKind {
    /// Excel workbook report.
    Xlsx,
    /// PDF report.
    Pdf,
}

/// A generated report artifact.
///
/// Exports are always written as new files under the project's exports
/// directory; source workbooks are never modified (product invariant).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ExportRecord {
    /// Unique id of this export.
    pub id: ExportId,
    /// Run whose findings the export renders.
    pub run_id: AnalysisRunId,
    /// File format.
    pub kind: ExportKind,
    /// Report language (BCP-47 tag, e.g. `en`, `ar`).
    pub language: String,
    /// Path of the artifact relative to the project's exports directory.
    /// Validated in the constructor with the same rules as
    /// [`crate::source::SourceRevision::stored_path`]: plain relative path,
    /// no root, no drive/UNC prefix, no `.`/`..` components.
    pub relative_path: String,
    /// SHA-256 of the artifact content, for integrity checks.
    pub sha256: Sha256Hash,
    /// When the export was generated.
    pub created_at: Timestamp,
}

#[derive(Deserialize)]
struct ExportRecordUnchecked {
    id: ExportId,
    run_id: AnalysisRunId,
    kind: ExportKind,
    language: String,
    relative_path: String,
    sha256: Sha256Hash,
    created_at: Timestamp,
}

impl TryFrom<ExportRecordUnchecked> for ExportRecord {
    type Error = DomainError;

    fn try_from(value: ExportRecordUnchecked) -> Result<Self, Self::Error> {
        Self::new(
            value.id,
            value.run_id,
            value.kind,
            value.language,
            value.relative_path,
            value.sha256,
            value.created_at,
        )
    }
}

impl<'de> Deserialize<'de> for ExportRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let unchecked = ExportRecordUnchecked::deserialize(deserializer)?;
        Self::try_from(unchecked).map_err(serde::de::Error::custom)
    }
}

impl ExportRecord {
    /// Create an export record, validating `relative_path` as a safe
    /// relative path.
    pub fn new(
        id: ExportId,
        run_id: AnalysisRunId,
        kind: ExportKind,
        language: String,
        relative_path: String,
        sha256: Sha256Hash,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        validate_relative_path(&relative_path)?;
        Ok(Self {
            id,
            run_id,
            kind,
            language,
            relative_path,
            sha256,
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn sample() -> ExportRecord {
        ExportRecord::new(
            ExportId::new(),
            AnalysisRunId::new(),
            ExportKind::Xlsx,
            "en".into(),
            "reports/run-1/boq-report.xlsx".into(),
            Sha256Hash::from_bytes([0x22; 32]),
            Timestamp::now(),
        )
        .expect("valid record")
    }

    #[test]
    fn export_kind_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ExportKind::Xlsx).expect("serialize"),
            "\"xlsx\""
        );
        let back: ExportKind = serde_json::from_str("\"pdf\"").expect("deserialize");
        assert_eq!(back, ExportKind::Pdf);
    }

    #[test]
    fn constructor_rejects_unsafe_relative_paths() {
        let base = sample();
        for bad in ["../escape.pdf", "/abs/x.pdf", "D:\\x.pdf"] {
            let result = ExportRecord::new(
                base.id,
                base.run_id,
                base.kind,
                base.language.clone(),
                bad.into(),
                base.sha256.clone(),
                base.created_at,
            );
            assert!(result.is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn serde_round_trip() {
        let record = sample();
        let json = serde_json::to_string(&record).expect("serialize");
        let back: ExportRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, record);
    }

    #[test]
    fn deserialization_enforces_relative_path_invariant() {
        let record = sample();
        let mut json = serde_json::to_value(record).expect("serialize");
        json["relative_path"] = serde_json::json!("../../source.xlsx");
        let parsed: Result<ExportRecord, _> = serde_json::from_value(json);
        assert!(parsed.is_err());
    }
}
