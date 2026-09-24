//! The speakers of a meeting (migration `0006-speakers`, ADR 0035): the
//! rows behind `MeetingRow::speakers`, replaced whole whenever the row is
//! written, and the names given through renames, remembered for the next
//! meeting's suggestions (`meetings.speakers.suggest`).

use dettivo_proto::methods::speakers::{Speaker, Suggestion};
use rusqlite::params;

use crate::time::now_iso;
use crate::{Store, StoreError};

/// The most suggestions `suggest_speaker_names` returns when asked for
/// none in particular.
pub const DEFAULT_SUGGESTIONS: u32 = 10;

impl Store {
    /// The speakers of `meeting_id`, in order of first appearance.
    pub fn speakers_of(&self, meeting_id: &str) -> Result<Vec<Speaker>, StoreError> {
        let mut stmt = self.conn().prepare(
            "SELECT speaker_id, name, color_index, talk_ms FROM meeting_speakers \
             WHERE meeting_id = ?1 ORDER BY position, speaker_id",
        )?;
        let rows = stmt.query_map(params![meeting_id], |r| {
            Ok(Speaker {
                speaker_id: r.get(0)?,
                name: r.get(1)?,
                color_index: r.get::<_, i64>(2).map(|n| n.max(0) as u32)?,
                talk_ms: r.get::<_, i64>(3).map(|n| n.max(0) as u64)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Replaces the speakers of `meeting_id` with `speakers`, in order.
    pub(crate) fn replace_speakers(
        &self,
        meeting_id: &str,
        speakers: &[Speaker],
    ) -> Result<(), StoreError> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM meeting_speakers WHERE meeting_id = ?1",
            params![meeting_id],
        )?;
        for (position, s) in speakers.iter().enumerate() {
            conn.execute(
                "INSERT INTO meeting_speakers \
                 (meeting_id, speaker_id, name, color_index, talk_ms, position) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    meeting_id,
                    s.speaker_id,
                    s.name,
                    i64::from(s.color_index),
                    i64::try_from(s.talk_ms).unwrap_or(i64::MAX),
                    position as i64
                ],
            )?;
        }
        Ok(())
    }

    /// Remembers `name` as used now (a rename), for the suggestions.
    pub fn remember_speaker_name(&self, name: &str) -> Result<(), StoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(());
        }
        self.conn().execute(
            "INSERT INTO speaker_names (name, last_used_at, uses) VALUES (?1, ?2, 1) \
             ON CONFLICT (name) DO UPDATE SET last_used_at = excluded.last_used_at, \
             uses = uses + 1",
            params![name, now_iso()],
        )?;
        Ok(())
    }

    /// The remembered names, most recently used first, narrowed to those
    /// starting with `prefix` (case-insensitive) when given.
    pub fn suggest_speaker_names(
        &self,
        prefix: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Suggestion>, StoreError> {
        let prefix = prefix.unwrap_or("").trim().to_lowercase();
        let mut stmt = self.conn().prepare(
            "SELECT name, last_used_at, uses FROM speaker_names \
             WHERE lower(name) LIKE ?1 ESCAPE '\\' \
             ORDER BY last_used_at DESC, uses DESC, name LIMIT ?2",
        )?;
        let pattern = format!(
            "{}%",
            prefix
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let rows = stmt.query_map(params![pattern, i64::from(limit.clamp(1, 1000))], |r| {
            Ok(Suggestion {
                name: r.get(0)?,
                last_used_at: r.get(1)?,
                uses: r.get::<_, i64>(2).map(|n| n.max(0) as u32)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::MeetingRow;

    #[test]
    fn speakers_follow_the_row_and_names_are_remembered_by_recency() {
        let store = Store::in_memory().unwrap();
        let mut m = MeetingRow::new();
        m.speakers = vec![
            Speaker {
                speaker_id: "you".into(),
                name: "You".into(),
                color_index: 0,
                talk_ms: 4000,
            },
            Speaker {
                speaker_id: "speaker_00".into(),
                name: "Speaker 1".into(),
                color_index: 1,
                talk_ms: 9000,
            },
        ];
        store.insert_meeting(&m).unwrap();
        let back = store.get_meeting(&m.id).unwrap().unwrap();
        assert_eq!(back.speakers, m.speakers);
        m.speakers[1].name = "Ada".into();
        store.update_meeting(&m).unwrap();
        assert_eq!(store.speakers_of(&m.id).unwrap()[1].name, "Ada");
        let hits = store.search_meetings("ada", 5).unwrap();
        assert_eq!(hits.len(), 1, "the speaker names join the FTS index");
        store.remember_speaker_name("Ada").unwrap();
        store.remember_speaker_name("Grace").unwrap();
        store.remember_speaker_name("Ada").unwrap();
        let names: Vec<(String, u32)> = store
            .suggest_speaker_names(None, DEFAULT_SUGGESTIONS)
            .unwrap()
            .into_iter()
            .map(|s| (s.name, s.uses))
            .collect();
        assert_eq!(names, [("Ada".to_string(), 2), ("Grace".to_string(), 1)]);
        let only = store.suggest_speaker_names(Some("gr"), 5).unwrap();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].name, "Grace");
        assert!(
            store
                .suggest_speaker_names(Some("%"), 5)
                .unwrap()
                .is_empty()
        );
        store.delete_meeting_row(&m.id).unwrap();
        assert!(store.speakers_of(&m.id).unwrap().is_empty());
    }
}
