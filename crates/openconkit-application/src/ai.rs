//! Safe DTOs for the optional Codex account and grounded-review UX.
//!
//! These values cross the desktop IPC boundary. They deliberately contain no
//! credentials, raw authentication responses, workbook paths, or protocol
//! internals.

use openconkit_domain::{AnalysisRunId, Sha256Hash};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Local runtime readiness without starting Codex or using the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AiRuntimeStatus {
    pub enabled: bool,
    pub bundled_runtime_available: bool,
    pub selected_runtime_available: bool,
    pub using_system_runtime: bool,
    pub codex_version: String,
}

/// ChatGPT plan classification safe to display in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AiPlanType {
    Free,
    Go,
    Plus,
    Pro,
    Prolite,
    Team,
    SelfServeBusinessUsageBased,
    Business,
    EnterpriseCbpUsageBased,
    Enterprise,
    Edu,
    Unknown,
}

/// Safe account snapshot returned by Codex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AiAccountSnapshot {
    /// Whether Codex currently has a ChatGPT session.
    pub signed_in: bool,
    /// Masked display-only email. Never a token or raw credential.
    pub masked_email: Option<String>,
    pub plan_type: Option<AiPlanType>,
    pub requires_openai_auth: bool,
    /// Pinned Codex runtime version used by this app.
    pub codex_version: String,
}

/// Login mechanism initiated by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AiLoginMode {
    Browser,
    DeviceCode,
}

/// Opaque login challenge metadata safe for the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AiLoginChallenge {
    /// Opaque identifier used only to cancel the pending login.
    pub login_id: String,
    pub mode: AiLoginMode,
    /// User-entered device code, present only for the fallback flow.
    pub user_code: Option<String>,
}

/// One account rate-limit window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AiRateLimitWindow {
    pub used_percent: u8,
    pub window_duration_minutes: Option<u32>,
    /// Unix epoch seconds, when supplied by Codex.
    pub resets_at: Option<u32>,
}

/// Safe rate-limit snapshot for settings and preflight UX.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AiRateLimitSnapshot {
    pub primary: Option<AiRateLimitWindow>,
    pub secondary: Option<AiRateLimitWindow>,
    pub plan_type: Option<AiPlanType>,
    pub rate_limit_reached: bool,
    pub spend_control_reached: bool,
}

/// Exact scope prepared for the informed-consent confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AiReviewScope {
    pub run_id: AnalysisRunId,
    pub source_sha256: Sha256Hash,
    pub source_row_count: u32,
    pub finding_count: u32,
    /// Number of deterministic source-bearing chunks.
    pub source_chunk_count: u32,
    /// Minimum planned model turns, including the final synthesis when the
    /// source needs more than one chunk.
    pub planned_turn_count: u32,
    /// UTF-8 bytes of the exact developer instructions and source-bearing
    /// prompts. Synthesis input is derived later from validated outputs.
    pub transmitted_bytes: u32,
    /// SHA-256 of the complete, canonical request envelope.
    pub input_scope_hash: Sha256Hash,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn account_snapshot_contains_no_credential_fields() {
        let snapshot = AiAccountSnapshot {
            signed_in: true,
            masked_email: Some("q***@example.com".to_string()),
            plan_type: Some(AiPlanType::Plus),
            requires_openai_auth: true,
            codex_version: "0.145.0".to_string(),
        };
        let value = serde_json::to_value(snapshot).expect("serialize");
        for prohibited in ["token", "api_key", "access_token", "refresh_token"] {
            assert!(value.get(prohibited).is_none());
        }
    }
}
