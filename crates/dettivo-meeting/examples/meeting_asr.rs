//! The diarization bench's meeting transcription (docs/diarization-bench.md):
//! a retained meeting's takes through the product's own finalisation
//! (`dettivo_meeting::finalize::run`) with a given Whisper engine build,
//! so the bench can score the segments and words that build would store.
//!
//! `meeting_asr <meeting dir> <engine dir> <model> <language>` prints
//! `{"segments": [<Segment>]}`; `<engine dir>` holds the
//! `dettivo-engine-whisper` binary the supervisor starts.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use dettivo_meeting::finalize;
use dettivo_speech::supervisor::{Settings, Supervisor, WhisperEngine};
use dettivo_transcribe::live::{LiveSettings, contract_segments};
use dettivo_transcribe::{Request, Settings as Transcribe};

fn run(args: &[String]) -> Result<String, String> {
    let [dir, engines, model, language] = args else {
        return Err("usage: meeting_asr <meeting dir> <engine dir> <model> <language>".into());
    };
    let dir = PathBuf::from(dir);
    let takes = finalize::takes_in(&dir).map_err(|e| format!("takes: {e}"))?;
    let supervisor = Supervisor::new(Settings {
        directory: Some(PathBuf::from(engines)),
        ..Settings::default()
    });
    let engine = WhisperEngine::new(supervisor, model.clone(), None);
    let request = Request {
        language: language.clone(),
        prompt: None,
        from_system: false,
    };
    let outcome = finalize::run(
        &takes,
        &finalize::journal_gaps(&dir),
        &engine,
        &request,
        &Transcribe::default(),
        &LiveSettings::default(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .map_err(|e| format!("finalisation: {e}"))?;
    let segments = contract_segments(&outcome.segments);
    serde_json::to_string(&serde_json::json!({ "segments": segments })).map_err(|e| e.to_string())
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(json) => {
            println!("{json}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("meeting_asr: {e}");
            std::process::ExitCode::from(2)
        }
    }
}
