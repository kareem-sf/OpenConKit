//! Embedded, append-only schema migrations.
//!
//! Rules (see ADR 0004):
//! - Migrations are never edited after release; only appended.
//! - Every migration runs in its own transaction.
//! - `schema_migrations` records applied versions.

/// One embedded migration step.
pub struct Migration {
    /// Monotonic version number, starting at 1.
    pub version: u32,
    /// Short description for logs.
    pub description: &'static str,
    /// SQL statements, embedded at compile time.
    pub sql: &'static str,
}

/// All migrations, in application order.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "initial schema",
    sql: include_str!("../migrations/0001_init.sql"),
}];
