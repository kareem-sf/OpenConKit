//! Codec helpers: map domain values to/from SQLite column types.

use jiff::Timestamp;
use openconkit_application::RepositoryError;
use openconkit_domain::DomainError;
use rusqlite::Error as SqliteError;

/// Map a SQLite error into a repository error.
pub(crate) fn map_sqlite(err: SqliteError) -> RepositoryError {
    RepositoryError::Storage(err.to_string())
}

/// Map a domain reconstitution error into a rusqlite error (for row mappers).
pub(crate) fn domain_to_sqlite(err: DomainError) -> SqliteError {
    SqliteError::ToSqlConversionFailure(Box::new(err))
}

/// Map a storage-layer error into a repository error.
pub(crate) fn map_storage(err: crate::StorageError) -> RepositoryError {
    RepositoryError::Storage(err.to_string())
}

/// Parse a jiff timestamp from its string form.
pub(crate) fn parse_timestamp(raw: &str) -> Result<Timestamp, SqliteError> {
    raw.parse::<Timestamp>()
        .map_err(|err| SqliteError::ToSqlConversionFailure(Box::new(err)))
}

/// Format a timestamp for storage (jiff's canonical string form).
pub(crate) fn format_timestamp(ts: Timestamp) -> String {
    ts.to_string()
}

/// JSON-encode a value for a TEXT column.
pub(crate) fn to_json<T: serde::Serialize>(value: &T) -> Result<String, RepositoryError> {
    serde_json::to_string(value).map_err(|err| RepositoryError::Storage(err.to_string()))
}

/// JSON-decode a value from a TEXT column (rusqlite error for row mappers).
pub(crate) fn from_json_sql<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, SqliteError> {
    serde_json::from_str(raw).map_err(|err| SqliteError::ToSqlConversionFailure(Box::new(err)))
}

/// Optional JSON column: `None` when NULL or empty.
pub(crate) fn from_json_opt_sql<T: serde::de::DeserializeOwned>(
    raw: Option<String>,
) -> Result<Option<T>, SqliteError> {
    match raw {
        None => Ok(None),
        Some(s) if s.is_empty() || s == "null" => Ok(None),
        Some(s) => Ok(Some(from_json_sql(&s)?)),
    }
}
