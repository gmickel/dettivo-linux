-- 0002: the segments with word timestamps an import or re-run produces
-- (a JSON array of the contract's Segment shape) and the notice an
-- empty transcript carries (`silent`). Both are NULL on rows the
-- dictation session stores.
ALTER TABLE dictations ADD COLUMN segments TEXT;
ALTER TABLE dictations ADD COLUMN notice TEXT;
