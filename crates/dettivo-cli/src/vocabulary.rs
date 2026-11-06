//! `dettivo vocabulary import <file>`: a plain-text list of terms added to
//! `[dictation] vocabulary` in one `config.vocabulary.add` (ADR 0101). One
//! term per line, `term = a, b` to give it `sounds_like` spellings; blank
//! lines and lines starting with `#` are skipped. The daemon appends the
//! new entries to the file under its edit lock, skips a term the
//! vocabulary already holds, and refuses the whole list when an entry
//! breaks a limit; this module reads the file and maps each entry the
//! daemon names back to its line.

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::Cli;
use crate::client::Client;
use crate::exit::{Exit, Failure};

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
    /// The entry as the daemon takes it: the plain term, or the table
    /// when the line names spellings.
    pub fn entry(&self) -> Value {
        if self.sounds_like.is_empty() {
            Value::String(self.term.clone())
        } else {
            json!({"term": self.term, "sounds_like": self.sounds_like})
        }
    }
}

/// The entry lines of `text`, or one message per line that names no term.
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

/// Runs a `vocabulary` subcommand.
pub fn run(cli: &Cli, client: &Client, what: &VocabularyCmd) -> Result<(), Failure> {
    let VocabularyCmd::Import { file, dry_run } = what;
    let text = std::fs::read_to_string(file).map_err(|e| {
        Failure::new(
            Exit::InvalidArgs,
            format!("cannot read {}: {e}", file.display()),
        )
    })?;
    let nothing_written = |errors: Vec<String>| {
        format!(
            "{}: nothing was written\n  {}",
            file.display(),
            errors.join("\n  ")
        )
    };
    let lines = parse(&text).map_err(|e| Failure::new(Exit::InvalidArgs, nothing_written(e)))?;
    let entries: Vec<Value> = lines.iter().map(Line::entry).collect();
    let params = json!({"entries": entries, "dry_run": dry_run});
    let result = client
        .call("config.vocabulary.add", params)
        .map_err(|mut failure| {
            if let Some(refused) = refused_lines(&failure, &lines) {
                failure.message = nothing_written(refused);
            }
            failure
        })?;
    if cli.quiet {
    } else if cli.json {
        println!("{result}");
    } else {
        print!("{}", report(&result, &lines, *dry_run));
    }
    Ok(())
}

/// The daemon's refusal, each entry it names as `line N: <limit>`.
fn refused_lines(failure: &Failure, lines: &[Line]) -> Option<Vec<String>> {
    let refused = failure.rpc.as_ref()?.data.details.0.get("refused")?;
    let named = refused.as_array()?.iter().map(|r| {
        let line = r["index"]
            .as_u64()
            .and_then(|i| lines.get(i as usize))
            .map_or(0, |l| l.number);
        format!("line {line}: {}", r["message"].as_str().unwrap_or(""))
    });
    Some(named.collect())
}

/// The human report of a `config.vocabulary.add` result: what was (or
/// would be) added, then each skipped line.
pub fn report(result: &Value, lines: &[Line], dry_run: bool) -> String {
    let line_of = |index: &Value| index.as_u64().and_then(|i| lines.get(i as usize));
    let added: Vec<&Line> = result["added"]
        .as_array()
        .map(|a| a.iter().filter_map(line_of).collect())
        .unwrap_or_default();
    let total = result["entries"].as_u64().unwrap_or(0);
    let count = added.len();
    let terms = if count == 1 { "term" } else { "terms" };
    let mut out = String::new();
    if count == 0 {
        out.push_str(&format!("nothing to add; {KEY} holds every term\n"));
    } else if dry_run {
        out.push_str(&format!(
            "would add {count} {terms} to {KEY} ({total} entries after)\n"
        ));
        for line in &added {
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
    let skipped = result["skipped"].as_array().cloned().unwrap_or_default();
    if !skipped.is_empty() {
        let n = skipped.len();
        let noun = if n == 1 { "duplicate" } else { "duplicates" };
        out.push_str(&format!("skipped {n} {noun}\n"));
        for s in &skipped {
            let Some(line) = line_of(&s["index"]) else {
                continue;
            };
            let why = match line_of(&s["repeats"]) {
                Some(first) => format!("repeats line {}", first.number),
                None => "is already in the vocabulary".to_string(),
            };
            out.push_str(&format!("  line {}: {:?} {why}\n", line.number, line.term));
        }
    }
    out
}

#[cfg(test)]
#[path = "vocabulary_tests.rs"]
mod tests;
