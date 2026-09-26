//! `dettivo meetings <verb>` and `dettivo audio devices`: the `meetings.*`
//! methods as a person uses them (ADR 0027), the speaker pass and the
//! speaker names (ADR 0035). Human output is the daemon's result
//! rendered; `--json` prints it raw. `segments` lives in
//! `meetings_segments` (ADR 0031); `notes`, `analyze`, `analysis`,
//! `export` and `disclosure --copy` in `meetings_notes` (ADR 0036).

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::strip_nulls;
use crate::exit::Failure;
use crate::{Cli, output};

/// `dettivo meetings <what>`.
#[derive(Debug, Subcommand)]
pub enum MeetingsCmd {
    /// meetings.start: record the microphone and the default sink's monitor.
    Start {
        /// A title; without one the start time names the meeting.
        #[arg(long)]
        title: Option<String>,
        /// Language code or auto (default: the `[dictation]` language).
        #[arg(long)]
        language: Option<String>,
        /// Record the microphone only.
        #[arg(long)]
        no_system_audio: bool,
        /// Acknowledge the recording disclosure for this and every later meeting.
        #[arg(long)]
        acknowledge_disclosure: bool,
    },
    /// meetings.stop: close the takes (answers stopping, then stopped).
    Stop {
        /// The meeting id.
        id: String,
    },
    /// meetings.cancel: drop the meeting and its takes.
    Cancel {
        /// The meeting id.
        id: String,
    },
    /// meetings.status: the state, the capture facts and the recoverable meetings.
    Status {
        /// The meeting id.
        id: String,
    },
    /// meetings.list: the newest meetings first.
    List {
        /// Meetings to show.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// meetings.get: one meeting with its transcript and segments.
    Get {
        /// The meeting id.
        id: String,
    },
    /// The transcript so far as lines: side, span on the meeting clock, text, provisional lines marked ~; --since prints only what is new, --follow streams.
    Segments {
        /// The meeting id.
        id: String,
        /// Only what is new after this cursor (the `cursor` of an earlier --json answer).
        #[arg(long, conflicts_with = "follow")]
        since: Option<String>,
        /// Print the backlog, then meeting.segment events as they arrive until the meeting settles, then the transcript.
        #[arg(long)]
        follow: bool,
    },
    /// meetings.search: word-start matches over the title, texts and summary.
    Search {
        /// The words to look for.
        query: String,
        /// Hits to show.
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// meetings.notes.get and meetings.notes.set: the notes, Markdown.
    Notes {
        #[command(subcommand)]
        what: NotesCmd,
    },
    /// meetings.analyze: summary, decisions and action items from the language model.
    Analyze {
        /// The meeting id.
        id: String,
        /// Run again over a meeting that already has an analysis.
        #[arg(long)]
        force: bool,
    },
    /// meetings.analysis.get: the analysis and where it stands.
    Analysis {
        /// The meeting id.
        id: String,
    },
    /// transcripts.export through a download transfer into a file: txt, md, srt, vtt or json.
    Export {
        /// The meeting id.
        id: String,
        /// txt, md, srt, vtt or json.
        #[arg(long, default_value = "md")]
        format: String,
        /// The file to write.
        #[arg(long)]
        out: PathBuf,
        /// The engine's words rather than the polished segments.
        #[arg(long)]
        raw: bool,
    },
    /// meetings.delete: remove the meeting per the artifact policy.
    Delete {
        /// The meeting id.
        id: String,
        /// transcript_only, transcript_and_audio or all (default: the daemon's meetings.delete_artifact_policy).
        #[arg(long)]
        policy: Option<String>,
    },
    /// meetings.recover: retry a partial, failed, cancelled or stopped meeting with retained audio.
    Recover {
        /// The meeting id.
        id: String,
    },
    /// meetings.discard: remove a meeting a daemon restart left partial.
    Discard {
        /// The meeting id.
        id: String,
    },
    /// meetings.disclosure.get, acknowledge it with --acknowledge, or copy the message with --copy.
    Disclosure {
        /// Record the acknowledgement with the time.
        #[arg(long)]
        acknowledge: bool,
        /// Put the disclosure message on the clipboard.
        #[arg(long, conflicts_with = "acknowledge")]
        copy: bool,
    },
    /// meetings.diarize: run (or re-run) the speaker pass on a completed meeting.
    Diarize {
        /// The meeting id.
        id: String,
        /// The speaker count when known; the clustering decides otherwise.
        #[arg(long)]
        speakers: Option<u32>,
    },
    /// meetings.speakers.list for a meeting id; `speakers rename <id> <speaker-id> <name>` and `speakers suggest` too.
    #[command(args_conflicts_with_subcommands = true)]
    Speakers {
        #[command(subcommand)]
        action: Option<SpeakersAction>,
        /// The meeting id (lists its speakers with their talk time).
        id: Option<String>,
    },
}

/// `dettivo meetings notes <get|set>`.
#[derive(Debug, Subcommand)]
pub enum NotesCmd {
    /// Print the notes.
    Get {
        /// The meeting id.
        id: String,
    },
    /// Store the notes: the text, --file PATH or --stdin.
    Set {
        /// The meeting id.
        id: String,
        /// The notes, Markdown.
        text: Option<String>,
        /// Read the notes from a file.
        #[arg(long, conflicts_with = "stdin")]
        file: Option<PathBuf>,
        /// Read the notes from standard input.
        #[arg(long)]
        stdin: bool,
        /// user (default) or live.
        #[arg(long)]
        source: Option<String>,
    },
}

/// `dettivo meetings speakers <action>`.
#[derive(Debug, Subcommand)]
pub enum SpeakersAction {
    /// meetings.speakers.rename: name a speaker across the meeting (an empty name restores the label).
    Rename {
        /// The meeting id.
        id: String,
        /// The speaker id (`you`, `speaker_00`, ...).
        speaker_id: String,
        /// The name, at most 64 characters.
        name: String,
    },
    /// meetings.speakers.suggest: the names used before, most recent first.
    Suggest {
        /// Only names starting with this.
        #[arg(long)]
        prefix: Option<String>,
        /// Names to show.
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
}

/// Runs a `meetings` subcommand.
pub fn run(cli: &Cli, client: &Client, what: &MeetingsCmd) -> Result<(), Failure> {
    let (method, params) = match what {
        MeetingsCmd::Segments { id, since, follow } => {
            return crate::meetings_segments::run(cli, client, id, since.as_deref(), *follow);
        }
        MeetingsCmd::Notes {
            what: NotesCmd::Get { id },
        } => return crate::meetings_notes::notes_get(cli, client, id),
        MeetingsCmd::Notes {
            what:
                NotesCmd::Set {
                    id,
                    text,
                    file,
                    stdin,
                    source,
                },
        } => {
            let body = crate::meetings_notes::notes_body(text.as_deref(), file.as_deref(), *stdin)?;
            return crate::meetings_notes::notes_set(cli, client, id, body, source.as_deref());
        }
        MeetingsCmd::Analyze { id, force } => {
            return crate::meetings_notes::analyze(cli, client, id, *force);
        }
        MeetingsCmd::Analysis { id } => return crate::meetings_notes::analysis(cli, client, id),
        MeetingsCmd::Export {
            id,
            format,
            out,
            raw,
        } => return crate::meetings_notes::export(cli, client, id, format, out, *raw),
        MeetingsCmd::Disclosure { copy: true, .. } => {
            return crate::meetings_notes::disclosure_copy(cli, client);
        }
        MeetingsCmd::Start {
            title,
            language,
            no_system_audio,
            acknowledge_disclosure,
        } => (
            "meetings.start",
            strip_nulls(json!({
                "capture": {"microphone": true, "system_audio": !no_system_audio},
                "title": title,
                "language": language,
                "acknowledge_meeting_disclosure": acknowledge_disclosure.then_some(true),
            })),
        ),
        MeetingsCmd::Stop { id } => ("meetings.stop", json!({"meeting_id": id})),
        MeetingsCmd::Cancel { id } => ("meetings.cancel", json!({"meeting_id": id})),
        MeetingsCmd::Status { id } => ("meetings.status", json!({"meeting_id": id})),
        MeetingsCmd::List { limit } => ("meetings.list", json!({"limit": limit, "cursor": null})),
        MeetingsCmd::Get { id } => ("meetings.get", json!({"meeting_id": id})),
        MeetingsCmd::Search { query, limit } => {
            ("meetings.search", json!({"query": query, "limit": limit}))
        }
        MeetingsCmd::Delete { id, policy } => (
            "meetings.delete",
            strip_nulls(json!({"meeting_id": id, "artifact_policy": policy})),
        ),
        MeetingsCmd::Recover { id } => ("meetings.recover", json!({"meeting_id": id})),
        MeetingsCmd::Discard { id } => ("meetings.discard", json!({"meeting_id": id})),
        MeetingsCmd::Disclosure {
            acknowledge: true, ..
        } => ("meetings.disclosure.acknowledge", json!({})),
        MeetingsCmd::Disclosure { .. } => ("meetings.disclosure.get", json!({})),
        MeetingsCmd::Diarize { id, speakers } => (
            "meetings.diarize",
            strip_nulls(json!({"meeting_id": id, "speakers": speakers})),
        ),
        MeetingsCmd::Speakers { action: None, id } => match id {
            Some(id) => ("meetings.speakers.list", json!({"meeting_id": id})),
            None => {
                return Err(Failure::new(
                    crate::exit::Exit::InvalidArgs,
                    "dettivo meetings speakers <id>, or speakers rename <id> <speaker-id> <name>, or speakers suggest",
                ));
            }
        },
        MeetingsCmd::Speakers {
            action:
                Some(SpeakersAction::Rename {
                    id,
                    speaker_id,
                    name,
                }),
            ..
        } => (
            "meetings.speakers.rename",
            json!({"meeting_id": id, "speaker_id": speaker_id, "name": name}),
        ),
        MeetingsCmd::Speakers {
            action: Some(SpeakersAction::Suggest { prefix, limit }),
            ..
        } => (
            "meetings.speakers.suggest",
            strip_nulls(json!({"prefix": prefix, "limit": limit})),
        ),
    };
    let result = client.call(method, params)?;
    if !cli.json && !cli.quiet {
        match method {
            "meetings.list" => {
                print!("{}", list_lines(&result));
                return Ok(());
            }
            "meetings.speakers.list" => {
                print!("{}", speaker_lines(&result));
                return Ok(());
            }
            _ => {}
        }
    }
    output::result(cli, method, &result);
    Ok(())
}

/// One row per speaker: id, name, talk time in seconds; then the
/// diarization status.
pub fn speaker_lines(result: &Value) -> String {
    let mut out = String::new();
    for s in result["speakers"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{:<12} {:<24} {:>7.1}s
",
            s["speaker_id"].as_str().unwrap_or("-"),
            s["name"].as_str().unwrap_or(""),
            s["talk_ms"].as_u64().unwrap_or(0) as f64 / 1000.0
        ));
    }
    let d = &result["diarization"];
    out.push_str(&format!(
        "diarization {}{}
",
        d["status"].as_str().unwrap_or("none"),
        d["error"]
            .as_str()
            .map(|e| format!(": {e}"))
            .unwrap_or_default()
    ));
    out
}

/// One row per meeting: id, start, seconds, status, title.
pub fn list_lines(result: &Value) -> String {
    let mut out = String::new();
    for item in result["items"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}  {}  {:>6}s  {:<10} {}\n",
            item["ref"]["id"].as_str().unwrap_or("-"),
            item["started_at"].as_str().unwrap_or("-"),
            item["duration_seconds"].as_u64().unwrap_or(0),
            item["status"].as_str().unwrap_or("-"),
            item["title"].as_str().unwrap_or("")
        ));
    }
    if out.is_empty() {
        out.push_str("no meetings\n");
    }
    out
}

/// `dettivo audio <what>`.
#[derive(Debug, Subcommand)]
pub enum AudioCmd {
    /// audio.devices: the capturable nodes, the default source and the default sink.
    Devices,
}

/// Runs an `audio` subcommand.
pub fn audio(cli: &Cli, client: &Client, what: &AudioCmd) -> Result<(), Failure> {
    let method = match what {
        AudioCmd::Devices => "audio.devices",
    };
    let result: Value = client.call(method, json!({}))?;
    output::result(cli, method, &result);
    Ok(())
}
