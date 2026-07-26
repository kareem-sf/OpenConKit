//! Format-neutral, already-localized report model.

use serde::{Deserialize, Serialize};

/// All labels used by the fixed XLSX/PDF templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportLabels {
    pub report_title: String,
    pub executive_summary: String,
    pub findings: String,
    pub detection: String,
    pub pareto: String,
    pub source_metadata: String,
    pub ai_review: String,
    pub limitations: String,
    pub field: String,
    pub value: String,
    pub severity: String,
    pub category: String,
    pub confidence: String,
    pub rule: String,
    pub title: String,
    pub explanation: String,
    pub action: String,
    pub sheet: String,
    pub cell: String,
    pub evidence: String,
    pub source_hash: String,
    pub source_file: String,
    pub run: String,
    pub tool_version: String,
    pub rule_set_version: String,
    pub app_version: String,
    pub report_timestamp: String,
    pub language: String,
    pub item_rows: String,
    pub finding_count: String,
    pub interpretation_confidence: String,
    pub context: String,
    pub currency: String,
    pub total_amount: String,
    pub top_item_count: String,
    pub total_item_count: String,
    pub cumulative_share: String,
    pub table_range: String,
    pub mapped_columns: String,
    pub warning: String,
    pub deterministic_origin: String,
    pub ai_origin: String,
    pub not_available: String,
}

/// Provenance displayed on every report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub source_filename: String,
    pub source_sha256: String,
    pub run_id: String,
    pub tool_name: String,
    pub tool_version: String,
    pub rule_set_version: String,
    pub app_version: String,
    pub report_timestamp: String,
    pub language: String,
}

/// Executive summary values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSummary {
    pub item_rows: usize,
    pub finding_count: usize,
    pub interpretation_confidence: f64,
    pub severity_counts: Vec<(String, usize)>,
    pub category_counts: Vec<(String, usize)>,
}

/// One source-evidence pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportEvidence {
    pub sheet: String,
    pub reference: String,
    pub description: String,
    pub snippet: Option<String>,
}

/// One localized finding row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportFinding {
    pub severity: String,
    pub category: String,
    pub confidence_percent: f64,
    pub rule_id: String,
    pub title: String,
    pub explanation: String,
    pub action: Option<String>,
    pub sheet: Option<String>,
    pub cell: Option<String>,
    pub original_value: Option<String>,
    pub original_formula: Option<String>,
    pub evidence: Vec<ReportEvidence>,
    pub origin: String,
}

/// One detected table or workbook-level structure warning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDetection {
    pub sheet: String,
    pub table_range: String,
    pub header_row: Option<u32>,
    pub mapped_columns: String,
    pub confidence_percent: f64,
    pub evidence: String,
    pub warning: Option<String>,
}

/// One currency/context-specific Pareto result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPareto {
    pub context: String,
    pub currency: Option<String>,
    pub total_amount: String,
    pub top_item_count: usize,
    pub total_item_count: usize,
    pub cumulative_share_percent: String,
}

/// Complete localized report consumed by both renderers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDocument {
    pub labels: ReportLabels,
    pub metadata: ReportMetadata,
    pub summary: ReportSummary,
    pub findings: Vec<ReportFinding>,
    pub detections: Vec<ReportDetection>,
    pub pareto: Vec<ReportPareto>,
    pub limitations: Vec<String>,
    /// Present only after a separately validated AI analysis.
    pub ai_commentary: Option<String>,
    pub right_to_left: bool,
}

impl ReportDocument {
    pub(crate) fn validate(&self) -> Result<(), crate::ReportingError> {
        if !matches!(self.metadata.language.as_str(), "en" | "ar") {
            return Err(crate::ReportingError::InvalidData(
                "report language must be `en` or `ar`".to_string(),
            ));
        }
        if self.metadata.source_sha256.len() != 64
            || !self
                .metadata
                .source_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(crate::ReportingError::InvalidData(
                "source SHA-256 must be 64 lowercase hexadecimal characters".to_string(),
            ));
        }
        if !self.summary.interpretation_confidence.is_finite()
            || !(0.0..=1.0).contains(&self.summary.interpretation_confidence)
        {
            return Err(crate::ReportingError::InvalidData(
                "interpretation confidence must be within 0..=1".to_string(),
            ));
        }
        if self.findings.iter().any(|finding| {
            !finding.confidence_percent.is_finite()
                || !(0.0..=100.0).contains(&finding.confidence_percent)
        }) {
            return Err(crate::ReportingError::InvalidData(
                "finding confidence percent must be within 0..=100".to_string(),
            ));
        }
        Ok(())
    }
}
