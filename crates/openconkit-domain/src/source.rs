//! Source workbook revisions: immutable snapshots of imported spreadsheets.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{Sha256Hash, SourceRevisionId};
use crate::paths::validate_relative_path;
use crate::project::ProjectId;
use crate::DomainError;

/// One imported revision of a source workbook.
///
/// Revisions are immutable: re-importing a changed workbook creates a new
/// `SourceRevision` with a new id rather than mutating an existing one.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SourceRevision {
    /// Unique id of this revision.
    pub id: SourceRevisionId,
    /// Project the workbook was imported into.
    pub project_id: ProjectId,
    /// SHA-256 of the stored file content; detects duplicate imports.
    pub sha256: Sha256Hash,
    /// Filename as the user supplied it (display only).
    pub original_filename: String,
    /// The path the file was imported from, if known.
    ///
    /// Recorded as optional metadata only — it is NEVER used for writing,
    /// re-reading, or any other filesystem access (see the threat model:
    /// source workbooks are read-only at import time and never touched
    /// afterwards).
    pub original_path: Option<String>,
    /// Path of the stored copy, relative to the project's sources
    /// directory. Validated in the constructor: must be a plain relative
    /// path with no root, drive/UNC prefix, or `.`/`..` components.
    pub stored_path: String,
    /// Size of the stored file in bytes.
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// When the workbook was imported.
    pub imported_at: Timestamp,
    /// Id of the tool that imported/parsed this revision.
    pub tool_id: String,
    /// Tool-specific workbook metadata (sheet counts, format details).
    /// Opaque to the domain; typed as `unknown` on the TypeScript side.
    #[ts(type = "unknown")]
    pub workbook_metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct SourceRevisionUnchecked {
    id: SourceRevisionId,
    project_id: ProjectId,
    sha256: Sha256Hash,
    original_filename: String,
    original_path: Option<String>,
    stored_path: String,
    size_bytes: u64,
    imported_at: Timestamp,
    tool_id: String,
    workbook_metadata: Option<serde_json::Value>,
}

impl TryFrom<SourceRevisionUnchecked> for SourceRevision {
    type Error = DomainError;

    fn try_from(value: SourceRevisionUnchecked) -> Result<Self, Self::Error> {
        Self::new(
            value.id,
            value.project_id,
            value.sha256,
            value.original_filename,
            value.original_path,
            value.stored_path,
            value.size_bytes,
            value.imported_at,
            value.tool_id,
            value.workbook_metadata,
        )
    }
}

impl<'de> Deserialize<'de> for SourceRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let unchecked = SourceRevisionUnchecked::deserialize(deserializer)?;
        Self::try_from(unchecked).map_err(serde::de::Error::custom)
    }
}

impl SourceRevision {
    /// Create a revision, validating `stored_path` as a safe relative path.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: SourceRevisionId,
        project_id: ProjectId,
        sha256: Sha256Hash,
        original_filename: String,
        original_path: Option<String>,
        stored_path: String,
        size_bytes: u64,
        imported_at: Timestamp,
        tool_id: String,
        workbook_metadata: Option<serde_json::Value>,
    ) -> Result<Self, DomainError> {
        validate_relative_path(&stored_path)?;
        Ok(Self {
            id,
            project_id,
            sha256,
            original_filename,
            original_path,
            stored_path,
            size_bytes,
            imported_at,
            tool_id,
            workbook_metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn sample() -> SourceRevision {
        SourceRevision::new(
            SourceRevisionId::new(),
            ProjectId::new("tower-a").expect("slug"),
            Sha256Hash::from_bytes([0x11; 32]),
            "boq.xlsx".into(),
            Some("C:\\Users\\qs\\Downloads\\boq.xlsx".into()),
            "sources/11-22/boq.xlsx".into(),
            48_000,
            Timestamp::now(),
            "boq-inspector".into(),
            Some(serde_json::json!({"sheets": 3})),
        )
        .expect("valid revision")
    }

    #[test]
    fn constructor_accepts_safe_relative_stored_path() {
        let revision = sample();
        assert_eq!(revision.stored_path, "sources/11-22/boq.xlsx");
    }

    #[test]
    fn constructor_rejects_unsafe_stored_paths() {
        let base = sample();
        for bad in ["../outside.xlsx", "/abs/x.xlsx", "C:\\x.xlsx", "a/./b"] {
            let result = SourceRevision::new(
                base.id,
                base.project_id.clone(),
                base.sha256.clone(),
                base.original_filename.clone(),
                None,
                bad.into(),
                base.size_bytes,
                base.imported_at,
                base.tool_id.clone(),
                None,
            );
            assert!(result.is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn serde_round_trip() {
        let revision = sample();
        let json = serde_json::to_string(&revision).expect("serialize");
        let back: SourceRevision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, revision);
    }

    #[test]
    fn deserialization_enforces_stored_path_invariant() {
        let revision = sample();
        let mut json = serde_json::to_value(revision).expect("serialize");
        json["stored_path"] = serde_json::json!("../outside.xlsx");
        let parsed: Result<SourceRevision, _> = serde_json::from_value(json);
        assert!(parsed.is_err());
    }

    #[test]
    fn ts_decl_maps_ids_and_timestamps_to_string() {
        let cfg = ts_rs::Config::default();
        let decl = <SourceRevision as TS>::decl(&cfg);
        assert!(decl.contains("imported_at: string"), "{decl}");
        assert!(decl.contains("workbook_metadata: unknown"), "{decl}");
    }
}
