//! `dettivo meetings segments <id> [--follow]`: the transcript of a
//! meeting as lines on the terminal, one per segment with its side and
//! its span on the meeting clock (ADR 0031). `--follow` streams the
//! `meeting.segment` events of a running meeting as they arrive,
//! provisional lines marked with `~`, until the meeting settles, then
//! prints the finalised transcript.

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

/// The side a row's `source_type` or an event's `source` names.
fn side(value: &Value) -> &str {
    match value.as_str() {
        Some("system") | Some("remote") => "remote",
        _ => "you",
    }
}

/// One line for a segment of the row (`meetings.get`): the speaker's
/// name once the pass assigned one (ADR 0035), else the side; the span,
/// a gap marker when capture was missing before it, the text.
pub fn row_line(segment: &Value) -> String {
    let gap = segment["gap_before_ms"]
        .as_u64()
        .map(|g| format!("  [gap {} ms]", g))
        .unwrap_or_default();
    format!(
        "{:<6} {}-{}{}  {}\n",
        segment["speaker"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| side(&segment["source_type"])),
        clock(segment["start_ms"].as_u64().unwrap_or(0)),
        clock(segment["end_ms"].as_u64().unwrap_or(0)),
        gap,
        segment["text"].as_str().unwrap_or("").trim()
    )
}

/// One line for a `meeting.segment` payload: `~` marks a provisional
/// segment a later window may still replace.
pub fn event_line(payload: &Value) -> String {
    let mark = if payload["provisional"] == json!(true) {
        "~"
    } else {
        " "
    };
    let gap = payload["gap_before_ms"]
        .as_u64()
        .map(|g| format!("  [gap {} ms]", g))
        .unwrap_or_default();
    format!(
        "{mark}{:<6} {}-{}{}  {}\n",
        side(&payload["source"]),
        clock(payload["start_ms"].as_u64().unwrap_or(0)),
        clock(payload["end_ms"].as_u64().unwrap_or(0)),
        gap,
        payload["text"].as_str().unwrap_or("").trim()
    )
}

/// The segment lines of a `meetings.get` result.
pub fn row_lines(result: &Value) -> String {
    let mut out = String::new();
    for segment in result["segments"].as_array().into_iter().flatten() {
        out.push_str(&row_line(segment));
    }
    if out.is_empty() {
        out.push_str(match result["status"].as_str() {
            Some("recording") | Some("stopping") => "no segments yet\n",
            Some("stopped") | Some("transcribing") | Some("partial") => {
                "no segments yet; the transcript arrives when the finalisation completes\n"
            }
            _ => "no segments\n",
        });
    }
    out
}

/// True when a `meeting.state` payload says the meeting settled.
fn settled(state: Option<&str>) -> bool {
    matches!(state, Some("completed" | "failed" | "cancelled"))
}

/// Prints the row's segments; with `follow`, streams the live events of
/// the meeting first while it runs.
pub fn run(cli: &Cli, client: &Client, id: &str, follow: bool) -> Result<(), Failure> {
    if follow {
        let current = client.call("meetings.get", json!({"meeting_id": id}))?;
        if !settled(current["status"].as_str()) {
            stream(cli, client, id)?;
        }
    }
    let result = client.call("meetings.get", json!({"meeting_id": id}))?;
    if cli.json {
        println!("{}", result["segments"]);
        return Ok(());
    }
    if !cli.quiet {
        print!("{}", row_lines(&result));
    }
    Ok(())
}

/// Streams `meeting.segment` for `id` until `meeting.state` settles it.
fn stream(cli: &Cli, client: &Client, id: &str) -> Result<(), Failure> {
    let mut reader = client.subscribe(
        &["meeting.segment", "meeting.state"],
        Duration::from_secs(2),
    )?;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
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
        let parsed: Value = serde_json::from_str(line.trim_end()).unwrap_or(Value::Null);
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
                if cli.json {
                    println!("{payload}");
                } else if !cli.quiet {
                    print!("{}", event_line(payload));
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
    fn lines_carry_the_side_the_span_the_gap_and_the_mark() {
        assert_eq!(clock(65_432), "01:05.432");
        let row = json!({
            "source_type": "system", "start_ms": 4200, "end_ms": 7350,
            "text": " ask not what your country can do for you ", "gap_before_ms": 1500
        });
        assert_eq!(
            row_line(&row),
            "remote 00:04.200-00:07.350  [gap 1500 ms]  ask not what your country can do for you\n"
        );
        let event = json!({
            "source": "you", "segment_id": "you-p1", "provisional": true,
            "start_ms": 0, "end_ms": 900, "text": "hello", "words": []
        });
        assert_eq!(event_line(&event), "~you    00:00.000-00:00.900  hello\n");
        let done = json!({"status": "stopped", "segments": []});
        assert!(row_lines(&done).contains("finalisation"));
        assert!(settled(Some("completed")) && !settled(Some("transcribing")));
    }
}
