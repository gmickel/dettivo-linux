//! Search both FTS indexes under one ordering, retaining only the requested candidates.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use dettivo_proto::runtime::RefKind;
use rusqlite::params;

use crate::search::{SearchHit, fts_expression, match_ranges, query_tokens};
use crate::timeline::{MARK, MATCH_ORDER, MeetingHit};
use crate::{Store, StoreError};

/// One selected hit, with the same snippet fields as the single-kind searches.
pub enum CombinedHit {
    /// A dictation and its character match ranges.
    Dictation(Box<SearchHit>),
    /// A meeting and the field containing its snippet.
    Meeting(Box<MeetingHit>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    exact_title: Reverse<bool>,
    created_at: Reverse<String>,
    id: String,
    meeting: bool,
    seq: i64,
}

impl Store {
    /// Folded exact titles first, then newest creation time, ID and kind.
    /// FTS determines membership; per-index rank never truncates either kind.
    /// The aggregate ceiling is 1000 for one requested kind, 2000 for both;
    /// only `min(limit.max(1), ceiling)` lightweight candidates are retained.
    pub fn search_combined(
        &self,
        query: &str,
        limit: u32,
        kinds: &[RefKind],
    ) -> Result<Vec<CombinedHit>, StoreError> {
        let dictations = kinds.contains(&RefKind::Dictation);
        let meetings = kinds.contains(&RefKind::Meeting);
        let ceiling = 1000 * (u32::from(dictations) + u32::from(meetings));
        if ceiling == 0 {
            return Ok(Vec::new());
        }
        let limit = limit.max(1).min(ceiling) as usize;
        let expression = fts_expression(query)?;
        let tokens = query_tokens(query);
        let mut candidates = BTreeSet::new();
        for (include, table, meeting) in [
            (dictations, "dictations", false),
            (meetings, "meetings", true),
        ] {
            if !include {
                continue;
            }
            let mut stmt = self.conn().prepare(&format!(
                "SELECT d.id, d.created_at, d.title, d.seq FROM {table}_fts f \
                 JOIN {table} d ON d.seq = f.rowid WHERE {table}_fts MATCH ?1"
            ))?;
            let rows = stmt.query_map([&expression], |r| {
                let title: String = r.get(2)?;
                Ok(Candidate {
                    exact_title: Reverse(query_tokens(&title) == tokens),
                    created_at: Reverse(r.get(1)?),
                    id: r.get(0)?,
                    meeting,
                    seq: r.get(3)?,
                })
            })?;
            for row in rows {
                candidates.insert(row?);
                if candidates.len() > limit {
                    candidates.pop_last();
                }
            }
        }
        let meeting_snippets = MATCH_ORDER
            .iter()
            .map(|(i, _)| format!("snippet(meetings_fts, {i}, '{MARK}', '{MARK}', '…', 12)"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut meeting_stmt = self.conn().prepare_cached(&format!(
            "SELECT {meeting_snippets} FROM meetings_fts WHERE meetings_fts MATCH ?1 AND rowid = ?2"
        ))?;
        let mut dictation_stmt = self.conn().prepare_cached(
            "SELECT snippet(dictations_fts, -1, '', '', '…', 12) FROM dictations_fts \
             WHERE dictations_fts MATCH ?1 AND rowid = ?2",
        )?;
        candidates
            .into_iter()
            .map(|candidate| {
                if candidate.meeting {
                    let (snippet, matched_field) =
                        meeting_stmt.query_row(params![expression, candidate.seq], |r| {
                            for (n, (_, field)) in MATCH_ORDER.iter().enumerate() {
                                let text: String = r.get(n)?;
                                if text.contains(MARK) {
                                    return Ok((text.replace(MARK, ""), *field));
                                }
                            }
                            Ok((String::new(), "transcript"))
                        })?;
                    Ok(CombinedHit::Meeting(Box::new(MeetingHit {
                        id: candidate.id,
                        snippet,
                        matched_field,
                    })))
                } else {
                    let item = self
                        .get(&candidate.id)?
                        .ok_or_else(|| StoreError::NotFound(candidate.id.clone()))?;
                    let snippet: String = dictation_stmt
                        .query_row(params![expression, candidate.seq], |r| r.get(0))?;
                    let matches = match_ranges(&snippet, &tokens);
                    Ok(CombinedHit::Dictation(Box::new(SearchHit {
                        item,
                        snippet,
                        matches,
                        score: None,
                    })))
                }
            })
            .collect()
    }
}
