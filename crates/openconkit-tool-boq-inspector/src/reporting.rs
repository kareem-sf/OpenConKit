//! BOQ-specific mapping from persisted run output to format-neutral reports.

use std::collections::BTreeMap;
use std::path::Path;

use openconkit_domain::{
    ColumnRole, ExportKind, Finding, FindingCategory, FindingOrigin, RunStatus, Severity,
};
use openconkit_reporting::{
    write_pdf_report, write_xlsx_report, ReportDetection, ReportDocument, ReportEvidence,
    ReportFinding, ReportLabels, ReportMetadata, ReportPareto, ReportSummary, ReportingError,
};
use openconkit_tool_sdk::{ExportContext, ExportProvider, ExportedArtifact, ToolError};
use serde_json::Value;

use crate::{BoqAiPriority, BoqAiReview, BoqInspectorOutput};

pub(crate) static XLSX_EXPORTER: BoqExportProvider = BoqExportProvider {
    kind: ExportKind::Xlsx,
};
pub(crate) static PDF_EXPORTER: BoqExportProvider = BoqExportProvider {
    kind: ExportKind::Pdf,
};

pub(crate) struct BoqExportProvider {
    kind: ExportKind,
}

impl ExportProvider for BoqExportProvider {
    fn kind(&self) -> ExportKind {
        self.kind
    }

    fn languages(&self) -> Vec<String> {
        vec!["en".to_string(), "ar".to_string()]
    }

    fn export(
        &self,
        context: &ExportContext,
        run_output: &Value,
        dest_dir: &Path,
        language: &str,
    ) -> Result<ExportedArtifact, ToolError> {
        if !matches!(language, "en" | "ar") {
            return Err(ToolError::InvalidSettings {
                message: "report language must be `en` or `ar`".to_string(),
            });
        }
        let output: BoqInspectorOutput =
            serde_json::from_value(run_output.clone()).map_err(|error| {
                ToolError::InvalidInput {
                    message: format!("BOQ run output is invalid: {error}"),
                }
            })?;
        validate_export_context(context, &output)?;
        let localizer = Localizer::new(language)?;
        let report = build_report(context, &output, &localizer)?;
        let extension = match self.kind {
            ExportKind::Xlsx => "xlsx",
            ExportKind::Pdf => "pdf",
        };
        let filename = format!("boq-inspector-{}-{language}.{extension}", context.run.id);
        let path = dest_dir.join(&filename);
        let sha256 = match self.kind {
            ExportKind::Xlsx => write_xlsx_report(&path, &report),
            ExportKind::Pdf => write_pdf_report(&path, &report),
        }
        .map_err(map_reporting_error)?;
        Ok(ExportedArtifact {
            kind: self.kind,
            language: language.to_string(),
            relative_path: filename,
            sha256,
        })
    }
}

fn validate_export_context(
    context: &ExportContext,
    output: &BoqInspectorOutput,
) -> Result<(), ToolError> {
    if context.run.status != RunStatus::Completed {
        return Err(ToolError::InvalidInput {
            message: "only completed runs can be exported".to_string(),
        });
    }
    if context.run.source_revision_id != context.source_revision.id
        || context.run.project_id != context.source_revision.project_id
    {
        return Err(ToolError::InvalidInput {
            message: "export provenance does not match the analysis run".to_string(),
        });
    }
    if output.diagnostics.rule_set_version != context.run.rule_set_version {
        return Err(ToolError::InvalidInput {
            message: "run output rule-set version does not match the persisted run".to_string(),
        });
    }
    if output.summary.finding_count != output.findings.len() {
        return Err(ToolError::InvalidInput {
            message: "run output summary count does not match its findings".to_string(),
        });
    }
    for finding in &output.findings {
        if finding.project_id != context.run.project_id
            || finding.source_revision_id != context.run.source_revision_id
            || finding.run_id != context.run.id
            || finding.rule_set_version != context.run.rule_set_version
            || finding.origin != FindingOrigin::Deterministic
        {
            return Err(ToolError::InvalidInput {
                message: "run output contains a finding with invalid provenance".to_string(),
            });
        }
    }
    Ok(())
}

fn build_report(
    context: &ExportContext,
    output: &BoqInspectorOutput,
    localizer: &Localizer,
) -> Result<ReportDocument, ToolError> {
    let labels = report_labels(localizer)?;
    let findings = output
        .findings
        .iter()
        .map(|finding| report_finding(finding, localizer, &labels))
        .collect::<Result<Vec<_>, _>>()?;
    let detections = report_detections(output, localizer, &labels)?;
    let severity_counts = severity_counts(&output.findings, localizer)?;
    let category_counts = category_counts(&output.findings, localizer)?;
    let pareto = output
        .summary
        .pareto
        .iter()
        .map(|item| ReportPareto {
            context: item.context.clone(),
            currency: item.currency.clone(),
            total_amount: item.total_amount.clone(),
            top_item_count: item.top_item_count,
            total_item_count: item.total_item_count,
            cumulative_share_percent: item.cumulative_share_percent.clone(),
        })
        .collect();
    let limitations = [
        "reports.boq.limitationsText.readOnly",
        "reports.boq.limitationsText.formulaSubset",
        "reports.boq.limitationsText.confidence",
    ]
    .into_iter()
    .map(|key| localizer.text(key, &BTreeMap::new()))
    .collect::<Result<Vec<_>, _>>()?;
    let ai_commentary = context
        .validated_ai_output
        .as_ref()
        .map(|value| {
            serde_json::from_value::<BoqAiReview>(value.clone())
                .map_err(|error| ToolError::InvalidInput {
                    message: format!("validated BOQ AI output is invalid: {error}"),
                })
                .and_then(|review| format_ai_commentary(&review, localizer))
        })
        .transpose()?;

    Ok(ReportDocument {
        labels,
        metadata: ReportMetadata {
            source_filename: context.source_revision.original_filename.clone(),
            source_sha256: context.source_revision.sha256.to_string(),
            project_id: context.run.project_id.to_string(),
            run_id: context.run.id.to_string(),
            tool_name: localizer.text("tools.boqInspector.name", &BTreeMap::new())?,
            tool_version: context.run.tool_version.clone(),
            rule_set_version: context.run.rule_set_version.clone(),
            app_version: context.run.app_version.clone(),
            report_timestamp: context.report_timestamp.to_string(),
            language: localizer.language.to_string(),
        },
        summary: ReportSummary {
            item_rows: output.summary.item_rows,
            finding_count: output.summary.finding_count,
            interpretation_confidence: output.diagnostics.interpretation_confidence.value(),
            severity_counts,
            category_counts,
        },
        findings,
        detections,
        pareto,
        limitations,
        ai_commentary,
        right_to_left: localizer.language == "ar",
    })
}

fn format_ai_commentary(review: &BoqAiReview, localizer: &Localizer) -> Result<String, ToolError> {
    let text = |suffix: &str| {
        localizer.text(
            &format!("reports.boq.aiSections.{suffix}"),
            &BTreeMap::<String, String>::new(),
        )
    };
    let mut sections = vec![review.summary.clone()];
    if !review.prioritized_risks.is_empty() {
        let mut lines = vec![text("prioritizedRisks")?];
        for risk in &review.prioritized_risks {
            let priority = match risk.priority {
                BoqAiPriority::High => text("priorityHigh")?,
                BoqAiPriority::Medium => text("priorityMedium")?,
                BoqAiPriority::Low => text("priorityLow")?,
            };
            let mut references = risk.finding_ids.join(", ");
            if !risk.evidence_refs.is_empty() {
                references.push_str(" · ");
                references.push_str(&risk.evidence_refs.join(", "));
            }
            lines.push(format!("• {priority}: {} [{references}]", risk.reason));
        }
        sections.push(lines.join("\n"));
    }
    append_ai_list(
        &mut sections,
        text("recommendations")?,
        &review.recommendations,
    );
    append_ai_list(
        &mut sections,
        text("rfiSuggestions")?,
        &review.rfi_suggestions,
    );
    append_ai_list(&mut sections, text("limitations")?, &review.limitations);
    append_ai_list(&mut sections, text("assumptions")?, &review.assumptions);
    Ok(sections.join("\n\n"))
}

fn append_ai_list(sections: &mut Vec<String>, title: String, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let mut lines = vec![title];
    lines.extend(values.iter().map(|value| format!("• {value}")));
    sections.push(lines.join("\n"));
}

fn report_labels(localizer: &Localizer) -> Result<ReportLabels, ToolError> {
    let t = |suffix: &str| {
        localizer.text(
            &format!("reports.boq.{suffix}"),
            &BTreeMap::<String, String>::new(),
        )
    };
    Ok(ReportLabels {
        report_title: t("reportTitle")?,
        executive_summary: t("executiveSummary")?,
        findings: t("findings")?,
        detection: t("detection")?,
        pareto: t("pareto")?,
        source_metadata: t("sourceMetadata")?,
        ai_review: t("aiReview")?,
        limitations: t("limitations")?,
        field: t("field")?,
        value: t("value")?,
        severity: t("severity")?,
        category: t("category")?,
        confidence: t("confidence")?,
        rule: t("rule")?,
        title: t("title")?,
        explanation: t("explanation")?,
        action: t("action")?,
        sheet: t("sheet")?,
        cell: t("cell")?,
        evidence: t("evidence")?,
        source_hash: t("sourceHash")?,
        source_file: t("sourceFile")?,
        project: t("project")?,
        run: t("run")?,
        tool_version: t("toolVersion")?,
        rule_set_version: t("ruleSetVersion")?,
        app_version: t("appVersion")?,
        report_timestamp: t("reportTimestamp")?,
        language: t("language")?,
        item_rows: t("itemRows")?,
        finding_count: t("findingCount")?,
        interpretation_confidence: t("interpretationConfidence")?,
        context: t("context")?,
        currency: t("currency")?,
        total_amount: t("totalAmount")?,
        top_item_count: t("topItemCount")?,
        total_item_count: t("totalItemCount")?,
        cumulative_share: t("cumulativeShare")?,
        table_range: t("tableRange")?,
        mapped_columns: t("mappedColumns")?,
        warning: t("warning")?,
        deterministic_origin: t("deterministicOrigin")?,
        ai_origin: t("aiOrigin")?,
        not_available: t("notAvailable")?,
    })
}

fn report_finding(
    finding: &Finding,
    localizer: &Localizer,
    labels: &ReportLabels,
) -> Result<ReportFinding, ToolError> {
    let evidence = finding
        .evidence
        .iter()
        .map(|item| {
            let reference = item
                .cell
                .as_ref()
                .map(ToString::to_string)
                .or_else(|| item.range.as_ref().map(ToString::to_string))
                .unwrap_or_else(|| labels.not_available.clone());
            let description = match item.description_key.as_deref() {
                Some(key) => localizer.text(key, &BTreeMap::new())?,
                None => labels.not_available.clone(),
            };
            Ok(ReportEvidence {
                sheet: item.sheet.clone(),
                reference,
                description,
                snippet: item.snippet.clone(),
            })
        })
        .collect::<Result<Vec<_>, ToolError>>()?;
    Ok(ReportFinding {
        severity: severity_label(finding.severity, localizer)?,
        category: category_label(finding.category, localizer)?,
        confidence_percent: finding.confidence.value() * 100.0,
        rule_id: finding.rule_id.clone(),
        title: localizer.text(&finding.title_key, &finding.title_params)?,
        explanation: localizer.text(&finding.explanation_key, &finding.explanation_params)?,
        action: finding
            .suggested_action_key
            .as_deref()
            .map(|key| localizer.text(key, &finding.suggested_action_params))
            .transpose()?,
        sheet: finding.sheet.clone(),
        cell: finding.cell.as_ref().map(ToString::to_string),
        original_value: finding.original_value.clone(),
        original_formula: finding.original_formula.clone(),
        evidence,
        origin: match finding.origin {
            FindingOrigin::Deterministic => labels.deterministic_origin.clone(),
            FindingOrigin::Ai => labels.ai_origin.clone(),
        },
    })
}

fn report_detections(
    output: &BoqInspectorOutput,
    localizer: &Localizer,
    labels: &ReportLabels,
) -> Result<Vec<ReportDetection>, ToolError> {
    let mut detections = Vec::new();
    for table in &output.diagnostics.tables {
        let first_column = table
            .columns
            .iter()
            .min_by_key(|column| column.column_index)
            .map(|column| column.column_letter.as_str())
            .unwrap_or("?");
        let last_column = table
            .columns
            .iter()
            .max_by_key(|column| column.column_index)
            .map(|column| column.column_letter.as_str())
            .unwrap_or("?");
        let mapped_columns = table
            .columns
            .iter()
            .map(|column| {
                Ok(format!(
                    "{}: {} ({:.0}%)",
                    column.column_letter,
                    column_role_label(column.role, localizer)?,
                    column.confidence.value() * 100.0
                ))
            })
            .collect::<Result<Vec<_>, ToolError>>()?
            .join(", ");
        let evidence = table
            .evidence
            .iter()
            .map(|code| diagnostic_label(code, localizer))
            .collect::<Result<Vec<_>, _>>()?
            .join("; ");
        detections.push(ReportDetection {
            sheet: table.sheet.clone(),
            table_range: format!(
                "{first_column}{}:{last_column}{}",
                table.start_row.saturating_add(1),
                table.end_row.saturating_add(1)
            ),
            header_row: table.header_row.map(|row| row.saturating_add(1)),
            mapped_columns,
            confidence_percent: table.interpretation_confidence.value() * 100.0,
            evidence,
            warning: None,
        });
    }
    for warning in &output.diagnostics.warnings {
        detections.push(ReportDetection {
            sheet: labels.not_available.clone(),
            table_range: labels.not_available.clone(),
            header_row: None,
            mapped_columns: labels.not_available.clone(),
            confidence_percent: output.diagnostics.interpretation_confidence.value() * 100.0,
            evidence: labels.not_available.clone(),
            warning: Some(diagnostic_label(warning, localizer)?),
        });
    }
    Ok(detections)
}

fn severity_counts(
    findings: &[Finding],
    localizer: &Localizer,
) -> Result<Vec<(String, usize)>, ToolError> {
    [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ]
    .into_iter()
    .filter_map(|severity| {
        let count = findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .count();
        (count > 0).then_some((severity, count))
    })
    .map(|(severity, count)| Ok((severity_label(severity, localizer)?, count)))
    .collect()
}

fn category_counts(
    findings: &[Finding],
    localizer: &Localizer,
) -> Result<Vec<(String, usize)>, ToolError> {
    [
        FindingCategory::Arithmetic,
        FindingCategory::Duplication,
        FindingCategory::Omission,
        FindingCategory::Inconsistency,
        FindingCategory::Structure,
        FindingCategory::Compliance,
        FindingCategory::Other,
    ]
    .into_iter()
    .filter_map(|category| {
        let count = findings
            .iter()
            .filter(|finding| finding.category == category)
            .count();
        (count > 0).then_some((category, count))
    })
    .map(|(category, count)| Ok((category_label(category, localizer)?, count)))
    .collect()
}

fn severity_label(severity: Severity, localizer: &Localizer) -> Result<String, ToolError> {
    let suffix = match severity {
        Severity::Info => "info",
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
        Severity::Critical => "critical",
    };
    localizer.text(
        &format!("reports.boq.severityValues.{suffix}"),
        &BTreeMap::new(),
    )
}

fn category_label(category: FindingCategory, localizer: &Localizer) -> Result<String, ToolError> {
    let suffix = match category {
        FindingCategory::Arithmetic => "arithmetic",
        FindingCategory::Duplication => "duplication",
        FindingCategory::Omission => "omission",
        FindingCategory::Inconsistency => "inconsistency",
        FindingCategory::Structure => "structure",
        FindingCategory::Compliance => "compliance",
        FindingCategory::Other => "other",
    };
    localizer.text(
        &format!("reports.boq.categoryValues.{suffix}"),
        &BTreeMap::new(),
    )
}

fn column_role_label(role: ColumnRole, localizer: &Localizer) -> Result<String, ToolError> {
    let suffix = match role {
        ColumnRole::ItemNumber => "itemNumber",
        ColumnRole::Description => "description",
        ColumnRole::Unit => "unit",
        ColumnRole::Quantity => "quantity",
        ColumnRole::UnitPrice => "unitPrice",
        ColumnRole::TotalPrice => "totalPrice",
        ColumnRole::Currency => "currency",
        ColumnRole::Notes => "notes",
        ColumnRole::Unknown => "unknown",
    };
    localizer.text(
        &format!("reports.boq.columnRoles.{suffix}"),
        &BTreeMap::new(),
    )
}

fn diagnostic_label(code: &str, localizer: &Localizer) -> Result<String, ToolError> {
    let suffix = match code {
        "headerless_distribution_inference" => "headerlessDistributionInference",
        "merged_header_region" => "mergedHeaderRegion",
        "bilingual_header_aliases" => "bilingualHeaderAliases",
        "no_tables_detected" => "noTablesDetected",
        "low_confidence_structure" => "lowConfidenceStructure",
        "row_column_visibility_unavailable" => "rowColumnVisibilityUnavailable",
        _ => {
            return Err(ToolError::Engine {
                message: format!("missing report translation for diagnostic code `{code}`"),
            });
        }
    };
    localizer.text(
        &format!("reports.boq.diagnostics.{suffix}"),
        &BTreeMap::new(),
    )
}

struct Localizer {
    language: &'static str,
    catalog: Value,
}

impl Localizer {
    fn new(language: &str) -> Result<Self, ToolError> {
        let (language, source) = match language {
            "en" => (
                "en",
                include_str!("../../../packages/i18n/src/locales/en/common.json"),
            ),
            "ar" => (
                "ar",
                include_str!("../../../packages/i18n/src/locales/ar/common.json"),
            ),
            _ => {
                return Err(ToolError::InvalidSettings {
                    message: "report language must be `en` or `ar`".to_string(),
                });
            }
        };
        let catalog = serde_json::from_str(source).map_err(|error| ToolError::Engine {
            message: format!("embedded translation catalog is invalid: {error}"),
        })?;
        Ok(Self { language, catalog })
    }

    fn text(&self, key: &str, params: &BTreeMap<String, String>) -> Result<String, ToolError> {
        let mut value = &self.catalog;
        for segment in key.split('.') {
            value = value.get(segment).ok_or_else(|| ToolError::Engine {
                message: format!("missing report translation key `{key}`"),
            })?;
        }
        let template = value.as_str().ok_or_else(|| ToolError::Engine {
            message: format!("report translation key `{key}` is not text"),
        })?;
        let mut rendered = template.to_string();
        for (name, replacement) in params {
            rendered = rendered.replace(&format!("{{{{{name}}}}}"), replacement);
        }
        if rendered.contains("{{") || rendered.contains("}}") {
            return Err(ToolError::Engine {
                message: format!("report translation key `{key}` has unresolved parameters"),
            });
        }
        Ok(rendered)
    }
}

fn map_reporting_error(error: ReportingError) -> ToolError {
    match error {
        ReportingError::AlreadyExists(_) => ToolError::Engine {
            message: "the requested report already exists".to_string(),
        },
        other => ToolError::Engine {
            message: format!("report generation failed: {other}"),
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn embedded_catalogs_have_complete_report_labels() {
        for language in ["en", "ar"] {
            let localizer = Localizer::new(language).expect("catalog");
            let labels = report_labels(&localizer).expect("all labels");
            assert!(!labels.report_title.is_empty());
            assert!(!labels.source_hash.is_empty());
            assert!(!labels.limitations.is_empty());
        }
    }

    #[test]
    fn interpolation_rejects_missing_parameters() {
        let localizer = Localizer::new("en").expect("catalog");
        assert!(localizer
            .text("findings.amountMismatch.title", &BTreeMap::new())
            .is_ok());
        assert!(localizer
            .text("findings.amountMismatch.explanation", &BTreeMap::new())
            .is_err());
    }

    #[test]
    fn diagnostic_codes_are_exhaustively_localized() {
        let localizer = Localizer::new("ar").expect("catalog");
        for code in [
            "headerless_distribution_inference",
            "merged_header_region",
            "bilingual_header_aliases",
            "no_tables_detected",
            "low_confidence_structure",
            "row_column_visibility_unavailable",
        ] {
            assert!(!diagnostic_label(code, &localizer)
                .expect("label")
                .is_empty());
        }
        assert!(diagnostic_label("new_unmapped_code", &localizer).is_err());
    }
}
