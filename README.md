# OpenConKit

**The open-source toolkit for construction professionals.**

OpenConKit is a local-first desktop application that hosts practical tools
for construction work - starting with **BOQ Inspector**, an automated Bill of
Quantities quality review. It runs on Windows, macOS and Linux, works fully
offline, and never sends your documents anywhere.

Built with Tauri 2, React 19 and Rust.

## Features

- **BOQ Inspector** (first tool): ingest XLS/XLSX bills of quantities, run
  automated quality checks, and export findings to Excel and PDF.
- **Bilingual UI**: English and Arabic, with full right-to-left support.
- **Local-first**: all data stays in `~/.openconkit` on your machine.
- **Optional AI**: an optional, bundled OpenAI Codex app-server sidecar can
  explain findings in plain language. The app is fully useful without it.

## Install

Pre-release: installers are published on the GitHub Releases page
(<https://github.com/kareem-sf/openconkit/releases>) once v0.0.1 ships.

- Windows: NSIS installer (per-user, no admin required) or portable zip.
- macOS / Linux: built on native CI runners at release time.

## Development

### Prerequisites

- **Node.js 26+**
- **pnpm 11.16** - via `corepack enable && corepack prepare pnpm@11.16.0 --activate`
  or `npm install -g pnpm@11.16.0`
- **Rust stable** (1.97+ recommended) via [rustup](https://rustup.rs)
- **Windows**: MSVC Build Tools 2022 (C++ workload) + WebView2 Runtime
  (preinstalled on Windows 11)

### Quickstart

```sh
pnpm install
pnpm build            # builds the frontend
cargo test --workspace
pnpm dev              # runs the Tauri app (vite dev server + shell)
```

### Commands

| Command                                                 | Description                                           |
| ------------------------------------------------------- | ----------------------------------------------------- |
| `pnpm dev`                                              | Run the desktop app in dev mode (Tauri + Vite HMR)    |
| `pnpm build`                                            | Build all workspace packages (frontend)               |
| `pnpm test`                                             | Run all TypeScript tests (Vitest)                     |
| `cargo test --workspace`                                | Run all Rust tests                                    |
| `pnpm lint`                                             | ESLint over the repo                                  |
| `pnpm format` / `pnpm format:check`                     | Prettier write / check                                |
| `pnpm typecheck`                                        | `tsc --noEmit` in every package                       |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lints                                            |
| `pnpm version:sync` / `pnpm version:check`              | Propagate / verify the `VERSION` file                 |
| `pnpm icons:generate`                                   | Regenerate Tauri icons from `branding/icon.svg`       |
| `pnpm tool:new <id>`                                    | Scaffold a new tool (implemented in phase 3)          |
| `pnpm tauri <args>`                                     | Run the Tauri CLI against `crates/openconkit-desktop` |

## Privacy

OpenConKit is local-first: **no telemetry, no analytics, no accounts**. Your
workbooks never leave your machine and are never modified - ingestion is
read-only and reports are written as new files. The optional AI sidecar runs
locally and is only invoked when you explicitly use an AI feature. Details:
`docs/privacy.md`.

## AI behavior

AI features (via the bundled Codex app server) are off unless you use them.
They operate on facts extracted from your documents, present suggestions for
review, and never change data silently. Without the sidecar, every non-AI
feature works offline.

## Contributing

See `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and the engineering rules in
`AGENTS.md`. Architecture decisions live in `docs/adr/`.

## License

[Apache-2.0](LICENSE) - Copyright OpenConKit contributors.
Third-party attributions: `THIRD_PARTY_NOTICES.md`.
