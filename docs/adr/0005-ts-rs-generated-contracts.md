# 0005. ts-rs generated TypeScript contracts

- Status: Accepted
- Date: 2026-07-23

## Context

The Rust backend and TypeScript frontend exchange typed data over Tauri IPC.
Hand-maintained mirror types drift silently.

## Decision

- Shared types derive `ts_rs::TS` in Rust and export TypeScript declarations
  into `packages/contracts/src/generated/`.
- Generated files are committed to the repository.
- CI regenerates and drift-checks them (build fails if Rust types and
  committed bindings differ).
- `@openconkit/contracts` additionally provides zod schemas for runtime
  validation at the frontend boundary.
- Field naming follows the serde representation of the Rust types
  (snake_case), so the wire format, the bindings and the validators agree.

## Consequences

- Positive: single source of truth for IPC shapes; drift is a build
  failure, not a runtime bug; runtime validation catches version skew from
  older frontends.
- Negative: export step must be wired into tests/CI (contracts phase);
  ts-rs type coverage constrains which Rust types can cross the boundary
  (e.g. `jiff::Timestamp` crosses as `string`).
- Alternatives considered: handwritten types (rejected: drift); JSON Schema
  codegen (rejected: heavier toolchain for current needs).
