# Support

## Where to get help

- **Documentation**: start with `README.md` and the `docs/` directory
  (`docs/architecture.md`, `docs/tool-authoring.md`, `docs/privacy.md`).
- **Bugs**: open an issue using the bug report template. Include your OS,
  OpenConKit version (`0.0.1`), and steps to reproduce.
- **Feature requests**: open an issue using the feature request template.
- **Security**: see `SECURITY.md` - never report vulnerabilities publicly.

## Response expectations

OpenConKit is maintained by volunteers. We triage issues weekly; there is no
SLA. Well-written reports with reproduction steps get answered first.

## Data safety

OpenConKit never modifies source workbooks and keeps all data on your
machine (`~/.openconkit`). If you suspect data loss or corruption caused by
the app, stop using it on the affected data and file a bug report
immediately. If diagnostic logging was enabled, inspect each JSONL file under
`~/.openconkit/logs` in a text editor before attaching it. The supported
Codex log contains only timestamp, direction, method, request id, envelope
kind/status, and byte count; do not attach a file if it contains anything
else.
