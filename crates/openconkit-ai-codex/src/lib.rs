//! OpenConKit AI integration with the bundled OpenAI Codex app server.
//!
//! The Codex sidecar is an OPTIONAL component: the application is fully
//! useful offline without it (see `docs/privacy.md`). This crate currently
//! provides the module structure and the version-pin plumbing; the stdio
//! JSON-RPC client lands in the Codex integration phase.
//!
//! Modules:
//! - [`pin`]: the pinned Codex release (`tools/codex-version.json`).
//! - [`sidecar`]: sidecar binary naming and staging layout.
//! - [`client`]: stdio client configuration surface.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod client;
pub mod pin;
pub mod sidecar;

pub use client::CodexClientConfig;
pub use pin::{pinned_release, CodexPin};
pub use sidecar::sidecar_binary_name;

/// Errors from the Codex integration.
#[derive(Debug, thiserror::Error)]
pub enum CodexError {
    /// The pinned version manifest could not be parsed.
    #[error("invalid codex version manifest: {0}")]
    Manifest(#[from] serde_json::Error),

    /// The sidecar binary is not available (not staged or unsupported target).
    #[error("codex sidecar is not available: {0}")]
    SidecarUnavailable(String),
}
