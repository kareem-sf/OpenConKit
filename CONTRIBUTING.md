# Contributing to OpenConKit

Thanks for helping build the open-source toolkit for construction
professionals. This document covers the ground rules; `AGENTS.md` covers the
engineering rules that also apply to AI assistants working in this repo.

## Getting started

1. Install the prerequisites (see the development quickstart in `README.md`).
2. `pnpm install`
3. Run the quality gates before opening a PR:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   pnpm lint
   pnpm format:check
   pnpm typecheck
   pnpm test
   pnpm build
   node scripts/version-check.mjs
   ```

## Ground rules

- **Never modify a user's source workbook.** All ingestion is read-only.
- **No telemetry, no analytics, no phone-home.** See `docs/privacy.md`.
- **All user-facing strings go through i18n** (`packages/i18n`), in English
  and Arabic. The key-parity test must stay green.
- **No `unwrap()`/`expect()` in production Rust paths** (tests and build
  scripts excepted). Use typed errors (`thiserror`).
- Migrations are append-only; never edit a released migration.
- Keep the layered architecture rules in `AGENTS.md` and `docs/architecture.md`.

## Commits and PRs

- Commit messages follow Conventional Commits:
  `feat(tool-boq-inspector): detect duplicate item codes`.
- Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`,
  `chore`. Scope is a crate, package, or area name.
- One logical change per PR. Fill out the PR template checklist.
- Architecture changes need an ADR (`docs/adr/`).

## Reporting issues

Use the issue templates (bug report / feature request). For security issues,
follow `SECURITY.md` instead of opening a public issue.

## License

By contributing, you agree that your contributions are licensed under the
Apache License, Version 2.0 (see `LICENSE`).
