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
    let out = add("[dictation]\nlanguage = \"en\"\n", &[entry("Niri", &[])]).unwrap();
    assert_eq!(
        out.text,
        "[dictation]\nlanguage = \"en\"\nvocabulary = [\"Niri\"]\n"
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

#[test]
fn a_vocabulary_of_tables_grows_by_tables_after_the_last() {
    // `[[dictation.vocabulary]]` tables, a comment and a later table all
    // stay as written; each new entry is a table after the last one.
    let before = concat!(
        "[dictation]\n",
        "language = \"en\"\n",
        "\n",
        "# my words\n",
        "[[dictation.vocabulary]]\n",
        "term = \"Clef\" # the app\n",
        "sounds_like = [\"cleff\"]\n",
        "\n",
        "[[dictation.vocabulary]]\n",
        "term = \"Dettivo\"\n",
        "\n",
        "[daemon]\n",
        "log_level = \"warn\"\n",
    );
    let out = add(
        before,
        &[
            entry("clef", &[]),
            entry("Hyprland", &[]),
            entry("SapienXT", &["sapient x t"]),
        ],
    )
    .unwrap();
    assert_eq!(
        out.text,
        concat!(
            "[dictation]\n",
            "language = \"en\"\n",
            "\n",
            "# my words\n",
            "[[dictation.vocabulary]]\n",
            "term = \"Clef\" # the app\n",
            "sounds_like = [\"cleff\"]\n",
            "\n",
            "[[dictation.vocabulary]]\n",
            "term = \"Dettivo\"\n",
            "\n",
            "[[dictation.vocabulary]]\n",
            "term = \"Hyprland\"\n",
            "\n",
            "[[dictation.vocabulary]]\n",
            "term = \"SapienXT\"\n",
            "sounds_like = [\"sapient x t\"]\n",
            "\n",
            "[daemon]\n",
            "log_level = \"warn\"\n",
        )
    );
    assert_eq!((out.added, out.entries), (vec![1, 2], 4));
    validate(&out.text).unwrap();
}

#[test]
fn a_term_added_to_tables_counts_as_the_table_it_loads_as() {
    // Tables close to the budget, then terms that fit it as strings but
    // not as the `{ term, sounds_like }` tables the file will load.
    let spellings = vec!["x".repeat(160); MAX_ALIASES];
    let mut base: Vec<VocabularyEntry> = Vec::new();
    loop {
        let next = VocabularyEntry::Entry {
            term: format!("f{}", base.len()),
            sounds_like: spellings.clone(),
        };
        base.push(next);
        if json_len(&base) > MAX_VOCABULARY_BYTES - 1000 {
            base.pop();
            break;
        }
    }
    let quoted: Vec<String> = spellings.iter().map(|s| format!("{s:?}")).collect();
    let text: String = base
        .iter()
        .map(|e| {
            format!(
                "[[dictation.vocabulary]]\nterm = {:?}\nsounds_like = [{}]\n\n",
                e.term(),
                quoted.join(", ")
            )
        })
        .collect();
    let slack = MAX_VOCABULARY_BYTES - json_len(&base);
    let imports: Vec<VocabularyEntry> = (0..slack / 63)
        .map(|i| VocabularyEntry::Word(format!("w{i:059}")))
        .collect();
    let mut as_strings = base.clone();
    as_strings.extend(imports.iter().cloned());
    assert!(json_len(&as_strings) <= MAX_VOCABULARY_BYTES);
    let mut loaded = base.clone();
    let first = imports
        .iter()
        .position(|e| {
            loaded.push(VocabularyEntry::Entry {
                term: e.term().to_string(),
                sounds_like: vec![],
            });
            json_len(&loaded) > MAX_VOCABULARY_BYTES
        })
        .expect("the tables overrun the budget");
    let AddError::Refused(refused) = add(&text, &imports).unwrap_err() else {
        panic!("refused");
    };
    let bytes = json_len(&loaded);
    assert_eq!(
        refused[0],
        (
            first,
            format!(
                "{:?} would take the vocabulary to {bytes} bytes; it holds at most {MAX_VOCABULARY_BYTES}",
                imports[first].term()
            )
        )
    );
    assert_eq!(refused.len(), imports.len() - first);
}

#[test]
fn extending_gives_a_held_term_the_spellings_it_lacks_and_keeps_its_text() {
    // fn-96: `vocabulary suggest --apply` adds heard spellings to terms the
    // file already holds; a plain term becomes a table in its place.
    let before = concat!(
        "[dictation]\n",
        "vocabulary = [\n",
        "  \"Clef\", # the app\n",
        "  { term = \"Omarchy\", sounds_like = [\n",
        "    \"oh marky\", # how I say it\n",
        "  ] },\n",
        "  \"Dettivo\",\n",
        "]\n",
    );
    let out = add_or_extend(
        before,
        &[
            entry("clef", &["clay", "cleff"]),
            entry("Omarchy", &["Oh Marky", "omar key"]),
            entry("Dettivo", &[]),
            entry("Hyprland", &["hyper land"]),
        ],
    )
    .unwrap();
    assert_eq!(
        out.text,
        concat!(
            "[dictation]\n",
            "vocabulary = [\n",
            "  { term = \"Clef\", sounds_like = [\"clay\", \"cleff\"] }, # the app\n",
            "  { term = \"Omarchy\", sounds_like = [\n",
            "    \"oh marky\", # how I say it\n",
            "    \"omar key\",\n",
            "  ] },\n",
            "  \"Dettivo\",\n",
            "  { term = \"Hyprland\", sounds_like = [\"hyper land\"] },\n",
            "]\n",
        )
    );
    assert_eq!((out.added, out.extended), (vec![3], vec![0, 1]));
    assert_eq!(
        out.skipped,
        vec![Skipped {
            index: 2,
            repeats: None
        }]
    );
    assert_eq!(out.entries, 4);
    // Without extending, a held term is skipped whatever it carries.
    let plain = add(before, &[entry("clef", &["clay"])]).unwrap();
    assert_eq!((plain.text.as_str(), plain.extended.len()), (before, 0));
}

#[test]
fn extending_a_table_and_a_spelling_limit_name_the_entry() {
    let before = "[[dictation.vocabulary]]\nterm = \"Clef\" # mine\n";
    let out = add_or_extend(before, &[entry("Clef", &["clay"])]).unwrap();
    assert_eq!(
        out.text,
        "[[dictation.vocabulary]]\nterm = \"Clef\" # mine\nsounds_like = [\"clay\"]\n"
    );
    assert_eq!(out.extended, vec![0]);
    let full: Vec<String> = (0..MAX_ALIASES).map(|i| format!("s{i}")).collect();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    let text = add_or_extend(before, &[entry("Clef", &full)]).unwrap().text;
    let AddError::Refused(refused) =
        add_or_extend(&text, &[entry("Clef", &["one more"])]).unwrap_err()
    else {
        panic!("refused by limit");
    };
    assert_eq!(
        refused,
        vec![(
            0,
            format!("\"Clef\" has 17 sounds_like spellings; a term takes at most {MAX_ALIASES}")
        )]
    );
}
