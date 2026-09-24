//! `dettivo history export --out` and `dettivo history import`: the CLI
//! drives `transfer.*` itself, pulling a bound download chunk by chunk into
//! a file and pushing a file up in order before `transcripts.import`.

use std::path::Path;
use std::time::{Duration, Instant};

use dettivo_proto::transfer_io::{self, Failure as TransferFailure};
use serde_json::{Value, json};

use crate::Cli;
use crate::client::Client;
use crate::exit::{Exit, Failure};
use crate::history::{print, reference, settled_row, settled_success, text_of, wait_settled};

#[allow(clippy::too_many_arguments)]
pub(crate) fn export(
    cli: &Cli,
    client: &Client,
    scope: &str,
    format: &str,
    out: &Path,
    id: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(), Failure> {
    let format = match format {
        "json" => "json",
        "md" | "markdown" => "md",
        "txt" | "text" => "txt",
        "zip" => "zip",
        other => {
            return Err(Failure::new(
                Exit::InvalidArgs,
                format!("{other} is not an export format (json, markdown, txt, zip)"),
            ));
        }
    };
    let mut params = json!({"format": format, "scope": scope});
    match scope {
        "item" => {
            let id = id.ok_or_else(|| {
                Failure::new(Exit::InvalidArgs, "--id is required for --scope item")
            })?;
            params["ref"] = reference(id);
        }
        "range" => {
            params["from"] = json!(from.ok_or_else(|| Failure::new(
                Exit::InvalidArgs,
                "--from is required for --scope range"
            ))?);
            params["to"] = json!(to.ok_or_else(|| Failure::new(
                Exit::InvalidArgs,
                "--to is required for --scope range"
            ))?);
        }
        "all" => {}
        other => {
            return Err(Failure::new(
                Exit::InvalidArgs,
                format!("{other} is not a scope (item, range, all)"),
            ));
        }
    }
    download(cli, client, params, out)
}

/// Begins a download transfer, runs `transcripts.export` with `params`
/// (the transfer id filled in), pulls every chunk into `out` and commits.
pub(crate) fn download(
    cli: &Cli,
    client: &Client,
    mut params: Value,
    out: &Path,
) -> Result<(), Failure> {
    let begun = client.call(
        "transfer.begin",
        json!({"direction": "download", "content_type": "application/octet-stream", "size_hint": 0}),
    )?;
    let transfer = begun["transfer_id"].as_str().unwrap_or("").to_string();
    let call = |method: &str, params| client.call(method, params);
    let (exported, bytes) = transfer_io::guarded(call, &transfer, || {
        params["transfer_id"] = json!(transfer);
        let exported = call("transcripts.export", params)
            .map_err(|e| TransferFailure::remote("export_failed", e))?;
        let bytes = transfer_io::download_file(call, &transfer, out)?;
        Ok((exported, bytes))
    })
    .map_err(transfer_error)?;
    print(cli, "transcripts.export", &exported, || {
        format!(
            "wrote {} ({} bytes, {})\n",
            out.display(),
            bytes,
            exported["content_type"]
        )
    });
    Ok(())
}

/// The content type an import file declares, from its extension.
pub fn content_type_of(file: &Path) -> Option<&'static str> {
    match file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav") => Some("audio/wav"),
        Some("mp3") => Some("audio/mpeg"),
        Some("mp4") => Some("audio/mp4"),
        Some("m4a") => Some("audio/m4a"),
        Some("aac") => Some("audio/aac"),
        Some("caf") => Some("audio/caf"),
        Some("aif" | "aiff") => Some("audio/aiff"),
        Some("flac") => Some("audio/flac"),
        Some("ogg" | "oga" | "opus") => Some("audio/ogg"),
        Some("zip") => Some("application/zip"),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn import(
    cli: &Cli,
    client: &Client,
    file: &Path,
    language: &str,
    mode: &str,
    provider: Option<&str>,
    model: Option<&str>,
    no_wait: bool,
) -> Result<(), Failure> {
    let content_type = content_type_of(file).ok_or_else(|| {
        Failure::new(
            Exit::InvalidArgs,
            format!("{}: not an audio file or Dettivo archive", file.display()),
        )
    })?;
    let mut reader = std::fs::File::open(file).map_err(|e| {
        Failure::new(
            Exit::InvalidArgs,
            format!("cannot read {}: {e}", file.display()),
        )
    })?;
    let size = reader
        .metadata()
        .map_err(|e| Failure::new(Exit::InvalidArgs, e.to_string()))?
        .len();
    let begun = client.call(
        "transfer.begin",
        json!({"direction":"upload", "content_type":content_type, "size_hint":size}),
    )?;
    let transfer = begun["transfer_id"].as_str().unwrap_or("").to_string();
    let call = |method: &str, params| client.call(method, params);
    let (result, progress) = transfer_io::guarded(call, &transfer, || {
        let chunk_bytes = upload_chunk_bytes(client, &begun, &transfer)
            .map_err(|e| TransferFailure::remote("stream_chunk_failed", e))?;
        transfer_io::upload(call, &transfer, &mut reader, chunk_bytes)?;
        let filename = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "import".into());
        // The subscription opens before the import starts so no progress
        // event is missed; the read timeout lets the loop poll the item.
        let progress = (!no_wait)
            .then(|| client.subscribe(&["job.progress"], Duration::from_millis(500)))
            .transpose().map_err(|e| TransferFailure::remote("import_failed", e))?;
        let mut params = json!({"transfer_id": transfer, "target_kind": "dictation", "filename": filename, "language": language, "mode": mode});
        if let Some(p) = provider {
            params["provider"] = json!(p);
        }
        if let Some(m) = model {
            params["model"] = json!(m);
        }
        let result = call("transcripts.import", params)
            .map_err(|e| TransferFailure::remote("import_failed", e))?;
        Ok((result, progress))
    })
    .map_err(transfer_error)?;
    let id = result["ref"]["id"].as_str().unwrap_or("").to_string();
    if no_wait || result["job"]["state"] == "succeeded" {
        print(cli, "transcripts.import", &result, || {
            format!(
                "{} {id} {}\n",
                result["job"]["job_id"],
                result["job"]["message"]
                    .as_str()
                    .unwrap_or(result["job"]["state"].as_str().unwrap_or(""))
            )
        });
        return Ok(());
    }
    let job_id = result["job"]["job_id"].as_str().unwrap_or("").to_string();
    let row = match progress {
        Some(reader) => follow(cli, client, reader, &job_id, &id)?,
        None => wait_settled(client, &id)?,
    };
    settled_success(&row, "import")?;
    let got = client.call("transcripts.get", json!({"ref": reference(&id)}))?;
    print(cli, "transcripts.get", &got, || {
        format!(
            "{id} {}\n{}",
            row["status"].as_str().unwrap_or(""),
            text_of(&got)
        )
    });
    Ok(())
}

fn transfer_error(error: transfer_io::Error<Failure>) -> Failure {
    match error {
        transfer_io::Error::Remote(e) => e,
        transfer_io::Error::Local(e) => Failure::new(Exit::Failure, e.to_string()),
    }
}

fn upload_chunk_bytes(client: &Client, begin: &Value, transfer: &str) -> Result<usize, Failure> {
    let config = client.call("config.get", json!({"key":"ipc.max_line_bytes"}))?;
    let size = begin["chunk_max_bytes"]
        .as_u64()
        .zip(config["entries"][0]["value"].as_u64())
        .and_then(|(raw, cap)| {
            dettivo_proto::upload::chunk_bytes(raw, cap, "1", transfer, client.token.as_deref())
        });
    size.ok_or_else(|| {
        Failure::new(
            Exit::Failure,
            "upload limits cannot fit a transfer.chunk request",
        )
    })
}

/// How many idle reads (500 ms each) a settled item waits for the job's
/// closing events before the follower gives up on them. The daemon stores
/// the item before it publishes `done`, so the row settles first and the
/// last events are still in flight.
const CLOSING_READS: u32 = 8;

/// Prints one progress line per `job.progress` event of `job_id` until
/// the job reports its end, polling the item between events; returns the
/// item's listing row once it has settled and the job's closing events
/// have arrived (or stopped coming).
fn follow(
    cli: &Cli,
    client: &Client,
    mut reader: dettivo_proto::transport::Reader,
    job_id: &str,
    id: &str,
) -> Result<Value, Failure> {
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut ended = false;
    let mut closed = false;
    let mut idle = 0u32;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                ended = true;
                closed = true;
            }
            Ok(_) => {
                idle = 0;
                let event: Value = serde_json::from_str(line.trim_end()).unwrap_or(Value::Null);
                let payload = &event["params"]["payload"];
                if event["params"]["topic"] == "job.progress" && payload["job_id"] == job_id {
                    let stage = payload["stage"].as_str().unwrap_or("");
                    if !cli.json && !cli.quiet {
                        match (
                            payload["chunks_done"].as_u64(),
                            payload["chunks_total"].as_u64(),
                        ) {
                            (Some(done), Some(total)) => {
                                eprintln!("{job_id} {stage} chunk {done}/{total}")
                            }
                            _ => eprintln!("{job_id} {stage}"),
                        }
                    }
                    ended |= matches!(stage, "done" | "failed" | "cancelled");
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                idle += 1
            }
            Err(e) => {
                return Err(Failure::new(
                    Exit::Unavailable,
                    format!("event stream: {e}"),
                ));
            }
        }
        let row = settled_row(client, id)?;
        let settled = row["status"] != "transcribing";
        if settled && (ended || idle >= CLOSING_READS) {
            return Ok(row);
        }
        if closed || Instant::now() > deadline {
            return wait_settled(client, id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_follow_the_extension() {
        assert_eq!(content_type_of(Path::new("a.WAV")), Some("audio/wav"));
        assert_eq!(content_type_of(Path::new("call.m4a")), Some("audio/m4a"));
        assert_eq!(content_type_of(Path::new("x.zip")), Some("application/zip"));
        assert_eq!(content_type_of(Path::new("x.txt")), None);
    }
}
