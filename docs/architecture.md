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
6. **tool-sdk** is depended on by tools and the host; it depends only on
   serde/ts-rs. Its contract is versioned (`TOOL_CONTRACT_VERSION`).

## Crate responsibilities

| Crate                           | Responsibility                                                                  |
| ------------------------------- | ------------------------------------------------------------------------------- |
| `openconkit-domain`             | Entities (e.g. `Project`), value objects (`ProjectId`), typed domain errors.    |
| `openconkit-application`        | Use cases (e.g. `RegisterProject`), orchestration, ports (`ProjectRepository`). |
| `openconkit-tool-sdk`           | `Tool` trait, `ToolDescriptor`, `ToolRegistry`, contract version.               |
| `openconkit-spreadsheet`        | Read-only XLS/XLSX ingestion via calamine. Never writes source files.           |
| `openconkit-storage`            | SQLite (bundled rusqlite), embedded append-only migrations, repositories.       |
| `openconkit-reporting`          | XLSX export via rust_xlsxwriter; PDF via Typst behind the `pdf` feature.        |
| `openconkit-ai-codex`           | Optional Codex app-server sidecar: version pin, sidecar layout, stdio client.   |
| `openconkit-tool-boq-inspector` | The BOQ Inspector tool (detection engine lands in its phase).                   |
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

## Data locations

- App home: `%USERPROFILE%\.openconkit` (Windows) / `$HOME/.openconkit`
  (elsewhere); `OPENCONKIT_HOME` overrides for dev/test.
- Database: `<app-home>/openconkit.db`.
- Source workbooks: wherever the user keeps them - opened read-only.

## Timestamp policy

All timestamps use `jiff` (ADR 0007); persisted as RFC 9557 / ISO 8601
strings in UTC.
