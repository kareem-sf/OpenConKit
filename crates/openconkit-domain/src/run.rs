//! Analysis runs: one tool execution over one source revision.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::finding::Confidence;
use crate::ids::{AnalysisRunId, SourceRevisionId};
use crate::project::ProjectId;
use crate::workbook::WorkbookDiagnostics;

/// Lifecycle status of an analysis run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RunStatus {
    /// Created but not yet started.
    Pending,
    /// Currently executing.
    Running,
    /// Finished successfully (findings may still be empty).
    Completed,
    /// Terminated with an error.
    Failed,
    /// Aborted by the user.
    Cancelled,
}

/// A single execution of a tool's rule set against a source revision.
///
/// The version fields make every finding attributable: given a run you can
/// tell exactly which tool build, rule set, and app version produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AnalysisRun {
    /// Unique id of this run.
    pub id: AnalysisRunId,
    /// Project the run belongs to.
    pub project_id: ProjectId,
    /// Source revision the run analyzed.
    pub source_revision_id: SourceRevisionId,
    /// Id of the tool that ran (e.g. `boq-inspector`).
    pub tool_id: String,
    /// Version of the tool.
    pub tool_version: String,
    /// Version of the rule set applied.
    pub rule_set_version: String,
    /// Version of the app hosting the tool.
    pub app_version: String,
    /// Current lifecycle status.
    pub status: RunStatus,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run finished (any terminal status), if it has.
    pub finished_at: Option<Timestamp>,
    /// Structural diagnostics of the analyzed workbook, once parsed.
    pub structure_diagnostics: Option<WorkbookDiagnostics>,
    /// Aggregate confidence across the run's findings, if computed.
    pub overall_confidence: Option<Confidence>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::workbook::SheetInventory;

    fn sample() -> AnalysisRun {
        AnalysisRun {
            id: AnalysisRunId::new(),
            project_id: ProjectId::new("tower-a").expect("slug"),
            source_revision_id: SourceRevisionId::new(),
            tool_id: "boq-inspector".into(),
            tool_version: "0.1.0".into(),
            rule_set_version: "2026.07".into(),
            app_version: "0.0.1".into(),
            status: RunStatus::Completed,
            started_at: Timestamp::now(),
            finished_at: Some(Timestamp::now()),
            structure_diagnostics: Some(WorkbookDiagnostics {
                rule_set_version: "2026.07.1".into(),
                sheets: vec![SheetInventory {
                    index: 0,
                    name: "BOQ".into(),
                    visibility: crate::workbook::SheetVisibility::Visible,
                    used_rows: 120,
                    used_columns: 8,
                    non_empty_cells: 512,
                    detected_tables: 1,
                }],
                tables: vec![],
                interpretation_confidence: Confidence::new(0.87).expect("in range"),
                warnings: vec![],
            }),
            overall_confidence: Some(Confidence::new(0.87).expect("in range")),
        }
    }

    #[test]
    fn run_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&RunStatus::Cancelled).expect("serialize"),
            "\"cancelled\""
        );
        let back: RunStatus = serde_json::from_str("\"pending\"").expect("deserialize");
        assert_eq!(back, RunStatus::Pending);
    }

    #[test]
    fn analysis_run_serde_round_trip() {
        let run = sample();
        let json = serde_json::to_string(&run).expect("serialize");
        let back: AnalysisRun = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, run);
    }

    #[test]
    fn analysis_run_ts_decl_maps_ids_and_timestamps_to_string() {
        let cfg = ts_rs::Config::default();
        let decl = <AnalysisRun as TS>::decl(&cfg);
        assert!(decl.contains("id: AnalysisRunId"), "{decl}");
        assert!(decl.contains("started_at: string"), "{decl}");
        assert!(decl.contains("finished_at: string | null"), "{decl}");
    }
}
