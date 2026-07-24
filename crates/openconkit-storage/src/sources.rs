//! SQLite adapter for [`SourceRevisionRepository`].

use openconkit_application::{RepositoryError, SourceRevisionRepository};
use openconkit_domain::{ProjectId, Sha256Hash, SourceRevision, SourceRevisionId};
use rusqlite::{params, OptionalExtension};

use crate::codecs::{
    domain_to_sqlite, format_timestamp, from_json_opt_sql, map_sqlite, map_storage,
    parse_timestamp, to_json,
};
use crate::database::Database;

/// SQLite-backed [`SourceRevisionRepository`].
pub struct SqliteSourceRevisionRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteSourceRevisionRepository<'a> {
    /// Borrow a database handle.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl SourceRevisionRepository for SqliteSourceRevisionRepository<'_> {
    fn save(&self, revision: &SourceRevision) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let metadata = match &revision.workbook_metadata {
            Some(value) => Some(to_json(value)?),
            None => None,
        };
        conn.execute(
            "INSERT INTO source_revisions (
                id, project_id, sha256, original_filename, original_path,
                stored_path, size_bytes, imported_at, tool_id, workbook_metadata
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                revision.id.to_string(),
                revision.project_id.as_str(),
                revision.sha256.as_str(),
                revision.original_filename,
                revision.original_path,
                revision.stored_path,
                revision.size_bytes as i64,
                format_timestamp(revision.imported_at),
                revision.tool_id,
                metadata,
            ],
        )
        .map_err(map_sqlite)?;
        Ok(())
    }

    fn find_by_id(&self, id: &SourceRevisionId) -> Result<Option<SourceRevision>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        conn.query_row(
            "SELECT id, project_id, sha256, original_filename, original_path,
                    stored_path, size_bytes, imported_at, tool_id, workbook_metadata
             FROM source_revisions WHERE id = ?1",
            params![id.to_string()],
            map_revision_row,
        )
        .optional()
        .map_err(map_sqlite)
    }

    fn find_by_project_and_hash(
        &self,
        project_id: &ProjectId,
        sha256: &Sha256Hash,
    ) -> Result<Option<SourceRevision>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        conn.query_row(
            "SELECT id, project_id, sha256, original_filename, original_path,
                    stored_path, size_bytes, imported_at, tool_id, workbook_metadata
             FROM source_revisions
             WHERE project_id = ?1 AND sha256 = ?2",
            params![project_id.as_str(), sha256.as_str()],
            map_revision_row,
        )
        .optional()
        .map_err(map_sqlite)
    }

    fn list_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<SourceRevision>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, sha256, original_filename, original_path,
                        stored_path, size_bytes, imported_at, tool_id, workbook_metadata
                 FROM source_revisions
                 WHERE project_id = ?1
                 ORDER BY imported_at ASC",
            )
            .map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![project_id.as_str()], map_revision_row)
            .map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }
}

fn map_revision_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceRevision> {
    let id = SourceRevisionId::parse(&row.get::<_, String>(0)?).map_err(domain_to_sqlite)?;
    let project_id = ProjectId::new(row.get::<_, String>(1)?).map_err(domain_to_sqlite)?;
    let sha256 = Sha256Hash::from_hex(&row.get::<_, String>(2)?).map_err(domain_to_sqlite)?;
    let original_filename: String = row.get(3)?;
    let original_path: Option<String> = row.get(4)?;
    let stored_path: String = row.get(5)?;
    let size_bytes = row.get::<_, i64>(6)? as u64;
    let imported_at = parse_timestamp(&row.get::<_, String>(7)?)?;
    let tool_id: String = row.get(8)?;
    let workbook_metadata = from_json_opt_sql(row.get(9)?)?;
    SourceRevision::new(
        id,
        project_id,
        sha256,
        original_filename,
        original_path,
        stored_path,
        size_bytes,
        imported_at,
        tool_id,
        workbook_metadata,
    )
    .map_err(domain_to_sqlite)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::database::test_db;
    use crate::projects::SqliteProjectRepository;
    use jiff::Timestamp;
    use openconkit_application::ProjectRepository;
    use openconkit_domain::Project;

    fn seed_project(db: &Database) -> ProjectId {
        let repo = SqliteProjectRepository::new(db);
        let project = Project::new(
            ProjectId::new("tower-a").expect("slug"),
            "Tower A",
            Timestamp::now(),
        )
        .expect("project");
        repo.save(&project).expect("save project");
        project.id().clone()
    }

    fn sample(project_id: ProjectId) -> SourceRevision {
        SourceRevision::new(
            SourceRevisionId::new(),
            project_id,
            Sha256Hash::from_bytes([0xab; 32]),
            "boq.xlsx".into(),
            Some(r"C:\Users\qs\Downloads\boq.xlsx".into()),
            "sources/ab/boq.xlsx".into(),
            12_345,
            Timestamp::now(),
            "boq-inspector".into(),
            Some(serde_json::json!({"sheets": 2})),
        )
        .expect("revision")
    }

    #[test]
    fn save_find_and_list_by_project() {
        let db = test_db();
        let project_id = seed_project(&db);
        let repo = SqliteSourceRevisionRepository::new(&db);
        let revision = sample(project_id.clone());
        repo.save(&revision).expect("save");

        let found = repo
            .find_by_id(&revision.id)
            .expect("find")
            .expect("present");
        assert_eq!(found.id, revision.id);
        assert_eq!(found.sha256, revision.sha256);
        assert_eq!(
            found.workbook_metadata,
            Some(serde_json::json!({"sheets": 2}))
        );

        let listed = repo.list_by_project(&project_id).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, revision.id);
        let by_hash = repo
            .find_by_project_and_hash(&project_id, &revision.sha256)
            .expect("find by hash")
            .expect("present by hash");
        assert_eq!(by_hash.id, revision.id);
    }
}
