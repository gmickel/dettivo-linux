//! `dettivo vocabulary suggest [term...] [--apply]` (ADR 0104): each term,
//! or every term of the vocabulary in force, goes to
//! `speech.vocabulary.suggest`, which has the selected speech engine hear
//! it spoken by two Piper voices. The terms it hears right need no entry;
//! for the rest the command prints the spellings it heard, and writes them
//! as `sounds_like` only with `--apply`, through `config.vocabulary.add`
//! with `extend`, so an entry the file holds gains them in place.

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
    let entries = entries(&results);
    let applied = if apply && !entries.is_empty() {
        let params = json!({ "entries": entries, "extend": true });
        Some(client.call("config.vocabulary.add", params)?)
    } else {
        None
    };
    if cli.json {
        println!("{}", json!({ "suggestions": results, "applied": applied }));
    } else if human {
        print!("{}", summary(entries.len(), applied.as_ref()));
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

/// The entries `--apply` writes: each term with spellings to propose.
pub fn entries(results: &[Value]) -> Vec<Value> {
    results
        .iter()
        .filter(|r| r["sounds_like"].as_array().is_some_and(|s| !s.is_empty()))
        .map(|r| json!({ "term": r["term"], "sounds_like": r["sounds_like"] }))
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

/// The closing line: what `--apply` wrote, or that nothing was written.
pub fn summary(proposed: usize, applied: Option<&Value>) -> String {
    let count = |key: &str| applied.and_then(|a| a[key].as_array()).map_or(0, Vec::len);
    let terms = |n: usize| if n == 1 { "term" } else { "terms" };
    match applied {
        _ if proposed == 0 => "nothing to write\n".to_string(),
        None => format!(
            "nothing written; run again with --apply to write the spellings of {proposed} {}\n",
            terms(proposed)
        ),
        Some(_) => {
            let (added, extended) = (count("added"), count("extended"));
            format!(
                "wrote sounds_like for {} {}: {added} added to dictation.vocabulary, {extended} already in it\n",
                added + extended,
                terms(added + extended)
            )
        }
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
    fn only_terms_with_spellings_are_written_and_the_summary_says_what_happened() {
        let results = vec![
            result("Clef", true, &[]),
            result("Hyprland", false, &["hyper land"]),
            result("Dettivo", false, &[]),
        ];
        assert_eq!(
            entries(&results),
            vec![json!({"term": "Hyprland", "sounds_like": ["hyper land"]})]
        );
        assert_eq!(
            summary(1, None),
            "nothing written; run again with --apply to write the spellings of 1 term\n"
        );
        assert_eq!(summary(0, None), "nothing to write\n");
        let applied =
            json!({"added": [1], "extended": [0], "skipped": [], "entries": 3, "written": true});
        assert_eq!(
            summary(2, Some(&applied)),
            "wrote sounds_like for 2 terms: 1 added to dictation.vocabulary, 1 already in it\n"
        );
    }
}
