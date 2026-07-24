//! Workbook structure diagnostics: what the parser inferred about a
//! workbook's sheets, tables, columns, and rows before rules run.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::finding::Confidence;

/// The semantic role a column plays in a detected BOQ table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ColumnRole {
    /// Item serial number.
    ItemNumber,
    /// Work item description.
    Description,
    /// Unit of measure (m, m2, kg, ...).
    Unit,
    /// Quantity column.
    Quantity,
    /// Unit price column.
    UnitPrice,
    /// Line total column.
    TotalPrice,
    /// Explicit currency code or symbol.
    Currency,
    /// Free-text notes.
    Notes,
    /// Role could not be determined.
    Unknown,
}

/// A detected column's assigned role, with detection confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ColumnRoleAssignment {
    /// 0-based column index within the sheet.
    pub column_index: u32,
    /// Excel-style column letter (e.g. `F`).
    pub column_letter: String,
    /// The detected role.
    pub role: ColumnRole,
    /// Confidence of the detection.
    pub confidence: Confidence,
}

/// What a row in a detected table represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RowClassification {
    /// A priced work item.
    Item,
    /// A section heading.
    Heading,
    /// A sub-section heading.
    Subheading,
    /// A free-text note row.
    Note,
    /// A subtotal row.
    Subtotal,
    /// A grand-total row.
    Total,
    /// An empty row.
    Blank,
    /// Could not be classified.
    Unknown,
}

/// A row with its classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ClassifiedRow {
    /// 0-based row index within the sheet.
    pub row_index: u32,
    /// The classification.
    pub classification: RowClassification,
    /// Confidence in the row classification.
    pub confidence: Confidence,
}

/// A table region detected within a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct DetectedTable {
    /// Sheet containing the table.
    pub sheet: String,
    /// 0-based row index of the header row; `None` for headerless inference.
    pub header_row: Option<u32>,
    /// 0-based row index of the first data row.
    pub start_row: u32,
    /// 0-based row index of the last data row (inclusive).
    pub end_row: u32,
    /// Column role assignments for the table.
    pub columns: Vec<ColumnRoleAssignment>,
    /// Row classifications for the table.
    pub rows: Vec<ClassifiedRow>,
    /// Aggregate confidence in this table interpretation.
    pub interpretation_confidence: Confidence,
    /// Stable diagnostic codes explaining why this interpretation was chosen.
    pub evidence: Vec<String>,
}

/// Visibility declared for a workbook sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SheetVisibility {
    Visible,
    Hidden,
    VeryHidden,
}

/// Per-sheet overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SheetInventory {
    /// Zero-based workbook order.
    pub index: u32,
    /// Sheet name.
    pub name: String,
    /// Sheet visibility.
    pub visibility: SheetVisibility,
    /// Number of rows with content.
    pub used_rows: u32,
    /// Number of columns with content.
    pub used_columns: u32,
    /// Number of retained non-empty or formula cells.
    pub non_empty_cells: u32,
    /// Number of tables detected in this sheet.
    pub detected_tables: u32,
}

/// Structural diagnostics for a whole workbook.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct WorkbookDiagnostics {
    /// Version of the deterministic detection/rule pipeline.
    pub rule_set_version: String,
    /// One entry per sheet.
    pub sheets: Vec<SheetInventory>,
    /// All detected tables across sheets.
    pub tables: Vec<DetectedTable>,
    /// Aggregate confidence across table interpretations.
    pub interpretation_confidence: Confidence,
    /// Stable warning codes for unavailable or ambiguous evidence.
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn sample() -> WorkbookDiagnostics {
        WorkbookDiagnostics {
            rule_set_version: "2026.07.1".into(),
            sheets: vec![SheetInventory {
                index: 0,
                name: "BOQ".into(),
                visibility: SheetVisibility::Visible,
                used_rows: 120,
                used_columns: 8,
                non_empty_cells: 512,
                detected_tables: 1,
            }],
            tables: vec![DetectedTable {
                sheet: "BOQ".into(),
                header_row: Some(3),
                start_row: 4,
                end_row: 119,
                columns: vec![ColumnRoleAssignment {
                    column_index: 5,
                    column_letter: "F".into(),
                    role: ColumnRole::TotalPrice,
                    confidence: Confidence::new(0.9).expect("in range"),
                }],
                rows: vec![ClassifiedRow {
                    row_index: 4,
                    classification: RowClassification::Item,
                    confidence: Confidence::new(0.8).expect("in range"),
                }],
                interpretation_confidence: Confidence::new(0.9).expect("in range"),
                evidence: vec!["header_aliases".into()],
            }],
            interpretation_confidence: Confidence::new(0.9).expect("in range"),
            warnings: vec![],
        }
    }

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&ColumnRole::ItemNumber).expect("serialize"),
            "\"item_number\""
        );
        assert_eq!(
            serde_json::to_string(&RowClassification::Subheading).expect("serialize"),
            "\"subheading\""
        );
    }

    #[test]
    fn workbook_diagnostics_serde_round_trip() {
        let diagnostics = sample();
        let json = serde_json::to_string(&diagnostics).expect("serialize");
        let back: WorkbookDiagnostics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, diagnostics);
    }

    #[test]
    fn workbook_diagnostics_ts_decl_has_expected_fields() {
        let cfg = ts_rs::Config::default();
        let decl = <WorkbookDiagnostics as TS>::decl(&cfg);
        assert!(decl.contains("sheets: Array<SheetInventory>"), "{decl}");
        assert!(decl.contains("tables: Array<DetectedTable>"), "{decl}");

        let table_decl = <DetectedTable as TS>::decl(&cfg);
        for field in [
            "sheet: string",
            "header_row: number | null",
            "start_row: number",
            "end_row: number",
            "columns: Array<ColumnRoleAssignment>",
            "rows: Array<ClassifiedRow>",
            "interpretation_confidence: Confidence",
            "evidence: Array<string>",
        ] {
            assert!(
                table_decl.contains(field),
                "missing `{field}` in {table_decl}"
            );
        }
    }
}
