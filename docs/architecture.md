# Architecture

OpenConKit is a modular monolith: one desktop application, one Cargo
workspace, one pnpm workspace, with strictly layered internal crates.
See ADR 0002 for the rationale.

## Layers and dependency rules

```
                 +-------------------------------+
                 |  crates/openconkit-desktop     |  Tauri host: IPC commands,
                 |  (composition root)            |  capabilities, window, CSP
                 +---------------+---------------+
                                 |
        +------------------------+------------------------+
        |                        |                        |
+-------v--------+      +--------v---------+      +-------v--------+
| tool crates    |      | infrastructure   |      | openconkit-    |
| (openconkit-   |      | (adapters)       |      | application    |
|  tool-*)       |      |                  |      | (use cases,    |
+-------+--------+      | - storage        |      |  ports/traits) |
        |               | - spreadsheet    |      +-------+--------+
        |               | - reporting      |              |
        |               | - ai-codex       |      +-------v--------+
        |               +--------+---------+      | openconkit-    |
        |                        |                | domain (pure)  |
        |                        +--------------->+                |
        +---------------------------------------->+----------------+
                                 |
                        +--------v---------+
                        | openconkit-      |
                        | tool-sdk (stable |
                        | contract)        |
                        +------------------+
```

Rules:

1. **domain** depends on nothing internal. No IO, no async, no infra types.
2. **application** depends only on domain. It declares ports (traits);
   infrastructure crates implement them.
3. **infrastructure** crates (storage, spreadsheet, reporting, ai-codex)
   depend on domain/application as needed, never on tools or the desktop host.
4. **tool crates** (`openconkit-tool-*`) depend on tool-sdk, application and
   infrastructure. They never depend on the desktop host or on each other.
5. **desktop** composes everything, owns the Tauri IPC surface and registers
   tools in the compile-time registry (ADR 0003).
6. **tool-sdk** is depended on by tools and the host; it depends on the
   pure domain model plus serialization/schema libraries, never on
   application or infrastructure. Its contract is versioned
   (`TOOL_CONTRACT_VERSION`).

The build-only `openconkit-contracts-export` binary may depend on compiled
tool crates solely to export their typed IPC DTOs with `ts-rs`. This is not a
runtime dependency edge and does not relax the one-way runtime layering
(ADR 0011).

## Crate responsibilities

| Crate                           | Responsibility                                                                  |
| ------------------------------- | ------------------------------------------------------------------------------- |
| `openconkit-domain`             | Entities (e.g. `Project`), value objects (`ProjectId`), typed domain errors.    |
| `openconkit-application`        | Use cases (e.g. `RegisterProject`), orchestration, ports (`ProjectRepository`). |
| `openconkit-tool-sdk`           | `Tool` trait, `ToolManifest`, `ToolRegistry`, engine/export/AI contracts.       |
| `openconkit-spreadsheet`        | Read-only XLS/XLSX ingestion via calamine. Never writes source files.           |
| `openconkit-storage`            | SQLite (bundled rusqlite), embedded append-only migrations, repositories.       |
| `openconkit-reporting`          | XLSX export via rust_xlsxwriter; PDF via Typst behind the `pdf` feature.        |
| `openconkit-ai-codex`           | Optional Codex app-server sidecar: version pin, sidecar layout, stdio client.   |
| `openconkit-tool-boq-inspector` | BOQ ingestion, table inference, deterministic checks, and report orchestration. |
| `openconkit-desktop`            | Tauri host: commands, capabilities, `tauri.conf.json`, bundling.                |

## Frontend

`apps/desktop-ui` (React 19 + Vite + Tailwind v4) talks to the host only
through typed Tauri commands. Shared code lives in pnpm packages:

- `@openconkit/ui` - design tokens (`tokens.css`, light/dark schemes) and
  accessible primitives.
- `@openconkit/i18n` - i18next resources (en + ar), direction helpers, key
  parity tests.
- `@openconkit/contracts` - ts-rs generated bindings (committed,
  drift-checked) plus zod schemas for runtime validation.

The host exposes durable onboarding, project/source/run workflows, exact
run-detail reopening, aggregate history, deterministic exports, and
persisted-ID report reveal. The browser never receives arbitrary filesystem
access.

## Data locations

- App home: `%USERPROFILE%\.openconkit` (Windows) / `$HOME/.openconkit`
  (elsewhere); `OPENCONKIT_HOME` overrides only in debug/test builds and is
  rejected by release builds.
- Database: `<app-home>/data/openconkit.sqlite3`.
  Completed runs retain their exact serialized tool output alongside
  findings, exports, and optional AI records, so reopening or exporting never
  re-parses a changed source.
- Imported source revisions:
  `<app-home>/projects/<project-id>/sources/<hash-prefix>/...`. The user's
  original workbook is opened read-only for a bounded streaming copy and is
  never modified. The managed revision is made owner-readable only on Unix
  and read-only on Windows.
- Reports:
  `<app-home>/projects/<project-id>/exports/<run-id>/<export-id>/...`.
  Each report has a persisted relative path and SHA-256 digest. Reveal and
  reopen operations re-check confinement, file type, symlink status, and
  digest before use.
- Migration and corrupt-config backups: `<app-home>/backups/`.
- Native webview cache: `<app-home>/cache/webview/`. The desktop host
  creates the main window only after resolving app home and passes this
  absolute directory to Tauri. The window uses a non-persistent browser
  session. Windows WebView2 uses the bounded directory instead of creating
  executable-adjacent `.WebView2` data; platforms that do not support a
  custom directory retain no persistent browser session.

## Timestamp policy

All timestamps use `jiff` (ADR 0007); persisted as RFC 9557 / ISO 8601
strings in UTC.
