//! Confidence-aware, template-independent BOQ structure detection.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use openconkit_domain::{
    ClassifiedRow, ColumnRole, ColumnRoleAssignment, Confidence, DetectedTable, RowClassification,
    SheetInventory, SheetVisibility as DomainSheetVisibility, WorkbookDiagnostics,
};
use openconkit_spreadsheet::{
    IngestedCell, IngestedSheet, IngestedWorkbook, NormalizedCellValue, SheetKind, SheetVisibility,
};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};

use crate::model::{
    DetectedBoqTable, DetectedColumn, DetectionOutput, NormalizedBoqRow, SourceValue,
};
use crate::normalization::{detect_currency, normalize_text, normalize_unit, parse_number};

pub(crate) const RULE_SET_VERSION: &str = "2026.07.2";

#[derive(Default)]
struct ColumnStats {
    non_empty: usize,
    numeric: usize,
    text: usize,
    long_text: usize,
    unit: usize,
    currency: usize,
}

/// Detect candidate BOQ tables and normalize their rows.
pub(crate) fn detect(workbook: &IngestedWorkbook) -> DetectionOutput {
    let mut tables = Vec::new();
    let mut rows = Vec::new();

    for sheet in &workbook.sheets {
        if sheet.kind != SheetKind::Worksheet || sheet.cells.is_empty() {
            continue;
        }
        let row_map = cells_by_row(sheet);
        for (segment_start, segment_end) in candidate_segments(&row_map) {
            let segment_rows: Vec<u32> = row_map
                .range(segment_start..=segment_end)
                .map(|(row, _)| *row)
                .collect();
            if segment_rows.len() < 2 {
                continue;
            }
            let header = detect_header(&row_map, &segment_rows);
            let data_start = header
                .as_ref()
                .map_or(segment_start, |candidate| candidate.row.saturating_add(1));
            if data_start > segment_end {
                continue;
            }
            let (columns, mut evidence) =
                infer_columns(&row_map, data_start, segment_end, header.as_ref());
            let recognized = columns
                .iter()
                .filter(|column| column.role != ColumnRole::Unknown)
                .count();
            if recognized < 2 {
                continue;
            }
            if header.is_none() {
                evidence.push("headerless_distribution_inference".to_string());
            }
            if header.as_ref().is_some_and(|header| {
                sheet
                    .merged_regions
                    .iter()
                    .any(|region| region.start_row <= header.row && region.end_row >= header.row)
            }) {
                evidence.push("merged_header_region".to_string());
            }
            let confidence_value = table_confidence(&columns, header.as_ref());
            let table_index = tables.len();
            let mut table = DetectedBoqTable {
                sheet: sheet.name.clone(),
                header_row: header.as_ref().map(|candidate| candidate.row),
                start_row: data_start,
                end_row: segment_end,
                columns,
                confidence: confidence(confidence_value),
                evidence,
            };
            let first_row_index = rows.len();
            for row in data_start..=segment_end {
                let Some(cells) = row_map.get(&row) else {
                    continue;
                };
                rows.push(normalize_row(&table, table_index, row, cells));
            }
            if reconstruct_section_paths(&mut rows[first_row_index..]) {
                table
                    .evidence
                    .push("section_hierarchy_reconstructed".to_string());
            }
            tables.push(table);
        }
    }

    let overall = if tables.is_empty() {
        0.0
    } else {
        tables
            .iter()
            .map(|table| table.confidence.value())
            .sum::<f64>()
            / tables.len() as f64
    };
    let diagnostics = build_diagnostics(workbook, &tables, &rows, overall);
    DetectionOutput {
        diagnostics,
        tables,
        rows,
    }
}

fn cells_by_row(sheet: &IngestedSheet) -> BTreeMap<u32, Vec<&IngestedCell>> {
    let mut rows: BTreeMap<u32, Vec<&IngestedCell>> = BTreeMap::new();
    for cell in &sheet.cells {
        rows.entry(cell.row).or_default().push(cell);
    }
    for cells in rows.values_mut() {
        cells.sort_unstable_by_key(|cell| cell.column);
    }
    rows
}

fn candidate_segments(rows: &BTreeMap<u32, Vec<&IngestedCell>>) -> Vec<(u32, u32)> {
    let mut output = Vec::new();
    let mut start = None;
    let mut previous = None;
    for row in rows.keys().copied() {
        if let Some(previous_row) = previous {
            // A single blank line is common between BOQ sections and must
            // not fragment one table. Two consecutive blank rows form the
            // conservative boundary between candidate regions.
            if row.saturating_sub(previous_row) >= 3 {
                if let Some(start_row) = start {
                    output.push((start_row, previous_row));
                }
                start = Some(row);
            }
        } else {
            start = Some(row);
        }
        previous = Some(row);
    }
    if let (Some(start_row), Some(end_row)) = (start, previous) {
        output.push((start_row, end_row));
    }
    output
}

struct HeaderCandidate {
    row: u32,
    roles: BTreeMap<u32, (ColumnRole, f64)>,
    score: f64,
}

fn detect_header(
    rows: &BTreeMap<u32, Vec<&IngestedCell>>,
    segment_rows: &[u32],
) -> Option<HeaderCandidate> {
    segment_rows
        .iter()
        .take(8)
        .filter_map(|row| {
            let cells = rows.get(row)?;
            let mut roles = BTreeMap::new();
            let mut text_cells = 0usize;
            for cell in cells {
                if let Some(text) = cell_text(cell) {
                    text_cells += 1;
                    if let Some((role, confidence)) = header_role(text) {
                        roles.insert(cell.column, (role, confidence));
                    }
                }
            }
            let distinct_roles: BTreeSet<ColumnRoleKey> = roles
                .values()
                .map(|(role, _)| ColumnRoleKey::from(*role))
                .collect();
            if distinct_roles.len() < 2 {
                return None;
            }
            let text_ratio = text_cells as f64 / cells.len().max(1) as f64;
            let alias_ratio = roles.len() as f64 / cells.len().max(1) as f64;
            let score = (0.55 + 0.25 * text_ratio + 0.20 * alias_ratio).min(1.0);
            Some(HeaderCandidate {
                row: *row,
                roles,
                score,
            })
        })
        .max_by(|left, right| left.score.total_cmp(&right.score))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ColumnRoleKey {
    Item,
    Description,
    Unit,
    Quantity,
    Rate,
    Amount,
    Currency,
    Notes,
    Unknown,
}

impl From<ColumnRole> for ColumnRoleKey {
    fn from(role: ColumnRole) -> Self {
        match role {
            ColumnRole::ItemNumber => Self::Item,
            ColumnRole::Description => Self::Description,
            ColumnRole::Unit => Self::Unit,
            ColumnRole::Quantity => Self::Quantity,
            ColumnRole::UnitPrice => Self::Rate,
            ColumnRole::TotalPrice => Self::Amount,
            ColumnRole::Currency => Self::Currency,
            ColumnRole::Notes => Self::Notes,
            ColumnRole::Unknown => Self::Unknown,
        }
    }
}

fn header_role(raw: &str) -> Option<(ColumnRole, f64)> {
    let value = normalize_text(raw);
    let compact = value.replace(' ', "");
    let role = if matches!(
        compact.as_str(),
        "item"
            | "itemno"
            | "no"
            | "serial"
            | "ref"
            | "reference"
            | "code"
            | "رقم"
            | "كود"
            | "البند"
    ) {
        ColumnRole::ItemNumber
    } else if matches!(
        compact.as_str(),
        "description"
            | "itemdescription"
            | "scope"
            | "workdescription"
            | "البيان"
            | "الوصف"
            | "وصفالبند"
            | "بندالاعمال"
    ) {
        ColumnRole::Description
    } else if matches!(
        compact.as_str(),
        "unit" | "uom" | "الوحده" | "وحده" | "وحدهالقياس"
    ) {
        ColumnRole::Unit
    } else if matches!(
        compact.as_str(),
        "qty" | "quantity" | "quantities" | "الكميه" | "كميه"
    ) {
        ColumnRole::Quantity
    } else if matches!(
        compact.as_str(),
        "rate" | "unitrate" | "unitprice" | "price" | "السعر" | "سعرالوحده" | "الفئه"
    ) {
        ColumnRole::UnitPrice
    } else if matches!(
        compact.as_str(),
        "amount" | "totalamount" | "totalprice" | "value" | "القيمه" | "الاجمالي" | "المبلغ"
    ) {
        ColumnRole::TotalPrice
    } else if matches!(compact.as_str(), "currency" | "curr" | "العمله" | "عمله") {
        ColumnRole::Currency
    } else if matches!(
        compact.as_str(),
        "remarks" | "remark" | "notes" | "note" | "ملاحظات" | "ملاحظه"
    ) {
        ColumnRole::Notes
    } else {
        return None;
    };
    Some((role, 0.97))
}

fn infer_columns(
    rows: &BTreeMap<u32, Vec<&IngestedCell>>,
    start_row: u32,
    end_row: u32,
    header: Option<&HeaderCandidate>,
) -> (Vec<DetectedColumn>, Vec<String>) {
    let mut stats: BTreeMap<u32, ColumnStats> = BTreeMap::new();
    for (_, cells) in rows.range(start_row..=end_row) {
        for cell in cells {
            let stat = stats.entry(cell.column).or_default();
            stat.non_empty += 1;
            if cell_number(cell).is_some() {
                stat.numeric += 1;
            }
            if let Some(text) = cell_text(cell) {
                stat.text += 1;
                if normalize_text(text).len() >= 16 {
                    stat.long_text += 1;
                }
                if normalize_unit(text).is_some() {
                    stat.unit += 1;
                }
                if detect_currency(text).is_some() {
                    stat.currency += 1;
                }
            }
        }
    }
    if let Some(header) = header {
        for column in header.roles.keys() {
            stats.entry(*column).or_default();
        }
    }
    let mut assignments: BTreeMap<u32, (ColumnRole, f64)> = BTreeMap::new();
    if let Some(header) = header {
        assignments.extend(header.roles.iter().map(|(column, value)| (*column, *value)));
    }

    assign_best_ratio(
        &stats,
        &mut assignments,
        ColumnRole::Description,
        |stat| stat.long_text,
        0.45,
        0.58,
    );
    assign_best_ratio(
        &stats,
        &mut assignments,
        ColumnRole::Unit,
        |stat| stat.unit,
        0.45,
        0.70,
    );
    assign_best_ratio(
        &stats,
        &mut assignments,
        ColumnRole::Currency,
        |stat| stat.currency,
        0.45,
        0.75,
    );

    let mut evidence = Vec::new();
    if let Some((quantity, rate, amount, score)) =
        infer_arithmetic_columns(rows, start_row, end_row, &stats, &assignments)
    {
        for (column, role) in [
            (quantity, ColumnRole::Quantity),
            (rate, ColumnRole::UnitPrice),
            (amount, ColumnRole::TotalPrice),
        ] {
            assignments
                .entry(column)
                .or_insert((role, (0.55 + 0.4 * score).min(0.95)));
        }
        evidence.push("quantity_rate_amount_relationship".to_string());
    }

    if !assignments
        .values()
        .any(|(role, _)| *role == ColumnRole::ItemNumber)
    {
        if let Some((column, _)) = stats
            .iter()
            .filter(|(column, stat)| {
                !assignments.contains_key(column)
                    && stat.non_empty > 0
                    && (stat.numeric + stat.text) * 100 / stat.non_empty >= 80
            })
            .min_by_key(|(column, _)| *column)
        {
            assignments.insert(*column, (ColumnRole::ItemNumber, 0.52));
        }
    }

    let columns = stats
        .keys()
        .map(|column| {
            let (role, score) = assignments
                .get(column)
                .copied()
                .unwrap_or((ColumnRole::Unknown, 0.25));
            DetectedColumn {
                index: *column,
                role,
                confidence: confidence(score),
            }
        })
        .collect();
    if header.is_some() {
        evidence.push("bilingual_header_aliases".to_string());
    }
    (columns, evidence)
}

fn assign_best_ratio(
    stats: &BTreeMap<u32, ColumnStats>,
    assignments: &mut BTreeMap<u32, (ColumnRole, f64)>,
    role: ColumnRole,
    numerator: impl Fn(&ColumnStats) -> usize,
    minimum_ratio: f64,
    base_confidence: f64,
) {
    if assignments.values().any(|(assigned, _)| *assigned == role) {
        return;
    }
    let best = stats
        .iter()
        .filter(|(column, stat)| !assignments.contains_key(column) && stat.non_empty > 0)
        .map(|(column, stat)| {
            (
                *column,
                numerator(stat) as f64 / stat.non_empty.max(1) as f64,
            )
        })
        .max_by(|left, right| left.1.total_cmp(&right.1));
    if let Some((column, ratio)) = best {
        if ratio >= minimum_ratio {
            assignments.insert(column, (role, (base_confidence + ratio * 0.2).min(0.95)));
        }
    }
}

fn infer_arithmetic_columns(
    rows: &BTreeMap<u32, Vec<&IngestedCell>>,
    start_row: u32,
    end_row: u32,
    stats: &BTreeMap<u32, ColumnStats>,
    assignments: &BTreeMap<u32, (ColumnRole, f64)>,
) -> Option<(u32, u32, u32, f64)> {
    let numeric_columns: Vec<u32> = stats
        .iter()
        .filter(|(column, stat)| {
            !assignments.contains_key(column)
                && stat.non_empty >= 2
                && stat.numeric as f64 / stat.non_empty as f64 >= 0.5
        })
        .map(|(column, _)| *column)
        .collect();
    let mut best = None;
    for (quantity_index, quantity) in numeric_columns.iter().enumerate() {
        for (rate_index, rate) in numeric_columns.iter().enumerate().skip(quantity_index + 1) {
            for amount in numeric_columns.iter().skip(rate_index + 1) {
                let mut comparable = 0usize;
                let mut matching = 0usize;
                for (_, cells) in rows.range(start_row..=end_row) {
                    let values = (
                        number_at(cells, *quantity),
                        number_at(cells, *rate),
                        number_at(cells, *amount),
                    );
                    if let (Some(quantity), Some(rate), Some(amount)) = values {
                        comparable += 1;
                        let expected = quantity * rate;
                        let tolerance = Decimal::new(1, 2).max(expected.abs() * Decimal::new(1, 3));
                        if (expected - amount).abs() <= tolerance {
                            matching += 1;
                        }
                    }
                }
                if comparable >= 2 {
                    let score = matching as f64 / comparable as f64;
                    if best
                        .as_ref()
                        .is_none_or(|(_, _, _, best_score)| score > *best_score)
                    {
                        best = Some((*quantity, *rate, *amount, score));
                    }
                }
            }
        }
    }
    best.filter(|(_, _, _, score)| *score >= 0.35)
}

fn table_confidence(columns: &[DetectedColumn], header: Option<&HeaderCandidate>) -> f64 {
    let recognized: Vec<&DetectedColumn> = columns
        .iter()
        .filter(|column| column.role != ColumnRole::Unknown)
        .collect();
    if recognized.is_empty() {
        return 0.0;
    }
    let column_confidence = recognized
        .iter()
        .map(|column| column.confidence.value())
        .sum::<f64>()
        / recognized.len() as f64;
    let coverage = (recognized.len() as f64 / 6.0).min(1.0);
    let header_confidence = header.map_or(0.5, |candidate| candidate.score);
    (column_confidence * 0.55 + coverage * 0.25 + header_confidence * 0.20).min(1.0)
}

fn normalize_row(
    table: &DetectedBoqTable,
    table_index: usize,
    row: u32,
    cells: &[&IngestedCell],
) -> NormalizedBoqRow {
    let classification = classify_row(table, cells);
    let item_code = text_source(table.column(ColumnRole::ItemNumber), cells);
    let description = text_source(table.column(ColumnRole::Description), cells);
    let unit_text = text_source(table.column(ColumnRole::Unit), cells);
    let unit = unit_text.clone().and_then(|source| {
        normalize_unit(&source.raw).map(|value| SourceValue {
            cell: source.cell,
            raw: source.raw,
            formula: source.formula,
            value,
        })
    });
    let quantity = number_source(table.column(ColumnRole::Quantity), cells);
    let rate_text = text_source(table.column(ColumnRole::UnitPrice), cells);
    let rate = number_source(table.column(ColumnRole::UnitPrice), cells);
    let amount = number_source(table.column(ColumnRole::TotalPrice), cells);
    let currency = text_source(table.column(ColumnRole::Currency), cells)
        .and_then(currency_source)
        .or_else(|| {
            cells.iter().find_map(|cell| {
                detect_currency(&cell.raw_value).map(|value| SourceValue {
                    cell: cell.address.clone(),
                    raw: cell.raw_value.clone(),
                    formula: cell.formula.clone(),
                    value: value.code,
                })
            })
        });
    let error_cells = cells
        .iter()
        .filter_map(|cell| match &cell.normalized_value {
            NormalizedCellValue::Error(error) => Some(SourceValue {
                cell: cell.address.clone(),
                raw: cell.raw_value.clone(),
                formula: cell.formula.clone(),
                value: error.clone(),
            }),
            NormalizedCellValue::Empty if cell.formula.is_some() => Some(SourceValue {
                cell: cell.address.clone(),
                raw: cell.raw_value.clone(),
                formula: cell.formula.clone(),
                value: "missing_cached_formula_result".to_string(),
            }),
            _ => None,
        })
        .collect();
    NormalizedBoqRow {
        source_row_id: stable_row_id(&table.sheet, row),
        sheet: table.sheet.clone(),
        row,
        table_index,
        classification: classification.0,
        classification_confidence: confidence(classification.1),
        row_text: cells
            .iter()
            .filter_map(|cell| cell_text(cell))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        section_path: Vec::new(),
        item_code,
        description,
        unit_text,
        unit,
        quantity,
        rate_text,
        rate,
        amount,
        currency,
        error_cells,
    }
}

fn currency_source(source: SourceValue<String>) -> Option<SourceValue<String>> {
    detect_currency(&source.raw).map(|value| SourceValue {
        cell: source.cell,
        raw: source.raw,
        formula: source.formula,
        value: value.code,
    })
}

fn reconstruct_section_paths(rows: &mut [NormalizedBoqRow]) -> bool {
    let mut heading = None;
    let mut subheading = None;
    let mut assigned = false;
    for row in rows {
        match row.classification {
            RowClassification::Heading => {
                heading = non_empty_text(&row.row_text);
                subheading = None;
            }
            RowClassification::Subheading => {
                subheading = non_empty_text(&row.row_text);
            }
            RowClassification::Item | RowClassification::Subtotal | RowClassification::Total => {
                row.section_path = heading.iter().chain(subheading.iter()).cloned().collect();
                assigned |= !row.section_path.is_empty();
            }
            _ => {}
        }
    }
    assigned
}

fn non_empty_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn classify_row(table: &DetectedBoqTable, cells: &[&IngestedCell]) -> (RowClassification, f64) {
    let text = cells
        .iter()
        .filter_map(|cell| cell_text(cell))
        .map(normalize_text)
        .collect::<Vec<_>>()
        .join(" ");
    let numeric_count = cells
        .iter()
        .filter(|cell| cell_number(cell).is_some())
        .count();
    let text_count = cells
        .iter()
        .filter(|cell| cell_text(cell).is_some())
        .count();

    if contains_any(
        &text,
        &[
            "grand total",
            "final total",
            "الاجمالي العام",
            "المجموع الكلي",
        ],
    ) {
        return (RowClassification::Total, 0.96);
    }
    if contains_any(
        &text,
        &["subtotal", "sub total", "اجمالي فرعي", "مجموع فرعي"],
    ) {
        return (RowClassification::Subtotal, 0.95);
    }
    if contains_any(
        &text,
        &["note", "notes", "remark", "ملاحظه", "ملاحظات", "تنويه"],
    ) && numeric_count == 0
    {
        return (RowClassification::Note, 0.88);
    }
    let has_description = table
        .column(ColumnRole::Description)
        .and_then(|column| cell_at(cells, column))
        .and_then(cell_text)
        .is_some_and(|value| !value.trim().is_empty());
    if has_description && numeric_count > 0 {
        return (RowClassification::Item, 0.92);
    }
    if numeric_count >= 2 {
        return (RowClassification::Item, 0.72);
    }
    if text_count == 1 && numeric_count == 0 {
        return (RowClassification::Heading, 0.78);
    }
    if text_count > 0 && numeric_count == 0 {
        return (RowClassification::Subheading, 0.62);
    }
    (RowClassification::Unknown, 0.35)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| text.contains(&normalize_text(needle)))
}

fn text_source(column: Option<u32>, cells: &[&IngestedCell]) -> Option<SourceValue<String>> {
    let cell = cell_at(cells, column?)?;
    let text = cell_text(cell)?.trim();
    if text.is_empty() {
        return None;
    }
    Some(SourceValue {
        cell: cell.address.clone(),
        raw: cell.raw_value.clone(),
        formula: cell.formula.clone(),
        value: text.to_string(),
    })
}

fn number_source(column: Option<u32>, cells: &[&IngestedCell]) -> Option<SourceValue<Decimal>> {
    let cell = cell_at(cells, column?)?;
    let value = cell_number(cell)?;
    Some(SourceValue {
        cell: cell.address.clone(),
        raw: cell.raw_value.clone(),
        formula: cell.formula.clone(),
        value,
    })
}

fn cell_at<'a>(cells: &[&'a IngestedCell], column: u32) -> Option<&'a IngestedCell> {
    cells.iter().find(|cell| cell.column == column).copied()
}

fn number_at(cells: &[&IngestedCell], column: u32) -> Option<Decimal> {
    cell_at(cells, column).and_then(cell_number)
}

fn cell_number(cell: &IngestedCell) -> Option<Decimal> {
    match &cell.normalized_value {
        NormalizedCellValue::Integer(value) => Some(Decimal::from(*value)),
        NormalizedCellValue::Number(value) => Decimal::from_str(value).ok(),
        NormalizedCellValue::Text(value) => parse_number(value).map(|parsed| parsed.value),
        _ => None,
    }
}

fn cell_text(cell: &IngestedCell) -> Option<&str> {
    match &cell.normalized_value {
        NormalizedCellValue::Text(value) => Some(value),
        _ => None,
    }
}

fn stable_row_id(sheet: &str, row: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sheet.as_bytes());
    hasher.update([0]);
    hasher.update(row.to_le_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(24);
    for byte in digest.iter().take(12) {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn build_diagnostics(
    workbook: &IngestedWorkbook,
    tables: &[DetectedBoqTable],
    rows: &[NormalizedBoqRow],
    overall: f64,
) -> WorkbookDiagnostics {
    let sheets = workbook
        .sheets
        .iter()
        .map(|sheet| {
            let (used_rows, used_columns) = sheet.used_range.map_or((0, 0), |range| {
                (
                    range.end_row.saturating_sub(range.start_row) + 1,
                    range.end_column.saturating_sub(range.start_column) + 1,
                )
            });
            SheetInventory {
                index: sheet.index,
                name: sheet.name.clone(),
                visibility: match sheet.visibility {
                    SheetVisibility::Visible => DomainSheetVisibility::Visible,
                    SheetVisibility::Hidden => DomainSheetVisibility::Hidden,
                    SheetVisibility::VeryHidden => DomainSheetVisibility::VeryHidden,
                },
                used_rows,
                used_columns,
                non_empty_cells: u32::try_from(sheet.cells.len()).unwrap_or(u32::MAX),
                detected_tables: u32::try_from(
                    tables
                        .iter()
                        .filter(|table| table.sheet == sheet.name)
                        .count(),
                )
                .unwrap_or(u32::MAX),
            }
        })
        .collect();
    let domain_tables = tables
        .iter()
        .enumerate()
        .map(|(table_index, table)| DetectedTable {
            sheet: table.sheet.clone(),
            header_row: table.header_row,
            start_row: table.start_row,
            end_row: table.end_row,
            columns: table
                .columns
                .iter()
                .map(|column| ColumnRoleAssignment {
                    column_index: column.index,
                    column_letter: column_letter(column.index),
                    role: column.role,
                    confidence: column.confidence,
                })
                .collect(),
            rows: rows
                .iter()
                .filter(|row| row.table_index == table_index)
                .map(|row| ClassifiedRow {
                    row_index: row.row,
                    classification: row.classification,
                    confidence: row.classification_confidence,
                })
                .collect(),
            interpretation_confidence: table.confidence,
            evidence: table.evidence.clone(),
        })
        .collect();
    let mut warnings = Vec::new();
    if tables.is_empty() {
        warnings.push("no_tables_detected".to_string());
    }
    if tables.iter().any(|table| table.confidence.value() < 0.65) {
        warnings.push("low_confidence_structure".to_string());
    }
    if workbook
        .sheets
        .iter()
        .any(|sheet| sheet.hidden_rows.is_none() || sheet.hidden_columns.is_none())
    {
        warnings.push("row_column_visibility_unavailable".to_string());
    }
    WorkbookDiagnostics {
        rule_set_version: RULE_SET_VERSION.to_string(),
        sheets,
        tables: domain_tables,
        interpretation_confidence: confidence(overall),
        warnings,
    }
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use openconkit_spreadsheet::{
        CellRegion, DateSystem, IngestedCell, IngestedSheet, IngestedWorkbook, NormalizedCellValue,
        SheetKind, SheetVisibility, WorkbookFormat,
    };

    use super::*;

    fn cell(row: u32, column: u32, value: NormalizedCellValue) -> IngestedCell {
        let raw_value = match &value {
            NormalizedCellValue::Integer(value) => value.to_string(),
            NormalizedCellValue::Number(value) | NormalizedCellValue::Text(value) => value.clone(),
            _ => String::new(),
        };
        IngestedCell {
            row,
            column,
            address: format!("{}{}", column_letter(column), row + 1),
            raw_value,
            normalized_value: value,
            displayed_value: None,
            formula: None,
        }
    }

    fn workbook(cells: Vec<IngestedCell>) -> IngestedWorkbook {
        IngestedWorkbook {
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
                    end_row: 10,
                    end_column: 5,
                }),
                used_range: Some(CellRegion {
                    start_row: 0,
                    start_column: 0,
                    end_row: 10,
                    end_column: 5,
                }),
                merged_regions: vec![],
                hidden_rows: None,
                hidden_columns: None,
                cells,
            }],
            total_cells: 0,
            total_text_bytes: 0,
        }
    }

    #[test]
    fn detects_english_header_and_item_row() {
        let model = workbook(vec![
            cell(0, 0, NormalizedCellValue::Text("Item".into())),
            cell(0, 1, NormalizedCellValue::Text("Description".into())),
            cell(0, 2, NormalizedCellValue::Text("Unit".into())),
            cell(0, 3, NormalizedCellValue::Text("Quantity".into())),
            cell(0, 4, NormalizedCellValue::Text("Rate".into())),
            cell(0, 5, NormalizedCellValue::Text("Amount".into())),
            cell(1, 0, NormalizedCellValue::Text("A1".into())),
            cell(1, 1, NormalizedCellValue::Text("Concrete wall".into())),
            cell(1, 2, NormalizedCellValue::Text("m2".into())),
            cell(1, 3, NormalizedCellValue::Integer(10)),
            cell(1, 4, NormalizedCellValue::Integer(5)),
            cell(1, 5, NormalizedCellValue::Integer(50)),
        ]);
        let detected = detect(&model);
        assert_eq!(detected.tables.len(), 1);
        assert_eq!(detected.tables[0].header_row, Some(0));
        assert_eq!(detected.tables[0].column(ColumnRole::Description), Some(1));
        assert_eq!(detected.rows[0].classification, RowClassification::Item);
        assert_eq!(
            detected.rows[0].amount.as_ref().expect("amount").value,
            Decimal::from(50)
        );
    }

    #[test]
    fn detects_arabic_headers() {
        let model = workbook(vec![
            cell(0, 0, NormalizedCellValue::Text("رقم".into())),
            cell(0, 1, NormalizedCellValue::Text("البيان".into())),
            cell(0, 2, NormalizedCellValue::Text("الوحدة".into())),
            cell(0, 3, NormalizedCellValue::Text("الكمية".into())),
            cell(0, 4, NormalizedCellValue::Text("سعر الوحدة".into())),
            cell(0, 5, NormalizedCellValue::Text("الإجمالي".into())),
            cell(1, 1, NormalizedCellValue::Text("أعمال الخرسانة".into())),
            cell(1, 2, NormalizedCellValue::Text("متر مربع".into())),
            cell(1, 3, NormalizedCellValue::Text("١٠".into())),
            cell(1, 4, NormalizedCellValue::Text("٥".into())),
            cell(1, 5, NormalizedCellValue::Text("٥٠".into())),
        ]);
        let detected = detect(&model);
        assert_eq!(detected.tables.len(), 1);
        assert_eq!(detected.tables[0].column(ColumnRole::UnitPrice), Some(4));
        assert_eq!(detected.tables[0].column(ColumnRole::TotalPrice), Some(5));
    }

    #[test]
    fn headerless_inference_uses_value_shapes_and_arithmetic() {
        let mut cells = Vec::new();
        for row in 0..3 {
            cells.push(cell(
                row,
                0,
                NormalizedCellValue::Text(format!("A{}", row + 1)),
            ));
            cells.push(cell(
                row,
                1,
                NormalizedCellValue::Text(format!("Long work description number {}", row + 1)),
            ));
            cells.push(cell(row, 2, NormalizedCellValue::Text("m2".into())));
            cells.push(cell(row, 3, NormalizedCellValue::Integer(10)));
            cells.push(cell(row, 4, NormalizedCellValue::Integer(5)));
            cells.push(cell(row, 5, NormalizedCellValue::Integer(50)));
        }
        let detected = detect(&workbook(cells));
        assert_eq!(detected.tables[0].header_row, None);
        assert_eq!(detected.tables[0].column(ColumnRole::Quantity), Some(3));
        assert_eq!(detected.tables[0].column(ColumnRole::UnitPrice), Some(4));
        assert_eq!(detected.tables[0].column(ColumnRole::TotalPrice), Some(5));
    }
}
