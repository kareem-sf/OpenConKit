# 0002. Modular monolith with Cargo + pnpm workspaces

- Status: Accepted
- Date: 2026-07-23

## Context

The app needs clean separation between domain logic, use cases,
infrastructure and tools, without the operational cost of multiple
repositories or services.

## Decision

A single repository ("modular monolith") with:

- One Cargo workspace (`crates/*`, resolver 2, shared
  `[workspace.dependencies]`) holding layered crates: domain, application,
  tool-sdk, infrastructure (spreadsheet, storage, reporting, ai-codex),
  tools, and the desktop host.
- One pnpm workspace (`apps/*`, `packages/*`) holding the desktop UI and
  shared frontend packages (ui, i18n, contracts).
- Dependency rules enforced by review and documented in
  `docs/architecture.md` and `AGENTS.md`.

## Consequences

- Positive: atomic refactors across layers, one lockfile per ecosystem, one
  CI pipeline, trivial code sharing, version alignment via workspace
  inheritance.
- Negative: requires discipline to keep layer boundaries; build graph
  grows within a single workspace.
- Alternatives considered: polyrepo (rejected: coordination overhead for a
  small team); single crate with modules (rejected: no compile-time
  enforcement of boundaries).

## Tauri host layout note

The Tauri host crate lives at `crates/openconkit-desktop` (not the
conventional `src-tauri` inside the frontend app). Its `tauri.conf.json`
points `build.frontendDist` to `../../apps/desktop-ui/dist` and
`build.devUrl` to the Vite dev server (`http://localhost:1420`). The Tauri
CLI (2.11) discovers the app directory by walking up from the current
directory looking for `tauri.conf.json` (or `src-tauri/tauri.conf.json`), so
root scripts invoke it from the crate directory
(`pnpm tauri ...` runs `cd crates/openconkit-desktop && tauri ...`). This
keeps all Rust code under `crates/` while the frontend stays under `apps/`.
