//! Optional AI capability declaration.
//!
//! AI is always optional and off by default: the app must be fully useful
//! offline, and AI output is grounded in extracted facts and shown as
//! suggestions, never silently applied to data (product invariant 4). A tool
//! that ships AI integration declares it through [`crate::Tool::ai_capability`]
//! so the shell can wire context emission and strict output validation.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Schemas describing a tool's AI integration surface.
///
/// Both schemas are JSON Schemas carried as opaque values; they are typed as
/// `unknown` on the TypeScript side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AiCapability {
    /// JSON Schema of the context the tool emits to the AI (extracted facts,
    /// never raw cell dumps).
    #[ts(type = "unknown")]
    pub context_schema: serde_json::Value,
    /// **Strict** JSON Schema the AI output must validate against before it
    /// is shown to the user as a suggestion.
    #[ts(type = "unknown")]
    pub output_schema: serde_json::Value,
}

/// Bounded, tool-owned context prepared from one authoritative stored output.
#[derive(Debug, Clone, PartialEq)]
pub struct AiPreparedContext {
    /// Structured extracted facts sent to Codex. It never contains a workbook
    /// path or grants access to the source file.
    pub payload: serde_json::Value,
    /// Normalized source rows included in the payload.
    pub source_row_count: u32,
    /// Deterministic findings included in the payload.
    pub finding_count: u32,
}

/// One deterministic source-bearing prompt in an AI plan.
///
/// `validation_context` is the exact authoritative subset represented by the
/// prompt. It lets a tool reject citations to facts that were not present in
/// that turn before a later synthesis can consume the output.
#[derive(Debug, Clone, PartialEq)]
pub struct AiPromptChunk {
    /// Model input containing only supplied, delimited OpenConKit data.
    pub input: String,
    /// Tool-owned authoritative subset used to validate the chunk output.
    pub validation_context: AiPreparedContext,
}

/// Safe failures from tool-specific context and output validation.
#[derive(Debug, thiserror::Error)]
pub enum AiProviderError {
    /// Stored output no longer matches the tool's versioned contract.
    #[error("stored tool output is invalid")]
    InvalidStoredOutput,
    /// The requested language is unsupported.
    #[error("unsupported AI output language")]
    UnsupportedLanguage,
    /// A single indivisible normalized fact cannot fit the safe prompt bound.
    #[error("AI context cannot be partitioned within the safe prompt bound")]
    ContextTooLarge,
    /// Model output failed strict structure, bounds, or grounding checks.
    #[error("AI output failed grounding validation")]
    InvalidModelOutput,
}

/// Tool-owned grounded AI contract.
///
/// The host controls authentication, process isolation and transport; the
/// tool alone knows which extracted facts are safe to send and how to reject
/// unsupported references in model output.
pub trait ToolAiProvider: Send + Sync {
    /// Strict context and output schemas.
    fn capability(&self) -> AiCapability;

    /// Build the exact context from a completed stored tool output.
    fn prepare_context(
        &self,
        authoritative_output: &serde_json::Value,
    ) -> Result<AiPreparedContext, AiProviderError>;

    /// Stable analyzer-only developer instructions.
    fn developer_instructions(&self, language: &str) -> Result<String, AiProviderError>;

    /// Render one user input containing the delimited normalized context.
    fn prompt(
        &self,
        language: &str,
        context: &AiPreparedContext,
    ) -> Result<String, AiProviderError>;

    /// Build one or more deterministic, source-bearing prompts.
    ///
    /// The default preserves the complete-context behavior for tools whose
    /// prompt fits one turn. Tools supporting larger scopes override this and
    /// must retain every source identifier across the returned chunks.
    fn prompt_chunks(
        &self,
        language: &str,
        context: &AiPreparedContext,
        maximum_input_bytes: usize,
    ) -> Result<Vec<AiPromptChunk>, AiProviderError> {
        let input = self.prompt(language, context)?;
        if input.len() > maximum_input_bytes {
            return Err(AiProviderError::ContextTooLarge);
        }
        Ok(vec![AiPromptChunk {
            input,
            validation_context: context.clone(),
        }])
    }

    /// Tighter schema used for intermediate chunk and reduction outputs.
    ///
    /// It defaults to the final output schema. A chunking provider should
    /// return a deliberately smaller schema so synthesis remains bounded.
    fn intermediate_output_schema(&self) -> serde_json::Value {
        self.capability().output_schema
    }

    /// Merge validated intermediate outputs into a grounded final review.
    ///
    /// This is called only when the plan contains multiple source chunks or
    /// when a bounded reduction pass is needed.
    fn synthesis_prompt(
        &self,
        _language: &str,
        _validated_outputs: &[serde_json::Value],
    ) -> Result<String, AiProviderError> {
        Err(AiProviderError::ContextTooLarge)
    }

    /// Parse, bound, and semantically ground a structured model response.
    fn validate_output(
        &self,
        context: &AiPreparedContext,
        model_output: serde_json::Value,
    ) -> Result<serde_json::Value, AiProviderError>;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn ai_capability_serde_round_trip() {
        let capability = AiCapability {
            context_schema: json!({ "type": "object" }),
            output_schema: json!({ "type": "object", "additionalProperties": false }),
        };
        let json = serde_json::to_string(&capability).expect("serializes");
        let back: AiCapability = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(capability, back);
    }

    #[test]
    fn typescript_binding_types_schemas_as_unknown() {
        let decl = AiCapability::decl(&ts_rs::Config::default());
        assert!(
            decl.contains("context_schema: unknown"),
            "context_schema not unknown in {decl}"
        );
        assert!(
            decl.contains("output_schema: unknown"),
            "output_schema not unknown in {decl}"
        );
    }
}
