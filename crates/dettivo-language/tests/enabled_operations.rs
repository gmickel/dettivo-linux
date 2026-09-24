//! Disabled editorial operations preserve literal text and paragraph boundaries.

use dettivo_language::polish::{PostProcessor, Style, Transform, apply_post_processors, rewrite};

#[test]
fn grammar_does_not_enable_punctuation_and_paragraphs_survive() {
    assert_eq!(
        rewrite(
            "teh command\n\nnext line",
            &[Transform::FixGrammar].into_iter().collect(),
            &[],
            Style::AsDictated
        ),
        "the command\n\nnext line"
    );
}

#[test]
fn processors_apply_only_the_selected_operation() {
    let text = "check foo.swift\n\n1000 plus page";
    for (processors, expected) in [
        (vec![], text),
        (
            vec![PostProcessor::AtPrefixFilePaths],
            "check @foo.swift\n\n1000 plus page",
        ),
        (
            vec![PostProcessor::NormalizePlusPagePhrases],
            "check foo.swift\n\n1,000-plus-page",
        ),
    ] {
        assert_eq!(apply_post_processors(&processors, text), expected);
    }
}
