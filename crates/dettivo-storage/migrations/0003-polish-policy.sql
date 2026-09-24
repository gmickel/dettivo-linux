-- 0003: the Polish layers (ADR 0023). A dictation now records the hash of
-- the policy it ran under; the `notice` column from 0002 also carries,
-- when the Enhanced pass inserted the deterministic result, the reason
-- why (`{ kind, reason }`). NULL on every row written before the layers.
ALTER TABLE dictations ADD COLUMN policy_hash TEXT;
