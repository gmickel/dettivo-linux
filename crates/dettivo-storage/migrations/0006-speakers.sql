-- 0006: speakers (ADR 0035). The diarization block lives on the meeting
-- row as JSON, the speakers of a meeting in their own table (id, name,
-- swatch index, talk time, order of first appearance), and the names
-- given through renames in `speaker_names` for the next meeting's
-- suggestions. `speaker_names` on the row (the names joined) joins the
-- FTS index so a search finds a meeting by who spoke in it.
ALTER TABLE meetings ADD COLUMN diarization TEXT;
ALTER TABLE meetings ADD COLUMN speaker_names TEXT NOT NULL DEFAULT '';

CREATE TABLE meeting_speakers (
    meeting_id TEXT NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    speaker_id TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    color_index INTEGER NOT NULL DEFAULT 0,
    talk_ms INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (meeting_id, speaker_id)
);

CREATE TABLE speaker_names (
    name TEXT PRIMARY KEY,
    last_used_at TEXT NOT NULL,
    uses INTEGER NOT NULL DEFAULT 1
);

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
    content = 'meetings',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER meetings_ai AFTER INSERT ON meetings BEGIN
    INSERT INTO meetings_fts (rowid, raw_text, final_text, title, summary, speaker_names)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary, new.speaker_names);
END;

CREATE TRIGGER meetings_ad AFTER DELETE ON meetings BEGIN
    INSERT INTO meetings_fts (meetings_fts, rowid, raw_text, final_text, title, summary, speaker_names)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary, old.speaker_names);
END;

CREATE TRIGGER meetings_au AFTER UPDATE ON meetings BEGIN
    INSERT INTO meetings_fts (meetings_fts, rowid, raw_text, final_text, title, summary, speaker_names)
    VALUES ('delete', old.seq, old.raw_text, old.final_text, old.title, old.summary, old.speaker_names);
    INSERT INTO meetings_fts (rowid, raw_text, final_text, title, summary, speaker_names)
    VALUES (new.seq, new.raw_text, new.final_text, new.title, new.summary, new.speaker_names);
END;

INSERT INTO meetings_fts (meetings_fts) VALUES ('rebuild');
