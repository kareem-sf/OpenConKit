# 0007. Timestamp library: jiff

- Status: Accepted
- Date: 2026-07-23

## Context

The workspace needs one date/time library for created-at timestamps, report
metadata and log times. Candidates: `chrono`, `time`, `jiff`.

## Decision

Use **jiff** (`0.2`, with the `serde` feature) as the single timestamp
library across all crates. Persist timestamps as RFC 9557 / ISO 8601 strings
in UTC. Do not add `chrono` or `time` to the workspace.

## Consequences

- Positive: modern, well-documented API designed around correctness
  pitfalls; first-class time-zone and RFC 9557 support (useful when
  rendering report dates in the user's locale, including Arabic);
  actively maintained.
- Negative: younger ecosystem than `chrono`; some third-party crates expect
  `chrono` types at their boundaries (convert at the edge if ever needed).
- Note: `jiff::Timestamp` crosses the IPC boundary as an ISO string; the
  ts-rs contracts treat it as `string` (see ADR 0005).
