//! OpenConKit spreadsheet ingestion.
//!
//! Read-only access to XLS/XLSX workbooks via calamine. Source workbooks are
//! NEVER modified (see `AGENTS.md`); every API here takes an immutable path
//! and opens files read-only.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use calamine::Reader;

/// Errors from spreadsheet ingestion.
#[derive(Debug, thiserror::Error)]
pub enum SpreadsheetError {
    /// The workbook could not be opened or parsed.
    #[error("failed to open workbook: {0}")]
    Open(#[from] calamine::Error),
}

/// List the sheet names of a workbook (XLS, XLSX or ODS, detected by content
/// and extension).
///
/// The workbook is opened read-only; the source file is never modified.
pub fn sheet_names(path: &Path) -> Result<Vec<String>, SpreadsheetError> {
    let workbook = calamine::open_workbook_auto(path)?;
    Ok(workbook.sheet_names().to_vec())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Write a minimal XLSX workbook to a unique temp file and return its path.
    fn write_test_workbook(sheet: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openconkit-spreadsheet-test-{}-{}.xlsx",
            std::process::id(),
            sheet
        ));
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet).expect("valid sheet name");
        worksheet.write_string(0, 0, "Item").expect("write cell");
        workbook.save(&path).expect("save test workbook");
        path
    }

    #[test]
    fn lists_sheet_names_of_xlsx() {
        let path = write_test_workbook("BOQ");
        let names = sheet_names(&path).expect("reads sheets");
        assert_eq!(names, vec!["BOQ".to_string()]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reports_error_for_missing_file() {
        let path = Path::new("definitely-not-present-3f9a1c.xlsx");
        assert!(sheet_names(path).is_err());
    }
}
