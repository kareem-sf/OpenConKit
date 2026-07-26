# Threat Model

Scope: the OpenConKit desktop application (Tauri 2 shell, Rust backend,
webview frontend), its optional Codex sidecar, the updater, and the build/
release pipeline. Assets at risk: user documents (BOQs, estimates), the app
home directory (`~/.openconkit`), credentials present in the environment,
host integrity, and release integrity.

Security posture: local-first, no telemetry, least-privilege IPC, read-only
ingestion. This file is maintained alongside the code: change the surface,
update the model.

## T1. Malicious spreadsheets (parser exploits)

**Threat.** A crafted XLS/XLSX exploits a parser bug or triggers excessive
CPU/memory use when opened.

**Mitigations.**

- Ingestion uses calamine (pure Rust, memory-safe); `#![forbid(unsafe_code)]`
  in `openconkit-spreadsheet`.
- Files are opened read-only; parsing happens in the app process with no
  elevated privileges.
- Source import rejects non-regular files, non-XLS/XLSX extensions, and files
  over the tool-declared 64 MiB limit before copying; the streaming copy
  rechecks the limit so a concurrently growing file cannot bypass it.
- The spreadsheet adapter enforces sheet, row, column, retained-cell,
  merged-region, formula, cell-text, total-text, archive-entry, and parser
  work limits before or while retaining workbook content.
- Parsing checks cooperative cancellation at bounded intervals. The BOQ
  engine, schemas, providers, route, tests, translations, and documentation
  are enforced by `pnpm tool:completeness`.

## T2. Zip bombs (XLSX is a zip)

**Threat.** A small XLSX expands to gigabytes in memory or on disk.

**Mitigations.**

- Never extract XLSX archives to disk.
- The outer source file is bounded at import. Before XLSX parsing, the
  spreadsheet adapter inspects ZIP metadata and enforces entry-count,
  per-entry uncompressed-size, total-uncompressed-size, and compression-ratio
  caps with checked arithmetic.
- Temporary artifacts are confined to the app home temp dir and cleaned up.

## T3. Formula injection in exports

**Threat.** Cell text starting with `=`, `+`, `-`, `@` (or DDE payloads)
written into exported XLSX executes when the user opens the report in Excel.

**Mitigations.**

- The BOQ XLSX exporter writes document data with string APIs, never as
  workbook formulas, and neutralizes formula-trigger prefixes before writing.
- Tests reopen the generated workbook and prove formula-like finding text is
  stored as inert text.
- CSV export (if added) applies the same rule.

## T4. Path traversal

**Threat.** Malicious file names, deep-link arguments, or config values
cause reads/writes outside intended directories.

**Mitigations.**

- All app-managed paths are built from the canonical app home
  (`openconkit_home()`); user-supplied segments are validated and joined
  with `Path::join`, never string concatenation.
- The static main window is disabled. After app-home bootstrap, the desktop
  host creates it with an absolute `<app-home>/cache/webview` data directory
  and a non-persistent browser session. This prevents WebView2's default
  executable-adjacent profile and keeps webview cache/state inside the
  canonical root.
- `OPENCONKIT_HOME` override is honored for dev/test only and documented as
  such.
- No archive extraction to disk (see T2), so archive-entry traversal does
  not apply; any future extraction must normalize and confine entries.
- The WebView has no filesystem plugin permission. It has only
  `dialog:allow-open` for an explicit workbook picker; the selected path is
  passed to Rust's bounded immutable-import command.
- Report reveal accepts persisted run/export ids, not an arbitrary path.
  Rust resolves the record below the project's managed exports directory,
  rejects links and escapes, verifies the recorded SHA-256, and only then
  starts the platform file manager with an argument list (never a shell).

## T5. Codex sidecar command execution

**Threat.** The bundled Codex app server can execute shell commands; a
prompt-injected or malicious instruction could act on the user's machine.

**Mitigations.**

- The sidecar is spawned only on explicit user action, with an explicit
  argument list (no shell interpolation; see `CodexClientConfig`).
- Approval-first design: AI proposes, the user applies. No silent writes to
  documents or settings.
- The Codex version, release tag, archive sizes, SHA-256 digests, license,
  notice, and v2 protocol schema are pinned in `tools/codex-version.json`.
  `scripts/fetch-codex.mjs` downloads only official OpenAI release/source
  origins, rejects unsafe archive layouts, verifies exact sizes and hashes,
  stages atomically, and executes a native staged binary to verify its exact
  version.
- Working directory for the sidecar is confined to the app home; sandboxing
  flags of the app server are enabled by default.
- The app is fully functional with the sidecar absent or disabled.

## T6. Credential leakage

**Threat.** API keys (e.g. for AI providers) leak via logs, crash dumps,
config files, or IPC responses.

**Mitigations.**

- OpenConKit never stores provider credentials; they live in the provider's
  own CLI config or environment.
- Logging policy (AGENTS.md): no cell contents, prompts, responses, stderr
  text, environment values, credentials, or user paths. Optional Codex
  diagnostics record only bounded routing metadata.
- The webview IPC surface exposes no command that returns environment
  variables or file contents outside the app home.

## T7. Updater compromise

**Threat.** A compromised update server or signing key pushes a malicious
update.

**Mitigations.**

- Tauri updater verification is mandatory and uses the embedded public key.
  The private key is stored only in restricted local recovery storage and
  GitHub Actions encrypted secrets.
- Rust selects one of two compiled-in HTTPS feed URLs on the project-owned
  `updates` branch. A user-writable feed URL is not supported; the WebView has
  no updater plugin permission and cannot choose a network destination.
- Stable feeds reject SemVer prereleases. Installation rechecks the selected
  channel and exact version immediately before downloading.
- Tauri verifies the downloaded package signature before Rust launches the
  installer. Feed metadata is length/type bounded at IPC, and manual browser
  URLs are derived from a validated version on the project GitHub domain,
  never accepted from the feed.
- The release workflow keeps releases in draft until all native builds and a
  merged cross-platform `latest.json` pass validation. It publishes beta
  releases only to the beta feed; a stable release advances both feeds.
- Portable packages contain a marker beside the executable. They may check
  and notify, but Rust refuses in-place replacement and opens only the
  allowlisted release page.
- Database schema is never auto-downgraded; rollback means reinstalling an
  older binary only when its supported schema is compatible.

## T8. Deep links

**Threat.** A registered `openconkit://` URL handler passes attacker-
controlled input into the app.

**Mitigations.**

- No custom URL scheme is registered in v0.0.1. If added later: strict
  scheme/action allow-list, arguments treated as untrusted (validated,
  length-capped, never executed), and no navigation to external URLs from
  deep-link input.
- `open-url` calls from the app use an allow-list of project domains.

## T9. Tauri IPC exposure

**Threat.** XSS or compromised web content in the webview invokes privileged
commands, or a command accepts malicious payloads.

**Mitigations.**

- Strict CSP in `tauri.conf.json` (`default-src 'self'`, no remote scripts,
  no `object-src`, `frame-ancestors 'none'`).
- Capabilities are minimal (`core:default` and `dialog:allow-open`); every
  additional permission is reviewed against this file. No frontend
  filesystem or shell permission is granted.
- Commands take typed parameters deserialized via serde; zod schemas in
  `@openconkit/contracts` validate on the frontend boundary.
- Workbook ingestion exposes one project-free quick-import command. The
  obsolete project registration, archive, and project-targeted import
  commands are not part of the WebView IPC surface.
- The destructive reset command requires an exact confirmation value, refuses
  to run during analyses or updates, writes only a fixed marker inside the
  resolved app home, and restarts before storage deletes data. Startup rejects
  reset targets that are roots, relative paths, symlinks, or non-directories.
- IPC failures serialize only stable localizable error codes; backend
  diagnostics and absolute paths do not cross into the WebView.
- No `eval`, no remote module loading, no `dangerousDisableAssetCspModification`.

## T10. Log leakage

**Threat.** Logs capture sensitive content (cell values, paths, keys) and
are later shared by the user or read by other processes.

**Mitigations.**

- The canonical local log directory is `~/.openconkit/logs`. Diagnostic
  logging is off by default and takes effect after restart.
- The Codex protocol logger stores only timestamp, direction, bounded
  allowlisted method name, numeric request id, envelope kind/status, and byte
  count. It never writes raw JSON, params, results, stderr text, workbook
  values, prompts, model responses, paths, or credentials.
- Logs rotate at 2 MiB with three retained files. Log and rotation targets
  reject symlinks and non-regular files; Unix files are mode `0600`.
- Synthetic JSONL contract fixtures contain no captured user/provider data.
- Support flows instruct users what logs contain before sharing
  (`SUPPORT.md`).

## T11. Migration / database corruption

**Threat.** A failed migration or crash leaves
`data/openconkit.sqlite3` corrupt;
malicious DB content triggers downgrade or injection.

**Mitigations.**

- Every migration runs in its own transaction; failures roll back cleanly.
- Migrations are append-only and embedded; the runner refuses databases
  newer than the build (`SchemaTooNew`).
- All queries use bound parameters (rusqlite), never string-built SQL with
  user input.
- Before applying pending migrations to an existing database, SQLite's
  online backup API writes a unique backup under `~/.openconkit/backups/`
  and verifies it with `quick_check`; existing backups are never overwritten.
- Bootstrap uses a first-launch interrupt marker, and settings/config writes
  are atomic with corrupt-file backup and field-level recovery.
- Completion of the first-run privacy welcome is stored atomically in
  canonical settings. Merely creating the app-home directory does not skip
  the acknowledgement after an interrupted first launch.

## T12. Supply chain

**Threat.** A compromised npm/crates dependency or typosquat ships malware
to contributors or users.

**Mitigations.**

- Lockfiles committed (`pnpm-lock.yaml`, `Cargo.lock`); CI installs are
  frozen (`pnpm install --frozen-lockfile`, `cargo build --locked`).
- Dependabot for npm, cargo and github-actions with weekly cadence; updates
  are reviewed, not auto-merged.
- Version pins for critical components (Tauri, Codex release) with checksum
  verification for fetched binaries.
- `cargo-deny` (advisories, licenses, bans, sources), `pnpm audit`, a
  production-dependency license review, and a high-confidence secret scan
  run in CI.
- GitHub Actions are pinned to commit SHAs with least-privilege workflow
  permissions; CodeQL scans Rust and JavaScript/TypeScript.
- Sidecar and icon binaries are generated or fetched at build time, never
  committed (`.gitignore`: `**/binaries/`).

## Out of scope (accepted)

- Physical access to an unlocked machine.
- Malware already present on the host OS.
- The security of third-party model providers when the user opts into AI
  features (covered by user choice + provider terms).
