# Privacy

OpenConKit is **local-first**. This document states the privacy guarantees
the project commits to; `AGENTS.md` makes them binding engineering rules.

## Guarantees

1. **No telemetry.** No analytics, no usage metrics, no crash-reporting
   beacons, no "phone home". There is no telemetry code to disable - it does
   not exist.
2. **Your documents stay on your machine.** Workbooks are opened read-only
   and are never modified, uploaded, or synced. Reports are written as new,
   immutable artifacts in the project's managed exports directory and can be
   revealed in the operating-system file manager.
3. **All app data lives locally** in `~/.openconkit`
   (`%USERPROFILE%\.openconkit` on Windows): the SQLite database, settings,
   logs, and native webview cache. Uninstalling leaves this folder behind;
   delete it to remove all traces. The webview runs with a non-persistent
   browser session; Windows WebView2's required runtime cache is explicitly
   rooted under `~/.openconkit/cache/webview`.
4. **No OpenConKit account, activation, or license check.** Optional AI uses
   a user-initiated ChatGPT sign-in managed by the isolated Codex runtime.

## Network access

OpenConKit permits network connections only in these cases:

- **Updater**: checks one of two project-owned signed release feeds. A
  best-effort check runs after startup only when the previous successful
  check is older than 24 hours; users can also check manually. No document
  content or usage data is sent. The request reveals the normal HTTP metadata,
  current app version, target platform, and updater user agent.
- **AI features (optional)**: when you explicitly invoke an AI feature, the
  locally running Codex app-server sidecar may contact OpenAI using a
  Codex-managed ChatGPT login. The exact bounded normalized facts and findings
  are shown for consent before transmission. Whole workbooks are never sent.

Everything else works with the network cable unplugged.

## AI features

- The Codex sidecar runs as a local subprocess only when an AI/account action
  is explicitly used.
- AI output is advisory: it is displayed as suggestions and never mutates
  your data silently.
- Codex owns authentication material and prefers the operating-system
  credential store. OpenConKit removes common API-token environment variables
  from the child process, never reads raw tokens, and exposes only masked
  account metadata to the WebView.
- Every AI response is schema-checked and semantically revalidated against the
  transmitted fact/finding identifiers before it can be displayed.

## Logs

Diagnostic logging is off by default and takes effect after restart. When
enabled, `~/.openconkit/logs/codex-protocol.jsonl` records only timestamp,
direction, bounded protocol method name, numeric request id, envelope
kind/status, and byte count. It excludes raw protocol JSON, workbook values,
prompts, responses, stderr text, paths, and credentials. The file rotates at
2 MiB with three retained files.
