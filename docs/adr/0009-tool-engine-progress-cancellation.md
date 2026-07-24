# 0009. Tool engine execution: synchronous engines, callback progress, cooperative cancellation

- Status: Accepted
- Date: 2026-07-22

## Context

Tools run potentially long analyses (large workbooks). The contract must
keep the UI responsive, report progress, and support cancellation — without
forcing every tool author into async Rust, and without runtime plugin
loading (ADR 0003).

## Decision

- The engine contract is **synchronous**:
  `ToolEngine::run(&self, context, input, settings, progress, cancel)`.
  The desktop host executes engines on a background thread (Tauri async
  runtime / thread pool), so the webview never blocks. Engine authors write
  plain synchronous code.
- **Progress** flows through a callback, `ProgressCallback = &dyn Fn(ToolProgress)`.
  `ToolProgress` carries an i18n `phase_key`, a `fraction` in `0.0..=1.0`,
  and an optional non-sensitive `detail` (never cell contents — logging
  policy). The host forwards progress to the frontend as Tauri events.
- **Cancellation** is cooperative: `CancellationToken` wraps a shared
  `Arc<AtomicBool>`; cheap clones share one flag. Engines check
  `is_cancelled()` at safe points and return `ToolError::Cancelled`
  promptly. There is no forced thread kill.
- **Type erasure at the registry boundary**: the object-safe `ToolEngine`
  speaks `serde_json::Value` so the registry stays `dyn`-compatible. Tool
  authors implement `TypedToolEngine` with concrete serde types and get the
  erased interface for free via `TypedEngineAdapter`; deserialization
  failures map to `ToolError::InvalidInput`/`InvalidSettings`.
- The SDK may depend on `openconkit-domain` (pure types only — no infra),
  so the contract speaks in domain types (`Finding`, `WorkbookDiagnostics`,
  `ExportKind`). This amends the original "sdk depends only on serde/ts-rs"
  note: domain types are the lingua franca of findings, and duplicating
  them inside the SDK would fork the model.

## Consequences

- Positive: simple mental model for tool authors; no async runtime leaks
  into tool crates; UI stays responsive; cancellation cannot corrupt state
  because engines decide safe interruption points; deterministic engines
  stay trivially testable (call `run` directly with a token).
- Negative: the host is responsible for threading (one place, owned by
  `openconkit-desktop`); JSON round-trip at the boundary costs a
  serialization pass per run (negligible next to workbook parsing).
- Alternatives considered: async trait engines (rejected: drags an async
  runtime into every tool crate and complicates deterministic tests);
  channel-based progress (rejected: callback is simpler and suffices for a
  single consumer).
