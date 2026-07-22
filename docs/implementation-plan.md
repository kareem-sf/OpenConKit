# OpenConKit v0.0.1 — Implementation Plan

Master prompt: `OpenConKit_Codex_Master_Prompt.md` (binding requirements).

## Environment discovery (2026-07-23)

- OS: Windows 11, Git Bash shell
- Node 26.4.0, npm 12.0.1, pnpm 11.16.0 (installed via npm)
- Rust 1.97.0 stable (`stable-x86_64-pc-windows-msvc` default); MSVC Build Tools 2022 (14.44.35207) + Windows SDK 10.0.26100 present
- WebView2 Runtime 150.0.4078.83 present
- git 2.55.0, gh 2.96.0 authenticated as `kareem-sf` (scopes: repo, workflow)
- Codex CLI 0.144.6 installed globally (reference only)
- No Python available → all repo scripts in Node.js

## Selected versions (checked against registries 2026-07-23)

| Component         | Version                           | Notes                                |
| ----------------- | --------------------------------- | ------------------------------------ |
| Tauri             | 2.11.5 (`@tauri-apps/cli` 2.11.4) | Tauri 2 current stable               |
| React             | 19.2.8                            |                                      |
| Vite              | 8.1.5                             |                                      |
| TypeScript        | current stable                    | strict mode                          |
| Tailwind CSS      | 4.3.3                             | v4 CSS-first config                  |
| Rust              | 1.97.0                            | stable                               |
| calamine          | 0.36.0                            | XLS + XLSX ingestion                 |
| rusqlite          | 0.40.1                            | bundled SQLite                       |
| rust_xlsxwriter   | 0.96.0                            | Excel reports                        |
| ts-rs             | 12.0.1                            | Rust→TS bindings                     |
| typst / typst-pdf | 0.15.1                            | embedded PDF with Arabic shaping     |
| Codex             | 0.145.0 (`rust-v0.145.0`)         | pinned in `tools/codex-version.json` |

## Phase checklist

- [x] Phase 1 — Discovery (environment, versions, risks)
- [ ] Phase 2 — Repository foundation (workspaces, lint, version sync, docs, identity, ADRs)
- [ ] Phase 3 — Core architecture (app home, config, SQLite, tool SDK/registry, contracts)
- [ ] Phase 4 — BOQ Inspector engine (ingestion, detection, checks, fixtures, benchmarks)
- [ ] Phase 5 — Reporting (Excel + PDF, AR/EN)
- [ ] Phase 6 — Desktop UX (full UI, i18n/RTL, theme, accessibility)
- [ ] Phase 7 — Codex integration (sidecar staging, stdio client, grounded AI)
- [ ] Phase 8 — Updates & packaging (channels, updater, Windows artifacts, CI)
- [ ] Phase 9 — Verification & hardening (reviews, full checks, smoke tests)
- [ ] Phase 10 — GitHub & release readiness (repo, CI green, v0.0.1 draft release)

## Risks

- Codex sidecar binaries are large; staged at build time via script, never committed.
- TypeScript 7.x is current stable; ESLint/tooling compatibility must be verified at foundation time (fall back to a documented supported version with ADR if needed).
- macOS/Linux artifacts are built only on native GitHub Actions runners, never claimed as locally built.
