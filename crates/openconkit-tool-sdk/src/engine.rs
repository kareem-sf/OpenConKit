//! The run engine: the dyn-safe boundary where analysis happens.
//!
//! The registry and shell speak to engines through [`ToolEngine`], which
//! uses [`serde_json::Value`] for type erasure: input, settings, and output
//! are tool-typed structures serialized at the boundary. Tool authors should
//! almost never implement [`ToolEngine`] directly — implement
//! [`TypedToolEngine`] with concrete types and wrap it in
//! [`TypedEngineAdapter`], which performs the (de)serialization once and maps
//! failures to [`ToolError::InvalidInput`] / [`ToolError::InvalidSettings`] /
//! [`ToolError::Engine`].
//!
//! Engines are synchronous; see the [`crate::progress`] module docs for the
//! threading, progress, and cancellation design.

use std::path::PathBuf;

use openconkit_domain::AnalysisRun;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ToolError;
use crate::progress::{CancellationToken, ProgressCallback};

/// Everything an engine needs to know about the run it is executing.
///
/// The host assigns all ids; the engine treats them as opaque strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ToolRunContext {
    /// `AnalysisRunId` assigned by the host for this run.
    pub run_id: String,
    /// Id of the project the run belongs to.
    pub project_id: String,
    /// Id of the source revision being analyzed.
    pub source_revision_id: String,
    /// Path to the **immutable stored copy** of the source workbook in app
    /// home — never the user's original file. Engines must treat it as
    /// read-only (product invariant: source workbooks are never modified).
    pub workbook_path: PathBuf,
    /// Version of the running application, for provenance in outputs.
    pub app_version: String,
}

/// Persisted run metadata together with its tool-specific output.
///
/// The output remains dynamically typed at the shared SDK boundary. The
/// active tool must validate it with its generated tool-specific schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ToolRunResponse {
    pub run: AnalysisRun,
    #[ts(type = "unknown")]
    pub output: serde_json::Value,
}

/// The dyn-safe engine boundary used by the registry and the shell.
///
/// Implementations **must** check `cancel` periodically and return
/// [`ToolError::Cancelled`] promptly, and **should** report progress through
/// `progress`. The host runs engines on a background thread, so
/// implementations may block freely but must not assume a particular thread.
pub trait ToolEngine: Send + Sync {
    /// Runs the analysis.
    ///
    /// - `input` — the tool-typed input, serialized; the tool validates and
    ///   deserializes it.
    /// - `settings` — the tool-typed settings, serialized.
    ///
    /// Returns the tool-typed output, serialized.
    fn run(
        &self,
        context: &ToolRunContext,
        input: &serde_json::Value,
        settings: &serde_json::Value,
        progress: ProgressCallback<'_>,
        cancel: &CancellationToken,
    ) -> Result<serde_json::Value, ToolError>;
}

/// Blanket-style adapter trait: implement this with concrete types and get
/// [`ToolEngine`] for free via [`TypedEngineAdapter`].
///
/// Tool authors write their analysis once, fully typed; the adapter handles
/// serialization at the dyn-safe boundary.
pub trait TypedToolEngine {
    /// Tool-typed run input.
    type Input: serde::de::DeserializeOwned;
    /// Tool-typed run settings.
    type Settings: serde::de::DeserializeOwned;
    /// Tool-typed run output.
    type Output: serde::Serialize;

    /// Runs the analysis with concrete types.
    ///
    /// Same obligations as [`ToolEngine::run`]: check `cancel` periodically,
    /// report progress through `progress`.
    fn run_typed(
        &self,
        context: &ToolRunContext,
        input: Self::Input,
        settings: Self::Settings,
        progress: ProgressCallback<'_>,
        cancel: &CancellationToken,
    ) -> Result<Self::Output, ToolError>;
}

/// Adapter from a [`TypedToolEngine`] to the dyn-safe [`ToolEngine`].
///
/// Deserialization failures map to [`ToolError::InvalidInput`] (input) or
/// [`ToolError::InvalidSettings`] (settings); output serialization failures
/// map to [`ToolError::Engine`].
pub struct TypedEngineAdapter<T> {
    inner: T,
}

impl<T> TypedEngineAdapter<T> {
    /// Wrap a typed engine.
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Borrow the wrapped typed engine.
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T> ToolEngine for TypedEngineAdapter<T>
where
    T: TypedToolEngine + Send + Sync,
{
    fn run(
        &self,
        context: &ToolRunContext,
        input: &serde_json::Value,
        settings: &serde_json::Value,
        progress: ProgressCallback<'_>,
        cancel: &CancellationToken,
    ) -> Result<serde_json::Value, ToolError> {
        let typed_input: T::Input =
            serde_json::from_value(input.clone()).map_err(|err| ToolError::InvalidInput {
                message: err.to_string(),
            })?;
        let typed_settings: T::Settings =
            serde_json::from_value(settings.clone()).map_err(|err| ToolError::InvalidSettings {
                message: err.to_string(),
            })?;
        let output =
            self.inner
                .run_typed(context, typed_input, typed_settings, progress, cancel)?;
        serde_json::to_value(output).map_err(|err| ToolError::Engine {
            message: err.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use crate::progress::ToolProgress;

    #[derive(Debug, Deserialize)]
    struct DummyInput {
        threshold: u32,
    }

    #[derive(Debug, Deserialize)]
    struct DummySettings {
        strict: bool,
    }

    #[derive(Debug, Serialize, PartialEq)]
    struct DummyOutput {
        count: u32,
        strict: bool,
    }

    /// A typed engine that doubles the threshold, honoring strict mode,
    /// reporting progress, and checking cancellation before doing work.
    struct DummyTypedEngine;

    impl TypedToolEngine for DummyTypedEngine {
        type Input = DummyInput;
        type Settings = DummySettings;
        type Output = DummyOutput;

        fn run_typed(
            &self,
            _context: &ToolRunContext,
            input: Self::Input,
            settings: Self::Settings,
            progress: ProgressCallback<'_>,
            cancel: &CancellationToken,
        ) -> Result<Self::Output, ToolError> {
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            progress(ToolProgress::new("tools.dummy.phase.half", 0.5));
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            progress(ToolProgress::new("tools.dummy.phase.done", 1.0));
            Ok(DummyOutput {
                count: input.threshold * 2,
                strict: settings.strict,
            })
        }
    }

    fn sample_context() -> ToolRunContext {
        ToolRunContext {
            run_id: "run-1".to_string(),
            project_id: "project-1".to_string(),
            source_revision_id: "rev-1".to_string(),
            workbook_path: PathBuf::from("stored/workbook.xlsx"),
            app_version: "0.0.1".to_string(),
        }
    }

    #[test]
    fn typed_engine_runs_end_to_end_through_dyn_boundary() {
        let adapter = TypedEngineAdapter::new(DummyTypedEngine);
        let engine: &dyn ToolEngine = &adapter;
        let seen = Mutex::new(Vec::new());
        let progress = |p: ToolProgress| seen.lock().expect("lock").push(p);

        let output = engine
            .run(
                &sample_context(),
                &json!({ "threshold": 21 }),
                &json!({ "strict": true }),
                &progress,
                &CancellationToken::new(),
            )
            .expect("run succeeds");

        assert_eq!(output, json!({ "count": 42, "strict": true }));
        let fractions: Vec<f64> = seen
            .lock()
            .expect("lock")
            .iter()
            .map(|p| p.fraction)
            .collect();
        assert_eq!(fractions, vec![0.5, 1.0]);
    }

    #[test]
    fn cancellation_before_run_yields_cancelled_error() {
        let adapter = TypedEngineAdapter::new(DummyTypedEngine);
        let engine: &dyn ToolEngine = &adapter;
        let cancel = CancellationToken::new();
        cancel.cancel();

        let err = engine
            .run(
                &sample_context(),
                &json!({ "threshold": 1 }),
                &json!({ "strict": false }),
                &|_| {},
                &cancel,
            )
            .expect_err("cancelled run fails");

        assert_eq!(err, ToolError::Cancelled);
    }

    #[test]
    fn invalid_input_json_maps_to_invalid_input_error() {
        let adapter = TypedEngineAdapter::new(DummyTypedEngine);
        let engine: &dyn ToolEngine = &adapter;

        let err = engine
            .run(
                &sample_context(),
                &json!({ "threshold": "not-a-number" }),
                &json!({ "strict": false }),
                &|_| {},
                &CancellationToken::new(),
            )
            .expect_err("bad input fails");

        match &err {
            ToolError::InvalidInput { message } => assert!(!message.is_empty()),
            other => assert!(
                matches!(other, ToolError::InvalidInput { .. }),
                "expected InvalidInput, got {other:?}"
            ),
        }
    }

    #[test]
    fn invalid_settings_json_maps_to_invalid_settings_error() {
        let adapter = TypedEngineAdapter::new(DummyTypedEngine);
        let engine: &dyn ToolEngine = &adapter;

        let err = engine
            .run(
                &sample_context(),
                &json!({ "threshold": 1 }),
                &json!({ "strict": "maybe" }),
                &|_| {},
                &CancellationToken::new(),
            )
            .expect_err("bad settings fail");

        match &err {
            ToolError::InvalidSettings { message } => assert!(!message.is_empty()),
            other => assert!(
                matches!(other, ToolError::InvalidSettings { .. }),
                "expected InvalidSettings, got {other:?}"
            ),
        }
    }
}
