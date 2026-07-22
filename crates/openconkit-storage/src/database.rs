//! Database handle and migration runner.

use std::path::Path;

use rusqlite::Connection;

use crate::migrations::MIGRATIONS;
use crate::StorageError;

/// An open SQLite database for the application.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (creating if necessary) a database file.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            conn: Connection::open(path)?,
        })
    }

    /// Open an in-memory database (used by tests and tooling).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        Ok(Self {
            conn: Connection::open_in_memory()?,
        })
    }

    /// Current schema version (0 before any migration ran).
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        let version: u32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .or_else(|err| {
                // Table does not exist yet: version 0.
                if let rusqlite::Error::SqliteFailure(_, Some(message)) = &err {
                    if message.contains("no such table") {
                        return Ok(0);
                    }
                }
                Err(err)
            })?;
        Ok(version)
    }

    /// Apply all pending migrations, each in its own transaction.
    ///
    /// Refuses to run against a database newer than this build.
    pub fn migrate(&mut self) -> Result<(), StorageError> {
        let current = self.schema_version()?;
        let latest = MIGRATIONS.last().map_or(0, |m| m.version);
        if current > latest {
            return Err(StorageError::SchemaTooNew {
                found: current,
                supported: latest,
            });
        }
        for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
            let tx = self.conn.transaction()?;
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn migrates_fresh_database_to_latest_version() {
        let mut db = Database::open_in_memory().expect("opens");
        assert_eq!(db.schema_version().expect("version"), 0);
        db.migrate().expect("migrates");
        let latest = MIGRATIONS.last().map_or(0, |m| m.version);
        assert_eq!(db.schema_version().expect("version"), latest);
    }

    #[test]
    fn migration_is_idempotent() {
        let mut db = Database::open_in_memory().expect("opens");
        db.migrate().expect("first run");
        db.migrate().expect("second run is a no-op");
        let latest = MIGRATIONS.last().map_or(0, |m| m.version);
        assert_eq!(db.schema_version().expect("version"), latest);
    }
}
