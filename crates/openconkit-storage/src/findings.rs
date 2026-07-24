//! SQLite adapter for [`FindingRepository`].

use std::collections::BTreeMap;

use openconkit_application::{FindingRepository, RepositoryError};
use openconkit_domain::{
    AnalysisRunId, CellRange, CellRef, Confidence, Evidence, Finding, FindingCategory, FindingId,
    FindingOrigin, ProjectId, Severity, SourceRevisionId,
};
use rusqlite::params;

use crate::codecs::{
    domain_to_sqlite, format_timestamp, from_json_sql, map_sqlite, map_storage, parse_timestamp,
    to_json,
};
use crate::database::Database;

/// SQLite-backed [`FindingRepository`].
pub struct SqliteFindingRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteFindingRepository<'a> {
    /// Borrow a database handle.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl FindingRepository for SqliteFindingRepository<'_> {
    fn list_by_run(&self, run_id: &AnalysisRunId) -> Result<Vec<Finding>, RepositoryError> {
        let conn = self.db.conn().map_err(map_storage)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, source_revision_id, run_id, rule_id, rule_set_version,
                        category, severity, confidence, title_key, title_params,
                        explanation_key, explanation_params, suggested_action_key,
                        suggested_action_params, sheet, cell, range_start, range_end,
                        source_row_id, original_value, original_formula, evidence,
                        origin, created_at
                 FROM findings
                 WHERE run_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![run_id.to_string()], map_finding_row)
            .map_err(map_sqlite)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sqlite)?);
        }
        Ok(out)
    }
}

/// Insert one finding (used inside a transaction by the run repository).
pub(crate) fn insert_finding(
    conn: &rusqlite::Connection,
    finding: &Finding,
) -> Result<(), RepositoryError> {
    let range_start = finding.range.as_ref().map(|r| r.start.as_str().to_string());
    let range_end = finding.range.as_ref().map(|r| r.end.as_str().to_string());
    conn.execute(
        "INSERT INTO findings (
            id, project_id, source_revision_id, run_id, rule_id, rule_set_version,
            category, severity, confidence, title_key, title_params,
            explanation_key, explanation_params, suggested_action_key,
            suggested_action_params, sheet, cell, range_start, range_end,
            source_row_id, original_value, original_formula, evidence,
            origin, created_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25
         )",
        params![
            finding.id.to_string(),
            finding.project_id.as_str(),
            finding.source_revision_id.to_string(),
            finding.run_id.to_string(),
            finding.rule_id,
            finding.rule_set_version,
            enum_str(&finding.category)?,
            enum_str(&finding.severity)?,
            finding.confidence.value(),
            finding.title_key,
            to_json(&finding.title_params)?,
            finding.explanation_key,
            to_json(&finding.explanation_params)?,
            finding.suggested_action_key,
            to_json(&finding.suggested_action_params)?,
            finding.sheet,
            finding.cell.as_ref().map(|c| c.as_str().to_string()),
            range_start,
            range_end,
            finding.source_row_id,
            finding.original_value,
            finding.original_formula,
            to_json(&finding.evidence)?,
            enum_str(&finding.origin)?,
            format_timestamp(finding.created_at),
        ],
    )
    .map_err(map_sqlite)?;
    Ok(())
}

/// Delete all findings for a run (used inside a transaction).
pub(crate) fn delete_by_run(
    conn: &rusqlite::Connection,
    run_id: &AnalysisRunId,
) -> Result<(), RepositoryError> {
    conn.execute(
        "DELETE FROM findings WHERE run_id = ?1",
        params![run_id.to_string()],
    )
    .map_err(map_sqlite)?;
    Ok(())
}

fn map_finding_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Finding> {
    let id = FindingId::parse(&row.get::<_, String>(0)?).map_err(domain_to_sqlite)?;
    let project_id = ProjectId::new(row.get::<_, String>(1)?).map_err(domain_to_sqlite)?;
    let source_revision_id =
        SourceRevisionId::parse(&row.get::<_, String>(2)?).map_err(domain_to_sqlite)?;
    let run_id = AnalysisRunId::parse(&row.get::<_, String>(3)?).map_err(domain_to_sqlite)?;
    let rule_id: String = row.get(4)?;
    let rule_set_version: String = row.get(5)?;
    let category: FindingCategory = enum_from_str(&row.get::<_, String>(6)?)?;
    let severity: Severity = enum_from_str(&row.get::<_, String>(7)?)?;
    let confidence = Confidence::new(row.get::<_, f64>(8)?).map_err(domain_to_sqlite)?;
    let title_key: String = row.get(9)?;
    let title_params: BTreeMap<String, String> = from_json_sql(&row.get::<_, String>(10)?)?;
    let explanation_key: String = row.get(11)?;
    let explanation_params: BTreeMap<String, String> = from_json_sql(&row.get::<_, String>(12)?)?;
    let suggested_action_key: Option<String> = row.get(13)?;
    let suggested_action_params: BTreeMap<String, String> =
        from_json_sql(&row.get::<_, String>(14)?)?;
    let sheet: Option<String> = row.get(15)?;
    let cell: Option<String> = row.get(16)?;
    let cell = match cell {
        Some(raw) => Some(CellRef::new(&raw).map_err(domain_to_sqlite)?),
        None => None,
    };
    let range_start: Option<String> = row.get(17)?;
    let range_end: Option<String> = row.get(18)?;
    let range = match (range_start, range_end) {
        (Some(start), Some(end)) => Some(
            CellRange::new(
                CellRef::new(&start).map_err(domain_to_sqlite)?,
                CellRef::new(&end).map_err(domain_to_sqlite)?,
            )
            .map_err(domain_to_sqlite)?,
        ),
        (None, None) => None,
        _ => {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "finding range columns are inconsistent",
                ),
            )))
        }
    };
    let source_row_id: Option<String> = row.get(19)?;
    let original_value: Option<String> = row.get(20)?;
    let original_formula: Option<String> = row.get(21)?;
    let evidence: Vec<Evidence> = from_json_sql(&row.get::<_, String>(22)?)?;
    let origin: FindingOrigin = enum_from_str(&row.get::<_, String>(23)?)?;
    let created_at = parse_timestamp(&row.get::<_, String>(24)?)?;
    Ok(Finding {
        id,
        project_id,
        source_revision_id,
        run_id,
        rule_id,
        rule_set_version,
        category,
        severity,
        confidence,
        title_key,
        title_params,
        explanation_key,
        explanation_params,
        suggested_action_key,
        suggested_action_params,
        sheet,
        cell,
        range,
        source_row_id,
        original_value,
        original_formula,
        evidence,
        origin,
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
