//! `dettivo speech <what>`: engines, providers, the selection and the
//! model downloads, each one contract method.

use clap::Subcommand;
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::strip_null_key;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// `dettivo speech <what>`: the `speech.*` methods that answer today.
#[derive(Debug, Subcommand)]
pub enum SpeechCmd {
    /// speech.engines: every engine binary, found or not, running or not.
    Engines,
    /// speech.providers.list: providers, their models and readiness.
    Providers,
    /// speech.selection.get / set: the dictation and meeting model.
    Selection {
        /// get (default) or set.
        #[command(subcommand)]
        what: Option<SelectionCmd>,
    },
    /// speech.models.download: fetch and verify a catalogue model.
    Download {
        /// Model id, e.g. large-v3-turbo.
        #[arg(long)]
        model: String,
        /// Provider (default whisper).
        #[arg(long, default_value = "whisper")]
        provider: String,
        /// Print progress until the download ends.
        #[arg(long)]
        wait: bool,
    },
    /// speech.models.cancel: stop a download, keeping the partial file.
    Cancel {
        /// Model id.
        #[arg(long)]
        model: String,
        /// Provider (default whisper).
        #[arg(long, default_value = "whisper")]
        provider: String,
    },
    /// speech.models.delete: remove a model from disk.
    Delete {
        /// Model id.
        #[arg(long)]
        model: String,
        /// Provider (default whisper).
        #[arg(long, default_value = "whisper")]
        provider: String,
        /// Delete even the selected model.
        #[arg(long)]
        force: bool,
    },
    /// speech.models.status: every model's readiness.
    Status {
        /// Only this provider.
        #[arg(long)]
        provider: Option<String>,
        /// Print again every second while a download runs.
        #[arg(long)]
        follow: bool,
    },
}

/// `dettivo speech selection <get|set>`.
#[derive(Debug, Subcommand)]
pub enum SelectionCmd {
    /// Print the selection in force.
    Get,
    /// Change the provider, the model or the meeting model.
    Set {
        /// Provider id (whisper).
        #[arg(long)]
        provider: Option<String>,
        /// Dictation model id.
        #[arg(long)]
        model: Option<String>,
        /// Meeting model id (empty string means the dictation model).
        #[arg(long)]
        meeting_model: Option<String>,
        /// Parakeet model id.
        #[arg(long)]
        parakeet_model: Option<String>,
    },
}

/// Runs a `speech` subcommand.
pub fn run(cli: &Cli, client: &Client, what: &SpeechCmd) -> Result<(), Failure> {
    let (method, params) = match what {
        SpeechCmd::Engines => ("speech.engines", json!({})),
        SpeechCmd::Providers => ("speech.providers.list", json!({})),
        SpeechCmd::Selection { what: None }
        | SpeechCmd::Selection {
            what: Some(SelectionCmd::Get),
        } => ("speech.selection.get", json!({})),
        SpeechCmd::Selection {
            what:
                Some(SelectionCmd::Set {
                    provider,
                    model,
                    meeting_model,
                    parakeet_model,
                }),
        } => (
            "speech.selection.set",
            strip_null_key(json!({
                "provider": provider,
                "model": model,
                "meeting_model": meeting_model,
                "parakeet_model_id": parakeet_model,
            })),
        ),
        SpeechCmd::Download {
            model,
            provider,
            wait,
        } => {
            let params = json!({ "provider": provider, "model": model });
            let result = client.call("speech.models.download", params.clone())?;
            if !*wait {
                output::result(cli, "speech.models.download", &result);
                return Ok(());
            }
            return follow_download(cli, client, "speech.models.status", params);
        }
        SpeechCmd::Cancel { model, provider } => (
            "speech.models.cancel",
            json!({ "provider": provider, "model": model }),
        ),
        SpeechCmd::Delete {
            model,
            provider,
            force,
        } => (
            "speech.models.delete",
            json!({ "provider": provider, "model": model, "force": force }),
        ),
        SpeechCmd::Status { provider, follow } => {
            let params = strip_null_key(json!({ "provider": provider }));
            if *follow {
                return follow_status(cli, client, params);
            }
            ("speech.models.status", params)
        }
    };
    let result = client.call(method, params)?;
    output::result(cli, method, &result);
    Ok(())
}

/// Polls one model until its download ends, printing progress lines;
/// `status_method` is `speech.models.status` or `llm.models.status`.
pub(crate) fn follow_download(
    cli: &Cli,
    client: &Client,
    status_method: &str,
    params: Value,
) -> Result<(), Failure> {
    loop {
        let status = client.call(
            status_method,
            crate::commands::strip_nulls(
                json!({ "provider": params["provider"], "model": params["model"] }),
            ),
        )?;
        let row = status["models"].get(0).cloned().unwrap_or(Value::Null);
        let readiness = row["readiness"].as_str().unwrap_or("").to_string();
        if !cli.quiet && !cli.json {
            println!(
                "{} {}/{} {} of {} bytes",
                readiness,
                row["provider"].as_str().unwrap_or(""),
                row["id"].as_str().unwrap_or(""),
                row["bytes_done"],
                row["bytes_total"]
            );
        }
        if readiness != "downloading" {
            output::result(cli, status_method, &row);
            return if readiness == "ready" {
                Ok(())
            } else {
                Err(Failure::reported(
                    Exit::Failure,
                    format!("download ended {readiness}"),
                ))
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Prints the status every second while a download runs.
fn follow_status(cli: &Cli, client: &Client, params: Value) -> Result<(), Failure> {
    loop {
        let result = client.call("speech.models.status", params.clone())?;
        output::result(cli, "speech.models.status", &result);
        let running = result["models"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|r| r["readiness"] == "downloading"));
        if !running {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
