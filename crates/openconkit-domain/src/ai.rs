//! AI analysis records: output of the optional Codex sidecar.
//!
//! AI features are optional (product invariant): the app is fully useful
//! offline, and AI output is only ever presented as suggestions grounded in
//! extracted facts — never silently applied to data.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{AiAnalysisId, AnalysisRunId, Sha256Hash};

/// Lifecycle status of an AI analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AiAnalysisStatus {
    /// Requested but not yet finished.
    Pending,
    /// Finished successfully.
    Completed,
    /// Terminated with an error.
    Failed,
}

/// Language requested for one generated review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum AiAnalysisLanguage {
    En,
    Ar,
}

/// Whether a human has reviewed an AI analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AiValidationStatus {
    /// Not yet reviewed; must be treated as a mere suggestion.
    Unvalidated,
    /// Reviewed and accepted by the user.
    Validated,
    /// Reviewed and rejected by the user.
    Rejected,
}

/// Result of strict structural and grounding validation performed by the
/// tool before any AI output can be displayed or exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AiGroundingStatus {
    /// The model request has not produced a response yet.
    Pending,
    /// Structure, bounds, finding IDs, and evidence references all passed.
    Validated,
    /// The response was rejected and its content was not persisted.
    Rejected,
}

/// One AI analysis pass over a run's extracted facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AiAnalysis {
    /// Unique id of this analysis.
    pub id: AiAnalysisId,
    /// Run whose facts were analyzed.
    pub run_id: AnalysisRunId,
    /// Model that produced the analysis (e.g. `gpt-5-codex`).
    pub model: String,
    /// Version of the Codex sidecar used.
    pub codex_version: String,
    /// Language of all generated user-facing text.
    pub language: AiAnalysisLanguage,
    /// SHA-256 over the exact input scope sent to the model, for
    /// reproducibility and audit.
    pub input_scope_hash: Sha256Hash,
    /// Lifecycle status.
    pub status: AiAnalysisStatus,
    /// Human validation status.
    pub validation_status: AiValidationStatus,
    /// Tool-owned strict grounding validation status.
    pub grounding_status: AiGroundingStatus,
    /// Raw model output, if the analysis completed. Opaque to the domain;
    /// typed as `unknown` on the TypeScript side.
    #[ts(type = "unknown")]
    pub output: Option<serde_json::Value>,
    /// When the analysis was requested.
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn sample() -> AiAnalysis {
        AiAnalysis {
            id: AiAnalysisId::new(),
            run_id: AnalysisRunId::new(),
            model: "gpt-5-codex".into(),
            codex_version: "0.44.0".into(),
            language: AiAnalysisLanguage::En,
            input_scope_hash: Sha256Hash::from_bytes([0x33; 32]),
            status: AiAnalysisStatus::Completed,
            validation_status: AiValidationStatus::Unvalidated,
            grounding_status: AiGroundingStatus::Validated,
            output: Some(serde_json::json!({"summary": "ok"})),
            created_at: Timestamp::now(),
        }
    }

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&AiAnalysisStatus::Pending).expect("serialize"),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&AiValidationStatus::Unvalidated).expect("serialize"),
            "\"unvalidated\""
        );
        assert_eq!(
            serde_json::to_string(&AiGroundingStatus::Validated).expect("serialize"),
            "\"validated\""
        );
        assert_eq!(
            serde_json::to_string(&AiAnalysisLanguage::Ar).expect("serialize"),
            "\"ar\""
        );
    }

    #[test]
    fn serde_round_trip() {
        let analysis = sample();
        let json = serde_json::to_string(&analysis).expect("serialize");
        let back: AiAnalysis = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, analysis);
    }

    #[test]
    fn ts_decl_maps_output_to_unknown() {
        let cfg = ts_rs::Config::default();
        let decl = <AiAnalysis as TS>::decl(&cfg);
        assert!(decl.contains("output: unknown"), "{decl}");
        assert!(decl.contains("created_at: string"), "{decl}");
    }
}
