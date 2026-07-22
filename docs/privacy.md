# Privacy

OpenConKit is **local-first**. This document states the privacy guarantees
the project commits to; `AGENTS.md` makes them binding engineering rules.

## Guarantees

1. **No telemetry.** No analytics, no usage metrics, no crash-reporting
   beacons, no "phone home". There is no telemetry code to disable - it does
   not exist.
2. **Your documents stay on your machine.** Workbooks are opened read-only
   and are never modified, uploaded, or synced. Reports are written as new
   files where you choose.
3. **All app data lives locally** in `~/.openconkit`
   (`%USERPROFILE%\.openconkit` on Windows): the SQLite database, settings,
   and logs. Uninstalling leaves this folder behind; delete it to remove all
   traces.
4. **No accounts, no activation, no license checks.**

## Network access

The application makes network connections only in these cases:

- **Updater** (when enabled at release time): checks the project's release
  server for new versions. No document content or usage data is sent; the
  check contains only the current version and target platform.
- **AI features (optional)**: when you explicitly invoke an AI feature, the
  locally-running Codex app-server sidecar may contact the configured model
  endpoint using your own credentials. Only the extracted facts shown in the
  AI panel are sent - never whole workbooks unless you attach them yourself.

Everything else works with the network cable unplugged.

## AI features

- The Codex sidecar runs as a local subprocess, spawned only when an AI
  feature is used.
- AI output is advisory: it is displayed as suggestions and never mutates
  your data silently.
- Credentials for AI providers are read from the environment or the provider's
  own CLI configuration; OpenConKit never logs or transmits them.

## Logs

Diagnostic logs (when enabled) stay under `~/.openconkit/logs` and must not
contain workbook cell contents or credentials (see `docs/threat-model.md`,
"log leakage").
