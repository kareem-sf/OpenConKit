CREATE TABLE analysis_run_outputs (
    run_id       TEXT PRIMARY KEY NOT NULL
                 REFERENCES analysis_runs(id) ON DELETE CASCADE,
    output_json  TEXT NOT NULL
);
