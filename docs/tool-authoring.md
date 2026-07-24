# Tool authoring

How to add a new tool to the OpenConKit shell. A tool is a Rust crate
(`crates/openconkit-tool-<slug>`) implementing the tool contract from
`openconkit-tool-sdk`, plus a frontend route, i18n keys, docs, and a
compile-time registry entry. There is no runtime plugin loading
(ADR 0003).

## Prerequisites

- Node 26 + pnpm 11, Rust stable (see `docs/implementation-plan.md`).
- Read `AGENTS.md` (architecture layers, product invariants) and
  `docs/architecture.md`.
- Tools never modify source workbooks, never use the network, and stay
  fully useful offline. AI is optional and always a suggestion.

## Scaffolding: `pnpm tool:new <slug>`

`<slug>` is kebab-case (e.g. `takeoff-assistant`). The scaffolder generates:

- `crates/openconkit-tool-<slug>/` — a compiling crate: manifest,
  capabilities, permissions, typed DTOs, a pass-through `TypedToolEngine`
  behind `TypedEngineAdapter`, and real unit tests.
- `apps/desktop-ui/src/routes/<Slug>Page.tsx` — an accessible route stub
  rendering the tool name/description, registered in
  `apps/desktop-ui/src/App.tsx` at `/tools/<slug>`.
- i18n keys `tools.<camelSlug>.*` in `packages/i18n` locales (`en` real
  text, `ar` `TODO(ar):` placeholders; parity test stays green).
- `docs/tools/<slug>.md` — documentation stub (Overview, Rules, Fixtures,
  Exports, AI).
- Registry registration (see below).

Every placeholder carries a `SCAFFOLD:` marker. Refusals: the crate
directory already exists, or the slug is `sdk` (reserved).

Verify immediately:

```
cargo test -p openconkit-tool-<slug>
```

## The SDK contract

A tool implements `openconkit_tool_sdk::Tool`:

- **`manifest()`** — identity shown by the shell: kebab-case `id`,
  `contract_version` (must equal `TOOL_CONTRACT_VERSION`), `tool_version`
  (the crate's own semver), i18n `name_key`/`description_key`, `icon`,
  `route`.
- **`input_capabilities()`** — accepted extensions (lowercase, leading dot;
  `accepts()` matches case-insensitively), max file size, single/multiple
  files.
- **`permissions()`** — `reads_source_files`, `writes_exports`, `network`,
  `ai`. Declared up front, surfaced to the user; `network`/`ai` stay
  `false` unless the tool ships an explicitly user-invoked AI feature.
- **`engine()`** — the analysis boundary. Implement `TypedToolEngine` with
  concrete `Input`/`Settings`/`Output` types and wrap it in
  `TypedEngineAdapter`; the adapter (de)serializes `serde_json::Value` at
  the dyn-safe `ToolEngine` boundary and maps failures to
  `ToolError::InvalidInput` / `InvalidSettings` / `Engine`.
- **Progress and cancellation** — engines are synchronous and run on a
  background thread. Report progress through the `ProgressCallback` with
  `ToolProgress { phase_key, fraction, detail }` (`phase_key` is an i18n
  key; `detail` never contains cell contents). Check the
  `CancellationToken` at well-defined points and return
  `ToolError::Cancelled` promptly (ADR 0009).
- **Exports** — implement `ExportProvider` and return instances from
  `export_providers()` to produce report artifacts from a finished run's
  output (always new files, never in place).
- **AI capability** — `ai_capability()` returns `Some(AiCapability)` only
  for optional, user-invoked AI features grounded in extracted facts.
- **State migrations** — `state_migrations()` lists tool-owned state
  migrations (see below).
- **Test hooks** — `test_hooks()` exposes fixtures/harness points the
  shell's integration tests can drive.
- **Schemas** — `input_schema()` / `settings_schema()` / `output_schema()`
  may return JSON Schemas for UI generation and validation.

## Adding a rule

- Rule ids are **kebab-case**, stable, and unique within the tool
  (e.g. `missing-unit-price`). Every user-visible finding carries its rule
  id plus evidence cells.
- Each tool versions its rule set independently: `rule_set_version` is
  **semver per tool**, bumped on any rule addition, removal, or behavior
  change, and recorded in run provenance.
- Rules ship with fixtures (below) proving the expected findings.

## Tool-owned state migrations

Tool state lives in app home (`~/.openconkit`). When a tool's persisted
state shape changes, append a `ToolStateMigration` to
`state_migrations()` — append-only, one logical change per migration,
matching the storage discipline in ADR 0004. Never edit an already-shipped
migration.

## Registering in the compile-time registry

The desktop host composes tools at startup in
`crates/openconkit-desktop/src/registry.rs` at the
`// tool-new: register here` marker:

- If that module exists, `pnpm tool:new` inserts
  `registry.register(Box::new(openconkit_tool_<snake_slug>::<Slug>Tool::new()));`
  and adds the crate to the desktop `Cargo.toml` automatically.
- Until it exists, the scaffolder appends the registration snippet and the
  Cargo.toml dependency line to
  `crates/openconkit-desktop/TOOL-REGISTRATIONS.md`; move each entry into
  the composition module when it lands and delete the entry.

## i18n keys (en + ar)

All user-facing strings live in `packages/i18n/src/locales/{en,ar}/common.json`
under `tools.<camelSlug>`: `name`, `description`, and `progress.*` phase
keys. The parity test (`packages/i18n/test/parity.test.ts`) requires
identical key structure and non-empty values in both locales — replace the
`TODO(ar):` placeholders with real translations. RTL is first-class.

## Contracts

Rust↔TypeScript bindings are generated with ts-rs into
`packages/contracts` (ADR 0005):

- Regenerate after changing any contract type: `pnpm contracts:export`.
- CI drift-checks them: `pnpm contracts:check`. Never hand-edit generated
  bindings.

## Fixtures

Real BOQs are confidential, so tests run on synthetic workbooks: add a
human-readable spec to `fixtures/source-specs/` (columns, planted defects,
expected findings) — specs are the source of truth, workbooks are
generated into `fixtures/generated/`. See `fixtures/README.md`.

## Test tiers

- **Unit** — colocated `#[cfg(test)]` modules in the tool crate:
  `cargo test -p openconkit-tool-<slug>`. The scaffold ships manifest,
  capabilities, engine pass-through, and cancellation tests; extend them
  per rule.
- **Component** — React Testing Library in `apps/desktop-ui`:
  `pnpm test` (or `pnpm --filter @openconkit/desktop-ui test`).
- **Integration** — crate-level tests over real adapters (in-memory
  SQLite, generated fixture workbooks).
- **Fixtures** — planted-defect workbooks asserting expected findings.

## The SCAFFOLD marker and `pnpm tool:completeness`

`pnpm tool:new` marks every generated placeholder with `SCAFFOLD:`.
`pnpm tool:completeness` greps tool crates, route stubs, and
`docs/tools/*.md` for the marker and exits 1 listing every `file:line` —
scaffold sections must be completed before release. A freshly scaffolded
tool fails this gate **by design**, so it is intentionally not part of
`pnpm lint`; maintainers run it explicitly and CI enforces it at release
time.

## Quality gates checklist

Before a tool ships:

- [ ] `cargo test -p openconkit-tool-<slug>` green (unit + integration).
- [ ] `pnpm lint` green — includes `scripts/arch-check.mjs` (dependency
      rules, no cycles, no `tauri` outside the desktop host).
- [ ] `pnpm test` green, including the i18n parity test with real `ar`
      translations.
- [ ] `pnpm contracts:check` green after `pnpm contracts:export`.
- [ ] Fixture specs in `fixtures/source-specs/` cover every rule.
- [ ] `docs/tools/<slug>.md` filled in; permissions reviewed.
- [ ] `pnpm tool:completeness` exits 0 — no `SCAFFOLD:` markers left.
