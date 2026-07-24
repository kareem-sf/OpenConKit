//! Serializable workbook evidence model independent of the parser backend.

use serde::{Deserialize, Serialize};

/// Workbook container format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbookFormat {
    /// Legacy binary Excel workbook.
    Xls,
    /// Office Open XML workbook.
    Xlsx,
}

/// Excel date epoch declared by the workbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateSystem {
    /// Excel's Windows/1900 date system.
    Excel1900,
    /// Excel's older Mac/1904 date system.
    Excel1904,
}

/// Type of a workbook sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetKind {
    Worksheet,
    DialogSheet,
    MacroSheet,
    ChartSheet,
    Vba,
}

/// Visibility declared for a sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetVisibility {
    Visible,
    Hidden,
    VeryHidden,
}

/// Inclusive, zero-based rectangular coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRegion {
    pub start_row: u32,
    pub start_column: u32,
    pub end_row: u32,
    pub end_column: u32,
}

impl CellRegion {
    /// Number of cells in the rectangle, saturating on malformed bounds.
    pub fn area(&self) -> u64 {
        let rows = self
            .end_row
            .checked_sub(self.start_row)
            .and_then(|value| value.checked_add(1))
            .unwrap_or(0);
        let columns = self
            .end_column
            .checked_sub(self.start_column)
            .and_then(|value| value.checked_add(1))
            .unwrap_or(0);
        u64::from(rows).saturating_mul(u64::from(columns))
    }
}

/// Canonical typed representation of a cell's cached or literal value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NormalizedCellValue {
    Empty,
    Integer(i64),
    /// Canonical decimal/scientific text emitted by Rust for an Excel float.
    Number(String),
    /// Trimmed text; [`IngestedCell::raw_value`] preserves the exact text.
    Text(String),
    Boolean(bool),
    /// Excel serial plus a timezone-free ISO-like rendering.
    DateTime {
        serial: String,
        rendered: String,
    },
    /// Excel duration serial for which no timezone/date interpretation applies.
    ExcelDuration {
        serial: String,
    },
    DateTimeIso(String),
    DurationIso(String),
    /// Excel error literal such as `#REF!`.
    Error(String),
}

/// One retained non-empty cell or formula cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestedCell {
    /// Zero-based row coordinate.
    pub row: u32,
    /// Zero-based column coordinate.
    pub column: u32,
    /// A1 address, e.g. `F12`.
    pub address: String,
    /// Exact cached/literal value text provided by the parser.
    pub raw_value: String,
    /// Typed canonical value used by downstream deterministic logic.
    pub normalized_value: NormalizedCellValue,
    /// Excel-rendered display text when the parser exposes it.
    ///
    /// Calamine does not currently expose formatted display text, so this is
    /// `None`; callers must not pretend `raw_value` is an Excel display value.
    pub displayed_value: Option<String>,
    /// Formula text where available. Cached value remains in the fields above.
    pub formula: Option<String>,
}

/// Evidence retained for one workbook sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestedSheet {
    /// Zero-based workbook order.
    pub index: u32,
    pub name: String,
    pub kind: SheetKind,
    pub visibility: SheetVisibility,
    /// Range declared by the workbook, before empty-cell filtering.
    pub declared_range: Option<CellRegion>,
    /// Tight range of retained non-empty/formula cells.
    pub used_range: Option<CellRegion>,
    pub merged_regions: Vec<CellRegion>,
    /// Hidden row metadata, when available from the parser.
    pub hidden_rows: Option<Vec<u32>>,
    /// Hidden column metadata, when available from the parser.
    pub hidden_columns: Option<Vec<u32>>,
    pub cells: Vec<IngestedCell>,
}

/// Bounded, serializable workbook evidence consumed by BOQ detection stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestedWorkbook {
    pub format: WorkbookFormat,
    pub date_system: DateSystem,
    pub sheets: Vec<IngestedSheet>,
    pub total_cells: usize,
    pub total_text_bytes: usize,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn model_round_trips_through_json() {
        let workbook = IngestedWorkbook {
            format: WorkbookFormat::Xlsx,
            date_system: DateSystem::Excel1900,
            sheets: vec![IngestedSheet {
                index: 0,
                name: "BOQ".into(),
                kind: SheetKind::Worksheet,
                visibility: SheetVisibility::Visible,
                declared_range: Some(CellRegion {
                    start_row: 0,
                    start_column: 0,
                    end_row: 1,
                    end_column: 1,
                }),
                used_range: None,
                merged_regions: vec![],
                hidden_rows: None,
                hidden_columns: None,
                cells: vec![],
            }],
            total_cells: 0,
            total_text_bytes: 0,
        };
        let json = serde_json::to_string(&workbook).expect("serialize");
        let back: IngestedWorkbook = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, workbook);
    }

    #[test]
    fn region_area_is_bounded_and_inclusive() {
        assert_eq!(
            CellRegion {
                start_row: 2,
                start_column: 3,
                end_row: 4,
                end_column: 5,
            }
            .area(),
            9
        );
    }
}
