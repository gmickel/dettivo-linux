-- 0007: notes and analysis on the meeting row (ADR 0036). The notes are
-- the user's Markdown (`notes.md` beside the audio is the copy a person
-- opens; the row is what search and export read), the analysis is the
-- model's summary, decisions and action items as JSON with its status,
-- error, model and time, and `analysis_text` is the flattened analysis
-- the FTS index reads. The FTS5 index is rebuilt over title, texts,
-- summary, speaker names (`0006-speakers`), notes and analysis so
-- `meetings.search` finds a word in any of them and names the column it
-- matched.
ALTER TABLE meetings ADD COLUMN notes_markdown TEXT NOT NULL DEFAULT '';
ALTER TABLE meetings ADD COLUMN notes_source TEXT NOT NULL DEFAULT 'user'
    CHECK (notes_source IN ('user', 'live'));
ALTER TABLE meetings ADD COLUMN notes_updated_at TEXT;
ALTER TABLE meetings ADD COLUMN analysis TEXT;
ALTER TABLE meetings ADD COLUMN analysis_text TEXT NOT NULL DEFAULT '';
ALTER TABLE meetings ADD COLUMN analysis_status TEXT NOT NULL DEFAULT 'none'
    CHECK (analysis_status IN ('none', 'running', 'ready', 'failed'));
ALTER TABLE meetings ADD COLUMN analysis_error TEXT;
ALTER TABLE meetings ADD COLUMN analysis_model TEXT;
ALTER TABLE meetings ADD COLUMN analysis_at TEXT;

DROP TRIGGER meetings_ai;
DROP TRIGGER meetings_ad;
DROP TRIGGER meetings_au;
DROP TABLE meetings_fts;

CREATE VIRTUAL TABLE meetings_fts USING fts5(
    raw_text,
    final_text,
    title,
    summary,
    speaker_names,
    notes_markdown,
    analysis_text,
    content = 'meetings',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER meetings_ai AFTER INSERT ON meetings BEGIN
    INSERT INTO meetings_fts (rowid, raw_text, final_text, title, summary, speaker_names, notes_markdown, analysis_text)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary, new.speaker_names, new.notes_markdown, new.analysis_text);
END;

CREATE TRIGGER meetings_ad AFTER DELETE ON meetings BEGIN
    INSERT INTO meetings_fts (meetings_fts, rowid, raw_text, final_text, title, summary, speaker_names, notes_markdown, analysis_text)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary, old.speaker_names, old.notes_markdown, old.analysis_text);
END;

CREATE TRIGGER meetings_au AFTER UPDATE ON meetings BEGIN
    INSERT INTO meetings_fts (meetings_fts, rowid, raw_text, final_text, title, summary, speaker_names, notes_markdown, analysis_text)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary, old.speaker_names, old.notes_markdown, old.analysis_text);
    INSERT INTO meetings_fts (rowid, raw_text, final_text, title, summary, speaker_names, notes_markdown, analysis_text)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary, new.speaker_names, new.notes_markdown, new.analysis_text);
END;

INSERT INTO meetings_fts (meetings_fts) VALUES ('rebuild');
