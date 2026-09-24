-- 0008: `queued` as an analysis status (ADR 0061). The finalisation marks
-- the analysis it plans on the completed row before the row is stored, so
-- a client that reloads on `completed` sees the pass that is about to
-- run. SQLite cannot widen a column CHECK in place: the column is
-- recreated with the wider set, the statuses copied over, and the old
-- column dropped (nothing else in the schema names it).
ALTER TABLE meetings ADD COLUMN analysis_status_next TEXT NOT NULL DEFAULT 'none'
    CHECK (analysis_status_next IN ('none', 'queued', 'running', 'ready', 'failed'));
UPDATE meetings SET analysis_status_next = analysis_status;
ALTER TABLE meetings DROP COLUMN analysis_status;
ALTER TABLE meetings RENAME COLUMN analysis_status_next TO analysis_status;
