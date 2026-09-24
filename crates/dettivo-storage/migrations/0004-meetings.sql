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
