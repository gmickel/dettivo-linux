//! fn-96 against a live daemon: `speech.vocabulary.suggest` without Piper
//! or its voices names each missing piece and how to get it, and a term
//! with no words is refused. The run with Piper and a real engine is the
//! CLI's end-to-end test (`crates/dettivo-cli/tests/vocabulary_suggest.rs`).

mod common;

use common::{Daemon, Tree};
use serde_json::json;

#[test]
fn suggest_names_what_is_missing_and_refuses_a_term_without_words() {
    let tree = Tree::new();
    let daemon = Daemon::spawn(tree, &[("PATH", "/nonexistent")]);
    let err = daemon.request("speech.vocabulary.suggest", json!({"term": "Clef"}));
    assert_eq!(err["error"]["data"]["app_code"], "NOT_FOUND", "{err}");
    let message = err["error"]["message"].as_str().unwrap();
    assert!(message.contains("pipx install piper-tts"), "{message}");
    assert!(message.contains("en_US-amy-low.onnx"), "{message}");
    assert!(message.contains("en_US-ryan-low.onnx"), "{message}");

    let err = daemon.request("speech.vocabulary.suggest", json!({"term": " - "}));
    assert_eq!(err["error"]["data"]["app_code"], "INVALID_PARAMS", "{err}");
}
