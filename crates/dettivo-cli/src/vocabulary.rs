//! `dettivo vocabulary import <file>`: a plain-text list of terms added to
//! `[dictation] vocabulary` in one write. One term per line, `term = a, b`
//! to give it `sounds_like` spellings; blank lines and lines starting with
//! `#` are skipped. The list is merged with the vocabulary in force, a term
//! already there (in any case) is skipped, every limit is checked line by
//! line, and the result goes to the daemon as one `config.set`, so the
//! file is written by the comment-preserving edit (ADR 0009) and reloaded.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::Subcommand;
use dettivo_proto::methods::config::vocabulary::{
    MAX_ALIASES, MAX_ENTRIES, MAX_TERM_CHARS, MAX_VOCABULARY_BYTES, term_key,
};
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// The key the import writes.
const KEY: &str = "dictation.vocabulary";

/// `dettivo vocabulary <what>`.
#[derive(Debug, Subcommand)]
pub enum VocabularyCmd {
    /// Add a plain-text list to [dictation] vocabulary: one term per line, `term = spelling, spelling` for how it is misheard.
    Import {
        /// The list; blank lines and lines starting with # are skipped.
        file: PathBuf,
        /// Print what would be added and skipped, and write nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// One entry line of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The line number, from 1.
    pub number: usize,
    /// The term.
    pub term: String,
    /// Its `sounds_like` spellings.
    pub sounds_like: Vec<String>,
}

impl Line {
    /// The entry as `config.set` takes it: the plain term, or the table
    /// when the line names spellings.
    fn entry(&self) -> Value {
        if self.sounds_like.is_empty() {
            Value::String(self.term.clone())
        } else {
            json!({"term": self.term, "sounds_like": self.sounds_like})
        }
    }
}

/// A line whose term the vocabulary already holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The line number.
    pub line: usize,
    /// The term as the line spells it.
    pub term: String,
    /// The earlier line it repeats; `None` when the vocabulary held it.
    pub repeats: Option<usize>,
}

/// What an import changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The lines whose terms are new, in file order.
    pub added: Vec<Line>,
    /// The lines skipped as duplicates.
    pub skipped: Vec<Skipped>,
    /// The whole vocabulary after the import: the entries in force, then
    /// the new ones.
    pub entries: Vec<Value>,
}

/// The entry lines of `text`, or one message per line that names no term
/// or breaks a limit of a single entry.
pub fn parse(text: &str) -> Result<Vec<Line>, Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (term, sounds_like) = match line.split_once('=') {
            Some((term, spellings)) => (
                term.trim(),
                spellings
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            None => (line, Vec::new()),
        };
        if term.is_empty() {
            errors.push(format!("line {number}: no term before `=`"));
            continue;
        }
        let chars = term.chars().count();
        if chars > MAX_TERM_CHARS {
            errors.push(format!(
                "line {number}: {term:?} is {chars} characters; a term holds at most {MAX_TERM_CHARS}"
            ));
        }
        if sounds_like.len() > MAX_ALIASES {
            errors.push(format!(
                "line {number}: {term:?} has {} sounds_like spellings; a term takes at most {MAX_ALIASES}",
                sounds_like.len()
            ));
        }
        lines.push(Line {
            number,
            term: term.to_string(),
            sounds_like,
        });
    }
    if errors.is_empty() {
        Ok(lines)
    } else {
        Err(errors)
    }
}

/// The term of an entry as `config.get` returns it.
fn term_of(entry: &Value) -> Option<&str> {
    entry
        .as_str()
        .or_else(|| entry.get("term").and_then(Value::as_str))
}

fn json_len(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |json| json.len())
}

/// `lines` merged into `existing`: a term the vocabulary or an earlier line
/// already holds, compared as the vocabulary compares terms, is skipped.
/// Every line whose entry would take the list past its entry or byte
/// limit is named, and then nothing may be written.
pub fn plan(existing: Vec<Value>, lines: Vec<Line>) -> Result<Plan, Vec<String>> {
    let mut seen: HashMap<String, Option<usize>> = existing
        .iter()
        .filter_map(term_of)
        .map(|term| (term_key(term), None))
        .collect();
    let mut added = Vec::new();
    let mut skipped = Vec::new();
    for line in lines {
        let key = term_key(&line.term);
        match seen.get(&key) {
            Some(&repeats) => skipped.push(Skipped {
                line: line.number,
                term: line.term,
                repeats,
            }),
            None => {
                seen.insert(key, Some(line.number));
                added.push(line);
            }
        }
    }
    let mut bytes = json_len(&Value::Array(existing.clone()));
    let mut entries = existing;
    let mut errors = Vec::new();
    for line in &added {
        let entry = line.entry();
        bytes = bytes.saturating_add(json_len(&entry) + usize::from(!entries.is_empty()));
        entries.push(entry);
        if entries.len() > MAX_ENTRIES {
            errors.push(format!(
                "line {}: {:?} would be entry {}; a vocabulary holds at most {MAX_ENTRIES}",
                line.number,
                line.term,
                entries.len()
            ));
        } else if bytes > MAX_VOCABULARY_BYTES {
            errors.push(format!(
                "line {}: {:?} would take the vocabulary to {bytes} bytes; it holds at most {MAX_VOCABULARY_BYTES}",
                line.number, line.term
            ));
        }
    }
    if errors.is_empty() {
        Ok(Plan {
            added,
            skipped,
            entries,
        })
    } else {
        Err(errors)
    }
}

/// Runs a `vocabulary` subcommand.
pub fn run(cli: &Cli, client: &Client, what: &VocabularyCmd) -> Result<(), Failure> {
    let VocabularyCmd::Import { file, dry_run } = what;
    let text = std::fs::read_to_string(file).map_err(|e| {
        Failure::new(
            Exit::InvalidArgs,
            format!("cannot read {}: {e}", file.display()),
        )
    })?;
    let refused = |errors: Vec<String>| {
        Failure::new(
            Exit::InvalidArgs,
            format!(
                "{}: nothing was written\n  {}",
                file.display(),
                errors.join("\n  ")
            ),
        )
    };
    let lines = parse(&text).map_err(refused)?;
    let plan = plan(current(cli, client)?, lines).map_err(refused)?;
    let write = !dry_run && !plan.added.is_empty();
    if write {
        client.call("config.set", json!({"key": KEY, "value": plan.entries}))?;
    }
    if cli.quiet {
    } else if cli.json {
        println!("{}", report_json(&plan, *dry_run, write));
    } else {
        print!("{}", report(&plan, *dry_run));
    }
    Ok(())
}

/// The vocabulary in force, read after `config.validate` has caught the
/// daemon up with a file saved a moment ago, so the import never builds on
/// a list the file no longer holds.
fn current(cli: &Cli, client: &Client) -> Result<Vec<Value>, Failure> {
    let validated = client.call("config.validate", json!({}))?;
    if validated.get("ok") != Some(&Value::Bool(true)) {
        output::result(cli, "config.validate", &validated);
        return Err(Failure::reported(
            Exit::Failure,
            "configuration is invalid; nothing was imported",
        ));
    }
    let got = client.call("config.get", json!({"key": KEY}))?;
    got["entries"][0]["value"]
        .as_array()
        .cloned()
        .ok_or_else(|| Failure::new(Exit::Failure, format!("config.get {KEY} returned no list")))
}

fn report_json(plan: &Plan, dry_run: bool, written: bool) -> Value {
    json!({
        "dry_run": dry_run,
        "written": written,
        "added": plan.added.iter().map(|l| json!({"line": l.number, "entry": l.entry()})).collect::<Vec<_>>(),
        "skipped": plan.skipped.iter().map(|s| json!({"line": s.line, "term": s.term, "repeats_line": s.repeats})).collect::<Vec<_>>(),
        "entries": plan.entries.len(),
    })
}

/// The human report: what was (or would be) added, then each skipped line.
pub fn report(plan: &Plan, dry_run: bool) -> String {
    let mut out = String::new();
    let count = plan.added.len();
    let terms = if count == 1 { "term" } else { "terms" };
    let total = plan.entries.len();
    if count == 0 {
        out.push_str(&format!("nothing to add; {KEY} holds every term\n"));
    } else if dry_run {
        out.push_str(&format!(
            "would add {count} {terms} to {KEY} ({total} entries after)\n"
        ));
        for line in &plan.added {
            let spellings = if line.sounds_like.is_empty() {
                String::new()
            } else {
                format!(" (sounds like {})", line.sounds_like.join(", "))
            };
            out.push_str(&format!(
                "  line {}: {}{spellings}\n",
                line.number, line.term
            ));
        }
    } else {
        out.push_str(&format!(
            "added {count} {terms} to {KEY} ({total} entries)\n"
        ));
    }
    if !plan.skipped.is_empty() {
        let n = plan.skipped.len();
        let noun = if n == 1 { "duplicate" } else { "duplicates" };
        out.push_str(&format!("skipped {n} {noun}\n"));
        for s in &plan.skipped {
            let why = match s.repeats {
                Some(first) => format!("repeats line {first}"),
                None => "is already in the vocabulary".to_string(),
            };
            out.push_str(&format!("  line {}: {:?} {why}\n", s.line, s.term));
        }
    }
    out
}

#[cfg(test)]
#[path = "vocabulary_tests.rs"]
mod tests;
