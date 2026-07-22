//! OpenConKit storage layer.
//!
//! SQLite (bundled via rusqlite) with embedded, append-only migrations.
//! See `docs/adr/0004-sqlite-rusqlite-embedded-migrations.md`.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod database;
pub mod migrations;

pub use database::Database;
pub use migrations::MIGRATIONS;

/// Errors from the storage layer.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// A SQLite operation failed.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The database schema is newer than this build supports.
    #[error("database schema version {found} is newer than supported version {supported}")]
    SchemaTooNew {
        /// Version found in the database.
        found: u32,
        /// Highest version this build can apply.
        supported: u32,
    },
}
