//! OpenConKit storage layer.
//!
//! SQLite (bundled via rusqlite) with embedded, append-only migrations and
//! repository adapters that implement the application ports. Also owns
//! app-home bootstrap and settings file persistence. See
//! `docs/adr/0004-sqlite-rusqlite-embedded-migrations.md`.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod ai_analyses;
pub mod atomic;
pub mod codecs;
pub mod database;
pub mod exports;
pub mod findings;
pub mod home;
pub mod migrations;
pub mod permissions;
pub mod projects;
pub mod runs;
pub mod settings_store;
pub mod source_storage;
pub mod sources;

pub use ai_analyses::SqliteAiAnalysisRepository;
pub use database::Database;
pub use exports::SqliteExportRepository;
pub use findings::SqliteFindingRepository;
pub use home::{bootstrap_home, request_factory_reset, resolve_home, BootstrapResult};
pub use migrations::MIGRATIONS;
pub use projects::SqliteProjectRepository;
pub use runs::SqliteAnalysisRunRepository;
pub use settings_store::SettingsStore;
pub use source_storage::FsSourceStorage;
pub use sources::SqliteSourceRevisionRepository;

/// Errors from the storage layer.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// A SQLite operation failed.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// A filesystem operation failed.
    #[error("I/O error: {message}")]
    Io {
        /// OS error detail.
        message: String,
    },

    /// The development/test home override was present but empty.
    #[error("OPENCONKIT_HOME is set but empty")]
    HomeOverrideEmpty,

    /// A release build attempted to use the development/test home override.
    #[error("OPENCONKIT_HOME is available only in development and tests")]
    HomeOverrideNotAllowed,

    /// No canonical operating-system home directory could be resolved.
    #[error("could not determine the user home directory")]
    HomeNotFound,

    /// A destructive reset target was not a safe, absolute app-home directory.
    #[error("refusing to reset an unsafe application-home target")]
    UnsafeFactoryResetTarget,

    /// A database backup target already exists and will not be overwritten.
    #[error("database backup target already exists: {path}")]
    BackupAlreadyExists {
        /// Existing path that was protected from overwrite.
        path: String,
    },

    /// SQLite produced a backup that failed its integrity check.
    #[error("database backup integrity check failed: {message}")]
    BackupVerification {
        /// Integrity-check result.
        message: String,
    },

    /// Embedded migration metadata is invalid.
    #[error("invalid migration plan: {message}")]
    InvalidMigrationPlan {
        /// Validation detail.
        message: String,
    },

    /// Settings/config serialization failed.
    #[error("config error: {0}")]
    Config(String),

    /// The connection mutex was poisoned (a prior holder panicked).
    #[error("database connection lock poisoned")]
    LockPoisoned,

    /// The database schema is newer than this build supports.
    #[error("database schema version {found} is newer than supported version {supported}")]
    SchemaTooNew {
        /// Version found in the database.
        found: u32,
        /// Highest version this build can apply.
        supported: u32,
    },
}
