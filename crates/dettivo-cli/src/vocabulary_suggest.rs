//! `dettivo vocabulary suggest [term...] [--apply]` (ADR 0104): each term,
//! or every term of the vocabulary in force, goes to
//! `speech.vocabulary.suggest`, which has the selected speech engine hear
//! it spoken by two Piper voices. The terms it hears right need no
//! spelling; for the rest the command prints the spellings it heard. Only
//! `--apply` writes, through `config.vocabulary.add` with `extend`: a term
//! the file lacks is added, and an entry it holds gains the spellings in
//! place. Without it the same call runs as a dry run to say what would be.

use std::time::Duration;

use serde_json::{Value, json};

use crate::Cli;
use crate::client::Client;
use crate::exit::{Exit, Failure};

/// What one term takes at most: four clips spoken and transcribed, the
/// first of them loading the model.
const PER_TERM: Duration = Duration::from_secs(600);

/// Runs `vocabulary suggest`.
pub fn run(cli: &Cli, client: &Client, terms: &[String], apply: bool) -> Result<(), Failure> {
    let terms = if terms.is_empty() {
        vocabulary_terms(client)?
    } else {
        terms.to_vec()
    };
    if terms.is_empty() {
        return Err(Failure::new(
            Exit::InvalidArgs,
            "no terms: name some, or add them to dictation.vocabulary first",
        ));
    }
    let mut slow = client.clone();
    slow.timeout = slow.timeout.max(PER_TERM);
    let human = !cli.json && !cli.quiet;
    let mut results = Vec::new();
    for term in &terms {
        let result = slow.call("speech.vocabulary.suggest", json!({ "term": term }))?;
        if human {
            print!("{}", line(&result));
        }
        results.push(result);
    }
    // Without --apply the daemon answers the same call as a dry run, so
    // the closing line says exactly what --apply writes.
    let params = json!({ "entries": entries(&results), "extend": true, "dry_run": !apply });
    let change = client.call("config.vocabulary.add", params)?;
    if cli.json {
        println!("{}", json!({ "suggestions": results, "change": change }));
    } else if human {
        print!("{}", summary(&change, apply));
    }
    Ok(())
}

/// The terms of `dictation.vocabulary` in force, in list order.
fn vocabulary_terms(client: &Client) -> Result<Vec<String>, Failure> {
    let got = client.call("config.get", json!({ "key": "dictation.vocabulary" }))?;
    let list = got["entries"][0]["value"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|e| e.as_str().or_else(|| e["term"].as_str()))
        .map(str::to_string)
        .collect())
}

/// The entries `--apply` writes: every term heard, with the spellings
/// proposed for it. The vocabulary skips a term it holds that gains no
/// spelling, and a term it lacks is added, since the clips were heard with
/// it in the vocabulary.
pub fn entries(results: &[Value]) -> Vec<Value> {
    results
        .iter()
        .map(|r| match r["sounds_like"].as_array() {
            Some(s) if !s.is_empty() => json!({ "term": r["term"], "sounds_like": s }),
            _ => r["term"].clone(),
        })
        .collect()
}

/// One term's line: heard right, the spellings heard, or neither.
pub fn line(result: &Value) -> String {
    let term = result["term"].as_str().unwrap_or("");
    let spellings: Vec<String> = result["sounds_like"]
        .as_array()
        .map(|s| {
            s.iter()
                .filter_map(Value::as_str)
                .map(|s| format!("{s:?}"))
                .collect()
        })
        .unwrap_or_default();
    if result["recognised"] == json!(true) {
        format!("{term}: heard right; needs no sounds_like\n")
    } else if spellings.is_empty() {
        format!("{term}: misheard, with no new spelling to propose\n")
    } else {
        format!("{term}: heard as {}\n", spellings.join(", "))
    }
}

/// The closing line, from `config.vocabulary.add`'s answer: what was
/// written, or with a dry run what `--apply` would write.
pub fn summary(change: &Value, applied: bool) -> String {
    let count = |key: &str| change[key].as_array().map_or(0, Vec::len);
    let terms = |n: usize| if n == 1 { "term" } else { "terms" };
    let (added, extended) = (count("added"), count("extended"));
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!(
            "{added} new {} to dictation.vocabulary",
            terms(added)
        ));
    }
    if extended > 0 {
        parts.push(format!(
            "new sounds_like for {extended} {} already in dictation.vocabulary",
            terms(extended)
        ));
    }
    match (parts.is_empty(), applied) {
        (true, _) => "nothing to write\n".to_string(),
        (false, true) => format!("wrote {}\n", parts.join(" and ")),
        (false, false) => format!(
            "nothing written; --apply would write {}\n",
            parts.join(" and ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(term: &str, recognised: bool, sounds_like: &[&str]) -> Value {
        json!({"term": term, "recognised": recognised, "sounds_like": sounds_like})
    }

    #[test]
    fn each_term_reads_right_heard_as_or_misheard() {
        assert_eq!(
            line(&result("Clef", true, &[])),
            "Clef: heard right; needs no sounds_like\n"
        );
        assert_eq!(
            line(&result("Hyprland", false, &["hyper land", "hyperland"])),
            "Hyprland: heard as \"hyper land\", \"hyperland\"\n"
        );
        assert_eq!(
            line(&result("Dettivo", false, &[])),
            "Dettivo: misheard, with no new spelling to propose\n"
        );
    }

    #[test]
    fn every_term_is_an_entry_and_the_summary_reads_the_daemons_answer() {
        let results = vec![
            result("Clef", true, &[]),
            result("Hyprland", false, &["hyper land"]),
        ];
        assert_eq!(
            entries(&results),
            vec![
                json!("Clef"),
                json!({"term": "Hyprland", "sounds_like": ["hyper land"]})
            ]
        );
        let change =
            json!({"added": [0], "extended": [1], "skipped": [], "entries": 3, "written": false});
        assert_eq!(
            summary(&change, false),
            "nothing written; --apply would write 1 new term to dictation.vocabulary and new sounds_like for 1 term already in dictation.vocabulary\n"
        );
        assert_eq!(
            summary(&change, true),
            "wrote 1 new term to dictation.vocabulary and new sounds_like for 1 term already in dictation.vocabulary\n"
        );
        let none = json!({"added": [], "skipped": [{"index": 0, "repeats": null}], "entries": 3, "written": false});
        assert_eq!(summary(&none, true), "nothing to write\n");
    }
}
