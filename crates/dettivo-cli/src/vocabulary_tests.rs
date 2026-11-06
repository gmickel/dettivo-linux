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
    assert_eq!(
        parse("Dettivo\n = clay\n=\n").unwrap_err(),
        vec!["line 2: no term before `=`", "line 3: no term before `=`"]
    );
}

#[test]
fn the_report_names_each_line_the_daemon_added_or_skipped() {
    let lines = vec![
        line(1, "dettivo", &[]),
        line(3, "Clef", &["clay", "cleff"]),
        line(4, "clef", &[]),
    ];
    let result = json!({
        "added": [1],
        "skipped": [{"index": 0, "repeats": null}, {"index": 2, "repeats": 1}],
        "entries": 4,
        "written": true,
    });
    let skipped = concat!(
        "skipped 2 duplicates\n",
        "  line 1: \"dettivo\" is already in the vocabulary\n",
        "  line 4: \"clef\" repeats line 3\n",
    );
    assert_eq!(
        report(&result, &lines, false),
        format!("added 1 term to dictation.vocabulary (4 entries)\n{skipped}")
    );
    assert_eq!(
        report(&result, &lines, true),
        format!(
            "would add 1 term to dictation.vocabulary (4 entries after)\n  line 3: Clef (sounds like clay, cleff)\n{skipped}"
        )
    );
    let none = json!({"added": [], "skipped": [], "entries": 4, "written": false});
    assert_eq!(
        report(&none, &lines, false),
        "nothing to add; dictation.vocabulary holds every term\n"
    );
}
