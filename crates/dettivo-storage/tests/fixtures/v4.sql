-- Schema version 4 as the daemon created it, plus one dictation and one
-- meeting; the migration test restores this dump and opens it with the
-- current code, which rebuilds the meeting FTS index over the row.
CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);
INSERT INTO schema_migrations (version, name, applied_at) VALUES (1, '0001-dictations', '2026-09-04T00:00:00Z');
INSERT INTO schema_migrations (version, name, applied_at) VALUES (2, '0002-segments', '2026-09-04T12:00:00Z');
INSERT INTO schema_migrations (version, name, applied_at) VALUES (3, '0003-polish-policy', '2026-09-04T18:00:00Z');
INSERT INTO schema_migrations (version, name, applied_at) VALUES (4, '0004-meetings', '2026-09-05T00:00:00Z');
-- 0001: dictation items with the macOS DictationItem fields, and the FTS5
-- index over their text kept in step by triggers. `seq` is the rowid the
-- external-content FTS table points at; `id` is the contract's UUID.
CREATE TABLE dictations (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    app_id TEXT NOT NULL DEFAULT '',
    app_name TEXT NOT NULL DEFAULT '',
    source_kind TEXT NOT NULL CHECK (source_kind IN ('dictation', 'audioImport', 'rerun')),
    status TEXT NOT NULL CHECK (status IN ('transcribing', 'completed', 'failed')),
    mode TEXT NOT NULL,
    stt_provider TEXT NOT NULL,
    stt_model TEXT NOT NULL,
    language TEXT NOT NULL,
    raw_text TEXT NOT NULL DEFAULT '',
    final_text TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    rerun_of_item_id TEXT REFERENCES dictations(id) ON DELETE SET NULL,
    error_code TEXT,
    error_message TEXT,
    audio_path TEXT,
    insertion TEXT,
    segments TEXT,
    notice TEXT,
    policy_hash TEXT
);

CREATE INDEX dictations_created ON dictations (created_at DESC, id);
CREATE INDEX dictations_app ON dictations (app_id, created_at DESC);
CREATE INDEX dictations_rerun ON dictations (rerun_of_item_id);

CREATE VIRTUAL TABLE dictations_fts USING fts5(
    raw_text,
    final_text,
    title,
    summary,
    content = 'dictations',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER dictations_ai AFTER INSERT ON dictations BEGIN
    INSERT INTO dictations_fts (rowid, raw_text, final_text, title, summary)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary);
END;

CREATE TRIGGER dictations_ad AFTER DELETE ON dictations BEGIN
    INSERT INTO dictations_fts (dictations_fts, rowid, raw_text, final_text, title, summary)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary);
END;

CREATE TRIGGER dictations_au AFTER UPDATE ON dictations BEGIN
    INSERT INTO dictations_fts (dictations_fts, rowid, raw_text, final_text, title, summary)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary);
    INSERT INTO dictations_fts (rowid, raw_text, final_text, title, summary)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary);
END;
INSERT INTO dictations (id, created_at, updated_at, app_id, app_name, source_kind, status, mode, stt_provider, stt_model, language, raw_text, final_text, title, summary, duration_ms, rerun_of_item_id, error_code, error_message, audio_path, insertion)
VALUES ('11111111-1111-4111-8111-111111111111', '2026-09-04T00:00:00Z', '2026-09-04T00:00:00Z', 'org.gnome.TextEditor', 'Text Editor', 'dictation', 'completed', 'raw', 'whisper', 'tiny.en', 'en', 'hello comma world', 'Hello, world.', 'Hello, world.', '', 1200, NULL, NULL, NULL, NULL, '{"outcome":"inserted","method":"paste"}');
-- 0004: meeting sessions with the macOS MeetingSession fields (ADR 0027):
-- the lifecycle status, the capture facts (takes, the system track, the
-- meeting directory), the partial-recovery fields a daemon restart fills
-- from the live checkpoint, the disclosure acknowledgement, and the
-- transcript columns the transcription spec fills. The FTS5 index over
-- title, texts and summary mirrors the dictation one so `meetings.search`
-- and the meeting half of `transcripts.search` run the same query.
CREATE TABLE meetings (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    title_source TEXT NOT NULL DEFAULT 'auto' CHECK (title_source IN ('auto', 'manual')),
    status TEXT NOT NULL CHECK (status IN ('recording', 'stopping', 'stopped', 'transcribing',
        'completed', 'failed', 'cancelled', 'partial')),
    source_kind TEXT NOT NULL DEFAULT 'capture' CHECK (source_kind IN ('capture', 'audioImport')),
    started_at TEXT NOT NULL,
    ended_at TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    language TEXT NOT NULL DEFAULT '',
    stt_provider TEXT NOT NULL DEFAULT '',
    stt_model TEXT NOT NULL DEFAULT '',
    is_partial INTEGER NOT NULL DEFAULT 0,
    chunks_completed INTEGER NOT NULL DEFAULT 0,
    chunks_total INTEGER NOT NULL DEFAULT 0,
    disclosure_acknowledged_at TEXT,
    system_audio INTEGER NOT NULL DEFAULT 0,
    microphone_takes INTEGER NOT NULL DEFAULT 0,
    audio_dir TEXT,
    raw_text TEXT NOT NULL DEFAULT '',
    final_text TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    segments TEXT,
    error_code TEXT,
    error_message TEXT,
    original_filename TEXT,
    recovery_reason TEXT
);

CREATE INDEX meetings_created ON meetings (created_at DESC, id);
CREATE INDEX meetings_status ON meetings (status);

CREATE VIRTUAL TABLE meetings_fts USING fts5(
    raw_text,
    final_text,
    title,
    summary,
    content = 'meetings',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER meetings_ai AFTER INSERT ON meetings BEGIN
    INSERT INTO meetings_fts (rowid, raw_text, final_text, title, summary)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary);
END;

CREATE TRIGGER meetings_ad AFTER DELETE ON meetings BEGIN
    INSERT INTO meetings_fts (meetings_fts, rowid, raw_text, final_text, title, summary)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary);
END;

CREATE TRIGGER meetings_au AFTER UPDATE ON meetings BEGIN
    INSERT INTO meetings_fts (meetings_fts, rowid, raw_text, final_text, title, summary)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary);
    INSERT INTO meetings_fts (rowid, raw_text, final_text, title, summary)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary);
END;
INSERT INTO meetings (id, created_at, updated_at, title, title_source, status, source_kind, started_at, ended_at, duration_ms, language, stt_provider, stt_model, raw_text, final_text, summary, segments)
VALUES ('22222222-2222-4222-8222-222222222222', '2026-09-04T10:00:00Z', '2026-09-04T10:30:00Z', 'Budget sync', 'manual', 'completed', 'capture', '2026-09-04T10:00:00Z', '2026-09-04T10:30:00Z', 1800000, 'en', 'whisper', 'tiny.en', 'the budget is agreed', 'The budget is agreed.', '', '[{"index":0,"start_ms":0,"end_ms":1500,"text":"The budget is agreed.","speaker":null,"source_type":"microphone"}]');
