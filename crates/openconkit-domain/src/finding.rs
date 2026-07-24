//! Findings: the individual issues a tool reports about a workbook.

use std::collections::BTreeMap;
use std::fmt;

use jiff::Timestamp;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::ids::{AnalysisRunId, FindingId, SourceRevisionId};
use crate::project::ProjectId;
use crate::DomainError;

/// Confidence in a result, in the inclusive range `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
pub struct Confidence(f64);

impl Default for Confidence {
    fn default() -> Self {
        Self(0.0)
    }
}

impl Confidence {
    /// Create a confidence value; rejects values outside `0.0..=1.0`
    /// (including NaN).
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidConfidence { value })
        }
    }

    /// The underlying value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Confidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// How severe a finding is for the reviewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum Severity {
    /// Worth knowing, no action expected.
    Info,
    /// Minor issue.
    Low,
    /// Should be reviewed.
    Medium,
    /// Likely a real defect.
    High,
    /// Must be fixed before sign-off.
    Critical,
}

/// What kind of problem a finding describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum FindingCategory {
    /// Wrong math (e.g. quantity times price mismatching the total).
    Arithmetic,
    /// The same item appears more than once.
    Duplication,
    /// Expected data is missing.
    Omission,
    /// Values contradict each other across cells/sheets.
    Inconsistency,
    /// The workbook structure itself is problematic.
    Structure,
    /// A contractual or standard compliance issue.
    Compliance,
    /// Anything not covered above.
    Other,
}

/// An Excel A1-style cell reference (`B12`) within worksheet bounds.
///
/// Construction normalizes to uppercase instead of rejecting lowercase
/// input, so `b12` becomes `B12`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
pub struct CellRef(String);

impl CellRef {
    /// Create a cell reference, normalizing letters to uppercase.
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let normalized = raw.to_ascii_uppercase();
        let bytes = normalized.as_bytes();
        let digit_start = bytes.iter().position(|b| b.is_ascii_digit());
        let valid = match digit_start {
            Some(split) => {
                let (letters, digits) = bytes.split_at(split);
                (1..=3).contains(&letters.len())
                    && letters.iter().all(|b| b.is_ascii_uppercase())
                    && (1..=7).contains(&digits.len())
                    && digits.iter().all(|b| b.is_ascii_digit())
                    && excel_coordinates(letters, digits).is_some()
            }
            None => false,
        };
        if valid {
            Ok(Self(normalized))
        } else {
            Err(DomainError::InvalidCellRef(raw.to_string()))
        }
    }

    /// Borrow the normalized reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn coordinates(&self) -> (u32, u32) {
        let bytes = self.0.as_bytes();
        let split = bytes
            .iter()
            .position(|b| b.is_ascii_digit())
            .unwrap_or(bytes.len());
        excel_coordinates(&bytes[..split], &bytes[split..]).unwrap_or((0, 0))
    }
}

impl<'de> Deserialize<'de> for CellRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

fn excel_coordinates(letters: &[u8], digits: &[u8]) -> Option<(u32, u32)> {
    let mut column = 0u32;
    for letter in letters {
        column = column
            .checked_mul(26)?
            .checked_add(u32::from(*letter - b'A' + 1))?;
    }
    let row = std::str::from_utf8(digits).ok()?.parse::<u32>().ok()?;
    if (1..=16_384).contains(&column) && (1..=1_048_576).contains(&row) {
        Some((column, row))
    } else {
        None
    }
}

impl fmt::Display for CellRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A rectangular cell range, e.g. `A1:B9`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CellRange {
    /// Top-left cell of the range.
    pub start: CellRef,
    /// Bottom-right cell of the range.
    pub end: CellRef,
}

#[derive(Deserialize)]
struct CellRangeUnchecked {
    start: CellRef,
    end: CellRef,
}

impl TryFrom<CellRangeUnchecked> for CellRange {
    type Error = DomainError;

    fn try_from(value: CellRangeUnchecked) -> Result<Self, Self::Error> {
        Self::new(value.start, value.end)
    }
}

impl<'de> Deserialize<'de> for CellRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = CellRangeUnchecked::deserialize(deserializer)?;
        Self::try_from(unchecked).map_err(serde::de::Error::custom)
    }
}

impl CellRange {
    /// Create an ordered rectangular range.
    pub fn new(start: CellRef, end: CellRef) -> Result<Self, DomainError> {
        let (start_column, start_row) = start.coordinates();
        let (end_column, end_row) = end.coordinates();
        if start_column > end_column || start_row > end_row {
            return Err(DomainError::InvalidCellRange(format!("{start}:{end}")));
        }
        Ok(Self { start, end })
    }

    /// Parse a range of the form `<CellRef>:<CellRef>` (e.g. `A1:B9`).
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let invalid = || DomainError::InvalidCellRange(raw.to_string());
        let (start, end) = raw.split_once(':').ok_or_else(invalid)?;
        if end.contains(':') {
            return Err(invalid());
        }
        let start = CellRef::new(start).map_err(|_| invalid())?;
        let end = CellRef::new(end).map_err(|_| invalid())?;
        Self::new(start, end).map_err(|_| invalid())
    }
}

impl fmt::Display for CellRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start, self.end)
    }
}

/// A piece of evidence backing a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Evidence {
    /// Sheet the evidence comes from.
    pub sheet: String,
    /// Specific cell, when the evidence is a single cell.
    pub cell: Option<CellRef>,
    /// Cell range, when the evidence spans multiple cells.
    pub range: Option<CellRange>,
    /// i18n key describing this evidence.
    pub description_key: Option<String>,
    /// Short verbatim excerpt of the cell content, if useful.
    pub snippet: Option<String>,
}

/// Where a finding came from.
///
/// Deterministic findings are produced by rules and are always
/// authoritative. AI findings are suggestions only: they must be presented
/// as such in the UI and never silently applied to data (product
/// invariant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum FindingOrigin {
    /// Produced by a deterministic rule.
    Deterministic,
    /// Suggested by the optional AI sidecar.
    Ai,
}

/// A single issue reported about a workbook.
///
/// Human-readable text is never stored: `*_key` fields hold i18n keys and
/// `*_params` their interpolation values, so the UI renders localized text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Finding {
    /// Unique id of this finding.
    pub id: FindingId,
    /// Project the finding belongs to.
    pub project_id: ProjectId,
    /// Source revision the finding was found in.
    pub source_revision_id: SourceRevisionId,
    /// Run that produced the finding.
    pub run_id: AnalysisRunId,
    /// Id of the rule that fired.
    pub rule_id: String,
    /// Version of the rule set containing the rule.
    pub rule_set_version: String,
    /// Category of the problem.
    pub category: FindingCategory,
    /// Severity for triage.
    pub severity: Severity,
    /// Confidence of the rule in this finding.
    pub confidence: Confidence,
    /// i18n key for the finding title.
    pub title_key: String,
    /// Interpolation parameters for `title_key`.
    pub title_params: BTreeMap<String, String>,
    /// i18n key for the longer explanation.
    pub explanation_key: String,
    /// Interpolation parameters for `explanation_key`.
    pub explanation_params: BTreeMap<String, String>,
    /// i18n key for the suggested remediation, if any.
    pub suggested_action_key: Option<String>,
    /// Interpolation parameters for `suggested_action_key`.
    pub suggested_action_params: BTreeMap<String, String>,
    /// Sheet the finding is located in, when applicable.
    pub sheet: Option<String>,
    /// Cell the finding points at, when applicable.
    pub cell: Option<CellRef>,
    /// Range the finding spans, when applicable.
    pub range: Option<CellRange>,
    /// Normalized deterministic row id (stable across re-runs), if the
    /// finding is tied to a table row.
    pub source_row_id: Option<String>,
    /// The value as found in the workbook, if relevant.
    pub original_value: Option<String>,
    /// The formula as found in the workbook, if relevant.
    pub original_formula: Option<String>,
    /// Supporting evidence.
    pub evidence: Vec<Evidence>,
    /// Deterministic (authoritative) or AI (suggestion) origin.
    pub origin: FindingOrigin,
    /// When the finding was created.
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn confidence_accepts_bounds_and_midrange() {
        for ok in [0.0, 0.5, 1.0] {
            let c = Confidence::new(ok).expect("in range");
            assert_eq!(c.value(), ok);
        }
    }

    #[test]
    fn confidence_rejects_out_of_range_and_nan() {
        for bad in [-0.1, 1.1, f64::NAN] {
            assert!(Confidence::new(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn confidence_deserialization_enforces_range() {
        let parsed: Result<Confidence, _> = serde_json::from_str("1.5");
        assert!(parsed.is_err());
    }

    #[test]
    fn cell_ref_normalizes_lowercase_and_validates() {
        let cell = CellRef::new("b12").expect("valid ref");
        assert_eq!(cell.as_str(), "B12");
        assert_eq!(cell.to_string(), "B12");
        assert_eq!(
            CellRef::new("XFD1048576")
                .expect("maximum Excel cell")
                .as_str(),
            "XFD1048576"
        );
    }

    #[test]
    fn cell_ref_rejects_invalid_shapes() {
        for bad in [
            "",
            "12",
            "B",
            "A0",
            "XFE1",
            "ZZZ1",
            "A1048577",
            "ABCD1",
            "A12345678",
            "A-1",
            "1A",
            "A1B",
        ] {
            assert!(CellRef::new(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn cell_range_parses_and_displays() {
        let range = CellRange::parse("a1:B9").expect("valid range");
        assert_eq!(range.start.as_str(), "A1");
        assert_eq!(range.end.as_str(), "B9");
        assert_eq!(range.to_string(), "A1:B9");
    }

    #[test]
    fn cell_range_rejects_invalid_input() {
        for bad in [
            "", "A1", "A1:", ":B9", "A1:B9:C3", "A1:12", "A1-B9", "B9:A1",
        ] {
            assert!(CellRange::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&Severity::Critical).expect("serialize"),
            "\"critical\""
        );
        assert_eq!(
            serde_json::to_string(&FindingCategory::Arithmetic).expect("serialize"),
            "\"arithmetic\""
        );
        assert_eq!(
            serde_json::to_string(&FindingOrigin::Ai).expect("serialize"),
            "\"ai\""
        );
    }

    fn sample_finding() -> Finding {
        let mut title_params = BTreeMap::new();
        title_params.insert("expected".into(), "100".into());
        title_params.insert("actual".into(), "90".into());
        Finding {
            id: FindingId::new(),
            project_id: ProjectId::new("tower-a").expect("slug"),
            source_revision_id: SourceRevisionId::new(),
            run_id: AnalysisRunId::new(),
            rule_id: "boq.arithmetic.total_mismatch".into(),
            rule_set_version: "2026.07".into(),
            category: FindingCategory::Arithmetic,
            severity: Severity::High,
            confidence: Confidence::new(0.95).expect("in range"),
            title_key: "findings.total_mismatch.title".into(),
            title_params,
            explanation_key: "findings.total_mismatch.explanation".into(),
            explanation_params: BTreeMap::new(),
            suggested_action_key: Some("findings.total_mismatch.action".into()),
            suggested_action_params: BTreeMap::new(),
            sheet: Some("BOQ".into()),
            cell: Some(CellRef::new("F12").expect("valid ref")),
            range: Some(CellRange::parse("D12:F12").expect("valid range")),
            source_row_id: Some("row-0042".into()),
            original_value: Some("90".into()),
            original_formula: Some("=D12*E12".into()),
            evidence: vec![Evidence {
                sheet: "BOQ".into(),
                cell: Some(CellRef::new("F12").expect("valid ref")),
                range: None,
                description_key: Some("evidence.total_cell".into()),
                snippet: Some("90".into()),
            }],
            origin: FindingOrigin::Deterministic,
            created_at: Timestamp::now(),
        }
    }

    #[test]
    fn finding_serde_round_trip() {
        let finding = sample_finding();
        let json = serde_json::to_string(&finding).expect("serialize");
        let back: Finding = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, finding);
    }

    #[test]
    fn finding_ts_decl_has_expected_fields_and_string_ids() {
        let cfg = ts_rs::Config::default();
        let decl = <Finding as TS>::decl(&cfg);
        for field in [
            "id: FindingId",
            "project_id: ProjectId",
            "source_revision_id: SourceRevisionId",
            "run_id: AnalysisRunId",
            "rule_id: string",
            "category: FindingCategory",
            "severity: Severity",
            "confidence: Confidence",
            "title_key: string",
            "title_params: { [key in string]: string }",
            "evidence: Array<Evidence>",
            "origin: FindingOrigin",
            "created_at: string",
        ] {
            assert!(decl.contains(field), "missing `{field}` in {decl}");
        }
        // The id/confidence aliases themselves resolve to primitives.
        let id_decl = <FindingId as TS>::decl(&cfg);
        assert!(id_decl.contains("= string"), "{id_decl}");
        let confidence_decl = <Confidence as TS>::decl(&cfg);
        assert!(confidence_decl.contains("= number"), "{confidence_decl}");
    }
}
