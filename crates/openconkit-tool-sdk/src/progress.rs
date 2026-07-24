//! Progress reporting and cooperative cancellation for tool runs.
//!
//! # Design: synchronous engine, callback progress, token cancellation
//!
//! The engine contract is deliberately **synchronous and callback-based**:
//!
//! - The host runs [`crate::ToolEngine::run`] on a background thread (the
//!   Tauri async runtime's blocking pool), so the UI thread never blocks on
//!   long analyses.
//! - Progress flows from the engine to the host through a plain
//!   [`ProgressCallback`]; the host forwards it to the frontend as an IPC
//!   event. No shared mutable state, no async inside the engine.
//! - Cancellation is **cooperative** via a [`CancellationToken`]: the host
//!   calls [`CancellationToken::cancel`], and the engine checks
//!   [`CancellationToken::is_cancelled`] at well-defined points (per sheet,
//!   per row batch, per analysis pass) and returns
//!   [`crate::ToolError::Cancelled`] promptly. The token is cheap to clone;
//!   clones share the same underlying flag.
//!
//! This design is documented in ADR 0009.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A progress update emitted by an engine during a run.
///
/// Progress must never leak sensitive content: `detail` may describe *what*
/// is being processed (e.g. a sheet name or a row count), never cell values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ToolProgress {
    /// i18n key identifying the current phase of the run.
    pub phase_key: String,
    /// Completion fraction, always within `0.0..=1.0` (clamped by
    /// [`ToolProgress::new`]).
    pub fraction: f64,
    /// Optional non-sensitive detail, e.g. `"sheet: BOQ (3 of 12)"`.
    /// Never cell contents.
    pub detail: Option<String>,
}

/// Host event payload used to associate progress with an active run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ToolProgressEvent {
    pub run_id: String,
    pub progress: ToolProgress,
}

impl ToolProgress {
    /// Create a progress update. `fraction` is clamped into `0.0..=1.0`;
    /// `NaN` is treated as `0.0`.
    pub fn new(phase_key: impl Into<String>, fraction: f64) -> Self {
        Self {
            phase_key: phase_key.into(),
            fraction: clamp_fraction(fraction),
            detail: None,
        }
    }

    /// Attach a non-sensitive detail string (never cell contents).
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Clamp a fraction into `0.0..=1.0`, mapping `NaN` to `0.0`.
fn clamp_fraction(fraction: f64) -> f64 {
    if fraction.is_nan() {
        0.0
    } else {
        fraction.clamp(0.0, 1.0)
    }
}

/// Sink through which an engine reports progress to the host.
///
/// The host supplies the closure when invoking [`crate::ToolEngine::run`];
/// typical implementations forward the update to the frontend as an IPC
/// event.
pub type ProgressCallback<'a> = &'a dyn Fn(ToolProgress);

/// Cooperative cancellation signal shared between host and engine.
///
/// Clones share the same underlying flag: cancelling through any clone is
/// observed by all of them. Cheap to clone and pass into the engine.
#[derive(Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a fresh, un-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal cancellation. Idempotent.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation has been signalled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Clone for CancellationToken {
    /// Clone the token; the clone shares the same underlying flag.
    fn clone(&self) -> Self {
        Self {
            cancelled: Arc::clone(&self.cancelled),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn progress_new_clamps_fraction_into_unit_range() {
        assert_eq!(ToolProgress::new("p", -0.5).fraction, 0.0);
        assert_eq!(ToolProgress::new("p", 0.0).fraction, 0.0);
        assert_eq!(ToolProgress::new("p", 0.42).fraction, 0.42);
        assert_eq!(ToolProgress::new("p", 1.0).fraction, 1.0);
        assert_eq!(ToolProgress::new("p", 7.25).fraction, 1.0);
        assert_eq!(ToolProgress::new("p", f64::NAN).fraction, 0.0);
    }

    #[test]
    fn progress_with_detail_attaches_detail() {
        let progress =
            ToolProgress::new("tools.dummy.phase.parse", 0.5).with_detail("sheet 2 of 5");
        assert_eq!(progress.phase_key, "tools.dummy.phase.parse");
        assert_eq!(progress.detail.as_deref(), Some("sheet 2 of 5"));
    }

    #[test]
    fn cancellation_token_clones_share_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        assert!(!clone.is_cancelled());

        clone.cancel();
        assert!(token.is_cancelled());
        assert!(clone.is_cancelled());

        // Idempotent.
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn independent_tokens_do_not_share_state() {
        let a = CancellationToken::new();
        let b = CancellationToken::new();
        a.cancel();
        assert!(a.is_cancelled());
        assert!(!b.is_cancelled());
    }
}
