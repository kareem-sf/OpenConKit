# 0004. SQLite via rusqlite (bundled) with embedded migrations

- Status: Accepted
- Date: 2026-07-23

## Context

The app needs durable local storage (projects, settings, findings) with
zero setup, offline operation, and a schema that can evolve across releases.

## Decision

- SQLite through `rusqlite` 0.40 with the `bundled` feature (SQLite compiled
  in; no system dependency).
- Database file at `<app-home>/data/openconkit.sqlite3`.
- Migrations are embedded SQL files (`crates/openconkit-storage/migrations/`),
  applied in order by `Database::migrate`, each in its own transaction,
  recorded in `schema_migrations`.
- Migrations are append-only: never edit a released migration; add a new
  one. The runner refuses databases newer than the build (`SchemaTooNew`).
- Before applying pending migrations to an existing database, bootstrap uses
  SQLite's online backup API to create a consistent, non-overwriting backup
  under `<app-home>/backups/`. It verifies the backup with `quick_check`
  before proceeding.

## Consequences

- Positive: no server or service account; consistent online backups;
  transactional safety for migration failures; works identically on all
  target OSes.
- Negative: single-writer concurrency limits (acceptable for a desktop
  app); schema changes require migration discipline.
- Alternatives considered: JSON files (rejected: no transactions, fragile
  concurrency); sqlx/Diesel (rejected: heavier, async not needed at this
  layer, bundled rusqlite is simpler for Tauri).
