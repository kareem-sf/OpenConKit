//! Macro-free Excel report renderer.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};
use sha2::{Digest, Sha256};

use crate::{ReportDocument, ReportFinding, ReportingError};

const HEADER_COLOR: Color = Color::RGB(0x176B68);
const ACCENT_COLOR: Color = Color::RGB(0xD6ECE9);

/// Render and atomically publish a new macro-free XLSX report.
///
/// Returns the lowercase SHA-256 of the generated artifact. Existing
/// destinations are never replaced.
pub fn write_xlsx_report(path: &Path, report: &ReportDocument) -> Result<String, ReportingError> {
    report.validate()?;
    let mut workbook = Workbook::new();
    write_summary(&mut workbook, report)?;
    write_findings(&mut workbook, report)?;
    write_detection(&mut workbook, report)?;
    if !report.pareto.is_empty() {
        write_pareto(&mut workbook, report)?;
    }
    write_source_metadata(&mut workbook, report)?;
    if let Some(commentary) = report.ai_commentary.as_deref() {
        write_ai_review(&mut workbook, report, commentary)?;
    }

    let bytes = workbook.save_to_buffer()?;
    publish_new_file(path, &bytes)?;
    Ok(hex_sha256(&bytes))
}

fn write_summary(workbook: &mut Workbook, report: &ReportDocument) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    configure_sheet(
        sheet,
        &report.labels.executive_summary,
        report.right_to_left,
    )?;
    let title = title_format();
    let label = label_format();
    let value = value_format();

    sheet.merge_range(0, 0, 0, 3, &safe_text(&report.labels.report_title), &title)?;
    let rows = [
        (
            report.labels.item_rows.as_str(),
            report.summary.item_rows.to_string(),
        ),
        (
            report.labels.finding_count.as_str(),
            report.summary.finding_count.to_string(),
        ),
        (
            report.labels.interpretation_confidence.as_str(),
            format!("{:.1}%", report.summary.interpretation_confidence * 100.0),
        ),
    ];
    for (index, (key, row_value)) in rows.iter().enumerate() {
        let row = checked_row(index + 2)?;
        sheet.write_string_with_format(row, 0, safe_text(key), &label)?;
        sheet.write_string_with_format(row, 1, safe_text(row_value), &value)?;
    }

    let mut row = 7_u32;
    sheet.write_string_with_format(row, 0, safe_text(&report.labels.severity), &header_format())?;
    sheet.write_string_with_format(
        row,
        1,
        safe_text(&report.labels.finding_count),
        &header_format(),
    )?;
    row = row.saturating_add(1);
    for (severity, count) in &report.summary.severity_counts {
        sheet.write_string(row, 0, safe_text(severity))?;
        sheet.write_number(row, 1, *count as f64)?;
        row = row.saturating_add(1);
    }

    row = row.saturating_add(2);
    sheet.write_string_with_format(row, 0, safe_text(&report.labels.category), &header_format())?;
    sheet.write_string_with_format(
        row,
        1,
        safe_text(&report.labels.finding_count),
        &header_format(),
    )?;
    row = row.saturating_add(1);
    for (category, count) in &report.summary.category_counts {
        sheet.write_string(row, 0, safe_text(category))?;
        sheet.write_number(row, 1, *count as f64)?;
        row = row.saturating_add(1);
    }

    if !report.limitations.is_empty() {
        row = row.saturating_add(2);
        sheet.write_string_with_format(
            row,
            0,
            safe_text(&report.labels.limitations),
            &header_format(),
        )?;
        row = row.saturating_add(1);
        for limitation in &report.limitations {
            sheet.merge_range(row, 0, row, 3, &safe_text(limitation), &wrapped_format())?;
            row = row.saturating_add(1);
        }
    }
    sheet.set_column_width(0, 34)?;
    sheet.set_column_width(1, 22)?;
    sheet.set_column_width(2, 18)?;
    sheet.set_column_width(3, 18)?;
    Ok(())
}

fn write_findings(workbook: &mut Workbook, report: &ReportDocument) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    configure_sheet(sheet, &report.labels.findings, report.right_to_left)?;
    let headers = [
        &report.labels.severity,
        &report.labels.category,
        &report.labels.confidence,
        &report.labels.rule,
        &report.labels.title,
        &report.labels.explanation,
        &report.labels.action,
        &report.labels.sheet,
        &report.labels.cell,
        &report.labels.evidence,
        &report.labels.value,
        &report.labels.deterministic_origin,
    ];
    write_headers(sheet, &headers)?;
    for (index, finding) in report.findings.iter().enumerate() {
        let row = checked_row(index + 1)?;
        write_finding_row(sheet, row, finding, &report.labels.not_available)?;
    }
    if !report.findings.is_empty() {
        sheet.autofilter(0, 0, checked_row(report.findings.len())?, 11)?;
    }
    sheet.set_freeze_panes(1, 0)?;
    let widths = [
        12.0, 17.0, 12.0, 25.0, 34.0, 52.0, 42.0, 20.0, 12.0, 52.0, 22.0, 16.0,
    ];
    set_widths(sheet, &widths)?;
    Ok(())
}

fn write_finding_row(
    sheet: &mut Worksheet,
    row: u32,
    finding: &ReportFinding,
    not_available: &str,
) -> Result<(), XlsxError> {
    let evidence = finding
        .evidence
        .iter()
        .map(|item| {
            let snippet = item
                .snippet
                .as_deref()
                .map(|value| format!(" - {value}"))
                .unwrap_or_default();
            format!(
                "{}!{}: {}{}",
                item.sheet, item.reference, item.description, snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let original = [
        finding.original_value.as_deref(),
        finding.original_formula.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" | ");
    let values = [
        finding.severity.as_str(),
        finding.category.as_str(),
        "",
        finding.rule_id.as_str(),
        finding.title.as_str(),
        finding.explanation.as_str(),
        finding.action.as_deref().unwrap_or(not_available),
        finding.sheet.as_deref().unwrap_or(not_available),
        finding.cell.as_deref().unwrap_or(not_available),
        evidence.as_str(),
        original.as_str(),
        finding.origin.as_str(),
    ];
    for (column, value) in values.iter().enumerate() {
        let column = checked_column(column)?;
        if column == 2 {
            sheet.write_number_with_format(
                row,
                column,
                finding.confidence_percent,
                &percent_format(),
            )?;
        } else {
            sheet.write_string_with_format(row, column, safe_text(value), &wrapped_format())?;
        }
    }
    Ok(())
}

fn write_detection(workbook: &mut Workbook, report: &ReportDocument) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    configure_sheet(sheet, &report.labels.detection, report.right_to_left)?;
    let headers = [
        &report.labels.sheet,
        &report.labels.table_range,
        &report.labels.mapped_columns,
        &report.labels.confidence,
        &report.labels.evidence,
        &report.labels.warning,
    ];
    write_headers(sheet, &headers)?;
    for (index, detection) in report.detections.iter().enumerate() {
        let row = checked_row(index + 1)?;
        let values = [
            detection.sheet.as_str(),
            detection.table_range.as_str(),
            detection.mapped_columns.as_str(),
            "",
            detection.evidence.as_str(),
            detection.warning.as_deref().unwrap_or(""),
        ];
        for (column, value) in values.iter().enumerate() {
            let column = checked_column(column)?;
            if column == 3 {
                sheet.write_number_with_format(
                    row,
                    column,
                    detection.confidence_percent,
                    &percent_format(),
                )?;
            } else {
                sheet.write_string_with_format(row, column, safe_text(value), &wrapped_format())?;
            }
        }
    }
    if !report.detections.is_empty() {
        sheet.autofilter(0, 0, checked_row(report.detections.len())?, 5)?;
    }
    sheet.set_freeze_panes(1, 0)?;
    set_widths(sheet, &[22.0, 18.0, 52.0, 12.0, 52.0, 42.0])?;
    Ok(())
}

fn write_pareto(workbook: &mut Workbook, report: &ReportDocument) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    configure_sheet(sheet, &report.labels.pareto, report.right_to_left)?;
    let headers = [
        &report.labels.context,
        &report.labels.currency,
        &report.labels.total_amount,
        &report.labels.top_item_count,
        &report.labels.total_item_count,
        &report.labels.cumulative_share,
    ];
    write_headers(sheet, &headers)?;
    for (index, item) in report.pareto.iter().enumerate() {
        let row = checked_row(index + 1)?;
        let values = [
            item.context.clone(),
            item.currency
                .clone()
                .unwrap_or_else(|| report.labels.not_available.clone()),
            item.total_amount.clone(),
            item.top_item_count.to_string(),
            item.total_item_count.to_string(),
            format!("{}%", item.cumulative_share_percent),
        ];
        for (column, value) in values.iter().enumerate() {
            sheet.write_string(row, checked_column(column)?, safe_text(value))?;
        }
    }
    sheet.autofilter(0, 0, checked_row(report.pareto.len())?, 5)?;
    sheet.set_freeze_panes(1, 0)?;
    set_widths(sheet, &[28.0, 12.0, 20.0, 18.0, 18.0, 20.0])?;
    Ok(())
}

fn write_source_metadata(
    workbook: &mut Workbook,
    report: &ReportDocument,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    configure_sheet(sheet, &report.labels.source_metadata, report.right_to_left)?;
    write_headers(sheet, &[&report.labels.field, &report.labels.value])?;
    let metadata = [
        (&report.labels.source_file, &report.metadata.source_filename),
        (&report.labels.source_hash, &report.metadata.source_sha256),
        (&report.labels.project, &report.metadata.project_id),
        (&report.labels.run, &report.metadata.run_id),
        (&report.labels.title, &report.metadata.tool_name),
        (&report.labels.tool_version, &report.metadata.tool_version),
        (
            &report.labels.rule_set_version,
            &report.metadata.rule_set_version,
        ),
        (&report.labels.app_version, &report.metadata.app_version),
        (
            &report.labels.report_timestamp,
            &report.metadata.report_timestamp,
        ),
        (&report.labels.language, &report.metadata.language),
    ];
    for (index, (key, value)) in metadata.iter().enumerate() {
        let row = checked_row(index + 1)?;
        sheet.write_string_with_format(row, 0, safe_text(key), &label_format())?;
        sheet.write_string_with_format(row, 1, safe_text(value), &wrapped_format())?;
    }
    set_widths(sheet, &[28.0, 76.0])?;
    sheet.set_freeze_panes(1, 0)?;
    Ok(())
}

fn write_ai_review(
    workbook: &mut Workbook,
    report: &ReportDocument,
    commentary: &str,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    configure_sheet(sheet, &report.labels.ai_review, report.right_to_left)?;
    sheet.merge_range(
        0,
        0,
        0,
        5,
        &safe_text(&report.labels.ai_review),
        &title_format(),
    )?;
    sheet.merge_range(2, 0, 18, 5, &safe_text(commentary), &wrapped_format())?;
    for column in 0..=5 {
        sheet.set_column_width(column, 18)?;
    }
    Ok(())
}

fn configure_sheet(
    sheet: &mut Worksheet,
    requested_name: &str,
    right_to_left: bool,
) -> Result<(), XlsxError> {
    sheet.set_name(safe_sheet_name(requested_name))?;
    sheet.set_right_to_left(right_to_left);
    sheet.set_landscape();
    sheet.set_margins(0.35, 0.35, 0.5, 0.5, 0.2, 0.2);
    Ok(())
}

fn write_headers(sheet: &mut Worksheet, headers: &[&String]) -> Result<(), XlsxError> {
    let format = header_format();
    for (index, header) in headers.iter().enumerate() {
        sheet.write_string_with_format(0, checked_column(index)?, safe_text(header), &format)?;
    }
    Ok(())
}

fn set_widths(sheet: &mut Worksheet, widths: &[f64]) -> Result<(), XlsxError> {
    for (index, width) in widths.iter().enumerate() {
        sheet.set_column_width(checked_column(index)?, *width)?;
    }
    Ok(())
}

fn checked_row(value: usize) -> Result<u32, XlsxError> {
    u32::try_from(value).map_err(|_| XlsxError::RowColumnLimitError)
}

fn checked_column(value: usize) -> Result<u16, XlsxError> {
    u16::try_from(value).map_err(|_| XlsxError::RowColumnLimitError)
}

fn title_format() -> Format {
    Format::new()
        .set_bold()
        .set_font_size(18)
        .set_font_color(Color::White)
        .set_background_color(HEADER_COLOR)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
}

fn header_format() -> Format {
    Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(HEADER_COLOR)
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
}

fn label_format() -> Format {
    Format::new()
        .set_bold()
        .set_background_color(ACCENT_COLOR)
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
}

fn value_format() -> Format {
    Format::new().set_border(FormatBorder::Thin)
}

fn wrapped_format() -> Format {
    Format::new()
        .set_border(FormatBorder::Thin)
        .set_text_wrap()
        .set_align(FormatAlign::Top)
}

fn percent_format() -> Format {
    wrapped_format().set_num_format("0.0")
}

/// Neutralize strings that spreadsheet programs may interpret as formulas or
/// external-data commands when opened.
fn safe_text(value: &str) -> String {
    let first = value.chars().next();
    if matches!(first, Some('=' | '+' | '-' | '@' | '\t' | '\r')) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn safe_sheet_name(value: &str) -> String {
    let filtered = value
        .chars()
        .map(|character| match character {
            ':' | '\\' | '/' | '?' | '*' | '[' | ']' => ' ',
            other => other,
        })
        .take(31)
        .collect::<String>();
    let trimmed = filtered.trim().trim_matches('\'');
    if trimmed.is_empty() {
        "Report".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn publish_new_file(path: &Path, bytes: &[u8]) -> Result<(), ReportingError> {
    if path.exists() {
        return Err(ReportingError::AlreadyExists(path.display().to_string()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ReportingError::AlreadyExists(path.display().to_string())
            } else {
                ReportingError::Io(error)
            }
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(ReportingError::Io(error));
    }
    Ok(())
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use calamine::Reader;

    use super::*;
    use crate::{
        ReportDetection, ReportEvidence, ReportLabels, ReportMetadata, ReportPareto, ReportSummary,
    };

    pub(crate) fn sample_report() -> ReportDocument {
        ReportDocument {
            labels: ReportLabels {
                report_title: "BOQ Inspector Report".into(),
                executive_summary: "Executive Summary".into(),
                findings: "Findings".into(),
                detection: "Sheet Table Detection".into(),
                pareto: "Pareto Analysis".into(),
                source_metadata: "Source Metadata".into(),
                ai_review: "AI Review".into(),
                limitations: "Limitations".into(),
                field: "Field".into(),
                value: "Value".into(),
                severity: "Severity".into(),
                category: "Category".into(),
                confidence: "Confidence".into(),
                rule: "Rule".into(),
                title: "Title".into(),
                explanation: "Explanation".into(),
                action: "Action".into(),
                sheet: "Sheet".into(),
                cell: "Cell".into(),
                evidence: "Evidence".into(),
                source_hash: "Source hash".into(),
                source_file: "Source file".into(),
                project: "Project".into(),
                run: "Run".into(),
                tool_version: "Tool version".into(),
                rule_set_version: "Rule set version".into(),
                app_version: "App version".into(),
                report_timestamp: "Report timestamp".into(),
                language: "Language".into(),
                item_rows: "Item rows".into(),
                finding_count: "Finding count".into(),
                interpretation_confidence: "Interpretation confidence".into(),
                context: "Context".into(),
                currency: "Currency".into(),
                total_amount: "Total amount".into(),
                top_item_count: "Top item count".into(),
                total_item_count: "Total item count".into(),
                cumulative_share: "Cumulative share".into(),
                table_range: "Table range".into(),
                mapped_columns: "Mapped columns".into(),
                warning: "Warning".into(),
                deterministic_origin: "Deterministic".into(),
                ai_origin: "AI".into(),
                not_available: "Not available".into(),
            },
            metadata: ReportMetadata {
                source_filename: "=danger.xlsx".into(),
                source_sha256: "a".repeat(64),
                project_id: "tower-a".into(),
                run_id: "run-1".into(),
                tool_name: "BOQ Inspector".into(),
                tool_version: "0.0.1".into(),
                rule_set_version: "2026.07.2".into(),
                app_version: "0.0.1".into(),
                report_timestamp: "2026-07-23T10:00:00Z".into(),
                language: "en".into(),
            },
            summary: ReportSummary {
                item_rows: 1,
                finding_count: 1,
                interpretation_confidence: 0.91,
                severity_counts: vec![("High".into(), 1)],
                category_counts: vec![("Arithmetic".into(), 1)],
            },
            findings: vec![ReportFinding {
                severity: "High".into(),
                category: "Arithmetic".into(),
                confidence_percent: 99.0,
                rule_id: "boq.amount_mismatch".into(),
                title: "=not a formula".into(),
                explanation: "Expected 10, found 11".into(),
                action: Some("Review the values".into()),
                sheet: Some("BOQ".into()),
                cell: Some("F2".into()),
                original_value: Some("11".into()),
                original_formula: Some("=B2*C2".into()),
                evidence: vec![ReportEvidence {
                    sheet: "BOQ".into(),
                    reference: "F2".into(),
                    description: "Amount".into(),
                    snippet: Some("11".into()),
                }],
                origin: "Deterministic".into(),
            }],
            detections: vec![ReportDetection {
                sheet: "BOQ".into(),
                table_range: "A1:F2".into(),
                header_row: Some(1),
                mapped_columns: "A: item, B: description".into(),
                confidence_percent: 91.0,
                evidence: "Header aliases and numeric columns".into(),
                warning: None,
            }],
            pareto: vec![ReportPareto {
                context: "table-1".into(),
                currency: Some("EGP".into()),
                total_amount: "11".into(),
                top_item_count: 1,
                total_item_count: 1,
                cumulative_share_percent: "100".into(),
            }],
            limitations: vec!["Unsupported formulas are reported, not evaluated.".into()],
            ai_commentary: None,
            right_to_left: false,
        }
    }

    #[test]
    fn writes_professional_report_and_neutralizes_formula_like_text() {
        let path = std::env::temp_dir().join(format!(
            "openconkit-reporting-xlsx-{}.xlsx",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let hash = write_xlsx_report(&path, &sample_report()).expect("writes report");
        assert_eq!(hash.len(), 64);

        let mut workbook = calamine::open_workbook_auto(&path).expect("reopens");
        assert_eq!(
            workbook.sheet_names(),
            &[
                "Executive Summary",
                "Findings",
                "Sheet Table Detection",
                "Pareto Analysis",
                "Source Metadata"
            ]
        );
        let findings = workbook.worksheet_range("Findings").expect("findings");
        assert_eq!(
            findings.get((1, 4)).map(ToString::to_string).as_deref(),
            Some("'=not a formula")
        );
        let source = workbook
            .worksheet_range("Source Metadata")
            .expect("metadata");
        assert_eq!(
            source.get((1, 1)).map(ToString::to_string).as_deref(),
            Some("'=danger.xlsx")
        );

        let second = write_xlsx_report(&path, &sample_report());
        assert!(matches!(second, Err(ReportingError::AlreadyExists(_))));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn validates_report_language_and_hash() {
        let mut report = sample_report();
        report.metadata.language = "fr".into();
        assert!(report.validate().is_err());
        report.metadata.language = "en".into();
        report.metadata.source_sha256 = "ABC".into();
        assert!(report.validate().is_err());
    }
}
