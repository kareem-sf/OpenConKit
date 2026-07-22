# Releasing (skeleton)

This document will be completed in the updates & packaging phase. It records
the intended release pipeline so foundation decisions line up with it.

## Version source

- The root `VERSION` file is canonical. `pnpm version:sync` propagates it to
  the Cargo workspace, all `package.json` files and `tauri.conf.json`;
  `pnpm version:check` gates releases (and CI).

## Intended pipeline (planned)

1. `pnpm version:sync` after editing `VERSION`; commit as
   `chore(release): vX.Y.Z`.
2. Update `CHANGELOG.md` (Keep a Changelog).
3. Tag `vX.Y.Z`; GitHub Actions builds artifacts:
   - Windows: NSIS installer (per-user), plus a portable zip produced by
     archiving the built binary (Tauri's bundler has no zip target).
   - macOS / Linux: built on native runners only.
4. Codex sidecar binaries are fetched per target from the pinned release in
   `tools/codex-version.json`, checksum-verified, and bundled.
5. `THIRD_PARTY_NOTICES.md` is regenerated (cargo-about + pnpm licenses).
6. GitHub Release is drafted with artifacts and updater manifests
   (signing keys in CI secrets; see `docs/threat-model.md`, "updater
   compromise").

## Channels

- `stable` only for now. A `next`/beta channel may be added with the updater.

## Rollback

Users can reinstall any previous release from the Releases page; the app
home directory (`~/.openconkit`) is versioned independently via schema
migrations (never downgraded automatically).
