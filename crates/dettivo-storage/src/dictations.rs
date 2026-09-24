//! The dictation repository: insert, update, read back, keyset-paged
//! listing, the newest item, deletion of a row, and the queries the
//! retention sweep needs.

use rusqlite::{OptionalExtension, Row, params};

use crate::item::{DictationItem, ItemStatus, SourceKind};
use crate::time::now_iso;
use crate::{Store, StoreError};

pub(crate) const COLUMNS: &str = "id, created_at, updated_at, app_id, app_name, source_kind, status, mode, \
stt_provider, stt_model, language, raw_text, final_text, title, summary, duration_ms, \
rerun_of_item_id, error_code, error_message, audio_path, insertion, segments, policy_hash, notice, \
timings, seq";

/// Which rows a listing returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListFilter {
    /// Rows per page.
    pub limit: u32,
    /// The cursor a previous page returned.
    pub cursor: Option<String>,
    /// Only this app id.
    pub app_id: Option<String>,
    /// Only rows created at or after this ISO 8601 time.
    pub since: Option<String>,
    /// Only rows created before this ISO 8601 time.
    pub until: Option<String>,
}

/// One page of a listing.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The rows, newest first.
    pub items: Vec<DictationItem>,
    /// The cursor for the next page, `None` when this was the last.
    pub next_cursor: Option<String>,
}

/// Appends the keyset condition of a `created_at|seq` cursor.
pub(crate) fn cursor_clause(
    cursor: Option<&str>,
    sql: &mut String,
    args: &mut Vec<Box<dyn rusqlite::ToSql>>,
) -> Result<(), StoreError> {
    let Some(cursor) = cursor else {
        return Ok(());
    };
    let (created, seq) = cursor.split_once('|').ok_or_else(|| {
        StoreError::InvalidQuery(format!("cursor {cursor:?} is not one this store issued"))
    })?;
    let seq: i64 = seq.parse().map_err(|_| {
        StoreError::InvalidQuery(format!("cursor {cursor:?} is not one this store issued"))
    })?;
    sql.push_str(&format!(
        " AND (created_at < ?{} OR (created_at = ?{} AND seq < ?{}))",
        args.len() + 1,
        args.len() + 1,
        args.len() + 2
    ));
    args.push(Box::new(created.to_string()));
    args.push(Box::new(seq));
    Ok(())
}

/// The segments column: a JSON array, or NULL when there are none.
fn segments_json(item: &DictationItem) -> Option<String> {
    (!item.segments.is_empty()).then(|| serde_json::to_string(&item.segments).unwrap_or_default())
}

/// The timings column: JSON, or NULL when the item inserted nothing.
fn timings_json(item: &DictationItem) -> Option<String> {
    item.timings
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_default())
}

/// The notice column: a bare word (`silent`) stays a bare word, a
/// structured notice (`{ kind, reason }`) is stored as JSON.
fn notice_json(item: &DictationItem) -> Option<String> {
    item.notice.as_ref().map(|v| match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// The inverse of [`notice_json`]: JSON parses back, anything else is the
/// bare word it was stored as.
fn notice_from_column(s: String) -> serde_json::Value {
    serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
}

pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<(DictationItem, i64)> {
    let kind: String = row.get(5)?;
    let status: String = row.get(6)?;
    let insertion: Option<String> = row.get(20)?;
    let segments: Option<String> = row.get(21)?;
    let notice: Option<String> = row.get(23)?;
    let timings: Option<String> = row.get(24)?;
    let duration: i64 = row.get(15)?;
    Ok((
        DictationItem {
            id: row.get(0)?,
            created_at: row.get(1)?,
            updated_at: row.get(2)?,
            app_id: row.get(3)?,
            app_name: row.get(4)?,
            source_kind: SourceKind::parse(&kind).unwrap_or(SourceKind::Dictation),
            status: ItemStatus::parse(&status).unwrap_or(ItemStatus::Completed),
            mode: row.get(7)?,
            stt_provider: row.get(8)?,
            stt_model: row.get(9)?,
            language: row.get(10)?,
            raw_text: row.get(11)?,
            final_text: row.get(12)?,
            title: row.get(13)?,
            summary: row.get(14)?,
            duration_ms: u64::try_from(duration).unwrap_or(0),
            rerun_of_item_id: row.get(16)?,
            error_code: row.get(17)?,
            error_message: row.get(18)?,
            audio_path: row.get(19)?,
            insertion: insertion.and_then(|s| serde_json::from_str(&s).ok()),
            segments: segments
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            policy_hash: row.get(22)?,
            notice: notice.map(notice_from_column),
            timings: timings.and_then(|s| serde_json::from_str(&s).ok()),
        },
        row.get(25)?,
    ))
}

impl Store {
    /// Inserts an item; its id must be new.
    pub fn insert(&self, item: &DictationItem) -> Result<(), StoreError> {
        let insertion = item
            .insertion
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        self.conn().execute(
            "INSERT INTO dictations (id, created_at, updated_at, app_id, app_name, source_kind, \
             status, mode, stt_provider, stt_model, language, raw_text, final_text, title, \
             summary, duration_ms, rerun_of_item_id, error_code, error_message, audio_path, \
             insertion, segments, policy_hash, notice, timings) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, \
             ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            params![
                item.id,
                item.created_at,
                item.updated_at,
                item.app_id,
                item.app_name,
                item.source_kind.as_str(),
                item.status.as_str(),
                item.mode,
                item.stt_provider,
                item.stt_model,
                item.language,
                item.raw_text,
                item.final_text,
                item.title,
                item.summary,
                i64::try_from(item.duration_ms).unwrap_or(i64::MAX),
                item.rerun_of_item_id,
                item.error_code,
                item.error_message,
                item.audio_path,
                insertion,
                segments_json(item),
                item.policy_hash,
                notice_json(item),
                timings_json(item),
            ],
        )?;
        Ok(())
    }

    /// Rewrites every mutable field of an item by id; `updated_at` is set
    /// to now.
    pub fn update(&self, item: &DictationItem) -> Result<(), StoreError> {
        let insertion = item
            .insertion
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let n = self.conn().execute(
            "UPDATE dictations SET updated_at = ?2, app_id = ?3, app_name = ?4, status = ?5, \
             mode = ?6, stt_provider = ?7, stt_model = ?8, language = ?9, raw_text = ?10, \
             final_text = ?11, title = ?12, summary = ?13, duration_ms = ?14, \
             rerun_of_item_id = ?15, error_code = ?16, error_message = ?17, audio_path = ?18, \
             insertion = ?19, segments = ?20, policy_hash = ?21, notice = ?22, timings = ?23 \
             WHERE id = ?1",
            params![
                item.id,
                now_iso(),
                item.app_id,
                item.app_name,
                item.status.as_str(),
                item.mode,
                item.stt_provider,
                item.stt_model,
                item.language,
                item.raw_text,
                item.final_text,
                item.title,
                item.summary,
                i64::try_from(item.duration_ms).unwrap_or(i64::MAX),
                item.rerun_of_item_id,
                item.error_code,
                item.error_message,
                item.audio_path,
                insertion,
                segments_json(item),
                item.policy_hash,
                notice_json(item),
                timings_json(item),
            ],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(item.id.clone()));
        }
        Ok(())
    }

    /// One item by id.
    pub fn get(&self, id: &str) -> Result<Option<DictationItem>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {COLUMNS} FROM dictations WHERE id = ?1"),
                params![id],
                |r| from_row(r).map(|(item, _)| item),
            )
            .optional()?)
    }

    /// The newest item.
    pub fn latest(&self) -> Result<Option<DictationItem>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM dictations ORDER BY created_at DESC, seq DESC LIMIT 1"
                ),
                [],
                |r| from_row(r).map(|(item, _)| item),
            )
            .optional()?)
    }

    /// How many items exist.
    pub fn count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM dictations", [], |r| r.get(0))?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    /// A page of items, newest first, filtered and keyset-paged.
    pub fn list(&self, filter: &ListFilter) -> Result<Page, StoreError> {
        let limit = i64::from(filter.limit.clamp(1, 1000));
        let mut sql = format!("SELECT {COLUMNS} FROM dictations WHERE 1 = 1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        cursor_clause(filter.cursor.as_deref(), &mut sql, &mut args)?;
        if let Some(app) = &filter.app_id {
            sql.push_str(&format!(" AND app_id = ?{}", args.len() + 1));
            args.push(Box::new(app.clone()));
        }
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
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), from_row)?;
        let mut items: Vec<(DictationItem, i64)> = rows.collect::<Result<_, _>>()?;
        let next_cursor = if items.len() as i64 > limit {
            items.truncate(limit as usize);
            items
                .last()
                .map(|(item, seq)| format!("{}|{seq}", item.created_at))
        } else {
            None
        };
        Ok(Page {
            items: items.into_iter().map(|(item, _)| item).collect(),
            next_cursor,
        })
    }

    /// Removes a row; returns it, or `NotFound`.
    pub fn delete_row(&self, id: &str) -> Result<DictationItem, StoreError> {
        let item = self
            .get(id)?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        self.conn()
            .execute("DELETE FROM dictations WHERE id = ?1", params![id])?;
        Ok(item)
    }

    /// Sets an item's status and error fields.
    pub fn set_status(
        &self,
        id: &str,
        status: ItemStatus,
        error: Option<(&str, &str)>,
    ) -> Result<(), StoreError> {
        let n = self.conn().execute(
            "UPDATE dictations SET status = ?2, error_code = ?3, error_message = ?4, \
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

    /// Marks every item still `transcribing` as failed: a daemon that
    /// restarts has lost the job that was producing the text.
    pub fn fail_stale_jobs(&self, reason: &str) -> Result<usize, StoreError> {
        Ok(self.conn().execute(
            "UPDATE dictations SET status = 'failed', error_code = 'job_lost', \
             error_message = ?1, updated_at = ?2 WHERE status = 'transcribing'",
            params![reason, now_iso()],
        )?)
    }

    /// Points an item at a take that already exists on disk without
    /// touching `updated_at` (the seed adopting its retained silence).
    pub fn attach_audio(&self, id: &str, path: &str) -> Result<(), StoreError> {
        let n = self.conn().execute(
            "UPDATE dictations SET audio_path = ?2 WHERE id = ?1",
            params![id, path],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Clears the audio path of an item whose file was removed.
    pub fn clear_audio(&self, id: &str) -> Result<(), StoreError> {
        self.conn().execute(
            "UPDATE dictations SET audio_path = NULL, updated_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
        Ok(())
    }

    /// Items with retained audio created before `cutoff` (ISO 8601).
    pub fn with_audio_before(&self, cutoff: &str) -> Result<Vec<DictationItem>, StoreError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM dictations WHERE audio_path IS NOT NULL AND created_at < ?1 \
             ORDER BY created_at"
        ))?;
        let rows = stmt.query_map(params![cutoff], from_row)?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(i, _)| i)
            .collect())
    }

    /// The oldest items beyond the newest `max_items`.
    pub fn beyond(&self, max_items: u64) -> Result<Vec<DictationItem>, StoreError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM dictations ORDER BY created_at DESC, seq DESC \
             LIMIT -1 OFFSET ?1"
        ))?;
        let rows = stmt.query_map(
            params![i64::try_from(max_items).unwrap_or(i64::MAX)],
            from_row,
        )?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(i, _)| i)
            .collect())
    }

    /// Items that re-ran `id`.
    pub fn reruns_of(&self, id: &str) -> Result<Vec<DictationItem>, StoreError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM dictations WHERE rerun_of_item_id = ?1 ORDER BY created_at"
        ))?;
        let rows = stmt.query_map(params![id], from_row)?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(i, _)| i)
            .collect())
    }

    /// Every item matching `filter` (its cursor ignored), newest first,
    /// walked page by page (exports).
    pub fn all(&self, filter: &ListFilter) -> Result<Vec<DictationItem>, StoreError> {
        let mut out = Vec::new();
        let mut filter = ListFilter {
            limit: 1000,
            cursor: None,
            ..filter.clone()
        };
        loop {
            let page = self.list(&filter)?;
            out.extend(page.items);
            match page.next_cursor {
                Some(c) => filter.cursor = Some(c),
                None => return Ok(out),
            }
        }
    }
}
