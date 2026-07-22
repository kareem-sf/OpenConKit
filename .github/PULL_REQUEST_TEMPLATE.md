## Summary

<!-- What does this PR change and why? Link the issue if one exists. -->

## Type of change

- [ ] feat
- [ ] fix
- [ ] docs
- [ ] refactor
- [ ] perf
- [ ] test
- [ ] build / ci
- [ ] chore

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `pnpm lint`, `pnpm format:check`, `pnpm typecheck`, `pnpm test`, `pnpm build` pass
- [ ] `node scripts/version-check.mjs` passes
- [ ] No `unwrap()`/`expect()` in production Rust paths
- [ ] User-facing strings added to `packages/i18n` (en AND ar)
- [ ] No telemetry, no source-workbook modification, no new network calls
- [ ] Threat model reviewed/updated if I touched ingestion, exports, IPC,
      deep links, updater, or the Codex sidecar
- [ ] ADR added for architecture changes

## Notes for reviewers

<!-- Anything risky, deferred, or needing special attention. -->
