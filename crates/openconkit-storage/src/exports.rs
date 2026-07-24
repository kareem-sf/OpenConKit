//! SQLite adapter for [`ExportRepository`].

use openconkit_application::{ExportRepository, RepositoryError};
use openconkit_domain::{AnalysisRunId, ExportId, ExportKind, ExportRecord, Sha256Hash};
use rusqlite::params;

use crate::codecs::{
    domain_to_sqlite, format_timestamp, from_json_sql, map_sqlite, map_storage, parse_timestamp,
    to_json,
};
use crate::database::Database;

/// SQLite-backed [`ExportRepository`].
pub struct SqliteExportRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteExportRepository<'a> {
    /// Borrow a database handle.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl ExportRepository for SqliteExportRepository<'_> {
    fn save(&self, export: &ExportRecord) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let kind = enum_str(&export.kind)?;
        conn.execute(
            "INSERT INTO exports (
                id, run_id, kind, language, relative_path, sha256, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                export.id.to_string(),
                export.run_id.to_string(),
                kind,
                export.language,
                export.relative_path,
                export.sha256.as_str(),
                format_timestamp(export.created_at),
            ],
        )
        .map_err(map_sqlite)?;
        Ok(())
    }

    fn list_by_run(&self, run_id: &AnalysisRunId) -> Result<Vec<ExportRecord>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, kind, language, relative_path, sha256, created_at
                 FROM exports
                 WHERE run_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![run_id.to_string()], map_export_row)
            .map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }
}

fn map_export_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExportRecord> {
    let id = ExportId::parse(&row.get::<_, String>(0)?).map_err(domain_to_sqlite)?;
    let run_id = AnalysisRunId::parse(&row.get::<_, String>(1)?).map_err(domain_to_sqlite)?;
    let kind_raw: String = row.get(2)?;
    let kind: ExportKind = from_json_sql(&format!("\"{kind_raw}\""))?;
    let language: String = row.get(3)?;
    let relative_path: String = row.get(4)?;
    let sha256 = Sha256Hash::from_hex(&row.get::<_, String>(5)?).map_err(domain_to_sqlite)?;
    let created_at = parse_timestamp(&row.get::<_, String>(6)?)?;
    ExportRecord::new(
        id,
        run_id,
        kind,
        language,
        relative_path,
        sha256,
        created_at,
    )
    .map_err(domain_to_sqlite)
}

fn enum_str<T: serde::Serialize>(value: &T) -> Result<String, RepositoryError> {
    let json = to_json(value)?;
    Ok(json.trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::database::test_db;
    use crate::projects::SqliteProjectRepository;
    use crate::runs::SqliteAnalysisRunRepository;
    use crate::sources::SqliteSourceRevisionRepository;
    use jiff::Timestamp;
    use openconkit_application::{
        AnalysisRunRepository, ProjectRepository, SourceRevisionRepository,
    };
    use openconkit_domain::{
        AnalysisRun, Confidence, Project, ProjectId, RunStatus, SourceRevision, SourceRevisionId,
    };

    fn seed_run(db: &Database) -> AnalysisRunId {
        let projects = SqliteProjectRepository::new(db);
        let project = Project::new(
            ProjectId::new("tower-a").expect("slug"),
            "Tower A",
            Timestamp::now(),
        )
        .expect("project");
        projects.save(&project).expect("save");

        let sources = SqliteSourceRevisionRepository::new(db);
        let revision = SourceRevision::new(
            SourceRevisionId::new(),
            project.id().clone(),
            Sha256Hash::from_bytes([0x22; 32]),
            "boq.xlsx".into(),
            None,
            "sources/22/boq.xlsx".into(),
            10,
            Timestamp::now(),
            "boq-inspector".into(),
            None,
        )
        .expect("revision");
        sources.save(&revision).expect("save");

        let runs = SqliteAnalysisRunRepository::new(db);
        let run = AnalysisRun {
            id: AnalysisRunId::new(),
            project_id: project.id().clone(),
            source_revision_id: revision.id,
            tool_id: "boq-inspector".into(),
            tool_version: "0.1.0".into(),
            rule_set_version: "2026.07".into(),
            app_version: "0.0.1".into(),
            status: RunStatus::Completed,
            started_at: Timestamp::now(),
            finished_at: Some(Timestamp::now()),
            structure_diagnostics: None,
            overall_confidence: Some(Confidence::new(1.0).expect("ok")),
        };
        runs.save(&run).expect("save run");
        run.id
    }

    #[test]
    fn save_and_list_exports() {
        let db = test_db();
        let run_id = seed_run(&db);
        let repo = SqliteExportRepository::new(&db);
        let export = ExportRecord::new(
            ExportId::new(),
            run_id,
            ExportKind::Xlsx,
            "en".into(),
            "reports/run-1.xlsx".into(),
            Sha256Hash::from_bytes([0x33; 32]),
            Timestamp::now(),
        )
        .expect("export");
        repo.save(&export).expect("save");
        let listed = repo.list_by_run(&run_id).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, export.id);
        assert_eq!(listed[0].kind, ExportKind::Xlsx);
    }
}
