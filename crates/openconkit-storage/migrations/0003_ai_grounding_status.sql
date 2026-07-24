-- Migration 3: persist machine grounding validation separately from human review.
-- Append-only: one transaction is supplied by the migration runner.

ALTER TABLE ai_analyses
ADD COLUMN grounding_status TEXT NOT NULL DEFAULT 'pending'
CHECK (grounding_status IN ('pending', 'validated', 'rejected'));
