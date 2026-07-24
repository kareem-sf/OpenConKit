//! SQLite adapter for [`AnalysisRunRepository`].

use openconkit_application::{
    AnalysisRunRepository, RepositoryError, RunHistoryEntry, RunHistoryRepository,
};
use openconkit_domain::{
    AiAnalysisStatus, AnalysisRun, AnalysisRunId, Confidence, Finding, ProjectId, RunStatus,
    Sha256Hash, SourceRevisionId, WorkbookDiagnostics,
};
use rusqlite::{params, OptionalExtension};

use crate::codecs::{
    domain_to_sqlite, format_timestamp, from_json_opt_sql, from_json_sql, map_sqlite, map_storage,
    parse_timestamp, to_json,
};
use crate::database::Database;
use crate::findings::{delete_by_run, insert_finding};

/// SQLite-backed [`AnalysisRunRepository`].
pub struct SqliteAnalysisRunRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteAnalysisRunRepository<'a> {
    /// Borrow a database handle.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl AnalysisRunRepository for SqliteAnalysisRunRepository<'_> {
    fn save(&self, run: &AnalysisRun) -> Result<(), RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        validate_run_relationships(&conn, run)?;
        upsert_run(&conn, run)
    }

    fn find_by_id(&self, id: &AnalysisRunId) -> Result<Option<AnalysisRun>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        conn.query_row(
            "SELECT id, project_id, source_revision_id, tool_id, tool_version,
                    rule_set_version, app_version, status, started_at, finished_at,
                    structure_diagnostics, overall_confidence
             FROM analysis_runs WHERE id = ?1",
            params![id.to_string()],
            map_run_row,
        )
        .optional()
        .map_err(map_sqlite)
    }

    fn list_by_project(&self, project_id: &ProjectId) -> Result<Vec<AnalysisRun>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, source_revision_id, tool_id, tool_version,
                        rule_set_version, app_version, status, started_at, finished_at,
                        structure_diagnostics, overall_confidence
                 FROM analysis_runs
                 WHERE project_id = ?1
                 ORDER BY started_at ASC",
            )
            .map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![project_id.as_str()], map_run_row)
            .map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }

    fn save_with_findings(
        &self,
        run: &AnalysisRun,
        findings: &[Finding],
    ) -> Result<(), RepositoryError> {
        let mut conn = self.db.conn().map_err(map_storage)?;
        let tx = conn.transaction().map_err(map_sqlite)?;
        validate_run_relationships(&tx, run)?;
        validate_findings_match_run(run, findings)?;
        upsert_run(&tx, run)?;
        delete_by_run(&tx, &run.id)?;
        for finding in findings {
            insert_finding(&tx, finding)?;
        }
        tx.commit().map_err(map_sqlite)?;
        Ok(())
    }

    fn save_with_findings_and_output(
        &self,
        run: &AnalysisRun,
        findings: &[Finding],
        output: &serde_json::Value,
    ) -> Result<(), RepositoryError> {
        if run.status != RunStatus::Completed {
            return Err(RepositoryError::Invariant(
                "typed output may only be persisted for a completed analysis run".to_string(),
            ));
        }
        let output_json = to_json(output)?;
        let mut conn = self.db.conn().map_err(map_storage)?;
        let tx = conn.transaction().map_err(map_sqlite)?;
        validate_run_relationships(&tx, run)?;
        validate_findings_match_run(run, findings)?;
        upsert_run(&tx, run)?;
        delete_by_run(&tx, &run.id)?;
        for finding in findings {
            insert_finding(&tx, finding)?;
        }
        tx.execute(
            "INSERT INTO analysis_run_outputs (run_id, output_json)
             VALUES (?1, ?2)
             ON CONFLICT(run_id) DO UPDATE SET output_json = excluded.output_json",
            params![run.id.to_string(), output_json],
        )
        .map_err(map_sqlite)?;
        tx.commit().map_err(map_sqlite)?;
        Ok(())
    }

    fn find_output(
        &self,
        id: &AnalysisRunId,
    ) -> Result<Option<serde_json::Value>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT output_json FROM analysis_run_outputs WHERE run_id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?;
        raw.map(|json| {
            serde_json::from_str(&json).map_err(|error| RepositoryError::Storage(error.to_string()))
        })
        .transpose()
    }
}

impl RunHistoryRepository for SqliteAnalysisRunRepository<'_> {
    fn list_history_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<RunHistoryEntry>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let mut stmt = conn
            .prepare(
                "SELECT ar.id, ar.project_id, ar.source_revision_id, ar.tool_id,
                        ar.tool_version, ar.rule_set_version, ar.app_version, ar.status,
                        ar.started_at, ar.finished_at, ar.structure_diagnostics,
                        ar.overall_confidence, sr.sha256,
                        (SELECT COUNT(*) FROM findings f WHERE f.run_id = ar.id),
                        (SELECT COUNT(*) FROM exports e WHERE e.run_id = ar.id),
                        (SELECT COUNT(*) FROM ai_analyses a WHERE a.run_id = ar.id),
                        (SELECT newest.status
                         FROM ai_analyses newest
                         WHERE newest.run_id = ar.id
                         ORDER BY newest.created_at DESC, newest.id DESC
                         LIMIT 1)
                 FROM analysis_runs ar
                 INNER JOIN source_revisions sr ON sr.id = ar.source_revision_id
                 WHERE ar.project_id = ?1
                 ORDER BY ar.started_at DESC, ar.id DESC",
            )
            .map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![project_id.as_str()], map_history_row)
            .map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }
}

fn map_history_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunHistoryEntry> {
    let latest_ai_status: Option<AiAnalysisStatus> = row
        .get::<_, Option<String>>(16)?
        .map(|raw| from_json_sql(&format!("\"{raw}\"")))
        .transpose()?;
    Ok(RunHistoryEntry {
        run: map_run_row(row)?,
        source_sha256: Sha256Hash::from_hex(&row.get::<_, String>(12)?)
            .map_err(domain_to_sqlite)?,
        finding_count: nonnegative_count(row, 13)?,
        export_count: nonnegative_count(row, 14)?,
        ai_analysis_count: nonnegative_count(row, 15)?,
        latest_ai_status,
    })
}

fn nonnegative_count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = row.get::<_, i64>(index)?;
    u32::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

fn validate_run_relationships(
    conn: &rusqlite::Connection,
    run: &AnalysisRun,
) -> Result<(), RepositoryError> {
    let source_matches_project: bool = conn
        .query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM source_revisions
                 WHERE id = ?1 AND project_id = ?2
             )",
            params![run.source_revision_id.to_string(), run.project_id.as_str()],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    if !source_matches_project {
        return Err(RepositoryError::Invariant(format!(
            "source revision {} does not belong to project {}",
            run.source_revision_id, run.project_id
        )));
    }

    let existing_identity: Option<(String, String, String, String, String, String)> = conn
        .query_row(
            "SELECT project_id, source_revision_id, tool_id, tool_version,
                    rule_set_version, app_version
             FROM analysis_runs WHERE id = ?1",
            params![run.id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_sqlite)?;
    if let Some(existing) = existing_identity {
        let proposed = (
            run.project_id.to_string(),
            run.source_revision_id.to_string(),
            run.tool_id.clone(),
            run.tool_version.clone(),
            run.rule_set_version.clone(),
            run.app_version.clone(),
        );
        if existing != proposed {
            return Err(RepositoryError::Invariant(format!(
                "analysis run {} immutable identity cannot be changed",
                run.id
            )));
        }
    }
    Ok(())
}

fn validate_findings_match_run(
    run: &AnalysisRun,
    findings: &[Finding],
) -> Result<(), RepositoryError> {
    for finding in findings {
        if finding.run_id != run.id
            || finding.project_id != run.project_id
            || finding.source_revision_id != run.source_revision_id
            || finding.rule_set_version != run.rule_set_version
        {
            return Err(RepositoryError::Invariant(format!(
                "finding {} does not belong to analysis run {}",
                finding.id, run.id
            )));
        }
    }
    Ok(())
}

fn upsert_run(conn: &rusqlite::Connection, run: &AnalysisRun) -> Result<(), RepositoryError> {
    let diagnostics = match &run.structure_diagnostics {
        Some(d) => Some(to_json(d)?),
        None => None,
    };
    let status = enum_str(&run.status)?;
    conn.execute(
        "INSERT INTO analysis_runs (
            id, project_id, source_revision_id, tool_id, tool_version,
            rule_set_version, app_version, status, started_at, finished_at,
            structure_diagnostics, overall_confidence
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            finished_at = excluded.finished_at,
            structure_diagnostics = excluded.structure_diagnostics,
            overall_confidence = excluded.overall_confidence",
        params![
            run.id.to_string(),
            run.project_id.as_str(),
            run.source_revision_id.to_string(),
            run.tool_id,
            run.tool_version,
            run.rule_set_version,
            run.app_version,
            status,
            format_timestamp(run.started_at),
            run.finished_at.map(format_timestamp),
            diagnostics,
            run.overall_confidence.map(|c| c.value()),
        ],
    )
    .map_err(map_sqlite)?;
    Ok(())
}

fn map_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AnalysisRun> {
    let id = AnalysisRunId::parse(&row.get::<_, String>(0)?).map_err(domain_to_sqlite)?;
    let project_id = ProjectId::new(row.get::<_, String>(1)?).map_err(domain_to_sqlite)?;
    let source_revision_id =
        SourceRevisionId::parse(&row.get::<_, String>(2)?).map_err(domain_to_sqlite)?;
    let tool_id: String = row.get(3)?;
    let tool_version: String = row.get(4)?;
    let rule_set_version: String = row.get(5)?;
    let app_version: String = row.get(6)?;
    let status_raw: String = row.get(7)?;
    let status: RunStatus = from_json_sql(&format!("\"{status_raw}\""))?;
    let started_at = parse_timestamp(&row.get::<_, String>(8)?)?;
    let finished_at: Option<String> = row.get(9)?;
    let finished_at = match finished_at {
        Some(raw) => Some(parse_timestamp(&raw)?),
        None => None,
    };
    let structure_diagnostics: Option<WorkbookDiagnostics> = from_json_opt_sql(row.get(10)?)?;
    let overall_confidence: Option<f64> = row.get(11)?;
    let overall_confidence = match overall_confidence {
        Some(v) => Some(Confidence::new(v).map_err(domain_to_sqlite)?),
        None => None,
    };
    Ok(AnalysisRun {
        id,
        project_id,
        source_revision_id,
        tool_id,
        tool_version,
        rule_set_version,
        app_version,
        status,
        started_at,
        finished_at,
        structure_diagnostics,
        overall_confidence,
    })
}

fn enum_str<T: serde::Serialize>(value: &T) -> Result<String, RepositoryError> {
    let json = to_json(value)?;
    Ok(json.trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::ai_analyses::SqliteAiAnalysisRepository;
    use crate::database::test_db;
    use crate::exports::SqliteExportRepository;
    use crate::findings::SqliteFindingRepository;
    use crate::projects::SqliteProjectRepository;
    use crate::sources::SqliteSourceRevisionRepository;
    use jiff::Timestamp;
    use openconkit_application::{
        AiAnalysisRepository, ExportRepository, FindingRepository, ProjectRepository,
        RunHistoryRepository, SourceRevisionRepository,
    };
    use openconkit_domain::{
        AiAnalysis, AiAnalysisId, AiAnalysisLanguage, AiGroundingStatus, AiValidationStatus,
        ExportId, ExportKind, ExportRecord, FindingCategory, FindingId, FindingOrigin, Project,
        Severity, Sha256Hash, SourceRevision,
    };
    use std::collections::BTreeMap;

    fn seed_project(
        db: &Database,
        slug: &str,
        name: &str,
        hash_byte: u8,
    ) -> (ProjectId, SourceRevisionId) {
        let projects = SqliteProjectRepository::new(db);
        let project = Project::new(ProjectId::new(slug).expect("slug"), name, Timestamp::now())
            .expect("project");
        projects.save(&project).expect("save project");

        let sources = SqliteSourceRevisionRepository::new(db);
        let revision = SourceRevision::new(
            SourceRevisionId::new(),
            project.id().clone(),
            Sha256Hash::from_bytes([hash_byte; 32]),
            "boq.xlsx".into(),
            None,
            "sources/11/boq.xlsx".into(),
            100,
            Timestamp::now(),
            "boq-inspector".into(),
            None,
        )
        .expect("revision");
        sources.save(&revision).expect("save revision");
        (project.id().clone(), revision.id)
    }

    fn seed(db: &Database) -> (ProjectId, SourceRevisionId) {
        seed_project(db, "tower-a", "Tower A", 0x11)
    }

    fn sample_run(project_id: ProjectId, source_revision_id: SourceRevisionId) -> AnalysisRun {
        AnalysisRun {
            id: AnalysisRunId::new(),
            project_id,
            source_revision_id,
            tool_id: "boq-inspector".into(),
            tool_version: "0.1.0".into(),
            rule_set_version: "2026.07".into(),
            app_version: "0.0.1".into(),
            status: RunStatus::Completed,
            started_at: Timestamp::now(),
            finished_at: Some(Timestamp::now()),
            structure_diagnostics: None,
            overall_confidence: Some(Confidence::new(0.9).expect("ok")),
        }
    }

    fn sample_finding(run: &AnalysisRun) -> Finding {
        Finding {
            id: FindingId::new(),
            project_id: run.project_id.clone(),
            source_revision_id: run.source_revision_id,
            run_id: run.id,
            rule_id: "missing-unit-price".into(),
            rule_set_version: run.rule_set_version.clone(),
            category: FindingCategory::Omission,
            severity: Severity::High,
            confidence: Confidence::new(0.8).expect("ok"),
            title_key: "findings.missing_unit_price.title".into(),
            title_params: BTreeMap::new(),
            explanation_key: "findings.missing_unit_price.explanation".into(),
            explanation_params: BTreeMap::new(),
            suggested_action_key: None,
            suggested_action_params: BTreeMap::new(),
            sheet: Some("BOQ".into()),
            cell: None,
            range: None,
            source_row_id: Some("row-1".into()),
            original_value: None,
            original_formula: None,
            evidence: vec![],
            origin: FindingOrigin::Deterministic,
            created_at: Timestamp::now(),
        }
    }

    #[test]
    fn save_and_find_run() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let repo = SqliteAnalysisRunRepository::new(&db);
        let run = sample_run(project_id.clone(), source_id);
        repo.save(&run).expect("save");
        let found = repo.find_by_id(&run.id).expect("find").expect("present");
        assert_eq!(found.id, run.id);
        assert_eq!(found.status, RunStatus::Completed);
        assert_eq!(repo.list_by_project(&project_id).expect("list").len(), 1);
    }

    #[test]
    fn history_projection_includes_source_and_aggregate_status() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let runs = SqliteAnalysisRunRepository::new(&db);
        let run = sample_run(project_id.clone(), source_id);
        let finding = sample_finding(&run);
        runs.save_with_findings(&run, &[finding]).expect("save run");

        let exports = SqliteExportRepository::new(&db);
        exports
            .save(
                &ExportRecord::new(
                    ExportId::new(),
                    run.id,
                    ExportKind::Pdf,
                    "en".into(),
                    "run/export/report.pdf".into(),
                    Sha256Hash::from_bytes([0x44; 32]),
                    Timestamp::now(),
                )
                .expect("export"),
            )
            .expect("save export");

        let ai = SqliteAiAnalysisRepository::new(&db);
        ai.save(&AiAnalysis {
            id: AiAnalysisId::new(),
            run_id: run.id,
            model: "gpt-5-codex".into(),
            codex_version: "0.1.0".into(),
            language: AiAnalysisLanguage::En,
            input_scope_hash: Sha256Hash::from_bytes([0x55; 32]),
            status: AiAnalysisStatus::Completed,
            validation_status: AiValidationStatus::Unvalidated,
            grounding_status: AiGroundingStatus::Validated,
            output: Some(serde_json::json!({"summary": "grounded"})),
            created_at: Timestamp::now(),
        })
        .expect("save AI analysis");

        let history = runs
            .list_history_by_project(&project_id)
            .expect("list history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].run.id, run.id);
        assert_eq!(history[0].source_sha256, Sha256Hash::from_bytes([0x11; 32]));
        assert_eq!(history[0].finding_count, 1);
        assert_eq!(history[0].export_count, 1);
        assert_eq!(history[0].ai_analysis_count, 1);
        assert_eq!(
            history[0].latest_ai_status,
            Some(AiAnalysisStatus::Completed)
        );
    }

    #[test]
    fn save_with_findings_is_atomic() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let runs = SqliteAnalysisRunRepository::new(&db);
        let findings = SqliteFindingRepository::new(&db);
        let run = sample_run(project_id, source_id);
        let finding = sample_finding(&run);
        runs.save_with_findings(&run, std::slice::from_ref(&finding))
            .expect("save with findings");
        assert_eq!(findings.list_by_run(&run.id).expect("list").len(), 1);

        let finding2 = sample_finding(&run);
        runs.save_with_findings(&run, std::slice::from_ref(&finding2))
            .expect("replace");
        let listed = findings.list_by_run(&run.id).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, finding2.id);
    }

    #[test]
    fn completed_output_is_saved_with_the_run_aggregate() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let runs = SqliteAnalysisRunRepository::new(&db);
        let findings = SqliteFindingRepository::new(&db);
        let run = sample_run(project_id, source_id);
        let finding = sample_finding(&run);
        let output = serde_json::json!({
            "findings": [finding.clone()],
            "diagnostics": {"probe": true},
            "summary": {"finding_count": 1}
        });

        runs.save_with_findings_and_output(&run, std::slice::from_ref(&finding), &output)
            .expect("save aggregate");

        assert_eq!(
            runs.find_output(&run.id).expect("load output"),
            Some(output)
        );
        assert_eq!(findings.list_by_run(&run.id).expect("list").len(), 1);
    }

    #[test]
    fn output_write_rejects_non_completed_runs_without_partial_state() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let runs = SqliteAnalysisRunRepository::new(&db);
        let mut run = sample_run(project_id, source_id);
        run.status = RunStatus::Running;
        run.finished_at = None;

        let error = runs
            .save_with_findings_and_output(&run, &[], &serde_json::json!({}))
            .expect_err("running output rejected");

        assert!(matches!(error, RepositoryError::Invariant(_)));
        assert!(runs.find_by_id(&run.id).expect("find run").is_none());
        assert!(runs.find_output(&run.id).expect("find output").is_none());
    }

    #[test]
    fn run_rejects_source_from_another_project() {
        let db = test_db();
        let (project_a, _) = seed_project(&db, "tower-a", "Tower A", 0x11);
        let (_, source_b) = seed_project(&db, "tower-b", "Tower B", 0x22);
        let runs = SqliteAnalysisRunRepository::new(&db);
        let run = sample_run(project_a, source_b);

        let error = runs.save(&run).expect_err("cross-project source rejected");
        assert!(matches!(error, RepositoryError::Invariant(_)));
        assert!(runs.find_by_id(&run.id).expect("find").is_none());
    }

    #[test]
    fn mismatched_finding_rejects_entire_aggregate() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let runs = SqliteAnalysisRunRepository::new(&db);
        let run = sample_run(project_id, source_id);
        let mut finding = sample_finding(&run);
        finding.run_id = AnalysisRunId::new();

        let error = runs
            .save_with_findings(&run, &[finding])
            .expect_err("mismatched finding rejected");
        assert!(matches!(error, RepositoryError::Invariant(_)));
        assert!(runs.find_by_id(&run.id).expect("find").is_none());
    }

    #[test]
    fn existing_run_identity_is_immutable() {
        let db = test_db();
        let (project_id, source_id) = seed(&db);
        let sources = SqliteSourceRevisionRepository::new(&db);
        let other_source = SourceRevision::new(
            SourceRevisionId::new(),
            project_id.clone(),
            Sha256Hash::from_bytes([0x33; 32]),
            "other.xlsx".into(),
            None,
            "sources/33/other.xlsx".into(),
            100,
            Timestamp::now(),
            "boq-inspector".into(),
            None,
        )
        .expect("revision");
        sources.save(&other_source).expect("save revision");

        let runs = SqliteAnalysisRunRepository::new(&db);
        let mut run = sample_run(project_id, source_id);
        runs.save(&run).expect("save");
        run.source_revision_id = other_source.id;

        let error = runs.save(&run).expect_err("identity change rejected");
        assert!(matches!(error, RepositoryError::Invariant(_)));
        let persisted = runs.find_by_id(&run.id).expect("find").expect("present");
        assert_eq!(persisted.source_revision_id, source_id);
    }
}
