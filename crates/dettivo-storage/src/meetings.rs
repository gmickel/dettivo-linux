//! The meeting repository: insert, update, read back, keyset listing, the
//! stale rows a restart promotes, and row deletion over the `meetings`
//! table. The speakers travel with the row: a write that changes them
//! replaces them in the row's transaction and detail reads attach them
//! (`speakers`). List reads select only display fields. The row itself is in
//! `meeting_row`, the notes and analysis writes and the delete policies in
//! `meeting_notes`, and the meeting directories beside the database in
//! `meeting_artifacts`.

use rusqlite::{OptionalExtension, params};

pub use crate::meeting_artifacts::MeetingArtifacts;
pub(crate) use crate::meeting_row::{COLUMNS, FIELDS, ROW_FIELD_COUNT, from_row, values};
pub use crate::meeting_row::{MeetingRow, MeetingStatus};

use crate::dictations::{ListFilter, cursor_clause};
use crate::meeting_reads::{MeetingSummary, SUMMARY_COLUMNS, status_selection, summary_from_row};
use crate::time::now_iso;
use crate::{Store, StoreError};

impl Store {
    /// Attaches the speakers of every row.
    pub(crate) fn attach_speakers(&self, rows: &mut [MeetingRow]) -> Result<(), StoreError> {
        for row in rows {
            row.speakers = self.speakers_of(&row.id)?;
        }
        Ok(())
    }

    /// Inserts a meeting (with its speakers); its id must be new. The row
    /// and its speakers land in one transaction.
    pub fn insert_meeting(&self, row: &MeetingRow) -> Result<(), StoreError> {
        let count = FIELDS.split(", ").count();
        let placeholders: Vec<String> = (2..=count + 1).map(|i| format!("?{i}")).collect();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(row.id.clone())];
        args.extend(values(row));
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO meetings (id, {FIELDS}) VALUES (?1, {})",
                placeholders.join(", ")
            ),
            rusqlite::params_from_iter(args.iter()),
        )?;
        self.replace_speakers(&row.id, &row.speakers)?;
        Ok(tx.commit()?)
    }

    /// Rewrites the fields of a meeting the capture, the transcription
    /// and the speaker pass own (the speakers included); `updated_at` is
    /// set to now. The summary, the notes and the analysis belong to their
    /// setters in `meeting_notes` and stay as they are, so a worker
    /// writing the row it started with cannot erase what was saved
    /// meanwhile. The row and its speakers change in one transaction.
    pub fn update_meeting(&self, row: &MeetingRow) -> Result<(), StoreError> {
        let sets: Vec<String> = FIELDS
            .split(", ")
            .take(ROW_FIELD_COUNT)
            .enumerate()
            .map(|(i, f)| format!("{f} = ?{}", i + 2))
            .collect();
        let mut stamped = row.clone();
        stamped.updated_at = now_iso();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(row.id.clone())];
        args.extend(values(&stamped).into_iter().take(ROW_FIELD_COUNT));
        let tx = self.conn().unchecked_transaction()?;
        let n = tx.execute(
            &format!("UPDATE meetings SET {} WHERE id = ?1", sets.join(", ")),
            rusqlite::params_from_iter(args.iter()),
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(row.id.clone()));
        }
        if self.speakers_of(&row.id)? != row.speakers {
            self.replace_speakers(&row.id, &row.speakers)?;
        }
        Ok(tx.commit()?)
    }

    /// One meeting by id.
    pub fn get_meeting(&self, id: &str) -> Result<Option<MeetingRow>, StoreError> {
        let row = self
            .conn()
            .query_row(
                &format!("SELECT {COLUMNS} FROM meetings WHERE id = ?1"),
                params![id],
                |r| from_row(r).map(|(row, _)| row),
            )
            .optional()?;
        let mut rows: Vec<MeetingRow> = row.into_iter().collect();
        self.attach_speakers(&mut rows)?;
        Ok(rows.pop())
    }

    /// The newest meeting.
    pub fn latest_meeting(&self) -> Result<Option<MeetingRow>, StoreError> {
        let row = self
            .conn()
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM meetings ORDER BY created_at DESC, seq DESC LIMIT 1"
                ),
                [],
                |r| from_row(r).map(|(row, _)| row),
            )
            .optional()?;
        let mut rows: Vec<MeetingRow> = row.into_iter().collect();
        self.attach_speakers(&mut rows)?;
        Ok(rows.pop())
    }

    /// How many meetings exist.
    pub fn count_meetings(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM meetings", [], |r| r.get(0))?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    /// A page of meetings, newest first, keyset-paged like the dictations.
    pub fn list_meetings(&self, filter: &ListFilter) -> Result<MeetingPage, StoreError> {
        let limit = i64::from(filter.limit.clamp(1, 1000));
        let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM meetings WHERE 1 = 1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        cursor_clause(filter.cursor.as_deref(), &mut sql, &mut args)?;
        if let Some(since) = &filter.since {
            sql.push_str(&format!(" AND created_at >= ?{}", args.len() + 1));
            args.push(Box::new(since.clone()));
        }
        if let Some(until) = &filter.until {
            sql.push_str(&format!(" AND created_at < ?{}", args.len() + 1));
            args.push(Box::new(until.clone()));
        }
        sql.push_str(&format!(
            " ORDER BY created_at DESC, seq DESC LIMIT ?{}",
            args.len() + 1
        ));
        args.push(Box::new(limit + 1));
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), summary_from_row)?;
        let mut items: Vec<(MeetingSummary, i64)> = rows.collect::<Result<_, _>>()?;
        let next_cursor = if items.len() as i64 > limit {
            items.truncate(limit as usize);
            items
                .last()
                .map(|(row, seq)| format!("{}|{seq}", row.created_at))
        } else {
            None
        };
        let items: Vec<MeetingSummary> = items.into_iter().map(|(row, _)| row).collect();
        Ok(MeetingPage { items, next_cursor })
    }

    /// Meetings whose status is one of `statuses`, oldest first.
    pub fn meetings_with_status(
        &self,
        statuses: &[MeetingStatus],
    ) -> Result<Vec<MeetingRow>, StoreError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM meetings WHERE status IN ({}) ORDER BY created_at, seq",
            status_selection(statuses)
        ))?;
        let rows = stmt.query_map([], from_row)?;
        let mut rows: Vec<MeetingRow> = rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(r, _)| r)
            .collect();
        self.attach_speakers(&mut rows)?;
        Ok(rows)
    }

    /// Removes a meeting row; returns it, or `NotFound`.
    pub fn delete_meeting_row(&self, id: &str) -> Result<MeetingRow, StoreError> {
        let row = self
            .get_meeting(id)?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        self.conn()
            .execute("DELETE FROM meetings WHERE id = ?1", params![id])?;
        Ok(row)
    }
}

/// One page of meetings.
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingPage {
    /// The rows, newest first.
    pub items: Vec<MeetingSummary>,
    /// The cursor for the next page, `None` when this was the last.
    pub next_cursor: Option<String>,
}

#[cfg(test)]
#[path = "meetings_tests.rs"]
mod tests;
