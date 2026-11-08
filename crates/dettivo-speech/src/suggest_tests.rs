use super::*;

/// `hear` for a clip the vocabulary left as the engine heard it unprimed.
fn same(term: &str, heard: &str, in_sentence: bool) -> (bool, Option<String>) {
    hear(term, heard, heard, in_sentence)
}

#[test]
fn a_term_heard_right_needs_nothing_and_a_misheard_one_gives_its_words() {
    assert_eq!(same("Hyprland", "Hyprland.", false), (true, None));
    assert_eq!(
        same("Hyprland", "I think hyprland is great.", true),
        (true, None)
    );
    assert_eq!(
        same("Hyprland", "Hyper land.", false),
        (false, Some("hyper land".into()))
    );
    assert_eq!(
        same("SapienXT", "I think Sapien XT is great.", true),
        (false, Some("sapien xt".into()))
    );
    // A spelt-apart term is a miss, as on the bench.
    assert_eq!(
        same("SapienXT", "Sapien-XT", false),
        (false, Some("sapien xt".into()))
    );
}

#[test]
fn a_clip_the_vocabulary_fixed_proposes_nothing_and_the_spelling_is_unprimed() {
    // The vocabulary in force already writes the term: no spelling needed.
    assert_eq!(hear("Omarchy", "Omarchy.", "Amarky.", false), (true, None));
    // It still misses: the spelling is what the engine heard with none.
    assert_eq!(
        hear("Hyprland", "Hyperland.", "Hypaland.", false),
        (false, Some("hypaland".into()))
    );
    // Heard right only without the vocabulary: nothing to propose.
    assert_eq!(hear("Clef", "Clay.", "Clef.", false), (false, None));
}

#[test]
fn a_sentence_that_did_not_come_back_or_a_long_stretch_proposes_nothing() {
    assert_eq!(
        same("Clef", "I thought clay was great.", true),
        (false, None)
    );
    assert_eq!(same("Clef", "I think is great.", true), (false, None));
    assert_eq!(same("Clef", "", false), (false, None));
    assert_eq!(
        same("Clef", "and so my fellow americans ask not", false),
        (false, None)
    );
    assert_eq!(
        same("Clef", "I think the clay pot is great.", true),
        (false, Some("the clay pot".into()))
    );
}

#[test]
fn proposals_keep_clip_order_once_each_and_skip_listed_spellings() {
    let heard = vec![
        (false, Some("hyper land".into())),
        (true, None),
        (false, Some("hyperland".into())),
        (false, Some("hyper land".into())),
        (false, None),
    ];
    assert_eq!(
        propose(&heard, &["Hyper-Land".into()]),
        (false, vec!["hyperland".to_string()])
    );
    assert_eq!(
        propose(&heard, &[]),
        (
            false,
            vec!["hyper land".to_string(), "hyperland".to_string()]
        )
    );
    assert_eq!(propose(&[(true, None), (true, None)], &[]), (true, vec![]));
}

#[test]
fn the_term_is_said_alone_and_in_its_sentence() {
    assert_eq!(
        sayings("Omarchy"),
        [
            "Omarchy".to_string(),
            "I think Omarchy is great.".to_string()
        ]
    );
}

/// A voice pair in `dir`, the second at `rate`.
fn voices(dir: &Path, rate: u32) {
    let piper = dir.join(VOICES_DIR);
    std::fs::create_dir_all(&piper).unwrap();
    for (voice, rate) in VOICES.iter().zip([16_000, rate]) {
        std::fs::write(piper.join(format!("{voice}.onnx")), b"model").unwrap();
        let config = format!("{{\"audio\": {{\"sample_rate\": {rate}}}}}");
        std::fs::write(piper.join(format!("{voice}.onnx.json")), config).unwrap();
    }
}

/// An executable `piper` in `dir` running `script`.
fn fake_piper(dir: &Path, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("piper");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn missing_piper_or_voices_are_named_with_how_to_get_them() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let path = std::env::join_paths([&bin]).unwrap();
    let err = Piper::find(dir.path(), Some(&path), None).unwrap_err();
    assert!(err.contains("pipx install piper-tts"), "{err}");
    assert!(
        err.contains("en_US-amy-low.onnx and en_US-amy-low.onnx.json"),
        "{err}"
    );
    assert!(err.contains(VOICES_URL), "{err}");

    voices(dir.path(), 22_050);
    let err = Piper::find(dir.path(), Some(&path), None).unwrap_err();
    assert!(err.contains("en_US-ryan-low speaks at 22050 Hz"), "{err}");
    assert!(!err.contains("en_US-amy-low"), "{err}");

    voices(dir.path(), 16_000);
    // `~/.local/bin` is where pipx puts it when PATH does not have it.
    let local = dir.path().join(".local/bin");
    std::fs::create_dir_all(&local).unwrap();
    let binary = fake_piper(&local, "exit 0");
    let piper = Piper::find(dir.path(), Some(&path), Some(dir.path())).unwrap();
    assert_eq!(piper.binary, binary);
    assert_eq!(
        piper
            .voices
            .iter()
            .map(|v| v.0.as_str())
            .collect::<Vec<_>>(),
        VOICES
    );
}

#[test]
fn speaking_reads_the_raw_samples_piper_writes_and_names_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    voices(dir.path(), 16_000);
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    // Echo the arguments and the text back as the "audio" to check them.
    fake_piper(
        &bin,
        "printf '\\001\\000\\377\\177'; cat > \"$0.text\"; echo \"$@\" > \"$0.args\"",
    );
    let path = std::env::join_paths([&bin]).unwrap();
    let piper = Piper::find(dir.path(), Some(&path), None).unwrap();
    let model = &piper.voices[0].1;
    assert_eq!(piper.speak(model, "Clef").unwrap(), vec![1, i16::MAX]);
    let read = |ext: &str| std::fs::read_to_string(bin.join(format!("piper.{ext}"))).unwrap();
    assert_eq!(read("text"), "Clef\n");
    assert_eq!(
        read("args").trim(),
        format!(
            "-m {} --noise-scale 0 --noise-w-scale 0 --output-raw",
            model.display()
        )
    );

    fake_piper(&bin, "echo 'voice failed to load' >&2; exit 3");
    let err = piper.speak(model, "Clef").unwrap_err();
    assert_eq!(err, "piper could not speak \"Clef\": voice failed to load");
}
