-- 0005: the split of the path from the key release to the inserted text
-- (capture, transcribe, insert, in milliseconds), the numbers the history
-- detail shows as the stop-to-insert time. Stored as JSON; NULL on every
-- row written before it and on imports and re-runs, which insert nothing.
ALTER TABLE dictations ADD COLUMN timings TEXT;
