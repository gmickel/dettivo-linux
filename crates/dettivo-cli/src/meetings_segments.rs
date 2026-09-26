//! `dettivo meetings segments <id> [--since <cursor>] [--follow]`: the
//! transcript of a meeting as lines on the terminal, one per segment
//! with its side, its span on the meeting clock and, once the speaker
//! pass named one, the speaker (ADR 0031). During a meeting it prints the
//! transcript so far (`meetings.segments`, ADR 0071), provisional lines
//! marked with `~`; `--since` prints only what is new after a cursor and
//! `--json` prints the whole answer, cursor included. `--follow`
//! subscribes first, prints the backlog, then streams the
//! `meeting.segment` events without a gap or a duplicate until the
//! meeting settles, then prints the finalised transcript.

use std::collections::HashSet;
use std::time::Duration;

use serde_json::{Value, json};

use crate::Cli;
use crate::client::Client;
use crate::exit::Failure;

/// `mm:ss.mmm` on the meeting clock.
pub fn clock(ms: u64) -> String {
    format!(
        "{:02}:{:02}.{:03}",
        ms / 60_000,
        (ms / 1000) % 60,
        ms % 1000
    )
}

/// One line for a segment of a `meetings.segments` answer or a
/// `meeting.segment` payload: `~` marks a provisional segment a later
/// window may still replace; the side (`you`, `remote`) always shows,
/// the speaker's name too once it says more than the side.
pub fn line(segment: &Value) -> String {
    let mark = if segment["provisional"] == json!(true) {
        "~"
    } else {
        " "
    };
    let side = match segment["source"].as_str() {
        Some("remote") => "remote",
        _ => "you",
    };
    let gap = segment["gap_before_ms"]
        .as_u64()
        .map(|g| format!("  [gap {} ms]", g))
        .unwrap_or_default();
    let speaker = segment["speaker"]
        .as_str()
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case(side))
        .map(|s| format!("{s}: "))
        .unwrap_or_default();
    format!(
        "{mark}{side:<6} {}-{}{gap}  {speaker}{}\n",
        clock(segment["start_ms"].as_u64().unwrap_or(0)),
        clock(segment["end_ms"].as_u64().unwrap_or(0)),
        segment["text"].as_str().unwrap_or("").trim()
    )
}

/// The lines of a `meetings.segments` answer: the finals, then the
/// provisional tail. An empty first read says why.
pub fn lines(result: &Value, first_read: bool) -> String {
    let mut out = String::new();
    for key in ["segments", "provisional"] {
        for segment in result[key].as_array().into_iter().flatten() {
            out.push_str(&line(segment));
        }
    }
    if out.is_empty() && first_read {
        out.push_str(match result["status"].as_str() {
            Some("recording" | "stopping" | "stopped" | "transcribing") => "no segments yet\n",
            Some("partial") => "no segments; recover the meeting to transcribe its audio\n",
            _ => "no segments\n",
        });
    }
    out
}

/// True when a status says the meeting settled.
fn settled(state: Option<&str>) -> bool {
    matches!(state, Some("completed" | "failed" | "cancelled"))
}

fn read(client: &Client, id: &str, since: Option<&str>) -> Result<Value, Failure> {
    let mut params = json!({"meeting_id": id});
    if let Some(cursor) = since {
        params["since"] = json!(cursor);
    }
    client.call("meetings.segments", params)
}

fn print(cli: &Cli, result: &Value, first_read: bool) {
    if cli.json {
        println!("{result}");
    } else if !cli.quiet {
        print!("{}", lines(result, first_read));
    }
}

/// Prints the transcript so far after `since`; with `follow`, the
/// backlog, the live events and the finalised transcript.
pub fn run(
    cli: &Cli,
    client: &Client,
    id: &str,
    since: Option<&str>,
    follow: bool,
) -> Result<(), Failure> {
    if !follow {
        let result = read(client, id, since)?;
        print(cli, &result, since.is_none());
        return Ok(());
    }
    // Subscribe before the backlog is read: an event that races the read
    // is buffered on the subscription, and the backlog's ids drop it.
    let reader = client.subscribe(
        &["meeting.segment", "meeting.state"],
        Duration::from_secs(2),
    )?;
    let backlog = read(client, id, None)?;
    print(cli, &backlog, true);
    if settled(backlog["status"].as_str()) {
        return Ok(());
    }
    stream(cli, client, id, reader, &backlog)?;
    print(cli, &read(client, id, None)?, true);
    Ok(())
}

/// An event payload with the contract's `source_type` beside its
/// `source`, the way every segment of the answer carries both.
fn with_source_type(payload: &Value) -> Value {
    let mut out = payload.clone();
    out["source_type"] = json!(match payload["source"].as_str() {
        Some("remote") => "system",
        _ => "microphone",
    });
    out
}

/// A provisional segment as the backlog printed it.
fn shape(segment: &Value) -> (Value, Value, Value, Value) {
    (
        segment["segment_id"].clone(),
        segment["text"].clone(),
        segment["start_ms"].clone(),
        segment["end_ms"].clone(),
    )
}

/// Streams `meeting.segment` for `id` until `meeting.state` settles it,
/// dropping every event the backlog already printed.
fn stream(
    cli: &Cli,
    client: &Client,
    id: &str,
    mut reader: dettivo_proto::transport::Reader,
    backlog: &Value,
) -> Result<(), Failure> {
    let finals: HashSet<Value> = backlog["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| s["segment_id"].clone())
        .collect();
    let mut provisional: Vec<_> = backlog["provisional"]
        .as_array()
        .into_iter()
        .flatten()
        .map(shape)
        .collect();
    loop {
        let mut line_text = String::new();
        match reader.read_line(&mut line_text) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Nothing arrived: the meeting may have settled while the
                // subscription was opening.
                let status = client.call("meetings.status", json!({"meeting_id": id}))?;
                if settled(status["status"].as_str()) {
                    return Ok(());
                }
                continue;
            }
            Err(e) => {
                return Err(Failure::new(
                    crate::exit::Exit::Failure,
                    format!("read: {e}"),
                ));
            }
        }
        let parsed: Value = serde_json::from_str(line_text.trim_end()).unwrap_or(Value::Null);
        if parsed["method"] != "events.notify" {
            continue;
        }
        let params = &parsed["params"];
        let payload = &params["payload"];
        if payload["meeting_id"] != json!(id) {
            continue;
        }
        match params["topic"].as_str() {
            Some("meeting.segment") => {
                if payload["provisional"] == json!(true) {
                    if let Some(at) = provisional.iter().position(|p| *p == shape(payload)) {
                        provisional.remove(at);
                        continue;
                    }
                } else if finals.contains(&payload["segment_id"]) {
                    continue;
                }
                if cli.json {
                    println!("{}", with_source_type(payload));
                } else if !cli.quiet {
                    print!("{}", line(payload));
                }
            }
            Some("meeting.state") if settled(payload["state"].as_str()) => return Ok(()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_carry_the_mark_the_side_the_span_the_gap_and_the_speaker() {
        assert_eq!(clock(65_432), "01:05.432");
        let stored = json!({
            "source": "remote", "source_type": "system", "provisional": false,
            "start_ms": 4200, "end_ms": 7350, "speaker": "Ada",
            "text": " ask not what your country can do for you ", "gap_before_ms": 1500
        });
        assert_eq!(
            line(&stored),
            " remote 00:04.200-00:07.350  [gap 1500 ms]  Ada: ask not what your country can do for you\n"
        );
        let event = json!({
            "source": "you", "segment_id": "you-p1", "provisional": true,
            "start_ms": 0, "end_ms": 900, "text": "hello", "words": [], "speaker": "You"
        });
        assert_eq!(line(&event), "~you    00:00.000-00:00.900  hello\n");
        let empty = json!({"status": "transcribing", "segments": [], "provisional": []});
        assert_eq!(lines(&empty, true), "no segments yet\n");
        assert_eq!(lines(&empty, false), "");
        let both = json!({"segments": [stored], "provisional": [event]});
        assert!(lines(&both, false).ends_with("~you    00:00.000-00:00.900  hello\n"));
        assert_eq!(with_source_type(&event)["source_type"], "microphone");
        assert!(settled(Some("completed")) && !settled(Some("transcribing")));
    }
}
