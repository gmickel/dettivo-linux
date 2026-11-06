use super::*;

fn line(number: usize, term: &str, sounds_like: &[&str]) -> Line {
    Line {
        number,
        term: term.into(),
        sounds_like: sounds_like.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn a_list_reads_one_term_a_line_with_optional_spellings() {
    let text = "\u{feff}# my names\nDettivo\n\n  Clef = clay, cleff  \nC#\nSapien X T =\n# done\n";
    assert_eq!(
        parse(text).unwrap(),
        vec![
            line(2, "Dettivo", &[]),
            line(4, "Clef", &["clay", "cleff"]),
            line(5, "C#", &[]),
            line(6, "Sapien X T", &[]),
        ]
    );
    assert_eq!(
        line(4, "Clef", &["clay"]).entry(),
        json!({"term": "Clef", "sounds_like": ["clay"]})
    );
    assert_eq!(line(2, "Dettivo", &[]).entry(), json!("Dettivo"));
}

#[test]
fn every_line_that_breaks_a_term_limit_is_named() {
    let long = "x".repeat(MAX_TERM_CHARS + 1);
    let spellings = vec!["s"; MAX_ALIASES + 1].join(", ");
    let ok = "ü".repeat(MAX_TERM_CHARS);
    let text = format!("Dettivo\n{long}\n{ok}\nClef = {spellings}\n = clay\n");
    let errors = parse(&text).unwrap_err();
    assert_eq!(
        errors,
        vec![
            format!("line 2: {long:?} is 65 characters; a term holds at most 64"),
            "line 4: \"Clef\" has 17 sounds_like spellings; a term takes at most 16".to_string(),
            "line 5: no term before `=`".to_string(),
        ]
    );
}

#[test]
fn duplicates_of_the_vocabulary_and_of_earlier_lines_are_skipped_in_any_case() {
    let existing = vec![
        json!("Dettivo"),
        json!({"term": "Clef", "sounds_like": ["clay"]}),
    ];
    let lines = vec![
        line(1, "dettivo", &[]),
        line(2, "Omarchy", &[]),
        line(3, "CLEF", &["cleff"]),
        line(4, "omarchy", &["oh marky"]),
    ];
    let plan = plan(existing.clone(), lines).unwrap();
    assert_eq!(plan.added, vec![line(2, "Omarchy", &[])]);
    assert_eq!(
        plan.skipped,
        vec![
            Skipped {
                line: 1,
                term: "dettivo".into(),
                repeats: None
            },
            Skipped {
                line: 3,
                term: "CLEF".into(),
                repeats: None
            },
            Skipped {
                line: 4,
                term: "omarchy".into(),
                repeats: Some(2)
            },
        ]
    );
    assert_eq!(plan.entries, [existing, vec![json!("Omarchy")]].concat());
    assert_eq!(
        report(&plan, false),
        concat!(
            "added 1 term to dictation.vocabulary (3 entries)\n",
            "skipped 3 duplicates\n",
            "  line 1: \"dettivo\" is already in the vocabulary\n",
            "  line 3: \"CLEF\" is already in the vocabulary\n",
            "  line 4: \"omarchy\" repeats line 2\n",
        )
    );
}

#[test]
fn every_line_past_the_entry_limit_is_named_and_a_full_list_passes() {
    let existing: Vec<Value> = (0..MAX_ENTRIES - 2)
        .map(|i| json!(format!("t{i}")))
        .collect();
    let fits: Vec<Line> = (0..2).map(|i| line(i + 1, &format!("n{i}"), &[])).collect();
    assert_eq!(
        plan(existing.clone(), fits).unwrap().entries.len(),
        MAX_ENTRIES
    );
    let over: Vec<Line> = (0..4).map(|i| line(i + 1, &format!("n{i}"), &[])).collect();
    assert_eq!(
        plan(existing, over).unwrap_err(),
        vec![
            "line 3: \"n2\" would be entry 501; a vocabulary holds at most 500",
            "line 4: \"n3\" would be entry 502; a vocabulary holds at most 500",
        ]
    );
}

#[test]
fn the_byte_limit_counts_the_list_as_the_daemon_does() {
    let spellings: Vec<String> = vec!["x".repeat(160); MAX_ALIASES];
    let lines: Vec<Line> = (0..MAX_ENTRIES)
        .map(|i| Line {
            number: i + 1,
            term: format!("t{i}"),
            sounds_like: spellings.clone(),
        })
        .collect();
    let errors = plan(Vec::new(), lines.clone()).unwrap_err();
    // The first named line is the first whose entry overruns the budget,
    // measured as the serialized list the daemon checks.
    let first: usize = errors[0]
        .strip_prefix("line ")
        .and_then(|e| e.split(':').next())
        .and_then(|n| n.parse().ok())
        .unwrap();
    let fitting: Vec<Value> = lines[..first - 1].iter().map(Line::entry).collect();
    assert!(json_len(&Value::Array(fitting.clone())) <= MAX_VOCABULARY_BYTES);
    let mut over = fitting;
    over.push(lines[first - 1].entry());
    let bytes = json_len(&Value::Array(over));
    assert!(bytes > MAX_VOCABULARY_BYTES);
    assert!(
        errors[0].ends_with(&format!(
            "would take the vocabulary to {bytes} bytes; it holds at most {MAX_VOCABULARY_BYTES}"
        )),
        "{}",
        errors[0]
    );
    assert_eq!(errors.len(), MAX_ENTRIES - first + 1);
}

#[test]
fn a_dry_run_report_lists_each_new_term_with_its_spellings() {
    let plan = plan(
        vec![json!("Dettivo")],
        vec![line(1, "Clef", &["clay", "cleff"]), line(3, "Omarchy", &[])],
    )
    .unwrap();
    assert_eq!(
        report(&plan, true),
        concat!(
            "would add 2 terms to dictation.vocabulary (3 entries after)\n",
            "  line 1: Clef (sounds like clay, cleff)\n",
            "  line 3: Omarchy\n",
        )
    );
    let none = super::plan(vec![json!("Dettivo")], Vec::new()).unwrap();
    assert_eq!(
        report(&none, false),
        "nothing to add; dictation.vocabulary holds every term\n"
    );
}
