# Third-Party Notices

OpenConKit bundles open-source components from the Rust and npm ecosystems.
This file will contain the full attribution list for distributed artifacts.

## Generation plan

The notice file is generated during the release phase from the resolved
dependency graphs, and verified in CI:

1. Rust: `cargo about` (configuration in `about.toml`, added in the release
   phase) renders crate names, versions and license texts for everything in
   `Cargo.lock` that ships in the desktop bundle.
2. npm: `pnpm licenses list --json` (or `license-checker-rseidelsohn`) covers
   frontend dependencies bundled into the webview assets.
3. The Codex app-server sidecar (Apache-2.0/MIT, fetched from the pinned
   OpenAI release in `tools/codex-version.json`) is attributed separately
   with its version and checksum.

Until generation is wired, this skeleton tracks the intent and the manual
spot-check list below.

## Spot-check list (foundation phase)

- Tauri (Apache-2.0 OR MIT) - application framework.
- React (MIT) - UI library.
- calamine (MIT) - spreadsheet ingestion.
- rusqlite / libsqlite3 (MIT / public domain) - embedded database.
- rust_xlsxwriter (MIT) - Excel report writer.
- Typst (Apache-2.0) - PDF report engine (optional feature).
- ts-rs (MIT) - TypeScript binding generation.
