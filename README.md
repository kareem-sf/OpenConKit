# OpenConKit

**The open-source toolkit for construction professionals.**

OpenConKit is a local-first desktop application that hosts practical tools
for construction work - starting with **BOQ Inspector**, an automated Bill of
Quantities quality review. It runs on Windows, macOS and Linux, works fully
offline, and never sends your documents anywhere.

Built with Tauri 2, React 19 and Rust.

## v0.0.1

- **BOQ Inspector** (first tool): ingest XLS/XLSX bills of quantities, run
  automated quality checks, and export findings to Excel and PDF.
- **Bilingual UI**: English and Arabic, with full right-to-left support.
- **Local-first**: all data stays in `~/.openconkit` on your machine.
- **Optional AI**: an optional, bundled OpenAI Codex app-server sidecar can
  explain findings in plain language. The app is fully useful without it.

## Current release status

The local-first desktop workflow, deterministic BOQ engine, reports, durable
history, optional grounded Codex review, signed updater, and native release
pipeline are implemented. OpenConKit v0.0.1 was released on 2026-07-24 after
the complete branch CI matrix and native Windows, macOS, and Linux release
matrix passed. Validation includes the four-flow Tauri browser-mode renderer
suite on Ubuntu, universal-binary checks on macOS, and installed plus portable
Windows package smoke tests.

## Install

Download v0.0.1 from the
[GitHub Releases page](https://github.com/kareem-sf/OpenConKit/releases/tag/v0.0.1).

- Windows: NSIS installer (per-user, no admin required) or portable zip.
- macOS: universal DMG or zipped app.
- Linux: x64 AppImage or deb package.

The updater packages are cryptographically signed, but v0.0.1 does not use a
paid Windows publisher certificate or Apple Developer ID. Windows SmartScreen
or macOS Gatekeeper may therefore show an unknown-publisher warning. Verify
downloads against the release checksum manifest when needed.

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

| Command                                                 | Description                                             |
| ------------------------------------------------------- | ------------------------------------------------------- |
| `pnpm dev`                                              | Run the desktop app in dev mode (Tauri + Vite HMR)      |
| `pnpm build`                                            | Build all workspace packages (frontend)                 |
| `pnpm test`                                             | Run all TypeScript tests (Vitest)                       |
| `pnpm test:e2e`                                         | Run official Tauri browser-mode renderer E2E flows      |
| `cargo test --workspace`                                | Run all Rust tests                                      |
| `pnpm lint`                                             | ESLint over the repo                                    |
| `pnpm format` / `pnpm format:check`                     | Prettier write / check                                  |
| `pnpm typecheck`                                        | `tsc --noEmit` in every package                         |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lints                                              |
| `pnpm version:sync` / `pnpm version:check`              | Propagate / verify the `VERSION` file                   |
| `pnpm icons:generate`                                   | Regenerate Tauri icons from `branding/icon.svg`         |
| `pnpm tool:new <id>`                                    | Scaffold a compiled-in tool for contributors            |
| `pnpm tool:completeness`                                | Fail while any tool has release blockers                |
| `pnpm contracts:check`                                  | Verify committed Rust-to-TypeScript bindings            |
| `pnpm bundle:check`                                     | Reject production source maps and development controls  |
| `pnpm codex:fetch`                                      | Fetch and verify the pinned native Codex sidecar        |
| `pnpm portable:package`                                 | Assemble the Windows portable ZIP after a release build |
| `pnpm package:smoke:windows`                            | Install, launch, and remove both Windows package forms  |
| `pnpm fixtures:generate`                                | Regenerate synthetic BOQ fixture workbooks              |
| `pnpm notices:generate` / `pnpm notices:check`          | Generate / verify third-party legal notices             |
| `pnpm licenses:check` / `pnpm security:secrets`         | Supply-chain license and secret checks                  |
| `pnpm tauri <args>`                                     | Run the Tauri CLI against `crates/openconkit-desktop`   |

## Privacy

OpenConKit is local-first: **no telemetry, no analytics, and no first-party
account system**. Your workbooks are never uploaded and are never modified -
ingestion is read-only and reports are written as new files. When you
explicitly approve an optional AI review, the local Codex sidecar may send
only the displayed normalized facts to the selected AI provider using your
Codex/ChatGPT sign-in. Details: `docs/privacy.md`.

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
