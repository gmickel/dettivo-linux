//! The one timeline over both kinds (`transcripts.list` with dictations
//! and meetings), the meeting side of search, and the stale rows a daemon
//! restart settles. The timeline is keyset-paged with a cursor of
//! `created_at|kind|seq`: newest first, a dictation before a meeting that
//! started at the same instant, then by row order.

use rusqlite::params;

use crate::dictations::{ListFilter, from_row as dictation_from_row};
use crate::item::DictationItem;
use crate::meeting_reads::{MeetingSummary, SUMMARY_COLUMNS, summary_from_row};
use crate::meetings::MeetingStatus;
use crate::search::fts_expression;
use crate::time::now_iso;
use crate::{Store, StoreError};

/// One row of the timeline: a dictation or the display fields of a meeting.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// A dictation item.
    Dictation(Box<DictationItem>),
    /// A meeting.
    Meeting(Box<MeetingSummary>),
}

impl Entry {
    fn created_at(&self) -> &str {
        match self {
            Self::Dictation(d) => &d.created_at,
            Self::Meeting(m) => &m.created_at,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Dictation(_) => "dictation",
            Self::Meeting(_) => "meeting",
        }
    }
}

/// A page of the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelinePage {
    /// The rows, newest first.
    pub items: Vec<Entry>,
    /// The cursor for the next page, `None` when this was the last.
    pub next_cursor: Option<String>,
}

/// A meeting search hit.
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingHit {
    /// The matched meeting identifier.
    pub id: String,
    /// The matching text with an ellipsis where it was cut.
    pub snippet: String,
    /// The column the snippet came from: `title`, `transcript`, `notes`,
    /// `analysis` or `speakers`.
    pub matched_field: &'static str,
}

/// The FTS columns in the order a hit is attributed: the title first,
/// then the transcript (the polished text, the raw text, the summary),
/// the notes, the analysis and the speaker names. The index is the
/// column's position in `meetings_fts` (`0007-notes-analysis`).
pub(crate) const MATCH_ORDER: &[(usize, &str)] = &[
    (2, "title"),
    (1, "transcript"),
    (0, "transcript"),
    (5, "notes"),
    (6, "analysis"),
    (3, "analysis"),
    (4, "speakers"),
];

/// A private-use character that marks the matched terms in a snippet so
/// the column that matched can be told apart from one that merely has
/// text; it is stripped before the snippet reaches the wire.
pub(crate) const MARK: &str = "\u{E000}";

fn parse_cursor(cursor: &str) -> Result<(String, String, i64), StoreError> {
    let bad =
        || StoreError::InvalidQuery(format!("cursor {cursor:?} is not one this store issued"));
    let mut parts = cursor.splitn(3, '|');
    let created = parts.next().ok_or_else(bad)?.to_string();
    let kind = parts.next().ok_or_else(bad)?.to_string();
    let seq: i64 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    if kind != "dictation" && kind != "meeting" {
        return Err(bad());
    }
    Ok((created, kind, seq))
}

/// The WHERE tail after the cursor for one table: rows created before
/// the cursor's instant, or at it and after the cursor in the fixed order.
fn after_cursor(
    table_kind: &str,
    cursor: &Option<(String, String, i64)>,
    sql: &mut String,
    args: &mut Vec<Box<dyn rusqlite::ToSql>>,
) {
    let Some((created, kind, seq)) = cursor else {
        return;
    };
    let n = args.len();
    match (table_kind, kind.as_str()) {
        ("dictation", "meeting") => {
            sql.push_str(&format!(" AND created_at < ?{}", n + 1));
            args.push(Box::new(created.clone()));
        }
        ("meeting", "dictation") => {
            sql.push_str(&format!(" AND created_at <= ?{}", n + 1));
            args.push(Box::new(created.clone()));
        }
        _ => {
            sql.push_str(&format!(
                " AND (created_at < ?{} OR (created_at = ?{} AND seq < ?{}))",
                n + 1,
                n + 1,
                n + 2
            ));
            args.push(Box::new(created.clone()));
            args.push(Box::new(*seq));
        }
    }
}

fn range_clauses(filter: &ListFilter, sql: &mut String, args: &mut Vec<Box<dyn rusqlite::ToSql>>) {
    if let Some(since) = &filter.since {
        sql.push_str(&format!(" AND created_at >= ?{}", args.len() + 1));
        args.push(Box::new(since.clone()));
    }
    if let Some(until) = &filter.until {
        sql.push_str(&format!(" AND created_at < ?{}", args.len() + 1));
        args.push(Box::new(until.clone()));
    }
}

impl Store {
    /// A page of the timeline over the kinds asked for. `app_id` narrows
    /// the dictations only; a meeting has no app.
    pub fn timeline(
        &self,
        filter: &ListFilter,
        dictations: bool,
        meetings: bool,
    ) -> Result<TimelinePage, StoreError> {
        let limit = filter.limit.clamp(1, 1000) as usize;
        let cursor = filter.cursor.as_deref().map(parse_cursor).transpose()?;
        let mut rows: Vec<(Entry, i64)> = Vec::new();
        if dictations {
            let mut sql = format!(
                "SELECT {} FROM dictations WHERE 1 = 1",
                crate::dictations::COLUMNS
            );
            let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            after_cursor("dictation", &cursor, &mut sql, &mut args);
            if let Some(app) = &filter.app_id {
                sql.push_str(&format!(" AND app_id = ?{}", args.len() + 1));
                args.push(Box::new(app.clone()));
            }
            range_clauses(filter, &mut sql, &mut args);
            sql.push_str(&format!(
                " ORDER BY created_at DESC, seq DESC LIMIT ?{}",
                args.len() + 1
            ));
            args.push(Box::new(limit as i64 + 1));
            let mut stmt = self.conn().prepare(&sql)?;
            let found = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
                dictation_from_row(r).map(|(item, seq)| (Entry::Dictation(Box::new(item)), seq))
            })?;
            rows.extend(found.collect::<Result<Vec<_>, _>>()?);
        }
        if meetings && filter.app_id.is_none() {
            let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM meetings WHERE 1 = 1");
            let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            after_cursor("meeting", &cursor, &mut sql, &mut args);
            range_clauses(filter, &mut sql, &mut args);
            sql.push_str(&format!(
                " ORDER BY created_at DESC, seq DESC LIMIT ?{}",
                args.len() + 1
            ));
            args.push(Box::new(limit as i64 + 1));
            let mut stmt = self.conn().prepare(&sql)?;
            let found = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
                summary_from_row(r).map(|(row, seq)| (Entry::Meeting(Box::new(row)), seq))
            })?;
            rows.extend(found.collect::<Result<Vec<_>, _>>()?);
        }
        rows.sort_by(|(a, sa), (b, sb)| {
            b.created_at()
                .cmp(a.created_at())
                .then(a.kind().cmp(b.kind()))
                .then(sb.cmp(sa))
        });
        let next_cursor = if rows.len() > limit {
            rows.truncate(limit);
            rows.last()
                .map(|(e, seq)| format!("{}|{}|{seq}", e.created_at(), e.kind()))
        } else {
            None
        };
        let items: Vec<Entry> = rows.into_iter().map(|(e, _)| e).collect();
        Ok(TimelinePage { items, next_cursor })
    }

    /// Searches meetings by title, transcript, notes, analysis and speaker
    /// names; best first, each hit naming the column its snippet came from.
    pub fn search_meetings(&self, query: &str, limit: u32) -> Result<Vec<MeetingHit>, StoreError> {
        let expression = fts_expression(query)?;
        let snippets = MATCH_ORDER
            .iter()
            .map(|(i, _)| format!("snippet(meetings_fts, {i}, '{MARK}', '{MARK}', '…', 12)"))
            .collect::<Vec<_>>()
            .join(", ");
        let first = 1;
        let mut stmt = self.conn().prepare(&format!(
            "SELECT m.id, {snippets} \
             FROM meetings_fts f JOIN meetings m ON m.seq = f.rowid \
             WHERE meetings_fts MATCH ?1 \
             ORDER BY rank, m.created_at DESC, m.seq DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![expression, i64::from(limit.clamp(1, 1000))], |r| {
            let mut hit = MeetingHit {
                id: r.get(0)?,
                snippet: String::new(),
                matched_field: "transcript",
            };
            for (n, (_, field)) in MATCH_ORDER.iter().enumerate() {
                let text: String = r.get(first + n)?;
                if text.contains(MARK) {
                    hit.snippet = text.replace(MARK, "");
                    hit.matched_field = field;
                    break;
                }
            }
            Ok(hit)
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Marks every meeting still `transcribing` as failed: the job that
    /// produced its text died with the daemon. Recording ones are the
    /// recovery's business (`meetings_with_status`).
    pub fn fail_stale_meeting_jobs(&self, reason: &str) -> Result<usize, StoreError> {
        Ok(self.conn().execute(
            "UPDATE meetings SET status = 'failed', error_code = 'job_lost', \
             error_message = ?1, updated_at = ?2 WHERE status = 'transcribing'",
            params![reason, now_iso()],
        )?)
    }

    /// Sets a meeting's status and error fields.
    pub fn set_meeting_status(
        &self,
        id: &str,
        status: MeetingStatus,
        error: Option<(&str, &str)>,
    ) -> Result<(), StoreError> {
        let n = self.conn().execute(
            "UPDATE meetings SET status = ?2, error_code = ?3, error_message = ?4, \
             updated_at = ?5 WHERE id = ?1",
            params![
                id,
                status.as_str(),
                error.map(|(c, _)| c),
                error.map(|(_, m)| m),
                now_iso()
            ],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::SourceKind;
    use crate::meetings::MeetingRow;

    fn dictation(created: &str, text: &str) -> DictationItem {
        let mut d = DictationItem::new(SourceKind::Dictation);
        d.created_at = created.into();
        d.final_text = text.into();
        d
    }

    fn meeting(created: &str, title: &str) -> MeetingRow {
        let mut m = MeetingRow::new();
        m.created_at = created.into();
        m.started_at = created.into();
        m.title = title.into();
        m.status = MeetingStatus::Completed;
        m
    }

    #[test]
    fn the_timeline_interleaves_both_kinds_and_pages_through_a_tie() {
        let store = Store::in_memory().unwrap();
        store
            .insert(&dictation("2026-02-13T16:00:00Z", "one"))
            .unwrap();
        store
            .insert_meeting(&meeting("2026-02-13T16:00:00Z", "Weekly sync"))
            .unwrap();
        store
            .insert(&dictation("2026-02-12T10:00:00Z", "two"))
            .unwrap();
        store
            .insert_meeting(&meeting("2026-02-14T09:00:00Z", "Standup"))
            .unwrap();
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = store
                .timeline(
                    &ListFilter {
                        limit: 1,
                        cursor: cursor.clone(),
                        ..ListFilter::default()
                    },
                    true,
                    true,
                )
                .unwrap();
            for e in &page.items {
                seen.push(format!("{}:{}", e.kind(), e.created_at()));
            }
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(
            seen,
            [
                "meeting:2026-02-14T09:00:00Z",
                "dictation:2026-02-13T16:00:00Z",
                "meeting:2026-02-13T16:00:00Z",
                "dictation:2026-02-12T10:00:00Z",
            ]
        );
        let only = store
            .timeline(
                &ListFilter {
                    limit: 10,
                    ..ListFilter::default()
                },
                false,
                true,
            )
            .unwrap();
        assert_eq!(only.items.len(), 2);
        assert!(
            store
                .timeline(
                    &ListFilter {
                        limit: 1,
                        cursor: Some("x".into()),
                        ..ListFilter::default()
                    },
                    true,
                    true
                )
                .is_err()
        );
    }

    #[test]
    fn meetings_are_searchable_and_stale_jobs_fail() {
        let store = Store::in_memory().unwrap();
        let mut m = meeting("2026-02-13T16:00:00Z", "API design review");
        m.final_text = "we discussed the API contract".into();
        store.insert_meeting(&m).unwrap();
        let hits = store.search_meetings("api contr", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("API"));
        assert_eq!(hits[0].matched_field, "title", "the title wins a tie");
        let body = store.search_meetings("contract", 10).unwrap();
        assert_eq!(body[0].matched_field, "transcript");
        assert!(store.search_meetings("nothing", 10).unwrap().is_empty());
        // A word in the notes or the analysis is found and attributed.
        store
            .set_meeting_notes(
                &m.id,
                "Remember the café lease.",
                dettivo_proto::methods::meetings_notes::NotesSource::User,
            )
            .unwrap();
        let analysis = dettivo_proto::methods::meetings_notes::Analysis {
            summary: "Rollout moved to Thursday.".into(),
            decisions: Vec::new(),
            action_items: Vec::new(),
        };
        store
            .set_meeting_analysis(&m.id, &analysis, "mock", "2026-02-13T16:31:00Z")
            .unwrap();
        let notes = store.search_meetings("cafe", 10).unwrap();
        assert_eq!(notes[0].matched_field, "notes");
        assert!(notes[0].snippet.contains("café"), "{}", notes[0].snippet);
        let analysis = store.search_meetings("thursday", 10).unwrap();
        assert_eq!(analysis[0].matched_field, "analysis");
        let title = store.search_meetings("design", 10).unwrap();
        assert_eq!(title[0].matched_field, "title");
        let mut job = meeting("2026-02-13T17:00:00Z", "Imported");
        job.status = MeetingStatus::Transcribing;
        store.insert_meeting(&job).unwrap();
        assert_eq!(store.fail_stale_meeting_jobs("restart").unwrap(), 1);
        let back = store.get_meeting(&job.id).unwrap().unwrap();
        assert_eq!(back.status, MeetingStatus::Failed);
        assert_eq!(back.error_code.as_deref(), Some("job_lost"));
        store
            .set_meeting_status(&job.id, MeetingStatus::Stopped, None)
            .unwrap();
        assert_eq!(
            store.get_meeting(&job.id).unwrap().unwrap().status,
            MeetingStatus::Stopped
        );
    }
}
