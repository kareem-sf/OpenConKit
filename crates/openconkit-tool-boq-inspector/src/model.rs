//! Internal normalized BOQ model between detection and deterministic rules.

use openconkit_domain::{ColumnRole, Confidence, RowClassification, WorkbookDiagnostics};
use rust_decimal::Decimal;

use crate::normalization::NormalizedUnit;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetectedColumn {
    pub index: u32,
    pub role: ColumnRole,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetectedBoqTable {
    pub sheet: String,
    pub header_row: Option<u32>,
    pub start_row: u32,
    pub end_row: u32,
    pub columns: Vec<DetectedColumn>,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
}

impl DetectedBoqTable {
    pub fn column(&self, role: ColumnRole) -> Option<u32> {
        self.columns
            .iter()
            .find(|column| column.role == role)
            .map(|column| column.index)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SourceValue<T> {
    pub cell: String,
    pub raw: String,
    pub formula: Option<String>,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NormalizedBoqRow {
    pub source_row_id: String,
    pub sheet: String,
    pub row: u32,
    pub table_index: usize,
    pub classification: RowClassification,
    pub classification_confidence: Confidence,
    pub row_text: String,
    pub section_path: Vec<String>,
    pub item_code: Option<SourceValue<String>>,
    pub description: Option<SourceValue<String>>,
    /// Original unit text, retained even when the alias is unknown.
    pub unit_text: Option<SourceValue<String>>,
    pub unit: Option<SourceValue<NormalizedUnit>>,
    pub quantity: Option<SourceValue<Decimal>>,
    /// Non-numeric rate text such as "included", retained for semantics.
    pub rate_text: Option<SourceValue<String>>,
    pub rate: Option<SourceValue<Decimal>>,
    pub amount: Option<SourceValue<Decimal>>,
    pub currency: Option<SourceValue<String>>,
    pub error_cells: Vec<SourceValue<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetectionOutput {
    pub diagnostics: WorkbookDiagnostics,
    pub tables: Vec<DetectedBoqTable>,
    pub rows: Vec<NormalizedBoqRow>,
}
