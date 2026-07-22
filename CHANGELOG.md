# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - 0.0.1

### Added

- Repository foundation: Cargo + pnpm workspaces, ESLint/Prettier, rustfmt/
  clippy, editorconfig.
- Version plumbing: canonical `VERSION` file with `version:sync` /
  `version:check` scripts propagating to Cargo, package.json files and
  `tauri.conf.json`.
- Rust crate skeletons: domain, application, tool-sdk, spreadsheet, storage,
  reporting, ai-codex, tool-boq-inspector, desktop (Tauri host).
- Frontend skeleton: React 19 + Vite + Tailwind CSS v4, react-router,
  i18next (en + ar with RTL support), zustand theme store, Vitest setup.
- Shared packages: `@openconkit/ui` (design tokens + primitives),
  `@openconkit/i18n` (locales + parity tests), `@openconkit/contracts`
  (ts-rs binding surface + zod schemas).
- Brand identity: original logo, mono variant and app icon (`branding/`),
  icon generation script.
- Documentation: architecture, privacy, threat model, ADRs 0001-0007,
  community health files, issue/PR templates, dependabot.
