use super::*;

fn entry(term: &str, sounds_like: &[&str]) -> VocabularyEntry {
    if sounds_like.is_empty() {
        VocabularyEntry::Word(term.into())
    } else {
        VocabularyEntry::Entry {
            term: term.into(),
            sounds_like: sounds_like.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[test]
fn new_entries_follow_the_last_and_every_written_entry_keeps_its_text() {
    // A repeat the loader drops, a comment inside an entry's spellings and
    // a comma-less last entry all stay exactly as written.
    let before = concat!(
        "# mine\n",
        "[dictation]\n",
        "vocabulary = [\n",
        "  \"Clef\", # the app\n",
        "  { term = \"CLEF\", sounds_like = [\"cleff\"] }, # a repeat\n",
        "  { term = \"Omarchy\", sounds_like = [\n",
        "    \"oh marky\", # how I say it\n",
        "  ] },\n",
        "  \"Dettivo\" # last\n",
        "]\n",
        "language = \"en\"\n",
    );
    let out = add(
        before,
        &[entry("Hyprland", &[]), entry("SapienXT", &["sapient x t"])],
    )
    .unwrap();
    assert_eq!(
        out.text,
        concat!(
            "# mine\n",
            "[dictation]\n",
            "vocabulary = [\n",
            "  \"Clef\", # the app\n",
            "  { term = \"CLEF\", sounds_like = [\"cleff\"] }, # a repeat\n",
            "  { term = \"Omarchy\", sounds_like = [\n",
            "    \"oh marky\", # how I say it\n",
            "  ] },\n",
            "  \"Dettivo\", # last\n",
            "  \"Hyprland\",\n",
            "  { term = \"SapienXT\", sounds_like = [\"sapient x t\"] }\n",
            "]\n",
            "language = \"en\"\n",
        )
    );
    assert_eq!((out.added, out.entries), (vec![0, 1], 6));
}

#[test]
fn a_one_line_list_grows_on_its_line_and_a_missing_key_is_written() {
    let out = add(
        "[dictation]\nvocabulary = [\"Clef\"] # mine\n",
        &[entry("Niri", &[])],
    );
    assert_eq!(
        out.unwrap().text,
        "[dictation]\nvocabulary = [\"Clef\", \"Niri\"] # mine\n"
    );
    let out = add(
        "[daemon]\n# quiet\nlog_level = \"warn\"\n",
        &[entry("Niri", &["near E"])],
    );
    assert_eq!(
        out.unwrap().text,
        "[daemon]\n# quiet\nlog_level = \"warn\"\n\n[dictation]\nvocabulary = [{ term = \"Niri\", sounds_like = [\"near E\"] }]\n"
    );
}

#[test]
fn a_term_the_file_or_an_earlier_entry_holds_is_skipped_in_any_case() {
    let text = "[dictation]\nvocabulary = [\"Clef\"]\n";
    let out = add(
        text,
        &[
            entry("clef", &[]),
            entry("Niri", &[]),
            entry("NIRI", &["near E"]),
        ],
    )
    .unwrap();
    assert_eq!(out.added, vec![1]);
    assert_eq!(
        out.skipped,
        vec![
            Skipped {
                index: 0,
                repeats: None
            },
            Skipped {
                index: 2,
                repeats: Some(1)
            },
        ]
    );
    let none = add(text, &[entry("CLEF", &[])]).unwrap();
    assert_eq!((none.text.as_str(), none.entries), (text, 1));
}

#[test]
fn every_entry_that_breaks_a_limit_is_refused_by_index() {
    let long = "x".repeat(MAX_TERM_CHARS + 1);
    let spellings: Vec<&str> = vec!["s"; MAX_ALIASES + 1];
    let refused = add(
        "",
        &[
            entry("Clef", &[]),
            entry(&long, &[]),
            entry("Sway", &spellings),
        ],
    )
    .unwrap_err();
    assert_eq!(
        refused,
        AddError::Refused(vec![
            (
                1,
                format!("{long:?} is 65 characters; a term holds at most 64")
            ),
            (
                2,
                "\"Sway\" has 17 sounds_like spellings; a term takes at most 16".into()
            ),
        ])
    );
    // The count includes a repeat the file writes, as the loader's limit does.
    let full: Vec<String> = (0..MAX_ENTRIES - 1).map(|i| format!("\"t{i}\"")).collect();
    let text = format!("[dictation]\nvocabulary = [{}, \"T0\"]\n", full.join(", "));
    let refused = add(&text, &[entry("n0", &[]), entry("n1", &[])]).unwrap_err();
    assert_eq!(
        refused,
        AddError::Refused(vec![
            (
                0,
                "\"n0\" would be entry 501; a vocabulary holds at most 500".into()
            ),
            (
                1,
                "\"n1\" would be entry 502; a vocabulary holds at most 500".into()
            ),
        ])
    );
}

#[test]
fn the_byte_budget_is_the_serialized_list_the_loader_checks() {
    let spellings = vec!["x".repeat(160); MAX_ALIASES];
    let entries: Vec<VocabularyEntry> = (0..MAX_ENTRIES)
        .map(|i| VocabularyEntry::Entry {
            term: format!("t{i}"),
            sounds_like: spellings.clone(),
        })
        .collect();
    let AddError::Refused(refused) = add("", &entries).unwrap_err() else {
        panic!("refused");
    };
    let first = refused[0].0;
    assert!(json_len(&entries[..first]) <= MAX_VOCABULARY_BYTES);
    let bytes = json_len(&entries[..=first]);
    assert!(bytes > MAX_VOCABULARY_BYTES);
    assert_eq!(
        refused[0].1,
        format!(
            "\"t{first}\" would take the vocabulary to {bytes} bytes; it holds at most {MAX_VOCABULARY_BYTES}"
        )
    );
    assert_eq!(refused.len(), MAX_ENTRIES - first);
}

#[test]
fn a_file_that_does_not_load_is_refused_as_it_is() {
    let err = add("[dictation]\nlanguage = 5\n", &[entry("Niri", &[])]).unwrap_err();
    assert!(
        matches!(&err, AddError::Invalid(e) if e.key.as_deref() == Some("dictation.language")),
        "{err:?}"
    );
}
