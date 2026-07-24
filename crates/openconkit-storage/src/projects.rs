//! SQLite adapter for [`ProjectRepository`].

use jiff::Timestamp;
use openconkit_application::{ProjectRepository, RepositoryError};
use openconkit_domain::{Project, ProjectId, ProjectMetadata};
use rusqlite::{params, OptionalExtension};

use crate::codecs::{domain_to_sqlite, format_timestamp, map_sqlite, map_storage, parse_timestamp};
use crate::database::Database;

/// SQLite-backed [`ProjectRepository`].
pub struct SqliteProjectRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteProjectRepository<'a> {
    /// Borrow a database handle.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl ProjectRepository for SqliteProjectRepository<'_> {
    fn create(&self, project: &Project) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let changed = conn
            .execute(
                "INSERT OR IGNORE INTO projects (
                    id, name, created_at, updated_at, archived_at,
                    description, client, location
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    project.id().as_str(),
                    project.name(),
                    format_timestamp(project.created_at()),
                    format_timestamp(project.updated_at()),
                    project.archived_at().map(format_timestamp),
                    project.metadata().description.as_deref(),
                    project.metadata().client.as_deref(),
                    project.metadata().location.as_deref(),
                ],
            )
            .map_err(map_sqlite)?;
        if changed == 0 {
            return Err(RepositoryError::Duplicate(project.id().clone()));
        }
        Ok(())
    }

    fn save(&self, project: &Project) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        conn.execute(
            "INSERT INTO projects (
                id, name, created_at, updated_at, archived_at,
                description, client, location
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                updated_at = excluded.updated_at,
                archived_at = excluded.archived_at,
                description = excluded.description,
                client = excluded.client,
                location = excluded.location",
            params![
                project.id().as_str(),
                project.name(),
                format_timestamp(project.created_at()),
                format_timestamp(project.updated_at()),
                project.archived_at().map(format_timestamp),
                project.metadata().description.as_deref(),
                project.metadata().client.as_deref(),
                project.metadata().location.as_deref(),
            ],
        )
        .map_err(map_sqlite)?;
        Ok(())
    }

    fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        conn.query_row(
            "SELECT id, name, created_at, updated_at, archived_at,
                    description, client, location
             FROM projects WHERE id = ?1",
            params![id.as_str()],
            map_project_row,
        )
        .optional()
        .map_err(map_sqlite)
    }

    fn list(&self, include_archived: bool) -> Result<Vec<Project>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let sql = if include_archived {
            "SELECT id, name, created_at, updated_at, archived_at,
                    description, client, location
             FROM projects
             ORDER BY created_at ASC"
        } else {
            "SELECT id, name, created_at, updated_at, archived_at,
                    description, client, location
             FROM projects
             WHERE archived_at IS NULL
             ORDER BY created_at ASC"
        };
        let mut stmt = conn.prepare(sql).map_err(map_sqlite)?;
        let rows = stmt.query_map([], map_project_row).map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }

    fn archive(&self, id: &ProjectId, archived_at: Timestamp) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let changed = conn
            .execute(
                "UPDATE projects
                 SET archived_at = ?1, updated_at = ?1
                 WHERE id = ?2",
                params![format_timestamp(archived_at), id.as_str()],
            )
            .map_err(map_sqlite)?;
        if changed == 0 {
            return Err(RepositoryError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let id_raw: String = row.get(0)?;
    let name: String = row.get(1)?;
    let created_at = parse_timestamp(&row.get::<_, String>(2)?)?;
    let updated_at = parse_timestamp(&row.get::<_, String>(3)?)?;
    let archived_at: Option<String> = row.get(4)?;
    let archived_at = match archived_at {
        Some(raw) => Some(parse_timestamp(&raw)?),
        None => None,
    };
    let metadata = ProjectMetadata {
        description: row.get(5)?,
        client: row.get(6)?,
        location: row.get(7)?,
    };
    let id = ProjectId::new(id_raw).map_err(domain_to_sqlite)?;
    Project::reconstitute(id, name, created_at, updated_at, archived_at, metadata)
        .map_err(domain_to_sqlite)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::database::test_db;
    use openconkit_domain::ProjectMetadata;

    fn sample(id: &str, name: &str) -> Project {
        Project::new(ProjectId::new(id).expect("slug"), name, Timestamp::now())
            .expect("project")
            .with_metadata(ProjectMetadata {
                description: Some("desc".into()),
                client: Some("ACME".into()),
                location: None,
            })
    }

    #[test]
    fn save_find_list_and_archive_round_trip() {
        let db = test_db();
        let repo = SqliteProjectRepository::new(&db);

        let project = sample("tower-a", "Tower A");
        repo.save(&project).expect("save");

        let found = repo
            .find_by_id(project.id())
            .expect("find")
            .expect("present");
        assert_eq!(found.id(), project.id());
        assert_eq!(found.name(), "Tower A");
        assert_eq!(found.metadata().client.as_deref(), Some("ACME"));
        assert!(!found.is_archived());

        let listed = repo.list(false).expect("list");
        assert_eq!(listed.len(), 1);

        let t = Timestamp::now();
        repo.archive(project.id(), t).expect("archive");
        assert!(repo.list(false).expect("active").is_empty());
        let archived = repo.list(true).expect("all");
        assert_eq!(archived.len(), 1);
        assert!(archived[0].is_archived());
    }

    #[test]
    fn create_is_atomic_and_rejects_duplicate_ids() {
        let db = test_db();
        let repo = SqliteProjectRepository::new(&db);
        let project = sample("tower-a", "Tower A");
        repo.create(&project).expect("create");

        let duplicate = sample("tower-a", "Replacement");
        let error = repo.create(&duplicate).expect_err("duplicate rejected");
        assert!(matches!(error, RepositoryError::Duplicate(_)));
        let found = repo
            .find_by_id(project.id())
            .expect("find")
            .expect("present");
        assert_eq!(found.name(), "Tower A");
    }

    #[test]
    fn archive_missing_returns_not_found() {
        let db = test_db();
        let repo = SqliteProjectRepository::new(&db);
        let err = repo
            .archive(&ProjectId::new("missing").expect("slug"), Timestamp::now())
            .expect_err("missing");
        assert!(matches!(err, RepositoryError::NotFound(_)));
    }

    #[test]
    fn save_is_upsert() {
        let db = test_db();
        let repo = SqliteProjectRepository::new(&db);
        let project = sample("tower-a", "Tower A");
        repo.save(&project).expect("save");
        let renamed = Project::new(
            project.id().clone(),
            "Tower A Renamed",
            project.created_at(),
        )
        .expect("project");
        repo.save(&renamed).expect("upsert");
        let found = repo
            .find_by_id(project.id())
            .expect("find")
            .expect("present");
        assert_eq!(found.name(), "Tower A Renamed");
        assert_eq!(repo.list(true).expect("list").len(), 1);
    }
}
