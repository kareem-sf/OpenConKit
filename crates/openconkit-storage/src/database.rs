//! Database handle and migration runner.

use std::fs::{self, OpenOptions};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};

use crate::migrations::{Migration, MIGRATIONS};
use crate::permissions::harden_file;
use crate::StorageError;

/// An open SQLite database for the application.
///
/// The connection is wrapped in a [`Mutex`] so repository adapters can share
/// a single handle across the (single-writer) desktop process.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open (creating if necessary) a database file and apply PRAGMAs.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        harden_file(path).map_err(io_err)?;
        configure(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (used by tests and tooling).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        configure(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Borrow the connection for a repository operation.
    pub(crate) fn conn(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.conn.lock().map_err(|_| StorageError::LockPoisoned)
    }

    /// Current schema version (0 before any migration ran).
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        let conn = self.conn()?;
        let migrations_table_exists: bool = conn.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'schema_migrations'
             )",
            [],
            |row| row.get(0),
        )?;
        if !migrations_table_exists {
            return Ok(0);
        }
        let version: u32 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        Ok(version)
    }

    /// Write a consistent SQLite backup without overwriting an existing file.
    ///
    /// The online backup API includes committed WAL state. The completed
    /// destination is accepted only when `PRAGMA quick_check` returns `ok`.
    pub fn backup_to(&self, path: &Path) -> Result<(), StorageError> {
        if path.exists() {
            return Err(StorageError::BackupAlreadyExists {
                path: path.to_string_lossy().into_owned(),
            });
        }

        let result = self.backup_to_inner(path);
        if result.is_err() {
            let _ = fs::remove_file(path);
        }
        result
    }

    fn backup_to_inner(&self, path: &Path) -> Result<(), StorageError> {
        let source = self.conn()?;
        // Reserve with create_new so a race can never overwrite another
        // backup, then let SQLite initialize that exact empty file.
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(io_err)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        let mut destination = Connection::open_with_flags(path, flags)?;
        harden_file(path).map_err(io_err)?;
        {
            let backup = Backup::new(&source, &mut destination)?;
            backup.run_to_completion(128, Duration::from_millis(10), None)?;
        }
        let quick_check: String =
            destination.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if quick_check != "ok" {
            return Err(StorageError::BackupVerification {
                message: quick_check,
            });
        }
        Ok(())
    }

    /// Apply all pending migrations, each in its own transaction.
    ///
    /// Refuses to run against a database newer than this build.
    pub fn migrate(&self) -> Result<(), StorageError> {
        self.migrate_plan(MIGRATIONS)
    }

    fn migrate_plan(&self, migrations: &[Migration]) -> Result<(), StorageError> {
        validate_migration_plan(migrations)?;
        let current = self.schema_version()?;
        let latest = migrations.last().map_or(0, |m| m.version);
        if current > latest {
            return Err(StorageError::SchemaTooNew {
                found: current,
                supported: latest,
            });
        }
        let mut conn = self.conn()?;
        for migration in migrations.iter().filter(|m| m.version > current) {
            let tx = conn.transaction()?;
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, description) VALUES (?1, ?2)",
                (migration.version, migration.description),
            )?;
            tx.commit()?;
        }
        Ok(())
    }
}

fn validate_migration_plan(migrations: &[Migration]) -> Result<(), StorageError> {
    let mut previous = 0;
    for migration in migrations {
        if migration.version == 0 || migration.version <= previous {
            return Err(StorageError::InvalidMigrationPlan {
                message: format!(
                    "version {} must be greater than previous version {previous}",
                    migration.version
                ),
            });
        }
        previous = migration.version;
    }
    Ok(())
}

fn configure(conn: &Connection) -> Result<(), StorageError> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn io_err(err: std::io::Error) -> StorageError {
    StorageError::Io {
        message: err.to_string(),
    }
}

/// Helper used by tests: open an in-memory DB and migrate it.
#[cfg(test)]
pub(crate) fn test_db() -> Database {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    let db = Database::open_in_memory().expect("opens");
    db.migrate().expect("migrates");
    db
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use rusqlite::OptionalExtension;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("openconkit-{name}-{nanos}.sqlite3"))
    }

    #[test]
    fn migrates_fresh_database_to_latest_version() {
        let db = Database::open_in_memory().expect("opens");
        assert_eq!(db.schema_version().expect("version"), 0);
        db.migrate().expect("migrates");
        let latest = MIGRATIONS.last().map_or(0, |m| m.version);
        assert_eq!(db.schema_version().expect("version"), latest);
    }

    #[test]
    fn migration_is_idempotent() {
        let db = Database::open_in_memory().expect("opens");
        db.migrate().expect("first run");
        db.migrate().expect("second run is a no-op");
        let latest = MIGRATIONS.last().map_or(0, |m| m.version);
        assert_eq!(db.schema_version().expect("version"), latest);
    }

    #[test]
    fn failed_migration_rolls_back_its_schema_and_version() {
        let db = Database::open_in_memory().expect("opens");
        db.migrate().expect("initial migration");
        let current_version = MIGRATIONS.last().map_or(0, |migration| migration.version);
        let failing = Migration {
            version: current_version + 1,
            description: "deliberate rollback probe",
            sql: "CREATE TABLE rollback_probe (id INTEGER PRIMARY KEY);
                  INSERT INTO table_that_does_not_exist (id) VALUES (1);",
        };

        assert!(db.migrate_plan(&[failing]).is_err());
        assert_eq!(db.schema_version().expect("version"), current_version);
        let conn = db.conn().expect("lock");
        let probe_exists: bool = conn
            .query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'table' AND name = 'rollback_probe'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert!(!probe_exists, "failed migration must roll back its DDL");
    }

    #[test]
    fn migration_plan_rejects_duplicate_or_zero_versions() {
        let duplicate = [
            Migration {
                version: 1,
                description: "one",
                sql: "",
            },
            Migration {
                version: 1,
                description: "duplicate",
                sql: "",
            },
        ];
        assert!(matches!(
            validate_migration_plan(&duplicate),
            Err(StorageError::InvalidMigrationPlan { .. })
        ));
        assert!(matches!(
            validate_migration_plan(&[Migration {
                version: 0,
                description: "zero",
                sql: "",
            }]),
            Err(StorageError::InvalidMigrationPlan { .. })
        ));
    }

    #[test]
    fn online_backup_is_consistent_and_never_overwrites() {
        let source_path = temp_path("backup-source");
        let backup_path = temp_path("backup-copy");
        let db = Database::open(&source_path).expect("opens");
        db.migrate().expect("migrates");
        {
            let conn = db.conn().expect("lock");
            conn.execute(
                "INSERT INTO projects (
                     id, name, created_at, updated_at
                 ) VALUES ('backup-probe', 'Backup Probe',
                     '2026-07-23T00:00:00Z', '2026-07-23T00:00:00Z')",
                [],
            )
            .expect("insert");
        }

        db.backup_to(&backup_path).expect("backup");
        let backup = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open backup");
        let count: i64 = backup
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE id = 'backup-probe'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1);
        assert!(matches!(
            db.backup_to(&backup_path),
            Err(StorageError::BackupAlreadyExists { .. })
        ));

        drop(backup);
        drop(db);
        let _ = fs::remove_file(&backup_path);
        let _ = fs::remove_file(&source_path);
    }

    #[test]
    fn foreign_keys_are_enabled() {
        let db = test_db();
        let conn = db.conn().expect("lock");
        let enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("pragma");
        assert_eq!(enabled, 1);
    }

    #[test]
    fn optional_extension_compiles() {
        // Smoke: OptionalExtension is used by repositories; keep the import live.
        let db = test_db();
        let conn = db.conn().expect("lock");
        let missing: Option<i64> = conn
            .query_row(
                "SELECT version FROM schema_migrations WHERE version = -1",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("query");
        assert!(missing.is_none());
    }
}
