//! Explicit resource limits for untrusted workbook ingestion.

use serde::{Deserialize, Serialize};

/// Safety and memory bounds applied before and during workbook ingestion.
///
/// Defaults are intentionally above OpenConKit's expected workload
/// (approximately 5,000 relevant BOQ rows) while remaining finite. Tools may
/// choose stricter values, but invalid zero limits are rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookLimits {
    /// Maximum size of the outer XLS/XLSX file.
    pub max_file_size_bytes: u64,
    /// Maximum number of entries in an XLSX ZIP container.
    pub max_archive_entries: usize,
    /// Maximum uncompressed size of one XLSX ZIP entry.
    pub max_archive_entry_uncompressed_bytes: u64,
    /// Maximum combined uncompressed size declared by all XLSX ZIP entries.
    pub max_archive_uncompressed_bytes: u64,
    /// Maximum `uncompressed / compressed` ratio for a non-empty ZIP entry.
    pub max_compression_ratio: u64,
    /// Maximum number of sheets in one workbook.
    pub max_sheets: usize,
    /// Maximum 1-based row coordinate accepted in one sheet.
    pub max_rows_per_sheet: u32,
    /// Maximum 1-based column coordinate accepted in one sheet.
    pub max_columns_per_sheet: u32,
    /// Maximum number of retained non-empty/formula cells across the workbook.
    pub max_cells: usize,
    /// Maximum number of merged regions retained for one sheet.
    pub max_merged_regions_per_sheet: usize,
    /// Maximum bytes in one text/cached value.
    pub max_cell_text_bytes: usize,
    /// Maximum bytes in one formula.
    pub max_formula_bytes: usize,
    /// Maximum combined bytes retained for cell values and formulas.
    pub max_total_text_bytes: usize,
}

impl Default for WorkbookLimits {
    fn default() -> Self {
        Self {
            max_file_size_bytes: 64 * 1024 * 1024,
            max_archive_entries: 4_096,
            max_archive_entry_uncompressed_bytes: 128 * 1024 * 1024,
            max_archive_uncompressed_bytes: 256 * 1024 * 1024,
            max_compression_ratio: 200,
            max_sheets: 128,
            max_rows_per_sheet: 200_000,
            max_columns_per_sheet: 512,
            max_cells: 2_000_000,
            max_merged_regions_per_sheet: 10_000,
            max_cell_text_bytes: 256 * 1024,
            max_formula_bytes: 64 * 1024,
            max_total_text_bytes: 128 * 1024 * 1024,
        }
    }
}

impl WorkbookLimits {
    pub(crate) fn first_invalid_field(&self) -> Option<&'static str> {
        [
            (self.max_file_size_bytes == 0, "max_file_size_bytes"),
            (self.max_archive_entries == 0, "max_archive_entries"),
            (
                self.max_archive_entry_uncompressed_bytes == 0,
                "max_archive_entry_uncompressed_bytes",
            ),
            (
                self.max_archive_uncompressed_bytes == 0,
                "max_archive_uncompressed_bytes",
            ),
            (self.max_compression_ratio == 0, "max_compression_ratio"),
            (self.max_sheets == 0, "max_sheets"),
            (self.max_rows_per_sheet == 0, "max_rows_per_sheet"),
            (self.max_columns_per_sheet == 0, "max_columns_per_sheet"),
            (self.max_cells == 0, "max_cells"),
            (
                self.max_merged_regions_per_sheet == 0,
                "max_merged_regions_per_sheet",
            ),
            (self.max_cell_text_bytes == 0, "max_cell_text_bytes"),
            (self.max_formula_bytes == 0, "max_formula_bytes"),
            (self.max_total_text_bytes == 0, "max_total_text_bytes"),
        ]
        .into_iter()
        .find_map(|(invalid, field)| invalid.then_some(field))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn defaults_are_finite_and_valid() {
        let limits = WorkbookLimits::default();
        assert!(limits.first_invalid_field().is_none());
        assert!(limits.max_rows_per_sheet < 1_048_576);
        assert!(limits.max_columns_per_sheet < 16_384);
    }

    #[test]
    fn zero_limit_is_rejected() {
        let limits = WorkbookLimits {
            max_cells: 0,
            ..WorkbookLimits::default()
        };
        assert_eq!(limits.first_invalid_field(), Some("max_cells"));
    }
}
