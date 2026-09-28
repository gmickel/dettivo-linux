//! The diarization bench's assignment stage (docs/diarization-bench.md):
//! the product's own speaker rule over cached engine turns and transcript
//! segments, so a change to `dettivo_meeting::diarize` moves the bench's
//! numbers without an engine or ASR rerun.
//!
//! Reads one JSON request on stdin and writes one JSON answer on stdout:
//!
//! ```json
//! {"variant": "product", "params": {"pause_ms": 250},
//!  "jobs": [{"id": "ES2011a", "room_audio": true, "track_ms": 1113845,
//!            "segments": [<Segment>], "turns": [<SpeakerTurn>],
//!            "probs": "<frame probabilities .npy or null>",
//!            "dir": "<a two-track meeting's directory, or null>",
//!            "embeddings": [<one vector or null per segment>]}]}
//! ```
//!
//! The answer lists, per job, the segments as the variant leaves them
//! (a variant may split, merge or drop segments) with their speaker id,
//! or null for an unlabelled segment, and what the two-track rules did
//! (`dettivo_meeting::two_track`, which reads both tracks' levels from
//! `dir` and the voices from `embeddings`). A variant is one arm of [`label`]:
//! add the arm and its name to [`VARIANTS`], select it with
//! `just diar-bench --variant <name>`, and pass its parameters with
//! `--set <key> <value>`.

use std::io::Read;

use dettivo_engine_proto::SpeakerTurn;
use dettivo_meeting::Track;
use dettivo_meeting::diarize::{Rule, read_track};
use dettivo_meeting::levels::Levels;
use dettivo_meeting::two_track::{self, Evidence, Rules};
use dettivo_proto::methods::meetings::Segment;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    variant: String,
    #[serde(default)]
    params: Map<String, Value>,
    jobs: Vec<Job>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    id: String,
    room_audio: bool,
    track_ms: u64,
    segments: Vec<Segment>,
    turns: Vec<SpeakerTurn>,
    /// Per-10 ms speaker probabilities (`.npy`), for engines that keep
    /// them; the product rule reads turns only.
    #[serde(default, rename = "probs")]
    _probs: Option<String>,
    /// A two-track meeting's directory, for both tracks' levels.
    #[serde(default)]
    dir: Option<String>,
    /// One embedding per segment, or empty.
    #[serde(default)]
    embeddings: Vec<Option<Vec<f32>>>,
}

/// The product rules with `params` over their defaults; an unknown key
/// is an error.
fn product_rule(params: &Map<String, Value>) -> Result<(Rule, Rules), String> {
    let mut rule = Rule::default();
    let mut rules = Rules::default();
    for (key, value) in params {
        let whole = || {
            value
                .as_u64()
                .ok_or_else(|| format!("parameter {key}: expected a whole number"))
        };
        let number = || {
            value
                .as_f64()
                .ok_or_else(|| format!("parameter {key}: expected a number"))
        };
        let flag = || {
            value
                .as_bool()
                .ok_or_else(|| format!("parameter {key}: expected true or false"))
        };
        match key.as_str() {
            "pause_ms" => rule.pause_ms = whole()?,
            "nearest_turn_ms" => rule.nearest_turn_ms = whole()?,
            "min_speaker_share" => rule.min_speaker_share = number()?,
            "bleed_min_voiced" => rules.bleed_min_voiced = number()?,
            "single_remote" => rules.single_remote = flag()?,
            "shared_mic" => rules.shared_mic = flag()?,
            "voiceprint" => rules.voiceprint = flag()?,
            "voice_match" => rules.voice_match = number()?,
            _ => {
                return Err(format!(
                    "variant product has no parameter {key} (pause_ms, nearest_turn_ms, \
                     min_speaker_share, bleed_min_voiced, single_remote, shared_mic, \
                     voiceprint, voice_match)"
                ));
            }
        }
    }
    Ok((rule, rules))
}

/// The variants the bench can name; each is one arm of [`label`].
const VARIANTS: &[&str] = &["product"];

/// Labels one job's segments under `variant`, with what the two-track
/// rules did.
fn label(
    variant: &str,
    params: &Map<String, Value>,
    job: Job,
) -> Result<(Vec<Segment>, Value), String> {
    match variant {
        "product" => {
            let (rule, rules) = product_rule(params)?;
            let levels = match &job.dir {
                Some(dir) => {
                    let dir = std::path::Path::new(dir);
                    let mic = read_track(dir, Track::Microphone)?;
                    let system = read_track(dir, Track::System)?;
                    Some(Levels::from_pcm(&mic, &system))
                }
                None => None,
            };
            // The bench holds the diarized track's turns only; a meeting
            // whose microphone the product would diarize is reported.
            let (_, shared_mic) = two_track::plan(job.dir.is_some(), levels.as_ref(), &rules);
            let evidence = Evidence {
                levels: levels.as_ref(),
                embeddings: job.embeddings,
                voiceprint: None,
            };
            let mut segments = job.segments;
            let (_, report) = two_track::label(
                &mut segments,
                &job.turns,
                job.room_audio,
                job.track_ms,
                &rule,
                &rules,
                &evidence,
            );
            let report = json!({
                "dropped_bleed": report.dropped_bleed,
                "single_remote": report.single_remote,
                "you_relabelled": report.you_relabelled,
                "enrolled": report.enrolment.is_some(),
                "shared_mic": shared_mic,
            });
            Ok((segments, report))
        }
        other => Err(format!(
            "unknown variant {other} (known: {})",
            VARIANTS.join(", ")
        )),
    }
}

fn run(input: &str) -> Result<Value, String> {
    let request: Request = serde_json::from_str(input).map_err(|e| format!("request: {e}"))?;
    if !VARIANTS.contains(&request.variant.as_str()) {
        return Err(format!(
            "unknown variant {} (known: {})",
            request.variant,
            VARIANTS.join(", ")
        ));
    }
    let mut results = Vec::with_capacity(request.jobs.len());
    for job in request.jobs {
        let id = job.id.clone();
        let (segments, report) = label(&request.variant, &request.params, job)?;
        let lines: Vec<Value> = segments
            .iter()
            .map(|s| {
                json!({
                    "start_ms": s.start_ms,
                    "end_ms": s.end_ms,
                    "source": s.source_type,
                    "speaker": s.speaker_id,
                })
            })
            .collect();
        results.push(json!({"id": id, "lines": lines, "report": report}));
    }
    Ok(json!({"variant": request.variant, "params": request.params, "results": results}))
}

fn main() -> std::process::ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("diar_assign: stdin: {e}");
        return std::process::ExitCode::FAILURE;
    }
    match run(&input) {
        Ok(answer) => {
            println!("{answer}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("diar_assign: {e}");
            std::process::ExitCode::from(2)
        }
    }
}
