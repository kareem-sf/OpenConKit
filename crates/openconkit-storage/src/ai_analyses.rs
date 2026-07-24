//! SQLite adapter for [`AiAnalysisRepository`].

use openconkit_application::{AiAnalysisRepository, RepositoryError};
use openconkit_domain::{
    AiAnalysis, AiAnalysisId, AiAnalysisLanguage, AiAnalysisStatus, AiGroundingStatus,
    AiValidationStatus, AnalysisRunId, Sha256Hash,
};
use rusqlite::params;

use crate::codecs::{
    domain_to_sqlite, format_timestamp, from_json_opt_sql, from_json_sql, map_sqlite, map_storage,
    parse_timestamp, to_json,
};
use crate::database::Database;

/// SQLite-backed [`AiAnalysisRepository`].
pub struct SqliteAiAnalysisRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteAiAnalysisRepository<'a> {
    /// Borrow a database handle.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl AiAnalysisRepository for SqliteAiAnalysisRepository<'_> {
    fn save(&self, analysis: &AiAnalysis) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let status = enum_str(&analysis.status)?;
        let validation_status = enum_str(&analysis.validation_status)?;
        let grounding_status = enum_str(&analysis.grounding_status)?;
        let language = enum_str(&analysis.language)?;
        let output = match &analysis.output {
            Some(value) => Some(to_json(value)?),
            None => None,
        };
        let changed = conn
            .execute(
                "INSERT INTO ai_analyses (
                id, run_id, model, codex_version, language, input_scope_hash,
                status, validation_status, grounding_status, output, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                model = excluded.model,
                codex_version = excluded.codex_version,
                language = excluded.language,
                input_scope_hash = excluded.input_scope_hash,
                status = excluded.status,
                validation_status = excluded.validation_status,
                grounding_status = excluded.grounding_status,
                output = excluded.output
             WHERE ai_analyses.run_id = excluded.run_id",
                params![
                    analysis.id.to_string(),
                    analysis.run_id.to_string(),
                    analysis.model,
                    analysis.codex_version,
                    language,
                    analysis.input_scope_hash.as_str(),
                    status,
                    validation_status,
                    grounding_status,
                    output,
                    format_timestamp(analysis.created_at),
                ],
            )
            .map_err(map_sqlite)?;
        if changed != 1 {
            return Err(RepositoryError::Invariant(
                "AI analysis id cannot be reassigned to another run".to_string(),
            ));
        }
        Ok(())
    }

    fn list_by_run(&self, run_id: &AnalysisRunId) -> Result<Vec<AiAnalysis>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, model, codex_version, language, input_scope_hash,
                        status, validation_status, grounding_status, output, created_at
                 FROM ai_analyses
                 WHERE run_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![run_id.to_string()], map_ai_row)
            .map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }
}

fn map_ai_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiAnalysis> {
    let id = AiAnalysisId::parse(&row.get::<_, String>(0)?).map_err(domain_to_sqlite)?;
    let run_id = AnalysisRunId::parse(&row.get::<_, String>(1)?).map_err(domain_to_sqlite)?;
    let model: String = row.get(2)?;
    let codex_version: String = row.get(3)?;
    let language: AiAnalysisLanguage = enum_from_str(&row.get::<_, String>(4)?)?;
    let input_scope_hash =
        Sha256Hash::from_hex(&row.get::<_, String>(5)?).map_err(domain_to_sqlite)?;
    let status: AiAnalysisStatus = enum_from_str(&row.get::<_, String>(6)?)?;
    let validation_status: AiValidationStatus = enum_from_str(&row.get::<_, String>(7)?)?;
    let grounding_status: AiGroundingStatus = enum_from_str(&row.get::<_, String>(8)?)?;
    let output = from_json_opt_sql(row.get(9)?)?;
    let created_at = parse_timestamp(&row.get::<_, String>(10)?)?;
    Ok(AiAnalysis {
        id,
        run_id,
        model,
        codex_version,
        language,
        input_scope_hash,
        status,
        validation_status,
        grounding_status,
        output,
        created_at,
    })
}

fn enum_str<T: serde::Serialize>(value: &T) -> Result<String, RepositoryError> {
    let json = to_json(value)?;
    Ok(json.trim_matches('"').to_string())
}

fn enum_from_str<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, rusqlite::Error> {
    from_json_sql(&format!("\"{raw}\""))
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
            Sha256Hash::from_bytes([0x44; 32]),
            "boq.xlsx".into(),
            None,
            "sources/44/boq.xlsx".into(),
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
    fn save_and_list_ai_analyses() {
        let db = test_db();
        let run_id = seed_run(&db);
        let repo = SqliteAiAnalysisRepository::new(&db);
        let analysis = AiAnalysis {
            id: AiAnalysisId::new(),
            run_id,
            model: "gpt-5-codex".into(),
            codex_version: "0.145.0".into(),
            language: AiAnalysisLanguage::En,
            input_scope_hash: Sha256Hash::from_bytes([0x55; 32]),
            status: AiAnalysisStatus::Completed,
            validation_status: AiValidationStatus::Unvalidated,
            grounding_status: AiGroundingStatus::Validated,
            output: Some(serde_json::json!({"summary": "ok"})),
            created_at: Timestamp::now(),
        };
        repo.save(&analysis).expect("save");
        let listed = repo.list_by_run(&run_id).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, analysis.id);
        assert_eq!(listed[0].validation_status, AiValidationStatus::Unvalidated);
        assert_eq!(listed[0].grounding_status, AiGroundingStatus::Validated);
        assert_eq!(listed[0].output, Some(serde_json::json!({"summary": "ok"})));

        let updated = AiAnalysis {
            status: AiAnalysisStatus::Failed,
            grounding_status: AiGroundingStatus::Rejected,
            output: None,
            ..analysis
        };
        repo.save(&updated).expect("update");
        let listed = repo.list_by_run(&run_id).expect("list updated");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, AiAnalysisStatus::Failed);
        assert_eq!(listed[0].output, None);
    }
}
