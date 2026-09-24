//! Diarization validates the complete timeline before reading PCM.

use dettivo_audio::takes::{Take, Takes};
use dettivo_meeting::{Track, diarize::read_track};

#[test]
fn an_overlong_import_or_take_is_rejected_from_metadata_before_reading_samples() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("system.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    writer.write_sample(1i16).unwrap();
    writer.finalize().unwrap();
    let valid = std::fs::read(&path).unwrap();
    let mut oversized = valid.clone();
    let data = oversized.windows(4).position(|w| w == b"data").unwrap();
    let bytes = ((dettivo_engine_proto::MAX_PCM_SAMPLES + 1) * 2) as u32;
    oversized[data + 4..data + 8].copy_from_slice(&bytes.to_le_bytes());
    std::fs::write(&path, oversized).unwrap();
    let error = read_track(dir.path(), Track::System).unwrap_err();
    assert!(error.contains("exceeds 14400 seconds"), "{error}");

    std::fs::write(&path, valid).unwrap();
    let takes = Takes {
        takes: vec![Take {
            index: 1,
            file: "system.wav".into(),
            start_offset_ms: 14_400_000,
            start_offset_ns: 14_400_000_000_000,
            gap_before: true,
            samples: 1,
        }],
    };
    std::fs::write(
        dir.path().join("system-takes.json"),
        serde_json::to_vec(&takes).unwrap(),
    )
    .unwrap();
    let error = read_track(dir.path(), Track::System).unwrap_err();
    assert!(error.contains("exceeds 14400 seconds"), "{error}");
}
