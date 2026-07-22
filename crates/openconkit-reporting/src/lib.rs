//! OpenConKit reporting.
//!
//! XLSX exports via rust_xlsxwriter. PDF exports via Typst land behind the
//! `pdf` feature in the reporting phase (see ROADMAP.md).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use rust_xlsxwriter::Workbook;

/// Errors from report generation.
#[derive(Debug, thiserror::Error)]
pub enum ReportingError {
    /// The XLSX writer failed.
    #[error("xlsx export failed: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),
}

/// Write a simple two-column key/value summary workbook.
///
/// Used as the foundation-level exporter; per-tool report layouts build on
/// the same writer in later phases.
pub fn write_key_value_sheet(
    path: &Path,
    sheet_name: &str,
    title: &str,
    rows: &[(String, String)],
) -> Result<(), ReportingError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(sheet_name)?;
    worksheet.write_string(0, 0, title)?;
    for (index, (key, value)) in rows.iter().enumerate() {
        let row = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
        worksheet.write_string(row, 0, key)?;
        worksheet.write_string(row, 1, value)?;
    }
    workbook.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use calamine::Reader;

    use super::*;

    #[test]
    fn writes_readable_xlsx_summary() {
        let path = std::env::temp_dir().join(format!(
            "openconkit-reporting-test-{}.xlsx",
            std::process::id()
        ));
        let rows = vec![
            ("Tool".to_string(), "BOQ Inspector".to_string()),
            ("Status".to_string(), "OK".to_string()),
        ];
        write_key_value_sheet(&path, "Summary", "OpenConKit Report", &rows).expect("writes xlsx");

        // Read back with calamine to prove the file is a valid workbook.
        let mut workbook = calamine::open_workbook_auto(&path).expect("reopens");
        let names = workbook.sheet_names().to_vec();
        assert_eq!(names, vec!["Summary".to_string()]);
        let range = workbook.worksheet_range("Summary").expect("reads range");
        assert_eq!(
            range.get((1, 1)).map(ToString::to_string).as_deref(),
            Some("BOQ Inspector")
        );
        std::fs::remove_file(&path).ok();
    }
}
