//! BOQ Inspector: deterministic, read-only Bill of Quantities review.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod ai;
mod detection;
mod formula;
mod model;
mod normalization;
mod reporting;
mod rules;

pub use ai::{BoqAiPrioritizedRisk, BoqAiPriority, BoqAiReview};

use std::collections::BTreeSet;
use std::str::FromStr;

use openconkit_domain::{
    AnalysisRunId, Confidence, Finding, ProjectId, SourceRevisionId, WorkbookDiagnostics,
};
use openconkit_spreadsheet::{
    ingest_with_observer, IngestionObserver, IngestionProgress, IngestionStage, SpreadsheetError,
    WorkbookLimits,
};
use openconkit_tool_sdk::{
    AiCapability, CancellationToken, ExportProvider, InputCapabilities, ProgressCallback, Tool,
    ToolAiProvider, ToolEngine, ToolError, ToolManifest, ToolPermissions, ToolProgress,
    ToolRunContext, ToolTestHooks, TypedEngineAdapter, TypedToolEngine, TOOL_CONTRACT_VERSION,
};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::rules::{RuleIdentity, RuleSettings};

/// Stable identifier of the BOQ Inspector tool.
pub const TOOL_ID: &str = "boq-inspector";

/// Typed run input for the BOQ Inspector engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct BoqInspectorInput {
    /// Source revision being analyzed. Must match the host run context.
    pub source_revision_id: String,
    /// Rule ids to apply (empty = all deterministic rules).
    #[serde(default)]
    pub rules: Vec<String>,
}

/// Typed run settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct BoqInspectorSettings {
    /// Locale used by the shell to render i18n keys.
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Absolute arithmetic tolerance, serialized as a decimal string.
    #[serde(default = "default_absolute_tolerance")]
    pub absolute_tolerance: String,
    /// Relative arithmetic tolerance, serialized as a decimal string.
    #[serde(default = "default_relative_tolerance")]
    pub relative_tolerance: String,
    /// Decimal places used before comparisons.
    #[serde(default = "default_decimal_precision")]
    pub decimal_precision: u8,
    /// Token-set similarity threshold, in the inclusive range 50..=100.
    #[serde(default = "default_fuzzy_threshold")]
    pub fuzzy_similarity_threshold_percent: u8,
    /// Interpretation confidence below which a structure finding is emitted.
    #[serde(default = "default_low_confidence_threshold")]
    pub low_confidence_threshold_percent: u8,
}

impl Default for BoqInspectorSettings {
    fn default() -> Self {
        Self {
            locale: default_locale(),
            absolute_tolerance: default_absolute_tolerance(),
            relative_tolerance: default_relative_tolerance(),
            decimal_precision: default_decimal_precision(),
            fuzzy_similarity_threshold_percent: default_fuzzy_threshold(),
            low_confidence_threshold_percent: default_low_confidence_threshold(),
        }
    }
}

/// One currency/context-specific 80/20 analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ParetoAnalysis {
    /// Stable table/currency context key. It does not include workbook content.
    pub context: String,
    /// Explicit currency evidence, or `None` when the workbook did not state it.
    pub currency: Option<String>,
    /// Sum of positive item amounts, as an exact decimal string.
    pub total_amount: String,
    /// Smallest leading item count reaching at least 80% of the total.
    pub top_item_count: usize,
    /// Total positive priced item count in this context.
    pub total_item_count: usize,
    /// Actual cumulative share reached, formatted as a percentage number.
    pub cumulative_share_percent: String,
}

/// Compact deterministic run summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct BoqInspectorSummary {
    pub item_rows: usize,
    pub finding_count: usize,
    pub pareto: Vec<ParetoAnalysis>,
}

/// One source cell and its normalized deterministic interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BoqNormalizedFact {
    /// Exact A1 cell address.
    pub cell: String,
    /// Verbatim source value.
    pub raw: String,
    /// Verbatim source formula, when present.
    pub formula: Option<String>,
    /// Stable normalized value serialized as text.
    pub normalized: String,
}

/// Normalized BOQ row retained with the run for grounded optional AI review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BoqNormalizedRow {
    pub source_row_id: String,
    pub sheet: String,
    /// One-based source row number.
    pub source_row_number: u32,
    pub classification: openconkit_domain::RowClassification,
    pub classification_confidence: Confidence,
    pub section_path: Vec<String>,
    pub item_code: Option<BoqNormalizedFact>,
    pub description: Option<BoqNormalizedFact>,
    pub unit: Option<BoqNormalizedFact>,
    pub quantity: Option<BoqNormalizedFact>,
    pub rate_text: Option<BoqNormalizedFact>,
    pub rate: Option<BoqNormalizedFact>,
    pub amount: Option<BoqNormalizedFact>,
    pub currency: Option<BoqNormalizedFact>,
    pub error_cells: Vec<BoqNormalizedFact>,
}

/// Typed run output.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BoqInspectorOutput {
    /// Deterministic findings with exact workbook evidence.
    pub findings: Vec<Finding>,
    /// Structural interpretation shown to the user for review.
    pub diagnostics: WorkbookDiagnostics,
    pub summary: BoqInspectorSummary,
    /// Complete normalized rows used by deterministic rules. Older stored
    /// outputs deserialize with an empty list for forward compatibility.
    #[serde(default)]
    pub normalized_rows: Vec<BoqNormalizedRow>,
}

struct EngineIngestionObserver<'a> {
    progress: ProgressCallback<'a>,
    cancel: &'a CancellationToken,
}

impl IngestionObserver for EngineIngestionObserver<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn on_progress(&self, update: &IngestionProgress) {
        let (phase_key, base, span) = match update.stage {
            IngestionStage::FileValidation => ("tools.boqInspector.progress.validate", 0.02, 0.03),
            IngestionStage::ArchiveValidation => {
                ("tools.boqInspector.progress.archive", 0.05, 0.05)
            }
            IngestionStage::WorkbookMetadata => {
                ("tools.boqInspector.progress.metadata", 0.10, 0.05)
            }
            IngestionStage::Worksheet => ("tools.boqInspector.progress.ingest", 0.15, 0.25),
            IngestionStage::Complete => ("tools.boqInspector.progress.ingest", 0.40, 0.0),
        };
        let sheet_fraction = match (update.sheet_index, update.sheet_count) {
            (Some(index), Some(count)) if count > 0 => index as f64 / count as f64,
            _ => 0.0,
        };
        (self.progress)(ToolProgress::new(phase_key, base + span * sheet_fraction));
    }
}

/// The typed deterministic engine.
struct BoqInspectorEngine;

impl TypedToolEngine for BoqInspectorEngine {
    type Input = BoqInspectorInput;
    type Settings = BoqInspectorSettings;
    type Output = BoqInspectorOutput;

    fn run_typed(
        &self,
        context: &ToolRunContext,
        input: Self::Input,
        settings: Self::Settings,
        progress: ProgressCallback<'_>,
        cancel: &CancellationToken,
    ) -> Result<Self::Output, ToolError> {
        validate_input(context, &input)?;
        let rule_settings = validate_settings(&settings)?;
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }

        progress(ToolProgress::new("tools.boqInspector.progress.start", 0.0));
        let observer = EngineIngestionObserver { progress, cancel };
        let workbook = ingest_with_observer(
            &context.workbook_path,
            &WorkbookLimits::default(),
            &observer,
        )
        .map_err(map_spreadsheet_error)?;
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }

        progress(ToolProgress::new(
            "tools.boqInspector.progress.detect",
            0.45,
        ));
        let detection = detection::detect(&workbook);
        if cancel.is_cancelled() {
            return Err(ToolError::Cancelled);
        }

        progress(ToolProgress::new("tools.boqInspector.progress.rules", 0.62));
        let identity = RuleIdentity {
            project_id: ProjectId::new(context.project_id.clone()).map_err(|err| {
                ToolError::InvalidInput {
                    message: err.to_string(),
                }
            })?,
            source_revision_id: SourceRevisionId::parse(&context.source_revision_id).map_err(
                |err| ToolError::InvalidInput {
                    message: err.to_string(),
                },
            )?,
            run_id: AnalysisRunId::parse(&context.run_id).map_err(|err| {
                ToolError::InvalidInput {
                    message: err.to_string(),
                }
            })?,
        };
        let selected: BTreeSet<String> = input.rules.into_iter().collect();
        let result = rules::run_rules(
            &workbook,
            &detection,
            &identity,
            &rule_settings,
            &selected,
            cancel,
        )?;
        let item_rows = detection
            .rows
            .iter()
            .filter(|row| row.classification == openconkit_domain::RowClassification::Item)
            .count();
        let finding_count = result.findings.len();
        let normalized_rows = detection
            .rows
            .iter()
            .map(normalized_row_for_output)
            .collect();
        progress(ToolProgress::new(
            "tools.boqInspector.progress.complete",
            1.0,
        ));
        Ok(BoqInspectorOutput {
            findings: result.findings,
            diagnostics: detection.diagnostics,
            summary: BoqInspectorSummary {
                item_rows,
                finding_count,
                pareto: result.pareto,
            },
            normalized_rows,
        })
    }
}

fn validate_input(context: &ToolRunContext, input: &BoqInspectorInput) -> Result<(), ToolError> {
    SourceRevisionId::parse(&input.source_revision_id).map_err(|err| ToolError::InvalidInput {
        message: err.to_string(),
    })?;
    if input.source_revision_id != context.source_revision_id {
        return Err(ToolError::InvalidInput {
            message: "source_revision_id does not match the host run context".to_string(),
        });
    }
    if input.rules.iter().any(|rule| {
        rule.is_empty()
            || rule.len() > 96
            || !rule.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
    }) {
        return Err(ToolError::InvalidInput {
            message: "rule ids must use lowercase ASCII letters, digits, dot, dash, or underscore"
                .to_string(),
        });
    }
    Ok(())
}

fn validate_settings(settings: &BoqInspectorSettings) -> Result<RuleSettings, ToolError> {
    if !matches!(settings.locale.as_str(), "en" | "ar") {
        return Err(ToolError::InvalidSettings {
            message: "locale must be `en` or `ar`".to_string(),
        });
    }
    let absolute_tolerance =
        parse_nonnegative_decimal("absolute_tolerance", &settings.absolute_tolerance)?;
    let relative_tolerance =
        parse_nonnegative_decimal("relative_tolerance", &settings.relative_tolerance)?;
    if settings.decimal_precision > 6 {
        return Err(ToolError::InvalidSettings {
            message: "decimal_precision must be in 0..=6".to_string(),
        });
    }
    if !(50..=100).contains(&settings.fuzzy_similarity_threshold_percent) {
        return Err(ToolError::InvalidSettings {
            message: "fuzzy_similarity_threshold_percent must be in 50..=100".to_string(),
        });
    }
    if !(1..=100).contains(&settings.low_confidence_threshold_percent) {
        return Err(ToolError::InvalidSettings {
            message: "low_confidence_threshold_percent must be in 1..=100".to_string(),
        });
    }
    Ok(RuleSettings {
        absolute_tolerance,
        relative_tolerance,
        decimal_precision: u32::from(settings.decimal_precision),
        fuzzy_similarity_threshold: f64::from(settings.fuzzy_similarity_threshold_percent) / 100.0,
        low_confidence_threshold: f64::from(settings.low_confidence_threshold_percent) / 100.0,
    })
}

fn parse_nonnegative_decimal(field: &str, value: &str) -> Result<Decimal, ToolError> {
    let parsed = Decimal::from_str(value).map_err(|_| ToolError::InvalidSettings {
        message: format!("{field} must be an exact decimal string"),
    })?;
    if parsed.is_sign_negative() {
        return Err(ToolError::InvalidSettings {
            message: format!("{field} must be non-negative"),
        });
    }
    Ok(parsed)
}

fn map_spreadsheet_error(error: SpreadsheetError) -> ToolError {
    if matches!(error, SpreadsheetError::Cancelled) {
        ToolError::Cancelled
    } else {
        ToolError::Engine {
            message: format!("spreadsheet ingestion failed ({})", error.code()),
        }
    }
}

fn default_locale() -> String {
    "en".to_string()
}

fn default_absolute_tolerance() -> String {
    "0.01".to_string()
}

fn default_relative_tolerance() -> String {
    "0.001".to_string()
}

fn default_decimal_precision() -> u8 {
    2
}

fn default_fuzzy_threshold() -> u8 {
    85
}

fn default_low_confidence_threshold() -> u8 {
    65
}

struct BoqInspectorTestHooks;

impl ToolTestHooks for BoqInspectorTestHooks {
    fn fixture_input(&self) -> Option<serde_json::Value> {
        Some(json!({
            "source_revision_id": "00000000-0000-4000-8000-000000000001",
            "rules": []
        }))
    }
}

static TEST_HOOKS: BoqInspectorTestHooks = BoqInspectorTestHooks;

/// The BOQ Inspector tool hosted by the OpenConKit shell.
pub struct BoqInspectorTool {
    engine: TypedEngineAdapter<BoqInspectorEngine>,
}

impl BoqInspectorTool {
    /// Create the tool instance.
    pub fn new() -> Self {
        Self {
            engine: TypedEngineAdapter::new(BoqInspectorEngine),
        }
    }
}

impl Default for BoqInspectorTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for BoqInspectorTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            id: TOOL_ID.to_string(),
            contract_version: TOOL_CONTRACT_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            name_key: "tools.boqInspector.name".to_string(),
            description_key: "tools.boqInspector.description".to_string(),
            icon: "tools/boq-inspector.svg".to_string(),
            route: "/tools/boq-inspector".to_string(),
        }
    }

    fn input_capabilities(&self) -> InputCapabilities {
        InputCapabilities {
            accepted_extensions: vec![".xls".to_string(), ".xlsx".to_string()],
            max_file_size_bytes: 64 * 1024 * 1024,
            accepts_multiple: false,
        }
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions {
            reads_source_files: true,
            writes_exports: true,
            network: false,
            ai: true,
        }
    }

    fn rule_set_version(&self) -> &'static str {
        detection::RULE_SET_VERSION
    }

    fn engine(&self) -> &dyn ToolEngine {
        &self.engine
    }

    fn export_providers(&self) -> Vec<&dyn ExportProvider> {
        vec![&reporting::XLSX_EXPORTER, &reporting::PDF_EXPORTER]
    }

    fn ai_capability(&self) -> Option<AiCapability> {
        Some(ai::BOQ_AI_PROVIDER.capability())
    }

    fn ai_provider(&self) -> Option<&dyn ToolAiProvider> {
        Some(&ai::BOQ_AI_PROVIDER)
    }

    fn test_hooks(&self) -> Option<&dyn ToolTestHooks> {
        Some(&TEST_HOOKS)
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(BoqInspectorInput)).ok()
    }

    fn settings_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(BoqInspectorSettings)).ok()
    }

    fn output_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "BoqInspectorOutput",
            "type": "object",
            "required": ["findings", "diagnostics", "summary", "normalized_rows"],
            "properties": {
                "findings": { "type": "array", "items": { "type": "object" } },
                "diagnostics": { "type": "object" },
                "summary": schemars::schema_for!(BoqInspectorSummary),
                "normalized_rows": {
                    "type": "array",
                    "items": {"type": "object"}
                }
            },
            "additionalProperties": false
        }))
    }
}

fn normalized_row_for_output(row: &model::NormalizedBoqRow) -> BoqNormalizedRow {
    BoqNormalizedRow {
        source_row_id: row.source_row_id.clone(),
        sheet: row.sheet.clone(),
        source_row_number: row.row.saturating_add(1),
        classification: row.classification,
        classification_confidence: row.classification_confidence,
        section_path: row.section_path.clone(),
        item_code: row.item_code.as_ref().map(normalized_text_fact),
        description: row.description.as_ref().map(normalized_text_fact),
        unit: row.unit.as_ref().map(|fact| BoqNormalizedFact {
            cell: fact.cell.clone(),
            raw: fact.raw.clone(),
            formula: fact.formula.clone(),
            normalized: fact.value.canonical.clone(),
        }),
        quantity: row.quantity.as_ref().map(normalized_decimal_fact),
        rate_text: row.rate_text.as_ref().map(normalized_text_fact),
        rate: row.rate.as_ref().map(normalized_decimal_fact),
        amount: row.amount.as_ref().map(normalized_decimal_fact),
        currency: row.currency.as_ref().map(normalized_text_fact),
        error_cells: row.error_cells.iter().map(normalized_text_fact).collect(),
    }
}

fn normalized_text_fact(fact: &model::SourceValue<String>) -> BoqNormalizedFact {
    BoqNormalizedFact {
        cell: fact.cell.clone(),
        raw: fact.raw.clone(),
        formula: fact.formula.clone(),
        normalized: fact.value.clone(),
    }
}

fn normalized_decimal_fact(fact: &model::SourceValue<Decimal>) -> BoqNormalizedFact {
    BoqNormalizedFact {
        cell: fact.cell.clone(),
        raw: fact.raw.clone(),
        formula: fact.formula.clone(),
        normalized: fact.value.normalize().to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rust_xlsxwriter::Workbook;
    use serde_json::json;

    use super::*;

    fn temp_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "openconkit-boq-engine-{}-{nanos}.xlsx",
            std::process::id()
        ))
    }

    fn write_fixture(path: &Path) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name("BOQ").expect("sheet");
        for (column, header) in ["Item", "Description", "Unit", "Quantity", "Rate", "Amount"]
            .into_iter()
            .enumerate()
        {
            sheet
                .write_string(0, u16::try_from(column).expect("column"), header)
                .expect("header");
        }
        sheet.write_string(1, 0, "A1").expect("item");
        sheet
            .write_string(1, 1, "Concrete wall")
            .expect("description");
        sheet.write_string(1, 2, "m2").expect("unit");
        sheet.write_number(1, 3, 10).expect("quantity");
        sheet.write_number(1, 4, 5).expect("rate");
        sheet.write_number(1, 5, 49).expect("amount");
        workbook.save(path).expect("fixture");
    }

    fn sample_context(path: PathBuf) -> ToolRunContext {
        ToolRunContext {
            run_id: "00000000-0000-4000-8000-000000000002".to_string(),
            project_id: "project-1".to_string(),
            source_revision_id: "00000000-0000-4000-8000-000000000001".to_string(),
            workbook_path: path,
            app_version: "0.0.1".to_string(),
        }
    }

    #[test]
    fn manifest_and_contract_surfaces_are_complete() {
        let tool = BoqInspectorTool::new();
        let manifest = tool.manifest();
        assert_eq!(manifest.id, TOOL_ID);
        assert_eq!(manifest.contract_version, TOOL_CONTRACT_VERSION);
        assert_eq!(manifest.route, "/tools/boq-inspector");
        assert!(tool.input_schema().is_some());
        assert!(tool.settings_schema().is_some());
        assert!(tool.output_schema().is_some());
        assert!(tool.test_hooks().is_some());
    }

    #[test]
    fn capabilities_and_permissions_are_local_first() {
        let tool = BoqInspectorTool::new();
        let capabilities = tool.input_capabilities();
        for accepted in [".xls", "xls", ".XLS", ".xlsx", "XLSX", ".Xlsx"] {
            assert!(capabilities.accepts(accepted));
        }
        for rejected in [".csv", ".pdf", ".xlsm", ""] {
            assert!(!capabilities.accepts(rejected));
        }
        assert!(!capabilities.accepts_multiple);
        let permissions = tool.permissions();
        assert!(permissions.reads_source_files);
        assert!(permissions.writes_exports);
        assert!(!permissions.network);
        assert!(permissions.ai);
        assert!(tool.ai_capability().is_some());
    }

    #[test]
    fn engine_ingests_and_finds_amount_mismatch() {
        let path = temp_path();
        write_fixture(&path);
        let tool = BoqInspectorTool::new();
        let context = sample_context(path.clone());
        let output = tool
            .engine()
            .run(
                &context,
                &json!({ "source_revision_id": context.source_revision_id, "rules": [] }),
                &json!({ "locale": "en" }),
                &|_| {},
                &CancellationToken::new(),
            )
            .expect("engine output");
        let output: BoqInspectorOutput = serde_json::from_value(output).expect("typed output");
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.rule_id == "boq.amount_mismatch"));
        assert_eq!(output.summary.item_rows, 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn cancellation_yields_cancelled_error_before_io() {
        let tool = BoqInspectorTool::new();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let context = sample_context(PathBuf::from("missing.xlsx"));
        let err = tool
            .engine()
            .run(
                &context,
                &json!({ "source_revision_id": context.source_revision_id, "rules": [] }),
                &json!({ "locale": "en" }),
                &|_| {},
                &cancel,
            )
            .expect_err("cancelled run fails");
        assert_eq!(err, ToolError::Cancelled);
    }

    #[test]
    fn progress_callback_can_cancel_during_the_rules_stage() {
        use std::sync::Mutex;

        let path = temp_path();
        write_fixture(&path);
        let tool = BoqInspectorTool::new();
        let context = sample_context(path.clone());
        let cancel = CancellationToken::new();
        let observed = Mutex::new(Vec::new());
        let err = tool
            .engine()
            .run(
                &context,
                &json!({ "source_revision_id": context.source_revision_id, "rules": [] }),
                &json!({ "locale": "en" }),
                &|progress| {
                    observed
                        .lock()
                        .expect("progress lock")
                        .push(progress.fraction);
                    if progress.fraction >= 0.62 {
                        cancel.cancel();
                    }
                },
                &cancel,
            )
            .expect_err("rules-stage cancellation");
        assert_eq!(err, ToolError::Cancelled);
        let observed = observed.lock().expect("progress lock");
        assert_eq!(observed.first(), Some(&0.0));
        assert!(observed.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(observed.iter().any(|fraction| *fraction >= 0.62));
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn settings_reject_negative_tolerance_and_unknown_locale() {
        let negative = validate_settings(&BoqInspectorSettings {
            absolute_tolerance: "-1".into(),
            ..BoqInspectorSettings::default()
        });
        assert!(matches!(negative, Err(ToolError::InvalidSettings { .. })));
        let locale = validate_settings(&BoqInspectorSettings {
            locale: "fr".into(),
            ..BoqInspectorSettings::default()
        });
        assert!(matches!(locale, Err(ToolError::InvalidSettings { .. })));
    }
}
