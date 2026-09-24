//! The alpha token fixture through the whole lane (fn-36, ADR 0039): two
//! voices, Alice on the microphone track and Ben on the system track,
//! play through the rig's null sinks where PipeWire runs and through
//! the mock variables elsewhere; the meeting is stopped and finalised,
//! the speaker pass runs over the system track, the two speakers are
//! named and the `md` export is pulled. The scenario passes when every
//! token in `alpha.tokens.json` is in the finalised transcript on its own
//! source, the row carries two speakers, and the export names both.
//! Needs the test model and the diarization model set; no driver.

use crate::child::ChildOwner;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::daemon::{DaemonHandle, local_model};
pub use super::meeting_token_support::{FIXTURE_DIR, NAMES, Token, TokenCoverage};
use super::meeting_token_support::{TokensFile, export_md, judge, write_evidence};
use super::{Context, Scenario, binary};
use crate::audio;
use crate::driver::Driver;

/// The scenario.
pub struct MeetingTokenCoverage;

fn rig(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(audio::rig_script(repo))
        .args(args)
        .output()
        .map_err(|e| format!("audio-rig: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "audio-rig {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn play(sink: &str, file: &Path) -> Result<ChildOwner, String> {
    Command::new("pw-play")
        .args(["--target", sink])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(ChildOwner::new)
        .map_err(|e| format!("pw-play: {e}"))
}

fn rig_available() -> bool {
    audio::pipewire_available()
        && Command::new("pw-play")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
}

fn config(repo_root: &Path, capture: Option<(&str, &str)>) -> String {
    let bin_dir = binary(repo_root, "dettivod")
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let mut toml = format!(
        "[engines]\ndirectory = \"{}\"\n[speech]\nmodel = \"tiny.en\"\n[dictation]\nlanguage = \"en\"\n[meetings]\nlive = false\n[meetings.diarization]\nauto = false\n[meetings.analysis]\nauto = false\n",
        bin_dir.display()
    );
    if let Some((mic, sys)) = capture {
        toml.push_str(&format!(
            "[audio]\ninput_device = \"{mic}\"\nsystem_source = \"{sys}\"\n"
        ));
    }
    toml
}

fn wait_status(
    daemon: &DaemonHandle,
    id: &str,
    done: impl Fn(&Value) -> bool,
    budget: Duration,
) -> Result<Value, String> {
    let deadline = Instant::now() + budget;
    loop {
        let status = daemon.call("meetings.status", json!({"meeting_id": id}))?;
        if done(&status) {
            return Ok(status);
        }
        if Instant::now() > deadline {
            return Err(format!("the meeting never settled: {status}"));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

impl Scenario for MeetingTokenCoverage {
    fn id(&self) -> &'static str {
        "meeting_token_coverage"
    }

    fn summary(&self) -> &'static str {
        "the two-voice alpha fixture through capture, finalisation and the speaker pass; every token on its source, both names in the md export"
    }

    fn needs_driver(&self) -> bool {
        false
    }

    fn models(&self) -> &'static [&'static str] {
        &["diarize/diarization-en"]
    }

    fn preconditions(&self, ctx: &Context<'_>) -> Result<(), String> {
        local_model(ctx.profile).map(|_| ()).ok_or_else(|| {
            "tiny.en or jfk.wav missing under the model directory (scripts/models/fetch-test-model.sh)".to_string()
        })
    }

    fn run(&self, _driver: &mut dyn Driver, ctx: &mut Context<'_>) -> Result<(), String> {
        let models = ctx
            .profile
            .root
            .join("data/dettivo/models/diarize/diarization-en");
        if !models.join("segmentation.onnx").is_file() {
            return Err(
                "the diarization model set is not downloaded (scripts/models/fetch-diarization-model.sh)".into(),
            );
        }
        let dettivod = binary(ctx.repo_root, "dettivod")?;
        let fixtures = ctx.repo_root.join(FIXTURE_DIR);
        let tokens: TokensFile = serde_json::from_str(
            &std::fs::read_to_string(fixtures.join("alpha.tokens.json"))
                .map_err(|e| format!("alpha.tokens.json: {e}"))?,
        )
        .map_err(|e| format!("alpha.tokens.json: {e}"))?;
        let mic = fixtures.join("alpha.mic.wav");
        let sys = fixtures.join("alpha.system.wav");
        let pid = std::process::id();
        let sinks = rig_available().then(|| {
            (
                format!("dettivo-qa-tmic-{pid}"),
                format!("dettivo-qa-tsys-{pid}"),
            )
        });
        if let Some((m, s)) = &sinks {
            rig(ctx.repo_root, &["up", m])?;
            if let Err(e) = rig(ctx.repo_root, &["up", s]) {
                let _ = rig(ctx.repo_root, &["down", m]);
                return Err(e);
            }
        }
        let result = self.drive(ctx, &dettivod, &tokens, (&mic, &sys), sinks.as_ref());
        if let Some((m, s)) = &sinks {
            let _ = rig(ctx.repo_root, &["down", m]);
            let _ = rig(ctx.repo_root, &["down", s]);
        }
        result
    }
}

impl MeetingTokenCoverage {
    fn drive(
        &self,
        ctx: &mut Context<'_>,
        dettivod: &Path,
        tokens: &TokensFile,
        (mic, sys): (&Path, &Path),
        sinks: Option<&(String, String)>,
    ) -> Result<(), String> {
        let mut env: BTreeMap<String, String> = BTreeMap::new();
        let capture = match sinks {
            Some(_) => {
                env.insert("DETTIVO_MOCK_MODE".into(), "0".into());
                env.insert("DETTIVO_MOCK_INSERT".into(), "1".into());
                "rig"
            }
            None => {
                env.insert(
                    "DETTIVO_MOCK_MIC".into(),
                    mic.to_string_lossy().into_owned(),
                );
                env.insert(
                    "DETTIVO_MOCK_SYSTEM_AUDIO".into(),
                    sys.to_string_lossy().into_owned(),
                );
                "mock"
            }
        };
        let config = config(ctx.repo_root, sinks.map(|(m, s)| (m.as_str(), s.as_str())));
        let mut daemon = DaemonHandle::spawn(dettivod, ctx.profile, &config, &env, ctx.timeout)?;
        ctx.timings.mark("daemon");
        let started = daemon.call(
            "meetings.start",
            json!({"capture": {"microphone": true, "system_audio": true}, "title": "Alpha", "acknowledge_meeting_disclosure": true, "expected_speakers": 1}),
        )?;
        let id = started["ref"]["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("no ref.id: {started}"))?;
        match sinks {
            Some((m, s)) => {
                std::thread::sleep(Duration::from_millis(400));
                let mut a = play(m, mic)?;
                let mut b = play(s, sys)?;
                let _ = a.wait();
                let _ = b.wait();
                std::thread::sleep(Duration::from_millis(600));
            }
            None => std::thread::sleep(Duration::from_millis(tokens.duration_ms + 1_500)),
        }
        ctx.timings.mark("played");
        daemon.call("meetings.stop", json!({"meeting_id": id}))?;
        let settled = wait_status(
            &daemon,
            &id,
            |s| {
                matches!(
                    s["status"].as_str(),
                    Some("completed" | "failed" | "stopped")
                ) && s["is_finalizing"] != json!(true)
            },
            Duration::from_secs(180),
        )?;
        if settled["status"] != json!("completed") {
            return Err(format!("the meeting did not complete: {settled}"));
        }
        ctx.timings.mark("finalised");
        daemon.call("meetings.diarize", json!({"meeting_id": id, "speakers": 1}))?;
        let deadline = Instant::now() + Duration::from_secs(180);
        let listed = loop {
            let listed = daemon.call("meetings.speakers.list", json!({"meeting_id": id}))?;
            let status = listed["diarization"]["status"].as_str().unwrap_or("");
            if matches!(status, "ready" | "failed" | "unavailable") {
                break listed;
            }
            if Instant::now() > deadline {
                return Err(format!("the speaker pass never settled: {listed}"));
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let diarization_status = listed["diarization"]["status"]
            .as_str()
            .unwrap_or("")
            .to_string();
        ctx.timings.mark("diarized");
        let mut speakers: Vec<String> = Vec::new();
        for speaker in listed["speakers"].as_array().into_iter().flatten() {
            let speaker_id = speaker["speaker_id"].as_str().unwrap_or("");
            let name = if speaker_id == "you" {
                NAMES[0].1
            } else {
                NAMES[1].1
            };
            daemon.call(
                "meetings.speakers.rename",
                json!({"meeting_id": id, "speaker_id": speaker_id, "name": name}),
            )?;
            speakers.push(format!("{speaker_id} -> {name}"));
        }
        let got = daemon.call("meetings.get", json!({"meeting_id": id}))?;
        let md = export_md(&daemon, &id)?;
        daemon.stop();
        ctx.timings.mark("exported");

        let (coverage, verdict) = judge(
            &id,
            tokens,
            &got,
            &md,
            capture,
            speakers,
            diarization_status,
        );
        write_evidence(ctx, &got, &md, &coverage)?;
        verdict
    }
}
