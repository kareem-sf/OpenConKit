# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1] - 2026-07-24

### Added

- Local-first Tauri desktop workspace with durable first-run setup, projects,
  immutable source revisions, run history, settings, exports, and app-home
  recovery under `~/.openconkit`.
- BOQ Inspector for bounded XLS/XLSX ingestion, confidence-aware structure
  detection, bilingual normalization, deterministic quality checks, exact
  sheet/cell evidence, Pareto analysis, progress, and cancellation.
- Reproducible English and Arabic XLSX/PDF reports generated only as new
  files, with embedded fonts, RTL layout, hashes, persisted export history,
  and Unicode extraction tests.
- Complete English/Arabic desktop workflow with RTL, light/dark/system themes,
  keyboard-friendly semantic controls, reduced-motion handling, and a
  minimum-width layout.
- Optional grounded Codex app-server integration with isolated profile,
  browser/device login, account and rate-limit status, bounded fact chunks,
  strict structured output validation, cancellation, and metadata-only
  diagnostics.
- Stable/beta update channels, signature-verified installer updates,
  portable-build manual update handling, progress UI, and fixed project-owned
  feeds.
- Native Windows, Linux, and universal macOS release automation with pinned
  Codex sidecars, updater signatures, checksums, SPDX SBOM, provenance,
  portable Windows packaging, and generated third-party legal notices.
- Compile-time tool SDK and registry, contributor tool scaffolder, generated
  Rust-to-TypeScript contracts, migration-backed repositories, and typed IPC
  errors.
- Synthetic BOQ fixture generator and expected-finding contracts, adversarial
  workbooks, source-immutability tests, Criterion benchmark harness, component
  tests, official Tauri browser-mode E2E flows, Actionlint, CodeQL, dependency
  policy, license, secret, and architecture gates.
- Original OpenConKit identity, icons, bilingual product copy, threat model,
  privacy and release documentation, ADRs, and community health files.

### Security

- No telemetry or unrestricted renderer networking; all privileged filesystem,
  process, updater, and AI operations remain behind narrow Rust-owned
  commands.
- Source imports are extension-gated, size-bounded, hash-verified, atomically
  copied into managed storage, and tested never to modify the original.
- AI receives only explicitly approved normalized facts and cannot silently
  apply suggestions or invent uncited BOQ values.
- The native webview uses a private session with its Windows data directory
  confined to `~/.openconkit/cache/webview`; installed and portable package
  smoke tests reject executable-adjacent profiles.
- Production desktop bundles contain no source maps, E2E controls, or
  development fixture loader text.

[Unreleased]: https://github.com/kareem-sf/OpenConKit/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/kareem-sf/OpenConKit/releases/tag/v0.0.1
