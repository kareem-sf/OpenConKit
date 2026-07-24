//! Progress and cooperative cancellation for workbook ingestion.

use serde::{Deserialize, Serialize};

/// Current ingestion stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionStage {
    FileValidation,
    ArchiveValidation,
    WorkbookMetadata,
    Worksheet,
    Complete,
}

/// Bounded progress event. Cell totals may be unknown before parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionProgress {
    pub stage: IngestionStage,
    pub sheet_index: Option<usize>,
    pub sheet_count: Option<usize>,
    pub cells_read: usize,
}

/// Observer supplied by the caller for progress and cancellation.
pub trait IngestionObserver {
    /// Return `true` to stop at the next cooperative cancellation point.
    fn is_cancelled(&self) -> bool {
        false
    }

    /// Receive a progress event. Implementations must return quickly.
    fn on_progress(&self, _progress: &IngestionProgress) {}
}

/// Observer used by the convenience API.
pub(crate) struct NoopObserver;

impl IngestionObserver for NoopObserver {}
