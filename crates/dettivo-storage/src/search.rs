//! Full-text search over raw text, final text, title and summary. A query
//! is split into tokens, each becomes a quoted prefix term (`"tok"*`) and
//! the terms are ANDed, so a search matches word starts the way the macOS
//! substring index does; operator characters never reach FTS5 unescaped.
//! `score` is `None` while `knowledge.semantic_search` is false.

use rusqlite::params;

use crate::dictations::from_row;
use crate::item::DictationItem;
use crate::{Store, StoreError};

/// The longest query accepted, in bytes.
pub const MAX_QUERY_BYTES: usize = 1024;

/// One search hit.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// The matched item.
    pub item: DictationItem,
    /// The matching text with an ellipsis where it was cut.
    pub snippet: String,
    /// Where the query's words start inside `snippet`, as character
    /// ranges (`start`, `end`), so a list can paint the matches.
    pub matches: Vec<(usize, usize)>,
    /// Always `None` here (FR-Y4).
    pub score: Option<f64>,
}

/// The query's words, lower-cased and accent-folded, the way FTS5's
/// `unicode61` tokenizer sees them.
pub fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(fold)
        .collect()
}

/// Lower case with the common Latin accents removed, so `café` and `cafe`
/// compare equal the way the FTS index folds them.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            'ß' => 's',
            other => other,
        })
        .collect()
}

/// The character ranges of `text` where a word starts with one of the
/// folded `tokens`: the same prefix rule the FTS expression applies, so
/// what is painted is what matched. Ranges come in text order.
pub fn match_ranges(text: &str, tokens: &[String]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        let folded = fold(&word);
        if let Some(token) = tokens
            .iter()
            .filter(|t| folded.starts_with(t.as_str()))
            .max_by_key(|t| t.chars().count())
        {
            // The painted span covers the matched prefix in the word's own
            // characters; folding keeps character counts except for `ß`.
            let len = token.chars().count().min(i - start);
            ranges.push((start, start + len));
        }
    }
    ranges
}

/// A page of hits.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPage {
    /// The hits, best first.
    pub hits: Vec<SearchHit>,
    /// The cursor for the next page, `None` when this was the last.
    pub next_cursor: Option<String>,
}

/// The FTS5 expression for a user query: every token quoted and
/// prefix-matched, ANDed. An over-long query or one with no word
/// characters is refused with the limit named.
pub fn fts_expression(query: &str) -> Result<String, StoreError> {
    if query.len() > MAX_QUERY_BYTES {
        return Err(StoreError::InvalidQuery(format!(
            "query is {} bytes; the limit is {MAX_QUERY_BYTES}",
            query.len()
        )));
    }
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        return Err(StoreError::InvalidQuery(
            "query has no word characters to search for".into(),
        ));
    }
    Ok(terms.join(" "))
}

impl Store {
    /// Searches; `cursor` is what a previous page returned.
    pub fn search(
        &self,
        query: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<SearchPage, StoreError> {
        let expression = fts_expression(query)?;
        let tokens = query_tokens(query);
        let offset: i64 = match cursor {
            None => 0,
            Some(c) => c.parse().map_err(|_| {
                StoreError::InvalidQuery(format!("cursor {c:?} is not one this store issued"))
            })?,
        };
        let limit = i64::from(limit.clamp(1, 1000));
        let mut stmt = self.conn().prepare(
            "SELECT d.id, d.created_at, d.updated_at, d.app_id, d.app_name, d.source_kind, \
             d.status, d.mode, d.stt_provider, d.stt_model, d.language, d.raw_text, \
             d.final_text, d.title, d.summary, d.duration_ms, d.rerun_of_item_id, \
             d.error_code, d.error_message, d.audio_path, d.insertion, d.segments, d.policy_hash, \
             d.notice, d.timings, d.seq, \
             snippet(dictations_fts, -1, '', '', '…', 12) \
             FROM dictations_fts f JOIN dictations d ON d.seq = f.rowid \
             WHERE dictations_fts MATCH ?1 \
             ORDER BY rank, d.created_at DESC, d.seq DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![expression, limit + 1, offset], |r| {
            let (item, _) = from_row(r)?;
            let snippet: String = r.get(26)?;
            let matches = match_ranges(&snippet, &tokens);
            Ok(SearchHit {
                item,
                snippet,
                matches,
                score: None,
            })
        })?;
        let mut hits: Vec<SearchHit> = rows.collect::<Result<_, _>>()?;
        let next_cursor = if hits.len() as i64 > limit {
            hits.truncate(limit as usize);
            Some((offset + limit).to_string())
        } else {
            None
        };
        Ok(SearchPage { hits, next_cursor })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_quote_prefix_and_and_the_tokens() {
        assert_eq!(fts_expression("api").unwrap(), "\"api\"*");
        assert_eq!(
            fts_expression("hello  world").unwrap(),
            "\"hello\"* \"world\"*"
        );
        assert_eq!(
            fts_expression("NOT \"quoted\" (x OR y)*").unwrap(),
            "\"NOT\"* \"quoted\"* \"x\"* \"OR\"* \"y\"*"
        );
        assert_eq!(fts_expression("café").unwrap(), "\"café\"*");
        assert!(fts_expression("... ---").is_err());
        let long = "a".repeat(MAX_QUERY_BYTES + 1);
        assert!(
            fts_expression(&long)
                .unwrap_err()
                .to_string()
                .contains("1024")
        );
    }

    #[test]
    fn match_ranges_follow_the_prefix_rule_with_accents_folded() {
        let tokens = query_tokens("Api caf");
        assert_eq!(tokens, vec!["api".to_string(), "caf".to_string()]);
        let text = "the API gateway, the café lease and a capital letter";
        assert_eq!(match_ranges(text, &tokens), vec![(4, 7), (21, 24)]);
        assert_eq!(
            match_ranges("…merger overlap…", &query_tokens("merger")),
            vec![(1, 7)]
        );
        assert!(match_ranges("nothing here", &query_tokens("zzz")).is_empty());
    }
}
