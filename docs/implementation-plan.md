# OpenConKit v0.0.1 — Implementation Plan

Master prompt: `OpenConKit_Codex_Master_Prompt.md` (binding requirements).

## Environment discovery (2026-07-23)

- OS: Windows 11; repository work is reproducible from PowerShell or Git Bash
- Node 26.4.0, npm 12.0.1, pnpm 11.16.0
- Rust 1.97.0 stable; MSVC Build Tools 2022 (14.44.35207) and Windows SDK
  10.0.26100 present
- WebView2 Runtime 150.0.4078.83 present
- git 2.55.0; GitHub CLI 2.96.0
- Codex CLI 0.144.6 installed globally (reference only)
- No Python required; repository automation is implemented in Node.js

## Selected versions (checked against registries 2026-07-23)

| Component         | Version                           | Notes                            |
| ----------------- | --------------------------------- | -------------------------------- |
| Tauri             | 2.11.5 (`@tauri-apps/cli` 2.11.4) | Tauri 2 current stable           |
| React             | 19.2.8                            |                                  |
| Vite              | 8.1.5                             |                                  |
| TypeScript        | 6.0.3                             | strict mode                      |
| Tailwind CSS      | 4.3.3                             | v4 CSS-first config              |
| Rust              | 1.97.0                            | pinned stable toolchain          |
| calamine          | 0.36.0                            | XLS + XLSX ingestion             |
| rusqlite          | 0.40.1                            | bundled SQLite + online backup   |
| rust_xlsxwriter   | 0.96.0                            | Excel reports                    |
| ts-rs             | 12.0.1                            | Rust-to-TypeScript bindings      |
| typst / typst-pdf | 0.15.1                            | embedded PDF with Arabic shaping |
| Codex             | 0.145.0 (`rust-v0.145.0`)         | pinned, verified and staged      |

## Phase checklist

- [x] Phase 1 — Discovery (environment, versions, risks)
- [x] Phase 2 — Repository foundation (workspaces, lint, version sync, docs,
      identity, ADRs)
- [x] Phase 3 — Core architecture (app home, config, SQLite, tool
      SDK/registry, contracts)
  - [x] Domain entities (project, source, run, finding, workbook, money, ids,
        AI, exports)
  - [x] Application ports and use cases (register/list/archive project,
        import source, quick import)
  - [x] Application config and bootstrap DTOs (`HomeLayout`, `AppSettings`,
        etc.)
  - [x] Tool SDK v1 contract (manifest, capabilities, engine,
        progress/cancel, export, AI, registry)
  - [x] ADRs 0008 (decimal money) and 0009 (engine
        progress/cancellation)
  - [x] `pnpm tool:new` scaffolder, tool-authoring docs, and
        architecture/completeness gates
  - [x] BOQ Inspector tool crate on the new contract
  - [x] Contracts pipeline (`openconkit-contracts-export`,
        `pnpm contracts:export`, `pnpm contracts:check`)
  - [x] SQLite schema and repositories (projects, sources, runs, findings,
        exports, AI)
  - [x] Filesystem source vault (hash, sanitized name, atomic bounded copy,
        read-only managed revision)
  - [x] App-home bootstrap, validation, interrupted-launch recovery, and
        owner-only permissions
  - [x] Settings persistence with atomic writes, field-level fallback,
        corruption backup, and update-state reconciliation
  - [x] Desktop composition root and BOQ Inspector registration
  - [x] Typed Tauri commands for bootstrap, settings, projects, and tool
        registry
  - [x] Single-instance startup ordered before database bootstrap/migration
  - [x] Consistent online database backup before pending migrations
  - [x] Domain deserialization invariants and relational aggregate validation
  - [x] Bounded, extension-gated, race-safe immutable source import
  - [x] Exact completed-run output persistence with atomic
        run/finding/output commits
  - [x] Reopenable run details with output, findings, exports, and AI records
  - [x] Read-optimized run history with source hash and aggregate status
  - [x] Privacy-safe structured IPC errors
- [x] Phase 4 — BOQ Inspector engine (ingestion, detection, checks, fixtures,
      benchmarks)
  - [x] Safety envelope: ZIP expansion, dimension, cell-count, and
        parser-work limits
  - [x] Serializable workbook model and diagnostics
  - [x] Multi-table structure, header, role, and row inference
  - [x] Locale-aware numeric, unit, and currency normalization
  - [x] Required deterministic checks with confidence and exact evidence
  - [x] Synthetic fixture generator, expected-findings harness, and
        adversarial fixtures
  - [x] Criterion benchmark harness plus cancellation/progress integration
  - [x] Record the native Windows benchmark baseline
  - [ ] Record the native Ubuntu benchmark baseline
- [x] Phase 5 — Reporting (Excel + PDF, Arabic + English)
  - [x] Deterministic XLSX and Typst PDF providers
  - [x] Independent English and Arabic exports with RTL PDF layout
  - [x] Unique, persisted, hash-verified reports regenerated from stored
        run output
  - [x] Generated bundled-font notices and Unicode English/Arabic PDF
        extraction validation
- [x] Phase 6 — Desktop UX (full workflow, i18n/RTL, theme, accessibility)
  - [x] Durable privacy/onboarding gate
  - [x] Home, Projects, import, run/progress/cancel, Results, evidence,
        Pareto, exports, History, Settings, and About workflows
  - [x] Native source picker without frontend filesystem permission
  - [x] Safe persisted-ID report reveal with confinement and hash checks
  - [x] English/Arabic parity, RTL, theme tokens, semantic controls, and
        reduced-motion support
  - [x] Rendered desktop/minimum-width/RTL QA and official Tauri browser-mode
        E2E coverage
  - [x] Codex account/AI and updater controls from Phases 7 and 8
- [x] Phase 7 — Codex integration (sidecar staging, stdio client, grounded AI)
- [ ] Phase 8 — Updates and packaging (channels, updater, native release
      artifacts)
  - [x] Baseline CI for frontend, Rust, supply chain, and CodeQL
  - [x] Native Windows/macOS/Linux packaging workflows
  - [x] Local Windows NSIS and portable package launch/uninstall smoke tests,
        enforced again on the Windows release runner
  - [x] Updater signing, SBOM, provenance, checksums, and generated
        third-party notices
- [ ] Phase 9 — Verification and hardening (reviews, full checks, smoke tests)
  - [x] Phase 3 full-gate snapshot: formatting, lint, strict TypeScript,
        frontend tests/build, 174 Rust tests with all features, Clippy
        `-D warnings`, contract drift, desktop compile, npm/Rust advisories,
        licenses, sources, bans, version parity, and secret scan
  - [x] 2026-07-24 implementation gate: strict Clippy; 33 application,
        44 storage, and 7 desktop Rust tests; 5 UI, 7 contract, and 5 i18n
        tests; strict TypeScript; ESLint; production build; contract drift;
        tool completeness; production-fixture pruning; and gradient
        prohibition
  - [x] BOQ correctness, security-envelope, fixture, cancellation, and
        performance-harness review
  - [x] Rendered accessibility, RTL, minimum-width, and source-hash checks
  - [x] Canonical WebView2 data-root regression test: installed and portable
        builds keep runtime data under `<app-home>/cache/webview` and create
        no executable-adjacent profile
  - [x] 2026-07-24 release-candidate gate: 252 Rust tests, 26 frontend tests,
        strict workspace Clippy and TypeScript, lint and formatting, generated
        contract drift, dependency audit, license and notice generation,
        secret scan, source-map/development-fixture bundle rejection, optimized
        Windows build, and NSIS/portable install-launch-uninstall smoke tests
  - [ ] Clean-machine, native E2E, offline-package, and cross-platform smoke
        tests
- [ ] Phase 10 — GitHub and release readiness (CI green, v0.0.1 draft
      release)

## Active risks and release blockers

- Codex sidecar binaries are large. The fetcher verifies exact upstream
  archive checksums and stages them at build time; binaries remain ignored.
- `citationberg` is temporarily revision-pinned to upstream security commit
  `06a591e2` so Typst resolves `quick-xml` 0.41.0. Remove the pin after a
  fixed crates.io release.
- Windows Application Control on the current workstation blocks the official
  checksum-verified `cargo-deny` 0.20.2 executable. The pinned
  `EmbarkStudios/cargo-deny-action` CI gate is authoritative for advisories,
  licenses, bans, and sources.
- The same workstation policy removes freshly downloaded ChromeDriver
  executables. The official Tauri browser-mode E2E suite is committed and
  fails closed locally; its Ubuntu CI execution remains a release gate.
- The all-feature Rust graph is large. CI caching and job separation should
  improve turnaround without weakening the release-equivalent gate.
- macOS and Linux artifacts are built only on native GitHub Actions runners;
  they must never be claimed as locally built on Windows.
- v0.0.1 has mandatory Tauri updater signatures but no paid Windows publisher
  certificate or Apple Developer ID. SmartScreen/Gatekeeper warnings are
  documented and must not be described as signed/notarized OS trust.
