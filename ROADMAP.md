# Roadmap

OpenConKit is a local-first toolkit that hosts multiple construction-domain
tools in one desktop shell. Tools ship in the shell; they are never runtime
downloads.

## v0.0.1 (in progress)

- Repository foundation: workspaces, quality gates, versioning, docs, ADRs,
  brand identity.
- BOQ Inspector: automated Bill of Quantities quality review (XLS/XLSX
  ingestion, defect detection, Excel + PDF reports, EN/AR UI).
- Optional AI-assisted explanations via the bundled Codex app-server
  sidecar, grounded only in extracted facts and explicitly invoked.
- Signed updater artifacts and native Windows, macOS, and Linux release
  packages as specified in the master build prompt.

## Near term (post-0.0.1)

- Additional deterministic BOQ rules and report customization driven by
  fixture evidence and user feedback.
- Additional release channels after the stable update path is proven.
- Performance tuning for very large, but still safety-bounded, workbooks.

## Future tool ideas (uncommitted)

- **Tender Comparator** - side-by-side bid normalization and deviation flags.
- **Quantity Takeoff Helper** - structured measurement sheets with audit trail.
- **Variation Tracker** - change-order register with cumulative impact.
- **Spec Cross-Checker** - consistency checks between BOQ items and specs.
- **Progress Reporter** - periodic progress reports from site measurements.

## Deferred features (deliberately not yet)

- Runtime plugin loading / extension marketplace (see ADR 0003; revisit only
  with a signed-plugin design).
- Cloud sync and multi-user collaboration (conflicts with the local-first
  privacy model; would need a separate design).
- Mobile companion app.
- Telemetry of any kind (permanently out of scope, see `docs/privacy.md`).
