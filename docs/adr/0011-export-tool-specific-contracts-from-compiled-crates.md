# ADR 0011: Export tool-specific contracts from compiled tool crates

- Status: Accepted
- Date: 2026-07-23

## Context

OpenConKit commits Rust-to-TypeScript bindings and rejects contract drift in
CI (ADR 0005). Tool run input, settings, summaries, and output are owned by
their tool crate; moving those DTOs into the domain or SDK would make the
generic layers conceptually depend on one hosted tool. Hand-written duplicate
TypeScript types would remove the drift guarantee.

The contracts exporter is build tooling outside the runtime dependency graph.
The architecture checker previously limited it to domain, application, and
the tool SDK, so it could not instantiate `ts-rs` for a tool-owned DTO.

## Decision

`openconkit-contracts-export` may have build-time Cargo dependencies on
compiled `openconkit-tool-*` crates and explicitly list their exported DTOs.
The exception applies only to this exporter:

- tool crates still never depend on other tool crates or the desktop host;
- runtime infrastructure still never depends on hosted tools;
- the exporter must not execute tool engines or access user data;
- every exported type remains explicit in the exporter source and committed;
- CI continues to regenerate into a temporary directory and reject drift.

## Consequences

Tool-specific IPC payloads have one Rust source of truth and generated
TypeScript bindings. Adding a tool contract also adds a build-time exporter
edge, increasing contract-check compile time slightly. This exception does
not change the production binary's dependency direction.
