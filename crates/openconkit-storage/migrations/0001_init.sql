-- Migration 1: initial schema.
-- Append-only: never edit released migrations; add a new file instead.
-- Pre-release: this file is the full v0.0.1 schema.
-- PRAGMAs (foreign_keys, journal_mode) are set on every open in Database::open.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    archived_at     TEXT,
    description     TEXT,
    client          TEXT,
    location        TEXT
);

CREATE TABLE IF NOT EXISTS source_revisions (
    id                  TEXT PRIMARY KEY NOT NULL,
    project_id          TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sha256              TEXT NOT NULL,
    original_filename   TEXT NOT NULL,
    original_path       TEXT,
    stored_path         TEXT NOT NULL,
    size_bytes          INTEGER NOT NULL CHECK (size_bytes >= 0),
    imported_at         TEXT NOT NULL,
    tool_id             TEXT NOT NULL,
    workbook_metadata   TEXT,
    UNIQUE (project_id, sha256)
);

CREATE INDEX IF NOT EXISTS idx_source_revisions_project
    ON source_revisions(project_id);

CREATE TABLE IF NOT EXISTS analysis_runs (
    id                      TEXT PRIMARY KEY NOT NULL,
    project_id              TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_revision_id      TEXT NOT NULL REFERENCES source_revisions(id) ON DELETE CASCADE,
    tool_id                 TEXT NOT NULL,
    tool_version            TEXT NOT NULL,
    rule_set_version        TEXT NOT NULL,
    app_version             TEXT NOT NULL,
    status                  TEXT NOT NULL,
    started_at              TEXT NOT NULL,
    finished_at             TEXT,
    structure_diagnostics   TEXT,
    overall_confidence      REAL
);

CREATE INDEX IF NOT EXISTS idx_analysis_runs_project
    ON analysis_runs(project_id);
CREATE INDEX IF NOT EXISTS idx_analysis_runs_source
    ON analysis_runs(source_revision_id);

CREATE TABLE IF NOT EXISTS findings (
    id                          TEXT PRIMARY KEY NOT NULL,
    project_id                  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_revision_id          TEXT NOT NULL REFERENCES source_revisions(id) ON DELETE CASCADE,
    run_id                      TEXT NOT NULL REFERENCES analysis_runs(id) ON DELETE CASCADE,
    rule_id                     TEXT NOT NULL,
    rule_set_version            TEXT NOT NULL,
    category                    TEXT NOT NULL,
    severity                    TEXT NOT NULL,
    confidence                  REAL NOT NULL,
    title_key                   TEXT NOT NULL,
    title_params                TEXT NOT NULL DEFAULT '{}',
    explanation_key             TEXT NOT NULL,
    explanation_params          TEXT NOT NULL DEFAULT '{}',
    suggested_action_key        TEXT,
    suggested_action_params     TEXT NOT NULL DEFAULT '{}',
    sheet                       TEXT,
    cell                        TEXT,
    range_start                 TEXT,
    range_end                   TEXT,
    source_row_id               TEXT,
    original_value              TEXT,
    original_formula            TEXT,
    evidence                    TEXT NOT NULL DEFAULT '[]',
    origin                      TEXT NOT NULL,
    created_at                  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_findings_run
    ON findings(run_id);

CREATE TABLE IF NOT EXISTS exports (
    id              TEXT PRIMARY KEY NOT NULL,
    run_id          TEXT NOT NULL REFERENCES analysis_runs(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL,
    language        TEXT NOT NULL,
    relative_path   TEXT NOT NULL,
    sha256          TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_exports_run
    ON exports(run_id);

CREATE TABLE IF NOT EXISTS ai_analyses (
    id                  TEXT PRIMARY KEY NOT NULL,
    run_id              TEXT NOT NULL REFERENCES analysis_runs(id) ON DELETE CASCADE,
    model               TEXT NOT NULL,
    codex_version       TEXT NOT NULL,
    input_scope_hash    TEXT NOT NULL,
    status              TEXT NOT NULL,
    validation_status   TEXT NOT NULL,
    output              TEXT,
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_analyses_run
    ON ai_analyses(run_id);
