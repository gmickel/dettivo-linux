//! The diarization bench's assignment stage (docs/diarization-bench.md):
//! the product's own speaker rule over cached engine turns and transcript
//! segments, so a change to `dettivo_meeting::diarize` moves the bench's
//! numbers without an engine or ASR rerun.
//!
//! Reads one JSON request on stdin and writes one JSON answer on stdout:
//!
//! ```json
//! {"variant": "product", "params": {"pause_ms": 250}, "mode": "label",
//!  "jobs": [{"id": "ES2011a", "room_audio": true, "track_ms": 1113845,
//!            "segments": [<Segment>], "turns": [<SpeakerTurn>],
//!            "probs": "<frame probabilities .npy or null>",
//!            "dir": "<a two-track meeting's directory, or null>",
//!            "embeddings": [<one vector or null per segment>],
//!            "unit_embeddings": [[<start_ms>, <end_ms>, <vector or null>]]}]}
//! ```
//!
//! The answer lists, per job, the segments as the variant leaves them
//! (a variant may split, merge or drop segments) with their speaker id,
//! or null for an unlabelled segment, and what the two-track rules did
//! (`dettivo_meeting::two_track`, which reads both tracks' levels from
//! `dir` and the voices from `embeddings`). With the voice check on
//! (ADR 0076) it also lists the lines as they stand without the check
//! (`before`), so the bench can count the words it fixed and broke. The
//! check's evidence is `unit_embeddings`, each sentence unit's voice on
//! the diarized track, or with `voice_evidence = "probs"` the engine's
//! frame probabilities. Mode `spans` answers, per job, the unit spans the
//! check would embed instead, for the bench to embed and send back. A
//! variant is one arm of [`label`]:
//! add the arm and its name to [`VARIANTS`], select it with
//! `just diar-bench --variant <name>`, and pass its parameters with
//! `--set <key> <value>`.

use std::collections::HashMap;
use std::io::Read;

use dettivo_engine_proto::SpeakerTurn;
use dettivo_meeting::Track;
use dettivo_meeting::diarize::{Rule, read_track};
use dettivo_meeting::levels::Levels;
use dettivo_meeting::two_track::{self, Evidence, Rules};
use dettivo_meeting::voice_check::{Evidence as UnitEvidence, Voices};
use dettivo_proto::methods::meetings::Segment;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[path = "support/probs.rs"]
mod probs;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    variant: String,
    #[serde(default)]
    params: Map<String, Value>,
    /// `label` (the default) or `spans`.
    #[serde(default)]
    mode: Option<String>,
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
    /// them; the voice check reads them with `voice_evidence = "probs"`.
    #[serde(default)]
    probs: Option<String>,
    /// A two-track meeting's directory, for both tracks' levels.
    #[serde(default)]
    dir: Option<String>,
    /// One embedding per segment, or empty.
    #[serde(default)]
    embeddings: Vec<Option<Vec<f32>>>,
    /// Each sentence unit's voice on the diarized track, by span.
    #[serde(default)]
    unit_embeddings: Vec<(u64, u64, Option<Vec<f32>>)>,
}

/// Where the voice check's evidence comes from.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Voice,
    Probs,
}

/// The product rules with `params` over their defaults; an unknown key
/// is an error.
fn product_rule(params: &Map<String, Value>) -> Result<(Rule, Rules, Source), String> {
    let mut rule = Rule::default();
    let mut rules = Rules::default();
    let mut source = Source::Voice;
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
            "voice_check" => rule.voice_check.on = flag()?,
            "voice_agree" => rule.voice_check.agree = number()?,
            "voice_overrule" => rule.voice_check.overrule = number()?,
            "voice_evidence" => {
                source = match value.as_str() {
                    Some("voice") => Source::Voice,
                    Some("probs") => Source::Probs,
                    _ => return Err(format!("parameter {key}: voice or probs")),
                }
            }
            _ => {
                return Err(format!(
                    "variant product has no parameter {key} (pause_ms, nearest_turn_ms, \
                     min_speaker_share, bleed_min_voiced, single_remote, shared_mic, \
                     voiceprint, voice_match, voice_check, voice_agree, voice_overrule, \
                     voice_evidence)"
                ));
            }
        }
    }
    Ok((rule, rules, source))
}

/// The variants the bench can name; each is one arm of [`label`].
const VARIANTS: &[&str] = &["product"];

/// The lines as the bench scores them.
fn lines(segments: &[Segment]) -> Vec<Value> {
    segments
        .iter()
        .map(|s| {
            json!({
                "start_ms": s.start_ms,
                "end_ms": s.end_ms,
                "source": s.source_type,
                "speaker": s.speaker_id,
            })
        })
        .collect()
}

/// One job's answer under `variant`: in `label` mode its lines and what
/// the rules did, in `spans` mode the unit spans the voice check embeds.
fn label(
    variant: &str,
    params: &Map<String, Value>,
    spans: bool,
    job: Job,
) -> Result<Value, String> {
    if variant != "product" {
        return Err(format!(
            "unknown variant {variant} (known: {})",
            VARIANTS.join(", ")
        ));
    }
    let (rule, rules, source) = product_rule(params)?;
    let levels = match &job.dir {
        Some(dir) => {
            let dir = std::path::Path::new(dir);
            let mic = read_track(dir, Track::Microphone)?;
            let system = read_track(dir, Track::System)?;
            Some(Levels::from_pcm(&mic, &system))
        }
        None => None,
    };
    // The bench holds the diarized track's turns only; a meeting whose
    // microphone the product would diarize is reported.
    let (_, shared_mic) = two_track::plan(job.dir.is_some(), levels.as_ref(), &rules);
    let evidence = Evidence {
        levels: levels.as_ref(),
        embeddings: job.embeddings.clone(),
        voiceprint: None,
    };
    let run = |voice: Option<&mut dyn UnitEvidence>| {
        let mut segments = job.segments.clone();
        let (out, report) = two_track::label(
            &mut segments,
            &job.turns,
            job.room_audio,
            job.track_ms,
            &rule,
            &rules,
            &evidence,
            voice,
        );
        (segments, out, report)
    };
    if spans {
        let mut asked: Vec<(u64, u64)> = Vec::new();
        if rule.voice_check.on && source == Source::Voice {
            let mut record = |spans: &[(u64, u64)]| {
                asked.extend_from_slice(spans);
                vec![None; spans.len()]
            };
            run(Some(&mut Voices { embed: &mut record }));
        }
        return Ok(json!({"id": job.id, "spans": asked}));
    }
    let known: HashMap<(u64, u64), Vec<f32>> = job
        .unit_embeddings
        .iter()
        .filter_map(|(a, b, v)| v.clone().map(|v| ((*a, *b), v)))
        .collect();
    let mut lookup = |spans: &[(u64, u64)]| spans.iter().map(|s| known.get(s).cloned()).collect();
    let mut voices = Voices { embed: &mut lookup };
    let mut frames;
    let voice: Option<&mut dyn UnitEvidence> = match (rule.voice_check.on, source, &job.probs) {
        (false, _, _) => None,
        (true, Source::Voice, _) => Some(&mut voices),
        (true, Source::Probs, Some(path)) => {
            frames = probs::Probs::open(path, &job.turns)?;
            Some(&mut frames)
        }
        (true, Source::Probs, None) => None,
    };
    let checked = voice.is_some();
    let (segments, out, report) = run(voice);
    let before = checked.then(|| lines(&run(None).0));
    let report = json!({
        "dropped_bleed": report.dropped_bleed,
        "single_remote": report.single_remote,
        "you_relabelled": report.you_relabelled,
        "enrolled": report.enrolment.is_some(),
        "shared_mic": shared_mic,
        "voice_checked": out.voice.checked,
        "voice_moved": out.voice.moved,
        "voice_scale": out.voice.scale,
    });
    Ok(json!({"id": job.id, "lines": lines(&segments), "before": before, "report": report}))
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
    let spans = match request.mode.as_deref() {
        None | Some("label") => false,
        Some("spans") => true,
        Some(other) => return Err(format!("unknown mode {other} (label, spans)")),
    };
    let results = request
        .jobs
        .into_iter()
        .map(|job| label(&request.variant, &request.params, spans, job))
        .collect::<Result<Vec<Value>, String>>()?;
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
