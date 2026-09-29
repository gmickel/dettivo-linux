//! ADR 0080: a clip whose decoding window holds fewer audio tokens than the
//! DTW median filter is wide made whisper.cpp abort the engine, which failed
//! a whole meeting on one chunk. About 0.12 s of the JFK fixture is such a
//! clip; the patched whisper.cpp answers it, and every word still has a
//! time inside the clip.

use std::path::PathBuf;
use std::process::Command;

const ENGINE: &str = env!("CARGO_BIN_EXE_dettivo-engine-whisper");

fn models() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    let dir = data.join("dettivo/models");
    dir.join("whisper/tiny.en/ggml-tiny.en.bin")
        .is_file()
        .then_some(dir)
}

/// `ms` of 16 kHz mono PCM from `from_ms` into `wav`, as a WAV file.
fn cut(wav: &std::path::Path, from_ms: usize, ms: usize) -> Vec<u8> {
    let bytes = std::fs::read(wav).unwrap();
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("a data chunk")
        + 8;
    let pcm = &bytes[data + from_ms * 32..data + (from_ms + ms) * 32];
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&16_000u32.to_le_bytes());
    out.extend_from_slice(&32_000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

#[test]
fn a_window_too_short_for_the_dtw_filter_does_not_abort() {
    let Some(models) = models() else {
        eprintln!("skip: test model missing (run scripts/models/fetch-test-model.sh)");
        return;
    };
    let model = models.join("whisper/tiny.en/ggml-tiny.en.bin");
    let dir = tempfile::tempdir().unwrap();
    for (from_ms, ms) in [(2000, 120), (500, 130), (4000, 115)] {
        let clip = dir.path().join("clip.wav");
        std::fs::write(&clip, cut(&models.join("fixtures/jfk.wav"), from_ms, ms)).unwrap();
        let out = Command::new(ENGINE)
            .args([
                "--wav",
                clip.to_str().unwrap(),
                "--model",
                model.to_str().unwrap(),
            ])
            .args(["--cpu", "--json"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{ms} ms from {from_ms} ms: {:?} {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        for s in result["segments"].as_array().unwrap() {
            for w in s["words"].as_array().cloned().unwrap_or_default() {
                let end = w["end_ms"].as_u64().unwrap();
                assert!(end <= ms as u64 + 10, "{w} past the {ms} ms clip");
                assert!(w["start_ms"].as_u64().unwrap() <= end, "{w}");
            }
        }
    }
}
