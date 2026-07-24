//! OpenConKit AI integration with the bundled OpenAI Codex app server.
//!
//! The Codex sidecar is an OPTIONAL component: the application is fully
//! useful offline without it (see `docs/privacy.md`). This crate currently
//! provides the pinned runtime, isolated profile, stable protocol bindings,
//! and supervised JSONL-over-stdio client.
//!
//! Modules:
//! - [`pin`]: the pinned Codex release (`tools/codex-version.json`).
//! - [`sidecar`]: sidecar binary naming and staging layout.
//! - [`client`]: shell-free process configuration.
//! - [`profile`]: isolated, privacy-preserving Codex home.
//! - [`protocol`]: stable protocol subset used by OpenConKit.
//! - [`transport`]: supervised JSONL-over-stdio client.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod client;
mod diagnostics;
pub mod pin;
pub mod profile;
pub mod protocol;
pub mod service;
pub mod sidecar;
pub mod transport;

pub use client::{CodexBinaryKind, CodexClientConfig};
pub use pin::{pinned_release, CodexPin, CodexResourcePin, CodexTargetPin};
pub use profile::prepare_codex_home;
pub use service::{
    CodexAnalysisRequest, CodexAnalysisResponse, CodexCancellationToken, CodexService,
    ANALYSIS_MODEL,
};
pub use sidecar::sidecar_binary_name;
pub use transport::{CodexClient, CodexNotification};

/// Errors from the Codex integration.
#[derive(Debug, thiserror::Error)]
pub enum CodexError {
    /// The pinned version manifest could not be parsed.
    #[error("invalid codex version manifest")]
    Manifest,

    /// Local JSON serialization failed.
    #[error("codex protocol serialization failed")]
    Json(#[from] serde_json::Error),

    /// The sidecar binary is not available (not staged or unsupported target).
    #[error("codex sidecar is not available: {0}")]
    SidecarUnavailable(String),

    /// A local path or timeout configuration is invalid.
    #[error("invalid codex configuration: {0}")]
    InvalidConfiguration(String),

    /// Local profile or process IO failed.
    #[error("codex process IO failed")]
    Io(#[from] std::io::Error),

    /// The sidecar emitted malformed or unsupported protocol data.
    #[error("codex protocol error")]
    Protocol,

    /// The sidecar rejected a request. The upstream message is deliberately
    /// omitted because it can contain user or account data.
    #[error("codex request failed with code {code}")]
    Server {
        /// Stable numeric JSON-RPC error code.
        code: i64,
    },

    /// A bounded request or turn exceeded its deadline.
    #[error("codex request timed out")]
    Timeout,

    /// The process stopped before a pending request completed.
    #[error("codex process exited")]
    ProcessExited,

    /// An inbound or outbound JSONL message exceeded the safety bound.
    #[error("codex protocol message exceeded the size limit")]
    MessageTooLarge,

    /// OpenConKit supports only Codex-managed ChatGPT authentication.
    #[error("unsupported codex authentication mode")]
    UnsupportedAuthentication,

    /// The model turn failed, was interrupted, or returned no structured
    /// assistant output.
    #[error("codex analysis failed")]
    AnalysisFailed,

    /// The user cancelled an active analysis.
    #[error("codex analysis was cancelled")]
    Cancelled,

    /// The sidecar exceeded the process restart budget for this app session.
    #[error("codex process restart limit reached")]
    RestartLimit,

    /// A supposedly tool-free analysis attempted an external action.
    #[error("codex analysis attempted a prohibited action")]
    UnsafeActivity,
}
