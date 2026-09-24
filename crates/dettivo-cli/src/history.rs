//! `dettivo history <verb>`: the `transcripts.*` methods as a person uses
//! them; `export --out` and `import` drive `transfer.*` through
//! `crate::transfer`. Human output is a table or the text; `--json` prints
//! the daemon's result.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Subcommand;
use serde_json::{Value, json};

use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// `dettivo history <what>`.
#[derive(Debug, Subcommand)]
pub enum HistoryCmd {
    /// transcripts.list: the newest items first.
    List {
        /// Items to show.
        #[arg(long, default_value_t = 20)]
        limit: u32,
        /// Only items inserted into this app id.
        #[arg(long)]
        app: Option<String>,
        /// Only items created at or after this date or time (ISO 8601).
        #[arg(long)]
        since: Option<String>,
    },
    /// transcripts.get: the final text of one item, or its segments and words.
    Get {
        /// The item id.
        id: String,
        /// Print the segments with their timestamps and words instead of the text.
        #[arg(long)]
        words: bool,
    },
    /// transcripts.cancel: stop the import or re-run job on an item between chunks.
    Cancel {
        /// The item id.
        id: String,
    },
    /// transcripts.latest, then its text.
    Latest,
    /// transcripts.search: word-start matches over every text field.
    Search {
        /// The words to look for.
        query: String,
        /// Hits to show.
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// transcripts.delete: the row and its artifacts per `[history] artifacts`.
    Delete {
        /// The item id.
        id: String,
    },
    /// transcripts.rerun: transcribe the retained audio again.
    Rerun {
        /// The item id.
        id: String,
        /// Model id (default: the selection in force).
        #[arg(long)]
        model: Option<String>,
        /// Provider (default: the item's).
        #[arg(long)]
        provider: Option<String>,
        /// Mode (raw).
        #[arg(long)]
        mode: Option<String>,
        /// Return as soon as the job started instead of waiting for it.
        #[arg(long)]
        no_wait: bool,
    },
    /// transcripts.export through a download transfer into a file.
    Export {
        /// item, range or all.
        #[arg(long, default_value = "all")]
        scope: String,
        /// json, markdown (md), txt or zip.
        #[arg(long, default_value = "json")]
        format: String,
        /// The file to write.
        #[arg(long)]
        out: PathBuf,
        /// The item id for scope item.
        #[arg(long)]
        id: Option<String>,
        /// The range start for scope range (ISO 8601).
        #[arg(long)]
        from: Option<String>,
        /// The range end for scope range (ISO 8601, exclusive).
        #[arg(long)]
        to: Option<String>,
    },
    /// Upload a file through transfer.* and transcripts.import it, following the job's progress.
    Import {
        /// An audio file (wav, mp3, m4a, aac, caf, aiff, flac, ogg) or a Dettivo archive (.zip).
        file: PathBuf,
        /// Language code (default en).
        #[arg(long, default_value = "en")]
        language: String,
        /// Mode (raw).
        #[arg(long, default_value = "raw")]
        mode: String,
        /// Provider (default: the configured speech provider).
        #[arg(long)]
        provider: Option<String>,
        /// Model id (default: the configured speech model).
        #[arg(long)]
        model: Option<String>,
        /// Return as soon as the job started instead of waiting for it.
        #[arg(long)]
        no_wait: bool,
    },
}

pub(crate) fn reference(id: &str) -> Value {
    json!({"kind": "dictation", "id": id})
}

pub(crate) fn print(cli: &Cli, method: &str, value: &Value, human: impl FnOnce() -> String) {
    if cli.quiet {
        return;
    }
    if cli.json {
        output::result(cli, method, value);
    } else {
        print!("{}", human());
    }
}

/// Runs a `history` subcommand.
pub fn run(cli: &Cli, client: &Client, what: &HistoryCmd) -> Result<(), Failure> {
    match what {
        HistoryCmd::List { limit, app, since } => {
            let mut params = json!({"kinds": ["dictation"], "limit": limit, "cursor": null});
            if let Some(app) = app {
                params["app_id"] = json!(app);
            }
            if let Some(since) = since {
                params["since"] = json!(since);
            }
            let result = client.call("transcripts.list", params)?;
            print(cli, "transcripts.list", &result, || table(&result));
            Ok(())
        }
        HistoryCmd::Get { id, words } => {
            let result = client.call("transcripts.get", json!({"ref": reference(id)}))?;
            print(cli, "transcripts.get", &result, || {
                if *words {
                    segments_of(&result)
                } else {
                    text_of(&result)
                }
            });
            Ok(())
        }
        HistoryCmd::Cancel { id } => {
            let result = client.call("transcripts.cancel", json!({"ref": reference(id)}))?;
            print(cli, "transcripts.cancel", &result, || {
                format!(
                    "cancelled {id} ({})\n",
                    result["job"]["job_id"].as_str().unwrap_or("")
                )
            });
            Ok(())
        }
        HistoryCmd::Latest => {
            let latest = client.call("transcripts.latest", json!({"kind": "dictation"}))?;
            let result = client.call("transcripts.get", json!({"ref": latest["ref"]}))?;
            print(cli, "transcripts.get", &result, || {
                format!(
                    "{}\n{}",
                    latest["ref"]["id"].as_str().unwrap_or(""),
                    text_of(&result)
                )
            });
            Ok(())
        }
        HistoryCmd::Search { query, limit } => {
            let result = client.call(
                "transcripts.search",
                json!({"query": query, "kinds": ["dictation"], "limit": limit}),
            )?;
            print(cli, "transcripts.search", &result, || {
                let mut out = String::new();
                for hit in result["items"].as_array().into_iter().flatten() {
                    out.push_str(&format!(
                        "{}  {}\n",
                        hit["ref"]["id"].as_str().unwrap_or(""),
                        hit["snippet"].as_str().unwrap_or("").replace('\n', " ")
                    ));
                }
                if out.is_empty() {
                    out.push_str("no matches\n");
                }
                out
            });
            Ok(())
        }
        HistoryCmd::Delete { id } => {
            let result = client.call("transcripts.delete", json!({"ref": reference(id)}))?;
            print(cli, "transcripts.delete", &result, || {
                format!("deleted {id}\n")
            });
            Ok(())
        }
        HistoryCmd::Rerun {
            id,
            model,
            provider,
            mode,
            no_wait,
        } => {
            let mut params = json!({"ref": reference(id)});
            for (key, value) in [("model", model), ("provider", provider), ("mode", mode)] {
                if let Some(v) = value {
                    params[key] = json!(v);
                }
            }
            let result = client.call("transcripts.rerun", params)?;
            let new_id = result["ref"]["id"].as_str().unwrap_or("").to_string();
            if *no_wait {
                print(cli, "transcripts.rerun", &result, || {
                    format!("{} {new_id} (re-run of {id})\n", result["job"]["job_id"])
                });
                return Ok(());
            }
            let row = wait_settled(client, &new_id)?;
            settled_success(&row, "re-run")?;
            let got = client.call("transcripts.get", json!({"ref": reference(&new_id)}))?;
            print(cli, "transcripts.get", &got, || {
                format!(
                    "{new_id} {}\n{}",
                    row["status"].as_str().unwrap_or(""),
                    text_of(&got)
                )
            });
            Ok(())
        }
        HistoryCmd::Export {
            scope,
            format,
            out,
            id,
            from,
            to,
        } => crate::transfer::export(
            cli,
            client,
            scope,
            format,
            out,
            id.as_deref(),
            from.as_deref(),
            to.as_deref(),
        ),
        HistoryCmd::Import {
            file,
            language,
            mode,
            provider,
            model,
            no_wait,
        } => crate::transfer::import(
            cli,
            client,
            file,
            language,
            mode,
            provider.as_deref(),
            model.as_deref(),
            *no_wait,
        ),
    }
}

/// The listing as a table.
fn table(result: &Value) -> String {
    let mut out = String::new();
    for row in result["items"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}  {}  {:>4}s  {:<12} {}\n",
            row["ref"]["id"].as_str().unwrap_or(""),
            row["started_at"].as_str().unwrap_or(""),
            row["duration_seconds"].as_u64().unwrap_or(0),
            row["status"].as_str().unwrap_or(""),
            row["title"].as_str().unwrap_or("")
        ));
    }
    if out.is_empty() {
        out.push_str("no dictations yet\n");
    }
    out
}

/// `mm:ss.mmm` of a millisecond offset.
pub(crate) fn clock(ms: u64) -> String {
    format!(
        "{:02}:{:02}.{:03}",
        ms / 60_000,
        (ms / 1000) % 60,
        ms % 1000
    )
}

/// The segments with their timestamps, the words indented under each.
pub(crate) fn segments_of(result: &Value) -> String {
    let mut out = String::new();
    for s in result["segments"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "[{} - {}] {}\n",
            clock(s["start_ms"].as_u64().unwrap_or(0)),
            clock(s["end_ms"].as_u64().unwrap_or(0)),
            s["text"].as_str().unwrap_or("")
        ));
        for w in s["words"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "  {} - {}  {}\n",
                clock(w["start_ms"].as_u64().unwrap_or(0)),
                clock(w["end_ms"].as_u64().unwrap_or(0)),
                w["text"].as_str().unwrap_or("")
            ));
        }
    }
    if out.is_empty() {
        out.push_str("no segments stored (a session dictation carries text only)\n");
    }
    out
}

/// The final text, ending in a newline.
pub(crate) fn text_of(result: &Value) -> String {
    let mut text = result["text_polish"].as_str().unwrap_or("").to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// The authoritative item facts, independent of listing pagination.
pub(crate) fn settled_row(client: &Client, id: &str) -> Result<Value, Failure> {
    let result = client.call("transcripts.get", json!({"ref": reference(id)}))?;
    let facts = &result["facts"];
    if facts["status"].as_str().is_none() {
        return Err(Failure::new(
            Exit::Failure,
            "transcripts.get omitted item status",
        ));
    }
    Ok(facts.clone())
}

/// Failed and cancelled jobs are failures before any success output.
pub(crate) fn settled_success(row: &Value, operation: &str) -> Result<(), Failure> {
    if matches!(row["status"].as_str(), Some("failed" | "cancelled")) {
        return Err(Failure::new(
            Exit::Failure,
            format!("the {operation} {}", row["status"].as_str().unwrap()),
        ));
    }
    Ok(())
}

/// Polls the item until it leaves `transcribing`.
pub(crate) fn wait_settled(client: &Client, id: &str) -> Result<Value, Failure> {
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        let row = settled_row(client, id)?;
        if row["status"] != "transcribing" {
            return Ok(row);
        }
        if Instant::now() > deadline {
            return Err(Failure::new(
                Exit::Timeout,
                format!("{id} is still transcribing"),
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_and_texts_render() {
        assert!(table(&json!({"items": []})).contains("no dictations yet"));
        let row = json!({"items": [{"ref": {"id": "a"}, "started_at": "t", "duration_seconds": 3, "status": "completed", "title": "Hi"}]});
        assert_eq!(table(&row), "a  t     3s  completed    Hi\n");
        assert_eq!(text_of(&json!({"text_polish": "Hi"})), "Hi\n");
        assert_eq!(clock(61_005), "01:01.005");
        let got = json!({"segments": [{"start_ms": 500, "end_ms": 1200, "text": "Hello team.", "words": [{"start_ms": 500, "end_ms": 800, "text": "Hello"}]}]});
        assert_eq!(
            segments_of(&got),
            "[00:00.500 - 00:01.200] Hello team.\n  00:00.500 - 00:00.800  Hello\n"
        );
        assert!(segments_of(&json!({"segments": []})).starts_with("no segments"));
    }
}
