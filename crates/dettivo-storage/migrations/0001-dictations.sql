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
    insertion TEXT
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
