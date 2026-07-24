//! OpenConKit tool SDK.
//!
//! Defines the stable, versioned contract that every tool hosted in the
//! OpenConKit shell implements. Tools are wired at compile time through a
//! registry; there is no runtime plugin loading
//! (see `docs/adr/0003-compile-time-tool-registry-no-runtime-plugins.md`).
//!
//! # Contract overview
//!
//! A tool author implements [`Tool`] plus a [`ToolEngine`] (usually via the
//! typed helper [`TypedToolEngine`] + [`TypedEngineAdapter`]), and the shell
//! composes the tools into a [`ToolRegistry`] at startup.
//!
//! - [`ToolManifest`] — metadata the shell displays (navigation, versioning).
//! - [`InputCapabilities`] / [`ToolPermissions`] — what the tool consumes and
//!   is allowed to do, declared up front and surfaced to the user.
//! - [`ToolEngine`] — the synchronous, callback-based run boundary; see the
//!   [`progress`] module docs for the threading and cancellation design.
//! - [`ExportProvider`] — report generation from a finished run's output.
//! - [`AiCapability`] — optional, always-off-by-default AI integration.
//! - [`ToolError`] — the single error type crossing the engine boundary,
//!   serializable to the frontend with stable machine-readable codes.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod ai;
pub mod capabilities;
pub mod engine;
pub mod error;
pub mod export;
pub mod hooks;
pub mod manifest;
pub mod progress;
pub mod registry;
pub mod state;
pub mod tool;

pub use ai::{AiCapability, AiPreparedContext, AiPromptChunk, AiProviderError, ToolAiProvider};
pub use capabilities::{InputCapabilities, ToolPermissions};
pub use engine::{
    ToolEngine, ToolRunContext, ToolRunResponse, TypedEngineAdapter, TypedToolEngine,
};
pub use error::ToolError;
pub use export::{ExportContext, ExportProvider, ExportedArtifact};
pub use hooks::ToolTestHooks;
pub use manifest::ToolManifest;
pub use progress::{CancellationToken, ProgressCallback, ToolProgress, ToolProgressEvent};
pub use registry::{RegistryError, ToolNavItem, ToolRegistry};
pub use state::ToolStateMigration;
pub use tool::Tool;

/// Version of the tool contract. Bumped on breaking changes to [`Tool`] or
/// any type crossing the tool/shell boundary; tools declare the contract
/// version they target in their [`ToolManifest`].
pub const TOOL_CONTRACT_VERSION: u32 = 2;
