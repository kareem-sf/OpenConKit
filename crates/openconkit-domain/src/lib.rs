//! OpenConKit domain layer.
//!
//! Pure entities, value objects and typed errors. This crate has no
//! infrastructure dependencies: no filesystem, no database, no UI.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod ai;
pub mod error;
pub mod exports;
pub mod finding;
pub mod ids;
pub mod money;
pub(crate) mod paths;
pub mod project;
pub mod run;
pub mod source;
pub mod workbook;

pub use ai::{
    AiAnalysis, AiAnalysisLanguage, AiAnalysisStatus, AiGroundingStatus, AiValidationStatus,
};
pub use error::{DomainError, ErrorCode};
pub use exports::{ExportKind, ExportRecord};
pub use finding::{
    CellRange, CellRef, Confidence, Evidence, Finding, FindingCategory, FindingOrigin, Severity,
};
pub use ids::{AiAnalysisId, AnalysisRunId, ExportId, FindingId, Sha256Hash, SourceRevisionId};
pub use money::{Currency, MoneyAmount};
pub use project::{Project, ProjectId, ProjectMetadata};
pub use run::{AnalysisRun, RunStatus};
pub use source::SourceRevision;
pub use workbook::{
    ClassifiedRow, ColumnRole, ColumnRoleAssignment, DetectedTable, RowClassification,
    SheetInventory, SheetVisibility, WorkbookDiagnostics,
};
