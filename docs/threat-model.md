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
- Detection-engine phase adds size guards: reject files above a configured
  size, cap sheet/cell counts, and time-box ingestion with progress
  cancellation.
- Fixture corpus includes adversarial files (deeply nested shared strings,
  huge dimensions) generated from `fixtures/source-specs/`.

## T2. Zip bombs (XLSX is a zip)

**Threat.** A small XLSX expands to gigabytes in memory or on disk.

**Mitigations.**

- Never extract XLSX archives to disk; calamine streams entries.
- Decompression ratio and total-size caps enforced at ingestion (detection
  phase); rejection is reported, not fatal.
- Temporary artifacts are confined to the app home temp dir and cleaned up.

## T3. Formula injection in exports

**Threat.** Cell text starting with `=`, `+`, `-`, `@` (or DDE payloads)
written into exported XLSX executes when the user opens the report in Excel.

**Mitigations.**

- `openconkit-reporting` writes user-derived values with
  `write_string`/typed writers only - never `write_formula` for document
  data.
- A sanitizer prefixes a leading quote to any string beginning with a
  formula trigger character (implemented with the first real exporter and
  covered by unit tests).
- CSV export (if added) applies the same rule.

## T4. Path traversal

**Threat.** Malicious file names, deep-link arguments, or config values
cause reads/writes outside intended directories.

**Mitigations.**

- All app-managed paths are built from the canonical app home
  (`openconkit_home()`); user-supplied segments are validated and joined
  with `Path::join`, never string concatenation.
- `OPENCONKIT_HOME` override is honored for dev/test only and documented as
  such.
- No archive extraction to disk (see T2), so archive-entry traversal does
  not apply; any future extraction must normalize and confine entries.
- Tauri FS/dialog capabilities are not enabled by default
  (`capabilities/default.json` grants `core:default` only).

## T5. Codex sidecar command execution

**Threat.** The bundled Codex app server can execute shell commands; a
prompt-injected or malicious instruction could act on the user's machine.

**Mitigations.**

- The sidecar is spawned only on explicit user action, with an explicit
  argument list (no shell interpolation; see `CodexClientConfig`).
- Approval-first design: AI proposes, the user applies. No silent writes to
  documents or settings.
- Sidecar binary is checksum-verified against `tools/codex-version.json`
  before staging.
- Working directory for the sidecar is confined to the app home; sandboxing
  flags of the app server are enabled by default.
- The app is fully functional with the sidecar absent or disabled.

## T6. Credential leakage

**Threat.** API keys (e.g. for AI providers) leak via logs, crash dumps,
config files, or IPC responses.

**Mitigations.**

- OpenConKit never stores provider credentials; they live in the provider's
  own CLI config or environment.
- Logging policy (AGENTS.md): no cell contents, no env vars, no paths
  containing usernames beyond the app home root.
- The webview IPC surface exposes no command that returns environment
  variables or file contents outside the app home.

## T7. Updater compromise

**Threat.** A compromised update server or signing key pushes a malicious
update.

**Mitigations.**

- Tauri updater with minisign public-key signature verification embedded in
  the app; private key held in CI secrets with restricted access.
- Updates delivered over HTTPS from the project's own release endpoints
  only; update manifests are part of release artifacts.
- Releases are reproducible from tagged commits; checksums published on the
  Releases page.
- Rollback path documented in `docs/releasing.md`; database schema is never
  auto-downgraded.

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
- Capabilities are minimal (`core:default` only); every additional
  permission is reviewed against this file.
- Commands take typed parameters deserialized via serde; zod schemas in
  `@openconkit/contracts` validate on the frontend boundary.
- No `eval`, no remote module loading, no `dangerousDisableAssetCspModification`.

## T10. Log leakage

**Threat.** Logs capture sensitive content (cell values, paths, keys) and
are later shared by the user or read by other processes.

**Mitigations.**

- Logs are local-only under `~/.openconkit/logs`, size-rotated.
- Content policy: log event metadata (rule ids, counts, durations), never
  cell text or credentials.
- Support flows instruct users what logs contain before sharing
  (`SUPPORT.md`).

## T11. Migration / database corruption

**Threat.** A failed migration or crash leaves `openconkit.db` corrupt;
malicious DB content triggers downgrade or injection.

**Mitigations.**

- Every migration runs in its own transaction; failures roll back cleanly.
- Migrations are append-only and embedded; the runner refuses databases
  newer than the build (`SchemaTooNew`).
- All queries use bound parameters (rusqlite), never string-built SQL with
  user input.
- Before applying migrations to an existing database, a backup copy is
  written next to the database (storage phase).

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
- `cargo audit` / `pnpm audit` run in CI (workflow phase).
- Sidecar and icon binaries are generated or fetched at build time, never
  committed (`.gitignore`: `**/binaries/`).

## Out of scope (accepted)

- Physical access to an unlocked machine.
- Malware already present on the host OS.
- The security of third-party model providers when the user opts into AI
  features (covered by user choice + provider terms).
