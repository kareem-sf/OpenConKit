//! OpenConKit spreadsheet ingestion.
//!
//! Read-only, bounded XLS/XLSX access via Calamine. Source workbooks are
//! never modified. The primary API returns a parser-independent,
//! serializable evidence model with exact cell coordinates, cached/literal
//! values, formulas, merged regions, sheet visibility, and explicit metadata
//! uncertainty.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod error;
mod limits;
mod model;
mod observer;
mod reader;

use std::path::Path;

pub use error::SpreadsheetError;
pub use limits::WorkbookLimits;
pub use model::{
    CellRegion, DateSystem, IngestedCell, IngestedSheet, IngestedWorkbook, NormalizedCellValue,
    SheetKind, SheetVisibility, WorkbookFormat,
};
pub use observer::{IngestionObserver, IngestionProgress, IngestionStage};
pub use reader::ingest_with_observer;

/// Ingest a workbook with the default finite safety bounds.
pub fn ingest_workbook(path: &Path) -> Result<IngestedWorkbook, SpreadsheetError> {
    ingest_with_observer(path, &WorkbookLimits::default(), &observer::NoopObserver)
}

/// List sheet names through the same bounded ingestion path.
///
/// Kept as a small convenience API; callers that need workbook intelligence
/// should retain the full [`IngestedWorkbook`].
pub fn sheet_names(path: &Path) -> Result<Vec<String>, SpreadsheetError> {
    Ok(ingest_workbook(path)?
        .sheets
        .into_iter()
        .map(|sheet| sheet.name)
        .collect())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn reports_error_for_missing_file() {
        let path = Path::new("definitely-not-present-3f9a1c.xlsx");
        let error = ingest_workbook(path).expect_err("missing file");
        assert_eq!(error.code(), "IO");
    }
}
