-- Migration 4: retain the requested AI output language for export selection.
-- Append-only: one transaction is supplied by the migration runner.

ALTER TABLE ai_analyses
ADD COLUMN language TEXT NOT NULL DEFAULT 'en'
CHECK (language IN ('en', 'ar'));
