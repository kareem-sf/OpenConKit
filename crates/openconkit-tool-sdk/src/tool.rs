//! The tool contract trait: the full surface a hosted tool implements.
//!
//! Required methods cover identity ([`Tool::manifest`]), declared behavior
//! ([`Tool::input_capabilities`], [`Tool::permissions`]), and the run engine
//! ([`Tool::engine`]). Everything else is optional and phased in via default
//! implementations, so a minimal tool compiles against the current contract
//! and opts into exports, AI, state migrations, test hooks, and schemas as
//! it matures.

use crate::ai::{AiCapability, ToolAiProvider};
use crate::capabilities::{InputCapabilities, ToolPermissions};
use crate::engine::ToolEngine;
use crate::export::ExportProvider;
use crate::hooks::ToolTestHooks;
use crate::manifest::ToolManifest;
use crate::state::ToolStateMigration;

/// A tool hosted in the OpenConKit shell.
///
/// Tools are composed into a [`crate::ToolRegistry`] at compile time; the
/// shell interacts with them only through this trait. Implementations must
/// be `Send + Sync` because engines run on background threads.
pub trait Tool: Send + Sync {
    /// Stable metadata describing this tool (identity, version, i18n keys,
    /// icon, route). Must declare `contract_version` equal to
    /// [`crate::TOOL_CONTRACT_VERSION`].
    fn manifest(&self) -> ToolManifest;

    /// What source files this tool can ingest.
    fn input_capabilities(&self) -> InputCapabilities;

    /// Permissions this tool declares; reviewed and surfaced to the user.
    fn permissions(&self) -> ToolPermissions;

    /// Version of the deterministic rules/interpretation pipeline.
    fn rule_set_version(&self) -> &'static str;

    /// The engine that runs the analysis. See the [`crate::engine`] module
    /// for the typed-authoring path (`TypedToolEngine` + `TypedEngineAdapter`).
    fn engine(&self) -> &dyn ToolEngine;

    /// Export providers this tool ships, if any.
    fn export_providers(&self) -> Vec<&dyn ExportProvider> {
        vec![]
    }

    /// AI capability declaration, if the tool ships AI integration.
    /// `None` by default — AI is optional and off unless declared.
    fn ai_capability(&self) -> Option<AiCapability> {
        self.ai_provider().map(|provider| provider.capability())
    }

    /// Tool-specific grounded context and semantic validator.
    fn ai_provider(&self) -> Option<&dyn ToolAiProvider> {
        None
    }

    /// Append-only migrations for the tool's own persisted state.
    fn state_migrations(&self) -> &[ToolStateMigration] {
        &[]
    }

    /// Test hooks for integration tests, if the tool ships them.
    fn test_hooks(&self) -> Option<&dyn ToolTestHooks> {
        None
    }

    /// JSON Schema (as a [`serde_json::Value`]) for the tool's typed input,
    /// used by the shell for validation and by the contracts pipeline.
    /// Return `None` while phased.
    fn input_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// JSON Schema for the tool's typed settings. Return `None` while phased.
    fn settings_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// JSON Schema for the tool's typed output. Return `None` while phased.
    fn output_schema(&self) -> Option<serde_json::Value> {
        None
    }
}
