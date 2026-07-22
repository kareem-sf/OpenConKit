# AGENTS.md - OpenConKit engineering rules

These rules bind every contributor, human or AI agent, working in this
repository. `CONTRIBUTING.md` covers process; this file covers engineering
invariants. Keep this file current when conventions change.

## Product invariants (never violate)

1. **Local-first, no telemetry.** No analytics, crash reporting beacons, or
   phone-home of any kind. Network access happens only for: the optional
   updater, and AI features the user explicitly invokes. See `docs/privacy.md`.
2. **Never modify source workbooks.** Spreadsheet ingestion is read-only,
   always. Reports are written as new files, never in place.
3. **App home is canonical.** All app data lives under `~/.openconkit`
   (`%USERPROFILE%\.openconkit` on Windows). `OPENCONKIT_HOME` overrides it
   for development and tests only.
4. **AI is optional.** The app must be fully useful offline without the
   Codex sidecar. AI output must be grounded in extracted facts and shown
   as suggestions, never silently applied to data.

## Layered architecture

Dependency direction is strictly one-way (see `docs/architecture.md`):

```
desktop (Tauri host) -> tool crates -> application -> domain
                                   \-> infrastructure (storage, spreadsheet, reporting, ai-codex)
tool-sdk <- tool crates, desktop
```

- `openconkit-domain`: pure entities/value objects/typed errors. No infra
  deps, no async, no IO.
- `openconkit-application`: use cases + ports (traits). Depends only on
  domain.
- Infrastructure crates implement ports; use cases never import them.
- Tool crates (`openconkit-tool-*`) depend on `openconkit-tool-sdk`,
  application and infrastructure crates - never on the desktop host.
- The desktop host composes everything and owns the IPC surface.

## Rust rules

- No `unwrap()`, `expect()`, or `panic!()` in production code paths. Tests
  (`#[cfg(test)]`) and build scripts may use them. Crates enforce this with
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`.
- `#![forbid(unsafe_code)]` in every crate.
- Errors are typed via `thiserror`; `anyhow` is allowed only at the outermost
  binary/application boundary.
- One timestamp library: `jiff` (ADR 0007). Do not add `chrono` or `time`.
- Database migrations are append-only, one transaction each (ADR 0004).
- Rust<->TypeScript contracts are generated with ts-rs and committed;
  CI drift-checks them (ADR 0005).

## Frontend / TypeScript rules

- TypeScript `strict` + `noUncheckedIndexedAccess` +
  `noFallthroughCasesInSwitch`. No `any` (ESLint error).
- All user-facing strings live in `packages/i18n` locale files (en + ar).
  The parity test must stay green. RTL is a first-class citizen.
- Styling via Tailwind v4 design tokens (`packages/ui/src/tokens.css`), not
  ad-hoc hex values. Contrast must meet WCAG AA.
- Version source of truth is the root `VERSION` file; use
  `pnpm version:sync` / `pnpm version:check`.

## Test tiers

1. **Unit** - pure logic, colocated `#[cfg(test)]` modules (Rust) or
   `*.test.ts(x)` (TS). Fast, no IO beyond temp dirs.
2. **Component** - React Testing Library + jsdom in `apps/desktop-ui` and
   shared packages.
3. **Integration** - crate-level tests over real adapters (in-memory SQLite,
   generated fixture workbooks).
4. **Fixtures** - synthetic BOQ workbooks generated from specs in
   `fixtures/source-specs/`; planted defects with expected findings.

Every crate/package ships at least one real test; no placeholder tests.

## Commits

- Conventional Commits: `type(scope): summary`, e.g.
  `fix(spreadsheet): reject zip bombs above decompression limit`.
- Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`,
  `chore`.
- Architecture changes require an ADR in `docs/adr/`.

## Security

Follow `docs/threat-model.md`. When touching ingestion, exports, IPC,
deep links, the updater, or the Codex sidecar, re-read the relevant threat
entries and update the model if the surface changes.
