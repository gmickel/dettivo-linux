//! `dettivo meetings notes|analyze|analysis|export` and `dettivo meetings
//! disclosure --copy` (ADR 0036): the notes as a file or standard input,
//! the analysis job and its result, the five export formats through a
//! download transfer into a file, and the disclosure text on the
//! clipboard through the daemon's clipboard-only insertion.

use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::strip_nulls;
use crate::exit::{Exit, Failure};
use crate::{Cli, output};

/// The Markdown `notes set` takes: `--file <path>`, `--stdin`, or the
/// text itself.
pub fn notes_body(text: Option<&str>, file: Option<&Path>, stdin: bool) -> Result<String, Failure> {
    match (text, file, stdin) {
        (_, Some(path), _) => std::fs::read_to_string(path).map_err(|e| {
            Failure::new(
                Exit::InvalidArgs,
                format!("cannot read {}: {e}", path.display()),
            )
        }),
        (_, None, true) => {
            let mut body = String::new();
            std::io::stdin()
                .read_to_string(&mut body)
                .map_err(|e| Failure::new(Exit::Failure, format!("stdin: {e}")))?;
            Ok(body)
        }
        (Some(text), None, false) => Ok(text.to_string()),
        (None, None, false) => Err(Failure::new(
            Exit::InvalidArgs,
            "notes set takes the text, --file <path> or --stdin",
        )),
    }
}

/// `dettivo meetings notes get <id>`.
pub fn notes_get(cli: &Cli, client: &Client, id: &str) -> Result<(), Failure> {
    let result = client.call("meetings.notes.get", json!({"meeting_id": id}))?;
    if cli.json || cli.quiet {
        output::result(cli, "meetings.notes.get", &result);
    } else {
        let text = result["markdown"].as_str().unwrap_or("");
        print!("{text}");
        if !text.is_empty() && !text.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

/// `dettivo meetings notes set <id> [text] [--file] [--stdin] [--source]`.
pub fn notes_set(
    cli: &Cli,
    client: &Client,
    id: &str,
    markdown: String,
    source: Option<&str>,
) -> Result<(), Failure> {
    let result = client.call(
        "meetings.notes.set",
        strip_nulls(json!({"meeting_id": id, "markdown": markdown, "source": source})),
    )?;
    crate::history::print(cli, "meetings.notes.set", &result, || {
        format!(
            "notes stored ({} bytes, {})\n",
            result["markdown"].as_str().map(str::len).unwrap_or(0),
            result["source"].as_str().unwrap_or("user")
        )
    });
    Ok(())
}

/// `dettivo meetings analyze <id> [--force]`.
pub fn analyze(cli: &Cli, client: &Client, id: &str, force: bool) -> Result<(), Failure> {
    let result = client.call(
        "meetings.analyze",
        strip_nulls(json!({"meeting_id": id, "force": force.then_some(true)})),
    )?;
    crate::history::print(cli, "meetings.analyze", &result, || {
        let status = result["analysis_status"].as_str().unwrap_or("-");
        match result["notice"]["reason"].as_str() {
            Some(reason) => format!("analysis {status}: {reason}\n"),
            None => format!(
                "analysis {status} ({})\n",
                result["job"]["job_id"].as_str().unwrap_or("-")
            ),
        }
    });
    Ok(())
}

/// The human rendering of an analysis result.
pub fn analysis_lines(result: &Value) -> String {
    let status = result["analysis_status"].as_str().unwrap_or("none");
    let mut out = format!("status: {status}\n");
    if let Some(error) = result["error"].as_str() {
        out.push_str(&format!("error: {error}\n"));
    }
    let Some(analysis) = result["analysis"].as_object() else {
        return out;
    };
    if let Some(model) = result["model"].as_str() {
        out.push_str(&format!("model: {model}\n"));
    }
    out.push('\n');
    out.push_str(analysis["summary"].as_str().unwrap_or("").trim());
    out.push('\n');
    let decisions = analysis["decisions"].as_array();
    if decisions.is_some_and(|d| !d.is_empty()) {
        out.push_str("\nDecisions\n");
        for d in decisions.into_iter().flatten() {
            out.push_str(&format!("- {}\n", d.as_str().unwrap_or("")));
        }
    }
    let items = analysis["action_items"].as_array();
    if items.is_some_and(|i| !i.is_empty()) {
        out.push_str("\nAction items\n");
        for a in items.into_iter().flatten() {
            let mut line = format!("- {}", a["text"].as_str().unwrap_or(""));
            let tail: Vec<String> = [
                a["owner"].as_str().map(|o| format!("owner: {o}")),
                a["due"].as_str().map(|d| format!("due: {d}")),
            ]
            .into_iter()
            .flatten()
            .collect();
            if !tail.is_empty() {
                line.push_str(&format!(" ({})", tail.join(", ")));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// `dettivo meetings analysis <id>`.
pub fn analysis(cli: &Cli, client: &Client, id: &str) -> Result<(), Failure> {
    let result = client.call("meetings.analysis.get", json!({"meeting_id": id}))?;
    crate::history::print(cli, "meetings.analysis.get", &result, || {
        analysis_lines(&result)
    });
    Ok(())
}

/// `dettivo meetings export <id> --format <f> --out <path> [--raw]`.
pub fn export(
    cli: &Cli,
    client: &Client,
    id: &str,
    format: &str,
    out: &Path,
    raw: bool,
) -> Result<(), Failure> {
    let format = match format {
        "json" => "json",
        "md" | "markdown" => "md",
        "txt" | "text" => "txt",
        "srt" => "srt",
        "vtt" => "vtt",
        other => {
            return Err(Failure::new(
                Exit::InvalidArgs,
                format!("{other} is not a meeting export format (txt, md, srt, vtt, json)"),
            ));
        }
    };
    let params = strip_nulls(json!({
        "ref": {"kind": "meeting", "id": id},
        "format": format,
        "raw": raw.then_some(true),
    }));
    crate::transfer::download(cli, client, params, out)
}

/// `dettivo meetings disclosure --copy`: the macOS notice through
/// `insert.perform` in clipboard-only mode, so the same backend (or the
/// QA mock sink) that copies a transcript copies the message.
pub fn disclosure_copy(cli: &Cli, client: &Client) -> Result<(), Failure> {
    let disclosure = client.call("meetings.disclosure.get", json!({}))?;
    let message = disclosure["message"].as_str().unwrap_or("").to_string();
    let result = client.call(
        "insert.perform",
        json!({"text": message, "mode": "clipboard_only"}),
    )?;
    if result["outcome"] == json!("failed") {
        return Err(Failure::new(
            Exit::Failure,
            format!(
                "copy failed: {}",
                result["reason"].as_str().unwrap_or("no reason")
            ),
        ));
    }
    crate::history::print(cli, "insert.perform", &result, || {
        "disclosure message copied to the clipboard\n".to_string()
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_analysis_renders_its_sections_and_a_failure_names_the_error() {
        let ready = json!({
            "analysis_status": "ready", "model": "qwen3-1.7b", "error": null,
            "analysis": {"summary": "Agreed.", "decisions": ["Ship"], "action_items": [{"text": "Write", "owner": "Mara", "due": null}]}
        });
        assert_eq!(
            analysis_lines(&ready),
            "status: ready\nmodel: qwen3-1.7b\n\nAgreed.\n\nDecisions\n- Ship\n\nAction items\n- Write (owner: Mara)\n"
        );
        let failed = json!({"analysis_status": "failed", "error": "provider_unavailable: x", "analysis": null});
        assert_eq!(
            analysis_lines(&failed),
            "status: failed\nerror: provider_unavailable: x\n"
        );
        assert!(notes_body(None, None, false).is_err());
        assert_eq!(notes_body(Some("hi"), None, false).unwrap(), "hi");
    }
}
