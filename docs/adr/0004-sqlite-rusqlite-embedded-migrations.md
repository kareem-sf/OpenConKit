# 0004. SQLite via rusqlite (bundled) with embedded migrations

- Status: Accepted
- Date: 2026-07-23

## Context

The app needs durable local storage (projects, settings, findings) with
zero setup, offline operation, and a schema that can evolve across releases.

## Decision

- SQLite through `rusqlite` 0.40 with the `bundled` feature (SQLite compiled
  in; no system dependency).
- Database file at `<app-home>/openconkit.db`.
- Migrations are embedded SQL files (`crates/openconkit-storage/migrations/`),
  applied in order by `Database::migrate`, each in its own transaction,
  recorded in `schema_migrations`.
- Migrations are append-only: never edit a released migration; add a new
  one. The runner refuses databases newer than the build (`SchemaTooNew`).

## Consequences

- Positive: no server, no service account, backup = copy one file,
  transactional safety for migration failures, works identically on all
  target OSes.
- Negative: single-writer concurrency limits (acceptable for a desktop
  app); schema changes require migration discipline.
- Alternatives considered: JSON files (rejected: no transactions, fragile
  concurrency); sqlx/Diesel (rejected: heavier, async not needed at this
  layer, bundled rusqlite is simpler for Tauri).
