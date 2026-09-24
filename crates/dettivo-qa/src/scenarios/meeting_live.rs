//! Live meeting transcription (fn-28 R2): the jfk clip plays as the other
//! side of the call through `DETTIVO_MOCK_SYSTEM_AUDIO`, its last phrase
//! plays as the microphone after four seconds of silence through
//! `DETTIVO_MOCK_MIC` so the two overlap in one span, and tiny.en
//! transcribes both. While the meeting runs, `meeting.segment` events
//! arrive provisional then final per source in time order; after the
//! stop, `meetings.stop` answers the contract's `transcribing` job, the
//! progress climbs per chunk, and the finalised transcript on the row is
//! compared with the two-source golden: each source's words within the
//! error ceiling and both sources interleaved by start on the meeting
//! clock. Needs the test model; no driver, no display, no PipeWire.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use super::daemon::{DaemonHandle, local_model};
use super::meeting_live_events::{PLAY_FOR, collect_live, provisional_sources, stamped};
use super::{Context, Scenario, binary};
use crate::pack::wer::{wer, words};

/// The golden, relative to the repository root: one line per source,
/// `source<TAB>text`.
pub const GOLDEN: &str = "crates/dettivo-qa/fixtures/meeting-live/golden.txt";
/// The word error rate each source's transcript must stay under.
pub const WER_CEILING: f64 = 0.25;
/// Where the microphone fixture cuts into the clip: between `you` (ends
/// at 7353 ms in the alignment golden) and the second `ask` (8395 ms).
const MIC_CUT_MS: u64 = 7900;
/// The silence before the microphone's phrase, so it overlaps the other
/// side's `what your country can do for you`.
const MIC_LEAD_MS: u64 = 4000;

/// The scenario.
pub struct MeetingLive;

/// What `meeting-live.json` records.
#[derive(Debug, Serialize)]
struct Evidence {
    meeting_id: String,
    segment_events: usize,
    provisional_events: usize,
    final_events: usize,
    first_provisional_before_first_final: BTreeMap<String, bool>,
    /// The sources whose provisional text arrived before the stop.
    provisional_before_stop: Vec<String>,
    finals_in_time_order: BTreeMap<String, bool>,
    stop_answered: String,
    progress_chunks: Vec<u64>,
    states: Vec<String>,
    transcript: String,
    segments: usize,
    first_segment_source: String,
    interleaved_by_start: bool,
    wer_per_source: BTreeMap<String, f64>,
    wer_ceiling: f64,
    search_hit: bool,
}

/// The golden per source.
fn golden(repo: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = std::fs::read_to_string(repo.join(GOLDEN)).map_err(|e| format!("{GOLDEN}: {e}"))?;
    let mut out = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let (source, words) = line
            .split_once('\t')
            .ok_or_else(|| format!("{GOLDEN}: a line without a tab: {line}"))?;
        out.insert(source.to_string(), words.trim().to_string());
    }
    Ok(out)
}

/// The microphone fixture: `MIC_LEAD_MS` of silence, then the clip from
/// `MIC_CUT_MS` on.
pub fn write_microphone_fixture(clip: &Path, out: &Path) -> Result<(), String> {
    let mut reader =
        hound::WavReader::open(clip).map_err(|e| format!("{}: {e}", clip.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 || spec.channels != 1 {
        return Err(format!(
            "{} is not 16 kHz mono ({} Hz, {} channels)",
            clip.display(),
            spec.sample_rate,
            spec.channels
        ));
    }
    let samples: Vec<i16> = reader.samples::<i16>().filter_map(Result::ok).collect();
    let cut = (MIC_CUT_MS * 16) as usize;
    if samples.len() <= cut {
        return Err(format!(
            "{} is shorter than {MIC_CUT_MS} ms",
            clip.display()
        ));
    }
    let mut writer = hound::WavWriter::create(out, spec).map_err(|e| e.to_string())?;
    for _ in 0..(MIC_LEAD_MS * 16) {
        writer.write_sample(0i16).map_err(|e| e.to_string())?;
    }
    for &s in &samples[cut..] {
        writer.write_sample(s).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

fn config(repo_root: &Path) -> String {
    let bin_dir = binary(repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n",
        bin_dir.display()
    )
}

fn wait_settled(daemon: &DaemonHandle, id: &str, budget: Duration) -> Result<Value, String> {
    let deadline = Instant::now() + budget;
    loop {
        let status = daemon.call("meetings.status", json!({"meeting_id": id}))?;
        let state = status["status"].as_str().unwrap_or("");
        if matches!(state, "completed" | "failed" | "stopped")
            && status["is_finalizing"] != json!(true)
        {
            return Ok(status);
        }
        if Instant::now() > deadline {
            return Err(format!("the meeting never settled: {status}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

impl Scenario for MeetingLive {
    fn id(&self) -> &'static str {
        "meeting_live"
    }

    fn summary(&self) -> &'static str {
        "the mock tracks play two overlapping clips; meeting.segment streams provisional then final per source and the finalised transcript matches the two-source golden"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })
    }

    fn run(
        &self,
        _driver: &mut dyn crate::driver::Driver,
        ctx: &mut Context<'_>,
    ) -> Result<(), String> {
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let (_, clip) = local_model(ctx.profile).ok_or("model missing")?;
        let golden = golden(ctx.repo_root)?;
        let mic_fixture = ctx.evidence_dir.join("microphone-fixture.wav");
        write_microphone_fixture(&clip, &mic_fixture)?;
        let env: BTreeMap<String, String> = BTreeMap::from([
            (
                "DETTIVO_MOCK_MIC".to_string(),
                mic_fixture.to_string_lossy().into_owned(),
            ),
            (
                "DETTIVO_MOCK_SYSTEM_AUDIO".to_string(),
                clip.to_string_lossy().into_owned(),
            ),
        ]);
        let mut daemon = DaemonHandle::spawn(
            &dettivod,
            ctx.profile,
            &config(ctx.repo_root),
            &env,
            ctx.timeout,
        )?;
        let mut events = daemon.subscribe(&["meeting.state", "meeting.segment", "job.progress"])?;
        ctx.timings.mark("daemon");

        let started = daemon.call(
            "meetings.start",
            json!({"capture": {"microphone": true, "system_audio": true}, "title": "Live", "acknowledge_meeting_disclosure": true}),
        )?;
        let id = started["ref"]["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("no ref.id: {started}"))?;
        // The clip is eleven seconds; a little more lets the last
        // segments harden behind the boundary gap before the stop. The
        // notifications are read while the meeting runs, each stamped
        // with its arrival, so provisional text is proven delivered to a
        // consumer before the stop and not merely ordered afterwards.
        let started_at = Instant::now();
        let mut seen = collect_live(&mut events, started_at, PLAY_FOR);
        ctx.timings.mark("played");
        let live_before_stop = provisional_sources(&seen);

        let stopped = daemon.call("meetings.stop", json!({"meeting_id": id}))?;
        let stop_answered = stopped["job"]["message"].as_str().unwrap_or("").to_string();
        let status = wait_settled(&daemon, &id, Duration::from_secs(120))?;
        ctx.timings.mark("finalised");
        events
            .set_read_timeout(Duration::from_secs(10))
            .map_err(|e| format!("event stream: {e}"))?;
        let stop_ms = started_at.elapsed().as_millis() as u64;
        seen.extend(
            events
                .collect(
                    |p| {
                        p["topic"] == "meeting.state"
                            && matches!(
                                p["payload"]["state"].as_str(),
                                Some("completed" | "failed")
                            )
                    },
                    Duration::from_secs(20),
                )
                .into_iter()
                .map(|p| stamped(p, stop_ms, false)),
        );
        let got = daemon.call("meetings.get", json!({"meeting_id": id}))?;
        let hits = daemon.call("meetings.search", json!({"query": "country", "limit": 10}))?;
        let search_hit = hits["items"]
            .as_array()
            .map(|h| h.iter().any(|i| i["ref"]["id"] == json!(id)))
            .unwrap_or(false);
        daemon.stop();
        ctx.timings.mark("stopped");

        let (evidence, verdict) = judge(
            &id,
            &seen,
            &got,
            &golden,
            stop_answered,
            search_hit,
            &live_before_stop,
        );
        write_evidence(ctx, &id, &seen, &got, &evidence)?;
        if status["status"] != json!("completed") {
            return Err(format!("the meeting did not complete: {status}"));
        }
        verdict
    }
}

/// The judgement over the events and the row; the evidence comes back
/// either way so a failure is readable. `live_before_stop` names the
/// sources whose provisional text reached the stream before the stop.
fn judge(
    id: &str,
    seen: &[Value],
    got: &Value,
    golden: &BTreeMap<String, String>,
    stop_answered: String,
    search_hit: bool,
    live_before_stop: &[String],
) -> (Evidence, Result<(), String>) {
    let segments: Vec<&Value> = seen
        .iter()
        .filter(|p| p["topic"] == "meeting.segment")
        .map(|p| &p["payload"])
        .collect();
    let mut first_provisional_before_first_final = BTreeMap::new();
    let mut finals_in_time_order = BTreeMap::new();
    let mut problems = Vec::new();
    for source in ["you", "remote"] {
        if !live_before_stop.iter().any(|s| s == source) {
            problems.push(format!(
                "{source}: no provisional segment reached the stream before the stop"
            ));
        }
        let mine: Vec<&&Value> = segments.iter().filter(|s| s["source"] == source).collect();
        if mine.is_empty() {
            problems.push(format!("{source} never produced a live segment"));
            first_provisional_before_first_final.insert(source.to_string(), false);
            finals_in_time_order.insert(source.to_string(), false);
            continue;
        }
        let first_final = mine.iter().position(|s| s["provisional"] == false);
        let ordered = first_final
            .is_some_and(|i| i > 0 && mine[..i].iter().all(|s| s["provisional"] == true));
        first_provisional_before_first_final.insert(source.to_string(), ordered);
        if !ordered {
            problems.push(format!("{source}: not provisional then final"));
        }
        let starts: Vec<u64> = mine
            .iter()
            .filter(|s| s["provisional"] == false)
            .filter_map(|s| s["start_ms"].as_u64())
            .collect();
        let in_order = starts.windows(2).all(|w| w[0] <= w[1]);
        finals_in_time_order.insert(source.to_string(), in_order);
        if !in_order {
            problems.push(format!("{source}: finals out of time order: {starts:?}"));
        }
        for s in &mine {
            let shaped = s["meeting_id"] == id
                && s["segment_id"]
                    .as_str()
                    .is_some_and(|x| x.starts_with(source))
                && s["end_ms"].as_u64() >= s["start_ms"].as_u64()
                && s["text"].is_string()
                && s["words"].is_array();
            if !shaped {
                problems.push(format!(
                    "a segment payload is off the registered shape: {s}"
                ));
                break;
            }
        }
    }
    let progress_chunks: Vec<u64> = seen
        .iter()
        .filter(|p| p["topic"] == "job.progress" && p["payload"]["stage"] == "transcribing")
        .filter_map(|p| p["payload"]["chunks_done"].as_u64())
        .collect();
    if progress_chunks.is_empty() {
        problems.push("job.progress never reported a transcribing chunk".into());
    }
    if stop_answered != "transcribing" {
        problems.push(format!(
            "meetings.stop answered {stop_answered:?}, not transcribing"
        ));
    }
    let states: Vec<String> = seen
        .iter()
        .filter(|p| p["topic"] == "meeting.state")
        .filter_map(|p| p["payload"]["state"].as_str().map(str::to_string))
        .collect();
    let rows = got["segments"].as_array().cloned().unwrap_or_default();
    let interleaved_by_start = rows
        .windows(2)
        .all(|w| w[0]["start_ms"].as_u64() <= w[1]["start_ms"].as_u64());
    if !interleaved_by_start {
        problems.push("the row's segments are not ordered by start".into());
    }
    let first_segment_source = rows
        .first()
        .and_then(|s| s["source_type"].as_str())
        .unwrap_or("")
        .to_string();
    let mut wer_per_source = BTreeMap::new();
    for (source, kind) in [("you", "microphone"), ("remote", "system")] {
        let text = rows
            .iter()
            .filter(|s| s["source_type"] == kind)
            .filter_map(|s| s["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let reference = golden.get(source).cloned().unwrap_or_default();
        let rate = wer(&words(&reference), &words(&text));
        wer_per_source.insert(source.to_string(), rate);
        if rate > WER_CEILING {
            problems.push(format!(
                "{source}: word error rate {rate:.3} over {WER_CEILING} ({text:?})"
            ));
        }
    }
    if !search_hit {
        problems.push("meetings.search does not find the transcript".into());
    }
    let evidence = Evidence {
        meeting_id: id.to_string(),
        segment_events: segments.len(),
        provisional_events: segments.iter().filter(|s| s["provisional"] == true).count(),
        final_events: segments
            .iter()
            .filter(|s| s["provisional"] == false)
            .count(),
        first_provisional_before_first_final,
        provisional_before_stop: live_before_stop.to_vec(),
        finals_in_time_order,
        stop_answered,
        progress_chunks,
        states,
        transcript: got["transcript"].as_str().unwrap_or("").to_string(),
        segments: rows.len(),
        first_segment_source,
        interleaved_by_start,
        wer_per_source,
        wer_ceiling: WER_CEILING,
        search_hit,
    };
    let verdict = if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    };
    (evidence, verdict)
}

fn write_evidence(
    ctx: &mut Context<'_>,
    id: &str,
    seen: &[Value],
    got: &Value,
    evidence: &Evidence,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(evidence).map_err(|e| e.to_string())?;
    std::fs::write(ctx.evidence_dir.join("meeting-live.json"), json).map_err(|e| e.to_string())?;
    ctx.evidence.push("meeting-live.json".into());
    let lines: Vec<String> = seen.iter().map(Value::to_string).collect();
    std::fs::write(
        ctx.evidence_dir.join("events.jsonl"),
        lines.join("\n") + "\n",
    )
    .map_err(|e| e.to_string())?;
    ctx.evidence.push("events.jsonl".into());
    std::fs::write(
        ctx.evidence_dir.join("transcript.json"),
        serde_json::to_string_pretty(&got["segments"]).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    ctx.evidence.push("transcript.json".into());
    let dir: PathBuf = ctx.profile.root.join("data/dettivo/meetings").join(id);
    if std::fs::copy(
        dir.join("journal.jsonl"),
        ctx.evidence_dir.join("journal.jsonl"),
    )
    .is_ok()
    {
        ctx.evidence.push("journal.jsonl".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(source: &str, provisional: bool, before_stop: bool) -> Value {
        stamped(
            json!({"topic": "meeting.segment", "payload": {"source": source, "provisional": provisional}}),
            if before_stop { 3_000 } else { 14_000 },
            before_stop,
        )
    }

    #[test]
    fn a_daemon_that_releases_every_notification_after_the_stop_fails() {
        let held = vec![
            segment("you", true, false),
            segment("remote", true, false),
            segment("you", false, false),
        ];
        let (_, verdict) = judge(
            "m",
            &held,
            &json!({"segments": [], "transcript": ""}),
            &BTreeMap::new(),
            "transcribing".into(),
            true,
            &provisional_sources(&held),
        );
        let why = verdict.unwrap_err();
        assert!(
            why.contains("you: no provisional segment reached the stream before the stop"),
            "{why}"
        );
        assert!(
            why.contains("remote: no provisional segment reached the stream before the stop"),
            "{why}"
        );
    }
}
