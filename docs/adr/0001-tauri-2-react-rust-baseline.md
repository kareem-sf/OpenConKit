# 0001. Tauri 2 + React + Rust baseline

- Status: Accepted
- Date: 2026-07-23

## Context

OpenConKit is a desktop toolkit for construction professionals. Requirements:
small install footprint, local-first data handling, strong OS integration
(file dialogs, installers, updater), a polished bilingual (EN/AR) UI, and a
single language for performance-critical document processing (large
spreadsheets).

## Decision

- **Tauri 2.11** as the desktop shell (Rust host + system WebView).
- **React 19 + TypeScript + Vite 8 + Tailwind CSS 4** for the frontend.
- **Rust** for all document processing, storage and reporting, behind Tauri
  commands.

## Consequences

- Positive: small bundles (vs Electron), memory-safe processing, first-class
  Windows installer support (NSIS per-user), capability-based IPC security,
  one Rust workspace shared by host and engine crates.
- Negative: two toolchains (Node + Rust) for contributors; webview
  differences across OSes must be tested; WebView2 dependency on Windows
  (preinstalled on Windows 11).
- Alternatives considered: Electron (rejected: bundle size, memory, larger
  attack surface); native WinUI/Qt (rejected: cross-platform cost, smaller
  hiring/contributor pool).
