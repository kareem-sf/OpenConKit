//! Deterministic BOQ checks over the normalized detection model.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use jiff::Timestamp;
use openconkit_domain::{
    AnalysisRunId, CellRange, CellRef, Confidence, Evidence, Finding, FindingCategory, FindingId,
    FindingOrigin, ProjectId, RowClassification, Severity, SourceRevisionId,
};
use openconkit_spreadsheet::{IngestedCell, IngestedWorkbook};
use openconkit_tool_sdk::{CancellationToken, ToolError};
use rust_decimal::Decimal;
use sha2::Digest as _;
use uuid::Uuid;

use crate::detection::RULE_SET_VERSION;
use crate::formula::{cell_decimal, evaluate, FormulaEvaluation};
use crate::model::{DetectedBoqTable, DetectionOutput, NormalizedBoqRow, SourceValue};
use crate::normalization::{normalize_text, text_similarity};
use crate::ParetoAnalysis;

const MAX_SIMILARITY_COMPARISONS: usize = 250_000;

pub(crate) struct RuleSettings {
    pub absolute_tolerance: Decimal,
    pub relative_tolerance: Decimal,
    pub decimal_precision: u32,
    pub fuzzy_similarity_threshold: f64,
    pub low_confidence_threshold: f64,
}

pub(crate) struct RuleIdentity {
    pub project_id: ProjectId,
    pub source_revision_id: SourceRevisionId,
    pub run_id: AnalysisRunId,
}

pub(crate) struct RuleOutput {
    pub findings: Vec<Finding>,
    pub pareto: Vec<ParetoAnalysis>,
}

struct RuleRunner<'a> {
    identity: &'a RuleIdentity,
    settings: &'a RuleSettings,
    selected: &'a BTreeSet<String>,
    findings: Vec<Finding>,
}

impl<'a> RuleRunner<'a> {
    fn enabled(&self, rule_id: &str) -> bool {
        self.selected.is_empty()
            || self.selected.contains(rule_id)
            || self
                .selected
                .iter()
                .any(|selected| rule_id.starts_with(&format!("{selected}.")))
    }

    fn push(&mut self, specification: FindingSpecification<'_>) {
        if !self.enabled(specification.rule_id) {
            return;
        }
        let stable_key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.identity.source_revision_id,
            specification.rule_id,
            specification.location.sheet.as_deref().unwrap_or_default(),
            specification.stable_subject
        );
        let id = FindingId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_OID, stable_key.as_bytes()));
        let cell = specification
            .location
            .cell
            .as_deref()
            .and_then(|address| CellRef::new(address).ok());
        self.findings.push(Finding {
            id,
            project_id: self.identity.project_id.clone(),
            source_revision_id: self.identity.source_revision_id,
            run_id: self.identity.run_id,
            rule_id: specification.rule_id.to_string(),
            rule_set_version: RULE_SET_VERSION.to_string(),
            category: specification.category,
            severity: specification.severity,
            confidence: confidence(specification.confidence),
            title_key: format!("findings.{}.title", specification.key),
            title_params: specification.title_params,
            explanation_key: format!("findings.{}.explanation", specification.key),
            explanation_params: specification.explanation_params,
            suggested_action_key: Some(format!("findings.{}.action", specification.key)),
            suggested_action_params: BTreeMap::new(),
            sheet: specification.location.sheet,
            cell,
            range: specification.location.range,
            source_row_id: specification.location.source_row_id,
            original_value: specification.location.original_value,
            original_formula: specification.location.original_formula,
            evidence: specification.location.evidence,
            origin: FindingOrigin::Deterministic,
            created_at: Timestamp::now(),
        });
    }
}

struct FindingSpecification<'a> {
    rule_id: &'a str,
    key: &'a str,
    stable_subject: String,
    category: FindingCategory,
    severity: Severity,
    confidence: f64,
    title_params: BTreeMap<String, String>,
    explanation_params: BTreeMap<String, String>,
    location: FindingLocation,
}

struct FindingLocation {
    sheet: Option<String>,
    cell: Option<String>,
    range: Option<CellRange>,
    source_row_id: Option<String>,
    original_value: Option<String>,
    original_formula: Option<String>,
    evidence: Vec<Evidence>,
}

/// Run every selected deterministic check.
pub(crate) fn run_rules(
    workbook: &IngestedWorkbook,
    detection: &DetectionOutput,
    identity: &RuleIdentity,
    settings: &RuleSettings,
    selected: &BTreeSet<String>,
    cancel: &CancellationToken,
) -> Result<RuleOutput, ToolError> {
    let mut runner = RuleRunner {
        identity,
        settings,
        selected,
        findings: Vec::new(),
    };

    check_structure(&mut runner, detection);
    check_missing_values(&mut runner, detection);
    check_numeric_values(&mut runner, &detection.rows);
    check_formulas(&mut runner, workbook, &detection.rows, cancel)?;
    check_spreadsheet_errors(&mut runner, &detection.rows);
    check_amounts(&mut runner, &detection.rows);
    check_exact_duplicates(&mut runner, &detection.rows);
    check_similar_rows(&mut runner, &detection.rows, cancel)?;
    check_subtotals(&mut runner, &detection.rows);
    check_value_outliers(&mut runner, &detection.rows);
    let pareto = check_pareto(&mut runner, &detection.rows);
    runner.findings.sort_by_key(finding_sort_key);
    Ok(RuleOutput {
        findings: runner.findings,
        pareto,
    })
}

fn check_formulas(
    runner: &mut RuleRunner<'_>,
    workbook: &IngestedWorkbook,
    rows: &[NormalizedBoqRow],
    cancel: &CancellationToken,
) -> Result<(), ToolError> {
    let mut visited = 0usize;
    for sheet in &workbook.sheets {
        for cell in sheet.cells.iter().filter(|cell| cell.formula.is_some()) {
            visited += 1;
            if visited % 256 == 0 && cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            let Some(raw_formula) = cell.formula.as_deref() else {
                continue;
            };
            let row = rows
                .iter()
                .find(|row| row.sheet == sheet.name && row.row == cell.row);
            let Some(actual) = cell_decimal(cell) else {
                runner.push(FindingSpecification {
                    rule_id: "boq.broken_formula",
                    key: "brokenFormula",
                    stable_subject: format!("{}:{}", sheet.name, cell.address),
                    category: FindingCategory::Arithmetic,
                    severity: Severity::High,
                    confidence: 1.0,
                    title_params: BTreeMap::new(),
                    explanation_params: BTreeMap::new(),
                    location: formula_location(&sheet.name, cell, row, "evidence.brokenFormula"),
                });
                continue;
            };
            match evaluate(sheet, raw_formula, (cell.row, cell.column)) {
                FormulaEvaluation::Value(expected) => {
                    let expected = expected.round_dp(runner.settings.decimal_precision);
                    let actual = actual.round_dp(runner.settings.decimal_precision);
                    let difference = (expected - actual).abs();
                    let base = expected.abs().max(Decimal::ONE);
                    if difference > runner.settings.absolute_tolerance
                        && difference / base > runner.settings.relative_tolerance
                    {
                        runner.push(FindingSpecification {
                            rule_id: "boq.formula_result_mismatch",
                            key: "formulaResultMismatch",
                            stable_subject: format!("{}:{}", sheet.name, cell.address),
                            category: FindingCategory::Arithmetic,
                            severity: Severity::High,
                            confidence: 1.0,
                            title_params: params([
                                ("expected", expected.to_string()),
                                ("actual", actual.to_string()),
                            ]),
                            explanation_params: BTreeMap::new(),
                            location: formula_location(
                                &sheet.name,
                                cell,
                                row,
                                "evidence.formulaResult",
                            ),
                        });
                    }
                }
                FormulaEvaluation::Unverifiable(_reason) => {
                    runner.push(FindingSpecification {
                        rule_id: "boq.formula_unverifiable",
                        key: "formulaUnverifiable",
                        stable_subject: format!("{}:{}", sheet.name, cell.address),
                        category: FindingCategory::Other,
                        severity: Severity::Info,
                        confidence: 1.0,
                        title_params: BTreeMap::new(),
                        explanation_params: BTreeMap::new(),
                        location: formula_location(
                            &sheet.name,
                            cell,
                            row,
                            "evidence.unverifiableFormula",
                        ),
                    });
                }
            }
        }
    }
    if cancel.is_cancelled() {
        return Err(ToolError::Cancelled);
    }
    Ok(())
}

fn check_structure(runner: &mut RuleRunner<'_>, detection: &DetectionOutput) {
    if detection.tables.is_empty() {
        runner.push(FindingSpecification {
            rule_id: "boq.low_confidence_structure",
            key: "lowConfidenceStructure",
            stable_subject: "workbook".to_string(),
            category: FindingCategory::Structure,
            severity: Severity::High,
            confidence: 1.0,
            title_params: BTreeMap::new(),
            explanation_params: params([("confidence", "0")]),
            location: workbook_location(),
        });
        return;
    }

    for (table_index, table) in detection.tables.iter().enumerate() {
        if table.confidence.value() < runner.settings.low_confidence_threshold {
            let row = table.header_row.unwrap_or(table.start_row);
            runner.push(FindingSpecification {
                rule_id: "boq.low_confidence_structure",
                key: "lowConfidenceStructure",
                stable_subject: format!("table:{table_index}"),
                category: FindingCategory::Structure,
                severity: Severity::Medium,
                confidence: 1.0 - table.confidence.value(),
                title_params: params([("sheet", table.sheet.as_str())]),
                explanation_params: params([(
                    "confidence",
                    format_percent(table.confidence.value()),
                )]),
                location: table_location(table, row, None),
            });
        }
        for column in table
            .columns
            .iter()
            .filter(|column| column.role == openconkit_domain::ColumnRole::Unknown)
        {
            runner.push(FindingSpecification {
                rule_id: "boq.unmapped_column",
                key: "unmappedColumn",
                stable_subject: format!("table:{table_index}:column:{}", column.index),
                category: FindingCategory::Structure,
                severity: Severity::Low,
                confidence: 0.9,
                title_params: params([("column", column_letter(column.index))]),
                explanation_params: BTreeMap::new(),
                location: table_location(
                    table,
                    table.header_row.unwrap_or(table.start_row),
                    Some(column.index),
                ),
            });
        }
        for (role, field) in [
            (openconkit_domain::ColumnRole::Description, "description"),
            (openconkit_domain::ColumnRole::Unit, "unit"),
            (openconkit_domain::ColumnRole::Quantity, "quantity"),
            (openconkit_domain::ColumnRole::UnitPrice, "rate"),
            (openconkit_domain::ColumnRole::TotalPrice, "amount"),
        ] {
            if table.column(role).is_none() {
                runner.push(FindingSpecification {
                    rule_id: "boq.missing_column",
                    key: "missingColumn",
                    stable_subject: format!("table:{table_index}:role:{field}"),
                    category: FindingCategory::Structure,
                    severity: if matches!(role, openconkit_domain::ColumnRole::Description) {
                        Severity::High
                    } else {
                        Severity::Medium
                    },
                    confidence: table.confidence.value(),
                    title_params: params([("field", field)]),
                    explanation_params: BTreeMap::new(),
                    location: table_location(
                        table,
                        table.header_row.unwrap_or(table.start_row),
                        None,
                    ),
                });
            }
        }
    }
}

fn check_missing_values(runner: &mut RuleRunner<'_>, detection: &DetectionOutput) {
    for row in item_rows(&detection.rows) {
        let Some(table) = detection.tables.get(row.table_index) else {
            continue;
        };
        if table
            .column(openconkit_domain::ColumnRole::Description)
            .is_some()
            && row.description.is_none()
        {
            push_missing(
                runner,
                table,
                row,
                "description",
                openconkit_domain::ColumnRole::Description,
            );
        }
        if table.column(openconkit_domain::ColumnRole::Unit).is_some() && row.unit_text.is_none() {
            push_missing(
                runner,
                table,
                row,
                "unit",
                openconkit_domain::ColumnRole::Unit,
            );
        }

        let unit = row.unit.as_ref().map(|unit| unit.value.canonical.as_str());
        let included_rate = row.rate_text.as_ref().is_some_and(|rate| {
            let rate = normalize_text(&rate.value);
            ["included", "incl", "ضمن", "مشمول", "مدمج"]
                .iter()
                .any(|alias| rate == normalize_text(alias))
        });
        if table
            .column(openconkit_domain::ColumnRole::Quantity)
            .is_some()
            && !matches!(unit, Some("ls" | "%"))
            && row.quantity.is_none()
        {
            push_missing(
                runner,
                table,
                row,
                "quantity",
                openconkit_domain::ColumnRole::Quantity,
            );
        }
        if table
            .column(openconkit_domain::ColumnRole::UnitPrice)
            .is_some()
            && !matches!(unit, Some("ls" | "%"))
            && !included_rate
            && row.rate.is_none()
        {
            push_missing(
                runner,
                table,
                row,
                "rate",
                openconkit_domain::ColumnRole::UnitPrice,
            );
        }
        if table
            .column(openconkit_domain::ColumnRole::TotalPrice)
            .is_some()
            && !included_rate
            && row.amount.is_none()
        {
            push_missing(
                runner,
                table,
                row,
                "amount",
                openconkit_domain::ColumnRole::TotalPrice,
            );
        }
    }
}

fn push_missing(
    runner: &mut RuleRunner<'_>,
    table: &DetectedBoqTable,
    row: &NormalizedBoqRow,
    field: &'static str,
    role: openconkit_domain::ColumnRole,
) {
    let column = table.column(role);
    runner.push(FindingSpecification {
        rule_id: match field {
            "description" => "boq.missing.description",
            "unit" => "boq.missing.unit",
            "quantity" => "boq.missing.quantity",
            "rate" => "boq.missing.rate",
            _ => "boq.missing.amount",
        },
        key: "missingValue",
        stable_subject: format!("{}:{field}", row.source_row_id),
        category: FindingCategory::Omission,
        severity: if field == "amount" {
            Severity::High
        } else {
            Severity::Medium
        },
        confidence: row.classification_confidence.value(),
        title_params: params([("field", field)]),
        explanation_params: BTreeMap::new(),
        location: row_location(table, row, column, None),
    });
}

fn check_numeric_values(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) {
    for row in item_rows(rows) {
        for (field, value) in [
            ("quantity", row.quantity.as_ref()),
            ("rate", row.rate.as_ref()),
            ("amount", row.amount.as_ref()),
        ] {
            let Some(value) = value else {
                continue;
            };
            if value.value.is_zero() {
                runner.push(FindingSpecification {
                    rule_id: "boq.zero_value",
                    key: "zeroValue",
                    stable_subject: format!("{}:{field}", row.source_row_id),
                    category: FindingCategory::Inconsistency,
                    severity: Severity::Low,
                    confidence: 0.98,
                    title_params: params([("field", field)]),
                    explanation_params: BTreeMap::new(),
                    location: source_location(row, value, "evidence.numericValue"),
                });
            } else if value.value.is_sign_negative() {
                runner.push(FindingSpecification {
                    rule_id: "boq.negative_value",
                    key: "negativeValue",
                    stable_subject: format!("{}:{field}", row.source_row_id),
                    category: FindingCategory::Inconsistency,
                    severity: Severity::Medium,
                    confidence: 0.96,
                    title_params: params([("field", field)]),
                    explanation_params: BTreeMap::new(),
                    location: source_location(row, value, "evidence.numericValue"),
                });
            }
        }
    }
}

fn check_spreadsheet_errors(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) {
    for row in rows {
        for error in &row.error_cells {
            runner.push(FindingSpecification {
                rule_id: "boq.spreadsheet_error",
                key: "spreadsheetError",
                stable_subject: format!("{}:{}", row.source_row_id, error.cell),
                category: FindingCategory::Arithmetic,
                severity: Severity::High,
                confidence: 1.0,
                title_params: params([("error", error.value.as_str())]),
                explanation_params: BTreeMap::new(),
                location: source_location(row, error, "evidence.spreadsheetError"),
            });
        }
    }
}

fn check_amounts(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) {
    for row in item_rows(rows) {
        let (Some(quantity), Some(rate), Some(amount)) = (&row.quantity, &row.rate, &row.amount)
        else {
            continue;
        };
        let expected = (quantity.value * rate.value).round_dp(runner.settings.decimal_precision);
        let actual = amount.value.round_dp(runner.settings.decimal_precision);
        let difference = (expected - actual).abs();
        let relative_base = expected.abs().max(Decimal::ONE);
        if difference <= runner.settings.absolute_tolerance
            || difference / relative_base <= runner.settings.relative_tolerance
        {
            continue;
        }
        let evidence = vec![
            evidence(row, quantity, "evidence.quantity"),
            evidence(row, rate, "evidence.rate"),
            evidence(row, amount, "evidence.amount"),
        ];
        runner.push(FindingSpecification {
            rule_id: "boq.amount_mismatch",
            key: "amountMismatch",
            stable_subject: row.source_row_id.clone(),
            category: FindingCategory::Arithmetic,
            severity: Severity::High,
            confidence: 0.99,
            title_params: params([
                ("expected", expected.to_string()),
                ("actual", actual.to_string()),
            ]),
            explanation_params: params([("difference", difference.to_string())]),
            location: FindingLocation {
                sheet: Some(row.sheet.clone()),
                cell: Some(amount.cell.clone()),
                range: range_for_sources([quantity, rate, amount]),
                source_row_id: Some(row.source_row_id.clone()),
                original_value: Some(amount.raw.clone()),
                original_formula: amount.formula.clone(),
                evidence,
            },
        });
    }
}

fn check_exact_duplicates(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) {
    let mut first_by_key: BTreeMap<String, &NormalizedBoqRow> = BTreeMap::new();
    let mut first_by_description: BTreeMap<String, &NormalizedBoqRow> = BTreeMap::new();
    for row in item_rows(rows) {
        let Some(description) = &row.description else {
            continue;
        };
        let normalized_description = normalize_text(&description.value);
        let key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            normalized_description,
            unit_key(row),
            decimal_key(row.quantity.as_ref()),
            decimal_key(row.rate.as_ref()),
            decimal_key(row.amount.as_ref())
        );
        let first_exact = first_by_key.get(&key).copied();
        if first_exact.is_none() {
            first_by_key.insert(key, row);
        }
        if let Some(first) = first_exact {
            let cross_sheet = first.sheet != row.sheet;
            let (rule_id, key) = if cross_sheet {
                ("boq.cross_sheet_duplicate", "crossSheetDuplicate")
            } else {
                ("boq.exact_duplicate", "exactDuplicate")
            };
            let mut location = source_location(row, description, "evidence.duplicateItem");
            if let Some(first_description) = &first.description {
                location
                    .evidence
                    .push(evidence(first, first_description, "evidence.originalItem"));
            }
            runner.push(FindingSpecification {
                rule_id,
                key,
                stable_subject: format!("{}:{}", first.source_row_id, row.source_row_id),
                category: FindingCategory::Duplication,
                severity: if cross_sheet {
                    Severity::High
                } else {
                    Severity::Medium
                },
                confidence: 0.99,
                title_params: params([("sheet", first.sheet.as_str())]),
                explanation_params: BTreeMap::new(),
                location,
            });
        }

        let first_description = *first_by_description
            .entry(normalized_description)
            .or_insert(row);
        if first_description.sheet != row.sheet
            && row_signature(first_description) != row_signature(row)
        {
            let mut location =
                source_location(row, description, "evidence.inconsistentCrossSheetItem");
            if let Some(original_description) = &first_description.description {
                location.evidence.push(evidence(
                    first_description,
                    original_description,
                    "evidence.originalItem",
                ));
            }
            runner.push(FindingSpecification {
                rule_id: "boq.cross_sheet_inconsistency",
                key: "crossSheetInconsistency",
                stable_subject: format!(
                    "{}:{}",
                    first_description.source_row_id, row.source_row_id
                ),
                category: FindingCategory::Inconsistency,
                severity: Severity::High,
                confidence: 0.97,
                title_params: params([("sheet", first_description.sheet.as_str())]),
                explanation_params: BTreeMap::new(),
                location,
            });
        }
    }
}

fn check_similar_rows(
    runner: &mut RuleRunner<'_>,
    rows: &[NormalizedBoqRow],
    cancel: &CancellationToken,
) -> Result<(), ToolError> {
    let items: Vec<&NormalizedBoqRow> = item_rows(rows)
        .filter(|row| row.description.is_some())
        .collect();
    let mut comparisons = 0usize;
    for (left_index, left) in items.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }
        let Some(left_description) = &left.description else {
            continue;
        };
        for right in items.iter().skip(left_index + 1) {
            if comparisons >= MAX_SIMILARITY_COMPARISONS {
                return Ok(());
            }
            comparisons += 1;
            let Some(right_description) = &right.description else {
                continue;
            };
            let left_normalized = normalize_text(&left_description.value);
            let right_normalized = normalize_text(&right_description.value);
            if left_normalized == right_normalized {
                continue;
            }
            let similarity = text_similarity(&left_normalized, &right_normalized);
            if similarity < runner.settings.fuzzy_similarity_threshold {
                continue;
            }
            let unit_disagreement = match (&left.unit, &right.unit) {
                (Some(left_unit), Some(right_unit)) => {
                    left_unit.value.canonical != right_unit.value.canonical
                }
                _ => false,
            };
            let (rule_id, key, category, severity) = if unit_disagreement {
                (
                    "boq.inconsistent_unit",
                    "inconsistentUnit",
                    FindingCategory::Inconsistency,
                    Severity::Medium,
                )
            } else {
                (
                    "boq.fuzzy_duplicate",
                    "fuzzyDuplicate",
                    FindingCategory::Duplication,
                    Severity::Low,
                )
            };
            let mut location = source_location(right, right_description, "evidence.similarItem");
            location
                .evidence
                .push(evidence(left, left_description, "evidence.originalItem"));
            runner.push(FindingSpecification {
                rule_id,
                key,
                stable_subject: format!("{}:{}", left.source_row_id, right.source_row_id),
                category,
                severity,
                confidence: similarity,
                title_params: params([("similarity", format_percent(similarity))]),
                explanation_params: BTreeMap::new(),
                location,
            });
        }
    }
    Ok(())
}

fn check_subtotals(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) {
    let mut by_table: BTreeMap<usize, Vec<&NormalizedBoqRow>> = BTreeMap::new();
    for row in rows {
        by_table.entry(row.table_index).or_default().push(row);
    }
    for table_rows in by_table.values_mut() {
        table_rows.sort_unstable_by_key(|row| row.row);
        let mut running = Decimal::ZERO;
        let mut item_count = 0usize;
        let mut grand_total = Decimal::ZERO;
        let mut grand_item_count = 0usize;
        for row in table_rows {
            match row.classification {
                RowClassification::Item => {
                    if let Some(amount) = &row.amount {
                        running += amount.value;
                        item_count += 1;
                        grand_total += amount.value;
                        grand_item_count += 1;
                    }
                }
                RowClassification::Subtotal => {
                    let Some(amount) = &row.amount else {
                        running = Decimal::ZERO;
                        item_count = 0;
                        continue;
                    };
                    if item_count > 0 {
                        let expected = running.round_dp(runner.settings.decimal_precision);
                        let actual = amount.value.round_dp(runner.settings.decimal_precision);
                        let difference = (expected - actual).abs();
                        let base = expected.abs().max(Decimal::ONE);
                        if difference > runner.settings.absolute_tolerance
                            && difference / base > runner.settings.relative_tolerance
                        {
                            runner.push(FindingSpecification {
                                rule_id: "boq.subtotal_mismatch",
                                key: "subtotalMismatch",
                                stable_subject: row.source_row_id.clone(),
                                category: FindingCategory::Arithmetic,
                                severity: Severity::High,
                                confidence: 0.97,
                                title_params: params([
                                    ("expected", expected.to_string()),
                                    ("actual", actual.to_string()),
                                ]),
                                explanation_params: params([("itemCount", item_count.to_string())]),
                                location: source_location(row, amount, "evidence.subtotalAmount"),
                            });
                        }
                    }
                    running = Decimal::ZERO;
                    item_count = 0;
                }
                RowClassification::Total => {
                    let Some(amount) = &row.amount else {
                        continue;
                    };
                    if grand_item_count == 0 {
                        continue;
                    }
                    let expected = grand_total.round_dp(runner.settings.decimal_precision);
                    let actual = amount.value.round_dp(runner.settings.decimal_precision);
                    let difference = (expected - actual).abs();
                    let base = expected.abs().max(Decimal::ONE);
                    if difference > runner.settings.absolute_tolerance
                        && difference / base > runner.settings.relative_tolerance
                    {
                        runner.push(FindingSpecification {
                            rule_id: "boq.total_mismatch",
                            key: "totalMismatch",
                            stable_subject: row.source_row_id.clone(),
                            category: FindingCategory::Arithmetic,
                            severity: Severity::High,
                            confidence: 0.97,
                            title_params: params([
                                ("expected", expected.to_string()),
                                ("actual", actual.to_string()),
                            ]),
                            explanation_params: params([(
                                "itemCount",
                                grand_item_count.to_string(),
                            )]),
                            location: source_location(row, amount, "evidence.totalAmount"),
                        });
                    }
                }
                _ => {}
            }
        }
    }
}

fn check_value_outliers(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) {
    for field in ["quantity", "rate", "amount"] {
        let mut groups: BTreeMap<String, Vec<(&NormalizedBoqRow, &SourceValue<Decimal>)>> =
            BTreeMap::new();
        for row in item_rows(rows) {
            let value = match field {
                "quantity" => row.quantity.as_ref(),
                "rate" => row.rate.as_ref(),
                _ => row.amount.as_ref(),
            };
            if let Some(value) = value {
                groups
                    .entry(format!(
                        "{field}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                        row.table_index,
                        unit_key(row),
                        currency_key(row),
                        section_key(row)
                    ))
                    .or_default()
                    .push((row, value));
            }
        }
        for values in groups.values() {
            // A robust statistic over a very small peer set creates noisy
            // review candidates. Seven is conservative for expected BOQs.
            if values.len() < 7 {
                continue;
            }
            let observed: Vec<Decimal> = values.iter().map(|(_, value)| value.value).collect();
            let median_value = median(&observed);
            if median_value.is_zero() {
                continue;
            }
            let deviations: Vec<Decimal> = observed
                .iter()
                .map(|value| (*value - median_value).abs())
                .collect();
            let mad = median(&deviations);
            for (row, value) in values {
                let deviation = (value.value - median_value).abs();
                let robust_outlier = if mad.is_zero() {
                    value.value.abs() >= median_value.abs() * Decimal::from(10u32)
                        || value.value.abs() * Decimal::from(10u32) <= median_value.abs()
                } else {
                    deviation > mad * Decimal::from(6u32)
                        && deviation > median_value.abs() * Decimal::new(5, 1)
                };
                if robust_outlier {
                    runner.push(FindingSpecification {
                        rule_id: "boq.value_outlier",
                        key: "valueOutlier",
                        stable_subject: format!("{}:{field}", row.source_row_id),
                        category: FindingCategory::Inconsistency,
                        severity: Severity::Medium,
                        confidence: 0.9,
                        title_params: params([
                            ("field", field.to_string()),
                            ("value", value.value.to_string()),
                            ("median", median_value.to_string()),
                        ]),
                        explanation_params: params([("peerCount", values.len().to_string())]),
                        location: source_location(
                            row,
                            value,
                            match field {
                                "quantity" => "evidence.quantity",
                                "rate" => "evidence.rate",
                                _ => "evidence.amount",
                            },
                        ),
                    });
                }
            }
        }
    }
}

fn check_pareto(runner: &mut RuleRunner<'_>, rows: &[NormalizedBoqRow]) -> Vec<ParetoAnalysis> {
    let mut groups: BTreeMap<String, Vec<(&NormalizedBoqRow, &SourceValue<Decimal>)>> =
        BTreeMap::new();
    for row in item_rows(rows) {
        if let Some(amount) = &row.amount {
            if amount.value.is_sign_positive() {
                groups
                    .entry(format!(
                        "{}\u{1f}{}\u{1f}{}",
                        row.table_index,
                        currency_key(row),
                        section_key(row)
                    ))
                    .or_default()
                    .push((row, amount));
            }
        }
    }
    let mut output = Vec::new();
    for (context, mut values) in groups {
        if values.is_empty() {
            continue;
        }
        values.sort_by_key(|entry| Reverse(entry.1.value));
        let total: Decimal = values.iter().map(|(_, amount)| amount.value).sum();
        if total.is_zero() {
            continue;
        }
        let target = total * Decimal::from(80u32) / Decimal::from(100u32);
        let mut cumulative = Decimal::ZERO;
        let mut top_count = 0usize;
        for (_, amount) in &values {
            cumulative += amount.value;
            top_count += 1;
            if cumulative >= target {
                break;
            }
        }
        let currency = values
            .first()
            .and_then(|(row, _)| row.currency.as_ref())
            .map(|currency| currency.value.clone());
        let analysis = ParetoAnalysis {
            context: context.clone(),
            currency: currency.clone(),
            total_amount: total.to_string(),
            top_item_count: top_count,
            total_item_count: values.len(),
            cumulative_share_percent: format_percent(decimal_ratio(cumulative, total)),
        };
        if let Some((row, amount)) = values.first() {
            runner.push(FindingSpecification {
                rule_id: "boq.pareto_summary",
                key: "paretoSummary",
                stable_subject: context,
                category: FindingCategory::Other,
                severity: Severity::Info,
                confidence: 1.0,
                title_params: params([
                    ("topItemCount", top_count.to_string()),
                    ("totalItemCount", values.len().to_string()),
                ]),
                explanation_params: params([
                    ("total", total.to_string()),
                    (
                        "currency",
                        currency.as_deref().unwrap_or("unknown").to_string(),
                    ),
                ]),
                location: source_location(row, *amount, "evidence.highestValueItem"),
            });
        }
        output.push(analysis);
    }
    output
}

fn item_rows(rows: &[NormalizedBoqRow]) -> impl Iterator<Item = &NormalizedBoqRow> {
    rows.iter()
        .filter(|row| row.classification == RowClassification::Item)
}

fn unit_key(row: &NormalizedBoqRow) -> String {
    row.unit.as_ref().map_or_else(
        || {
            row.unit_text
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |unit| normalize_text(&unit.value))
        },
        |unit| unit.value.canonical.clone(),
    )
}

fn currency_key(row: &NormalizedBoqRow) -> String {
    row.currency
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |currency| currency.value.clone())
}

fn decimal_key(value: Option<&SourceValue<Decimal>>) -> String {
    value.map_or_else(
        || "missing".to_string(),
        |value| value.value.normalize().to_string(),
    )
}

fn row_signature(row: &NormalizedBoqRow) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        unit_key(row),
        decimal_key(row.quantity.as_ref()),
        decimal_key(row.rate.as_ref()),
        decimal_key(row.amount.as_ref())
    )
}

fn section_key(row: &NormalizedBoqRow) -> String {
    if row.section_path.is_empty() {
        return "root".to_string();
    }
    let mut hasher = sha2::Sha256::new();
    for section in &row.section_path {
        hasher.update(normalize_text(section).as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    let mut output = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn median(values: &[Decimal]) -> Decimal {
    if values.is_empty() {
        return Decimal::ZERO;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[middle - 1] + sorted[middle]) / Decimal::from(2u32)
    } else {
        sorted[middle]
    }
}

fn decimal_ratio(numerator: Decimal, denominator: Decimal) -> f64 {
    if denominator.is_zero() {
        return 0.0;
    }
    let scaled = (numerator * Decimal::from(10_000u32) / denominator).round();
    scaled.to_string().parse::<f64>().unwrap_or_default() / 10_000.0
}

fn source_location<T>(
    row: &NormalizedBoqRow,
    value: &SourceValue<T>,
    description_key: &str,
) -> FindingLocation {
    FindingLocation {
        sheet: Some(row.sheet.clone()),
        cell: Some(value.cell.clone()),
        range: None,
        source_row_id: Some(row.source_row_id.clone()),
        original_value: Some(value.raw.clone()),
        original_formula: value.formula.clone(),
        evidence: vec![evidence(row, value, description_key)],
    }
}

fn formula_location(
    sheet: &str,
    cell: &IngestedCell,
    row: Option<&NormalizedBoqRow>,
    description_key: &str,
) -> FindingLocation {
    FindingLocation {
        sheet: Some(sheet.to_string()),
        cell: Some(cell.address.clone()),
        range: None,
        source_row_id: row.map(|row| row.source_row_id.clone()),
        original_value: Some(cell.raw_value.clone()),
        original_formula: cell.formula.clone(),
        evidence: vec![Evidence {
            sheet: sheet.to_string(),
            cell: CellRef::new(&cell.address).ok(),
            range: None,
            description_key: Some(description_key.to_string()),
            snippet: cell.formula.clone(),
        }],
    }
}

fn evidence<T>(row: &NormalizedBoqRow, value: &SourceValue<T>, description_key: &str) -> Evidence {
    Evidence {
        sheet: row.sheet.clone(),
        cell: CellRef::new(&value.cell).ok(),
        range: None,
        description_key: Some(description_key.to_string()),
        snippet: Some(value.raw.clone()),
    }
}

fn table_location(table: &DetectedBoqTable, row: u32, column: Option<u32>) -> FindingLocation {
    let address = column.map(|column| format!("{}{}", column_letter(column), row + 1));
    FindingLocation {
        sheet: Some(table.sheet.clone()),
        cell: address.clone(),
        range: table_row_range(table, row),
        source_row_id: None,
        original_value: None,
        original_formula: None,
        evidence: vec![Evidence {
            sheet: table.sheet.clone(),
            cell: address
                .as_deref()
                .and_then(|value| CellRef::new(value).ok()),
            range: table_row_range(table, row),
            description_key: Some("evidence.detectedTable".to_string()),
            snippet: None,
        }],
    }
}

fn row_location(
    table: &DetectedBoqTable,
    row: &NormalizedBoqRow,
    column: Option<u32>,
    original_value: Option<String>,
) -> FindingLocation {
    let address = column.map(|column| format!("{}{}", column_letter(column), row.row + 1));
    FindingLocation {
        sheet: Some(row.sheet.clone()),
        cell: address.clone(),
        range: table_row_range(table, row.row),
        source_row_id: Some(row.source_row_id.clone()),
        original_value,
        original_formula: None,
        evidence: vec![Evidence {
            sheet: row.sheet.clone(),
            cell: address
                .as_deref()
                .and_then(|value| CellRef::new(value).ok()),
            range: table_row_range(table, row.row),
            description_key: Some("evidence.itemRow".to_string()),
            snippet: None,
        }],
    }
}

fn workbook_location() -> FindingLocation {
    FindingLocation {
        sheet: None,
        cell: None,
        range: None,
        source_row_id: None,
        original_value: None,
        original_formula: None,
        evidence: Vec::new(),
    }
}

fn table_row_range(table: &DetectedBoqTable, row: u32) -> Option<CellRange> {
    let start = table.columns.iter().map(|column| column.index).min()?;
    let end = table.columns.iter().map(|column| column.index).max()?;
    CellRange::new(
        CellRef::new(&format!("{}{}", column_letter(start), row + 1)).ok()?,
        CellRef::new(&format!("{}{}", column_letter(end), row + 1)).ok()?,
    )
    .ok()
}

fn range_for_sources<T, const N: usize>(sources: [&SourceValue<T>; N]) -> Option<CellRange> {
    let mut addresses: Vec<&str> = sources.iter().map(|source| source.cell.as_str()).collect();
    addresses.sort_by_key(|address| cell_coordinates(address));
    let start = CellRef::new(addresses.first()?).ok()?;
    let end = CellRef::new(addresses.last()?).ok()?;
    CellRange::new(start, end).ok()
}

fn cell_coordinates(address: &str) -> (u32, u32) {
    let split = address
        .bytes()
        .position(|byte| byte.is_ascii_digit())
        .unwrap_or(address.len());
    let mut column = 0u32;
    for byte in address.as_bytes()[..split]
        .iter()
        .map(u8::to_ascii_uppercase)
    {
        column = column
            .saturating_mul(26)
            .saturating_add(u32::from(byte.saturating_sub(b'A') + 1));
    }
    let row = address[split..].parse::<u32>().unwrap_or_default();
    (row, column)
}

fn params<const N: usize>(values: [(&str, impl ToString); N]) -> BTreeMap<String, String> {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn column_letter(column: u32) -> String {
    let mut value = u64::from(column) + 1;
    let mut letters = Vec::new();
    while value > 0 {
        let remainder = ((value - 1) % 26) as u8;
        letters.push(char::from(b'A' + remainder));
        value = (value - 1) / 26;
    }
    letters.reverse();
    letters.into_iter().collect()
}

fn confidence(value: f64) -> Confidence {
    Confidence::new(value.clamp(0.0, 1.0)).unwrap_or_default()
}

fn format_percent(value: f64) -> String {
    format!("{:.1}", value.clamp(0.0, 1.0) * 100.0)
}

fn finding_sort_key(finding: &Finding) -> (String, String, String) {
    (
        finding.sheet.clone().unwrap_or_default(),
        finding
            .cell
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        finding.rule_id.clone(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use openconkit_domain::{ColumnRole, WorkbookDiagnostics};
    use openconkit_spreadsheet::{DateSystem, WorkbookFormat};

    use super::*;
    use crate::model::{DetectedColumn, DetectionOutput};
    use crate::normalization::NormalizedUnit;

    fn identity() -> RuleIdentity {
        RuleIdentity {
            project_id: ProjectId::new("test-project").expect("project"),
            source_revision_id: SourceRevisionId::new(),
            run_id: AnalysisRunId::new(),
        }
    }

    fn settings() -> RuleSettings {
        RuleSettings {
            absolute_tolerance: Decimal::new(1, 2),
            relative_tolerance: Decimal::new(1, 3),
            decimal_precision: 2,
            fuzzy_similarity_threshold: 0.8,
            low_confidence_threshold: 0.65,
        }
    }

    fn source(cell: &str, value: i64) -> SourceValue<Decimal> {
        SourceValue {
            cell: cell.to_string(),
            raw: value.to_string(),
            formula: None,
            value: Decimal::from(value),
        }
    }

    fn row(row: u32, amount: i64) -> NormalizedBoqRow {
        NormalizedBoqRow {
            source_row_id: format!("row-{row}"),
            sheet: "BOQ".into(),
            row,
            table_index: 0,
            classification: RowClassification::Item,
            classification_confidence: confidence(0.95),
            row_text: format!("Concrete item {row}"),
            section_path: vec![],
            item_code: None,
            description: Some(SourceValue {
                cell: format!("B{}", row + 1),
                raw: format!("Concrete item {row}"),
                formula: None,
                value: format!("Concrete item {row}"),
            }),
            unit_text: Some(SourceValue {
                cell: format!("C{}", row + 1),
                raw: "m2".into(),
                formula: None,
                value: "m2".into(),
            }),
            unit: Some(SourceValue {
                cell: format!("C{}", row + 1),
                raw: "m2".into(),
                formula: None,
                value: NormalizedUnit {
                    canonical: "m2".into(),
                    dimension: "area".into(),
                },
            }),
            quantity: Some(source(&format!("D{}", row + 1), 10)),
            rate_text: None,
            rate: Some(source(&format!("E{}", row + 1), 5)),
            amount: Some(source(&format!("F{}", row + 1), amount)),
            currency: None,
            error_cells: vec![],
        }
    }

    fn detection(rows: Vec<NormalizedBoqRow>) -> DetectionOutput {
        DetectionOutput {
            diagnostics: WorkbookDiagnostics {
                rule_set_version: RULE_SET_VERSION.into(),
                sheets: vec![],
                tables: vec![],
                interpretation_confidence: confidence(0.9),
                warnings: vec![],
            },
            tables: vec![DetectedBoqTable {
                sheet: "BOQ".into(),
                header_row: Some(0),
                start_row: 1,
                end_row: 10,
                columns: vec![
                    DetectedColumn {
                        index: 1,
                        role: ColumnRole::Description,
                        confidence: confidence(0.9),
                    },
                    DetectedColumn {
                        index: 2,
                        role: ColumnRole::Unit,
                        confidence: confidence(0.9),
                    },
                    DetectedColumn {
                        index: 3,
                        role: ColumnRole::Quantity,
                        confidence: confidence(0.9),
                    },
                    DetectedColumn {
                        index: 4,
                        role: ColumnRole::UnitPrice,
                        confidence: confidence(0.9),
                    },
                    DetectedColumn {
                        index: 5,
                        role: ColumnRole::TotalPrice,
                        confidence: confidence(0.9),
                    },
                ],
                confidence: confidence(0.9),
                evidence: vec![],
            }],
            rows,
        }
    }

    fn workbook() -> IngestedWorkbook {
        IngestedWorkbook {
            format: WorkbookFormat::Xlsx,
            date_system: DateSystem::Excel1900,
            sheets: vec![],
            total_cells: 0,
            total_text_bytes: 0,
        }
    }

    #[test]
    fn arithmetic_mismatch_has_stable_id_and_exact_evidence() {
        let detected = detection(vec![row(1, 49)]);
        let identity = identity();
        let first = run_rules(
            &workbook(),
            &detected,
            &identity,
            &settings(),
            &BTreeSet::new(),
            &CancellationToken::new(),
        )
        .expect("rules");
        let second = run_rules(
            &workbook(),
            &detected,
            &identity,
            &settings(),
            &BTreeSet::new(),
            &CancellationToken::new(),
        )
        .expect("rules");
        let first_mismatch = first
            .findings
            .iter()
            .find(|finding| finding.rule_id == "boq.amount_mismatch")
            .expect("mismatch");
        let second_mismatch = second
            .findings
            .iter()
            .find(|finding| finding.rule_id == "boq.amount_mismatch")
            .expect("mismatch");
        assert_eq!(first_mismatch.id, second_mismatch.id);
        assert_eq!(first_mismatch.cell.as_ref().expect("cell").as_str(), "F2");
        assert_eq!(first_mismatch.evidence.len(), 3);
    }

    #[test]
    fn missing_rules_do_not_require_quantity_for_lump_sum() {
        let mut item = row(1, 50);
        item.unit_text.as_mut().expect("unit").value = "LS".into();
        item.unit_text.as_mut().expect("unit").raw = "LS".into();
        item.unit.as_mut().expect("unit").value.canonical = "ls".into();
        item.unit.as_mut().expect("unit").value.dimension = "lump_sum".into();
        item.quantity = None;
        let output = run_rules(
            &workbook(),
            &detection(vec![item]),
            &identity(),
            &settings(),
            &BTreeSet::new(),
            &CancellationToken::new(),
        )
        .expect("rules");
        assert!(output
            .findings
            .iter()
            .all(|finding| finding.rule_id != "boq.missing.quantity"));
    }

    #[test]
    fn included_rate_text_does_not_create_missing_price_findings() {
        let mut item = row(1, 50);
        item.rate = None;
        item.amount = None;
        item.rate_text = Some(SourceValue {
            cell: "E2".into(),
            raw: "Included".into(),
            formula: None,
            value: "Included".into(),
        });
        let output = run_rules(
            &workbook(),
            &detection(vec![item]),
            &identity(),
            &settings(),
            &BTreeSet::new(),
            &CancellationToken::new(),
        )
        .expect("rules");
        assert!(output.findings.iter().all(|finding| {
            finding.rule_id != "boq.missing.rate" && finding.rule_id != "boq.missing.amount"
        }));
    }
}
