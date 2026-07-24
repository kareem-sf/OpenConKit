//! Ports: traits the infrastructure layer implements.

use openconkit_domain::{
    AiAnalysis, AiAnalysisStatus, AnalysisRun, AnalysisRunId, ErrorCode, ExportRecord, Finding,
    Project, ProjectId, Sha256Hash, SourceRevision, SourceRevisionId,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Errors reported by repository adapters.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// The underlying store failed.
    #[error("storage failure: {0}")]
    Storage(String),

    /// A project with the same id already exists.
    #[error("project already exists: {0}")]
    Duplicate(ProjectId),

    /// The requested entity does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Related records violate aggregate identity or ownership.
    #[error("repository invariant violation: {0}")]
    Invariant(String),
}

impl ErrorCode for RepositoryError {
    fn code(&self) -> &'static str {
        match self {
            RepositoryError::Storage(_) => "REPOSITORY_STORAGE",
            RepositoryError::Duplicate(_) => "REPOSITORY_DUPLICATE",
            RepositoryError::NotFound(_) => "REPOSITORY_NOT_FOUND",
            RepositoryError::Invariant(_) => "REPOSITORY_INVARIANT",
        }
    }
}

/// Persistence port for projects.
pub trait ProjectRepository {
    /// Insert a new project, returning [`RepositoryError::Duplicate`] when
    /// its id already exists. This operation must be atomic.
    fn create(&self, project: &Project) -> Result<(), RepositoryError>;

    /// Persist a project (insert or update).
    fn save(&self, project: &Project) -> Result<(), RepositoryError>;

    /// Look up a project by id.
    fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, RepositoryError>;

    /// List projects, optionally including archived ones.
    fn list(&self, include_archived: bool) -> Result<Vec<Project>, RepositoryError>;

    /// Mark a project archived at the given time.
    /// Returns [`RepositoryError::NotFound`] if the project does not exist.
    fn archive(&self, id: &ProjectId, archived_at: jiff::Timestamp) -> Result<(), RepositoryError>;
}

/// Persistence port for source revisions.
pub trait SourceRevisionRepository {
    /// Persist a new source revision.
    fn save(&self, revision: &SourceRevision) -> Result<(), RepositoryError>;

    /// Look up a revision by id.
    fn find_by_id(&self, id: &SourceRevisionId) -> Result<Option<SourceRevision>, RepositoryError>;

    /// Find an existing revision with the same content hash in a project.
    fn find_by_project_and_hash(
        &self,
        project_id: &ProjectId,
        sha256: &Sha256Hash,
    ) -> Result<Option<SourceRevision>, RepositoryError>;

    /// List all revisions imported into a project.
    fn list_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<SourceRevision>, RepositoryError>;
}

/// Persistence port for analysis runs.
pub trait AnalysisRunRepository {
    /// Persist an analysis run (insert or update).
    fn save(&self, run: &AnalysisRun) -> Result<(), RepositoryError>;

    /// Look up a run by id.
    fn find_by_id(&self, id: &AnalysisRunId) -> Result<Option<AnalysisRun>, RepositoryError>;

    /// List all runs of a project.
    fn list_by_project(&self, project_id: &ProjectId) -> Result<Vec<AnalysisRun>, RepositoryError>;

    /// Transaction-safe multi-write: the run plus all its findings commit
    /// atomically or not at all.
    fn save_with_findings(
        &self,
        run: &AnalysisRun,
        findings: &[Finding],
    ) -> Result<(), RepositoryError>;

    /// Transaction-safe aggregate write for a completed run, its
    /// authoritative findings, and the exact typed tool output used to
    /// reproduce the results and reports.
    fn save_with_findings_and_output(
        &self,
        run: &AnalysisRun,
        findings: &[Finding],
        output: &serde_json::Value,
    ) -> Result<(), RepositoryError>;

    /// Load the exact typed tool output persisted for a completed run.
    fn find_output(&self, id: &AnalysisRunId)
        -> Result<Option<serde_json::Value>, RepositoryError>;
}

/// Query-side history projection for one persisted analysis run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct RunHistoryEntry {
    /// Authoritative run metadata and detected workbook structure.
    pub run: AnalysisRun,
    /// Content hash of the immutable source revision analyzed by the run.
    pub source_sha256: Sha256Hash,
    /// Number of authoritative deterministic findings stored for the run.
    pub finding_count: u32,
    /// Number of generated report artifacts stored for the run.
    pub export_count: u32,
    /// Number of optional AI analyses attached to the run.
    pub ai_analysis_count: u32,
    /// Lifecycle status of the newest AI analysis, when one exists.
    pub latest_ai_status: Option<AiAnalysisStatus>,
}

/// Read-optimized history port implemented by the persistence layer.
pub trait RunHistoryRepository {
    /// List complete history projections for a project, newest first.
    fn list_history_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<RunHistoryEntry>, RepositoryError>;
}

/// Persistence port for findings.
pub trait FindingRepository {
    /// List all findings produced by a run.
    fn list_by_run(&self, run_id: &AnalysisRunId) -> Result<Vec<Finding>, RepositoryError>;
}

/// Persistence port for export records.
pub trait ExportRepository {
    /// Persist a new export record.
    fn save(&self, export: &ExportRecord) -> Result<(), RepositoryError>;

    /// List all exports produced from a run.
    fn list_by_run(&self, run_id: &AnalysisRunId) -> Result<Vec<ExportRecord>, RepositoryError>;
}

/// Persistence port for AI analyses.
pub trait AiAnalysisRepository {
    /// Persist a new analysis or update its lifecycle state. An existing
    /// analysis id may never be reassigned to a different run.
    fn save(&self, analysis: &AiAnalysis) -> Result<(), RepositoryError>;

    /// List all AI analyses attached to a run.
    fn list_by_run(&self, run_id: &AnalysisRunId) -> Result<Vec<AiAnalysis>, RepositoryError>;
}

/// Metadata for a file safely imported into the app home as an immutable
/// revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSource {
    /// SHA-256 of the stored file content, computed while streaming.
    pub sha256: Sha256Hash,
    /// Size of the stored file in bytes.
    pub size_bytes: u64,
    /// Path of the stored copy, relative to the app home.
    pub stored_relative_path: String,
}

/// Bounds enforced while copying a source into the immutable vault.
///
/// The caller derives this from the selected tool's declared input
/// capabilities; the storage adapter enforces it before and during copying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImportPolicy {
    /// Accepted extensions, compared case-insensitively with or without a
    /// leading dot.
    pub accepted_extensions: Vec<String>,
    /// Maximum number of source bytes that may be copied.
    pub max_file_size_bytes: u64,
}

impl SourceImportPolicy {
    /// Whether a source extension is declared by this policy.
    pub fn accepts_extension(&self, extension: &str) -> bool {
        let probe = extension.trim_start_matches('.');
        self.accepted_extensions
            .iter()
            .any(|accepted| accepted.trim_start_matches('.').eq_ignore_ascii_case(probe))
    }
}

/// File-vault port: copies source workbooks into the app home as immutable
/// files. Implemented by an infrastructure crate.
pub trait SourceStorage {
    /// Copies `source` into the project sources dir as an immutable file:
    /// streams + SHA-256, sanitized filename (path-traversal defense),
    /// atomic write (temp-then-rename).
    /// NEVER opens the original for writing.
    fn import(
        &self,
        project_id: &ProjectId,
        source: &std::path::Path,
        policy: &SourceImportPolicy,
    ) -> Result<ImportedSource, SourceStorageError>;

    /// Remove an imported copy that could not be committed to persistence.
    ///
    /// Implementations must confine deletion to the managed source vault.
    fn discard(&self, imported: &ImportedSource) -> Result<(), SourceStorageError>;
}

/// Errors reported by the source file vault.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SourceStorageError {
    /// Reading the source or writing the stored copy failed.
    #[error("source storage I/O failure: {message}")]
    Io {
        /// OS error detail.
        message: String,
    },

    /// The source path had no usable (sanitizable) file name.
    #[error("invalid source file name: {name:?}")]
    InvalidFileName {
        /// The offending name/path.
        name: String,
    },

    /// The selected tool does not accept the source extension.
    #[error("unsupported source extension {extension:?}")]
    UnsupportedExtension {
        /// Extension found on the source, including a leading dot when
        /// present.
        extension: String,
    },

    /// The source is not a regular file.
    #[error("source is not a regular file: {path}")]
    NotRegularFile {
        /// Rejected path.
        path: String,
    },

    /// The source exceeds the selected tool's declared byte limit.
    #[error("source is too large: {actual_bytes} bytes exceeds {max_bytes} bytes")]
    FileTooLarge {
        /// Observed source size. During streaming this is the first size known
        /// to exceed the limit.
        actual_bytes: u64,
        /// Declared maximum.
        max_bytes: u64,
    },
}

impl ErrorCode for SourceStorageError {
    fn code(&self) -> &'static str {
        match self {
            SourceStorageError::Io { .. } => "SOURCE_STORAGE_IO",
            SourceStorageError::InvalidFileName { .. } => "SOURCE_STORAGE_INVALID_FILE_NAME",
            SourceStorageError::UnsupportedExtension { .. } => {
                "SOURCE_STORAGE_UNSUPPORTED_EXTENSION"
            }
            SourceStorageError::NotRegularFile { .. } => "SOURCE_STORAGE_NOT_REGULAR_FILE",
            SourceStorageError::FileTooLarge { .. } => "SOURCE_STORAGE_FILE_TOO_LARGE",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn repository_error_codes_are_stable() {
        let cases: [(RepositoryError, &str); 4] = [
            (RepositoryError::Storage("x".into()), "REPOSITORY_STORAGE"),
            (
                RepositoryError::Duplicate(ProjectId::new("p").expect("slug")),
                "REPOSITORY_DUPLICATE",
            ),
            (
                RepositoryError::NotFound("x".into()),
                "REPOSITORY_NOT_FOUND",
            ),
            (
                RepositoryError::Invariant("x".into()),
                "REPOSITORY_INVARIANT",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected);
        }
    }

    #[test]
    fn source_storage_error_codes_are_stable() {
        let cases: [(SourceStorageError, &str); 5] = [
            (
                SourceStorageError::Io {
                    message: "x".into(),
                },
                "SOURCE_STORAGE_IO",
            ),
            (
                SourceStorageError::InvalidFileName { name: "x".into() },
                "SOURCE_STORAGE_INVALID_FILE_NAME",
            ),
            (
                SourceStorageError::UnsupportedExtension {
                    extension: ".pdf".into(),
                },
                "SOURCE_STORAGE_UNSUPPORTED_EXTENSION",
            ),
            (
                SourceStorageError::NotRegularFile { path: "x".into() },
                "SOURCE_STORAGE_NOT_REGULAR_FILE",
            ),
            (
                SourceStorageError::FileTooLarge {
                    actual_bytes: 2,
                    max_bytes: 1,
                },
                "SOURCE_STORAGE_FILE_TOO_LARGE",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected);
        }
    }
}
