//! R2: the meeting analysis golden cases under
//! `tests/goldens/meeting_analysis.json`. Each case scripts the model's
//! answers in order and states the analysis that must come out, or the
//! failure code; the scripted provider records the prompts so the repair
//! pass and the merge pass are proven by their wording, and the call and
//! part counts prove map then reduce over a transcript longer than
//! `chunk_chars`, and the halving of a part whose answer was cut off. An
//! output `__TIMEOUT__` stages a provider timeout, `__TOO_LONG__` a
//! prompt that did not fit the context, and a `__LENGTH__:` prefix an
//! answer the provider reports as cut off by the output budget.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use dettivo_language::analysis::{self, Failure, Settings};
use dettivo_language::provider::{
    Answer, Availability, Finish, LlmProvider, ProviderError, RewriteRequest,
};
use dettivo_proto::methods::meetings_notes::Analysis;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Suite {
    source: String,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    transcript: String,
    #[serde(default)]
    chunk_chars: Option<usize>,
    outputs: Vec<String>,
    #[serde(default)]
    expected: Option<Analysis>,
    #[serde(default)]
    failure: Option<String>,
    #[serde(default)]
    failure_contains: Option<String>,
    #[serde(default)]
    calls: Option<u32>,
    #[serde(default)]
    parts: Option<u32>,
    #[serde(default)]
    repair_prompt_contains: Option<String>,
    #[serde(default)]
    merge_prompt_contains: Option<String>,
    #[serde(default)]
    merge_system_prompt_contains: Option<String>,
    /// Prompts, in order, that must each be the user message of one call.
    #[serde(default)]
    prompts_in_order: Vec<String>,
}

/// Answers from a script and keeps every request it saw.
struct Scripted {
    answers: Mutex<Vec<String>>,
    seen: Mutex<Vec<RewriteRequest>>,
}

impl LlmProvider for Scripted {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> String {
        "scripted".into()
    }

    fn probe(&self) -> Availability {
        Availability::up("scripted")
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        timeout: Duration,
    ) -> Result<String, ProviderError> {
        self.answer(request, timeout).map(|a| a.text)
    }

    fn answer(&self, request: &RewriteRequest, _: Duration) -> Result<Answer, ProviderError> {
        self.seen.lock().unwrap().push(request.clone());
        let mut answers = self.answers.lock().unwrap();
        if answers.is_empty() {
            return Err(ProviderError::Unavailable("the script ran out".into()));
        }
        let answer = answers.remove(0);
        if answer == "__TIMEOUT__" {
            return Err(ProviderError::Timeout);
        }
        if answer == "__TOO_LONG__" {
            return Err(ProviderError::PromptTooLong(
                "prompt of 4200 tokens exceeds the context of 4096".into(),
            ));
        }
        if let Some(cut) = answer.strip_prefix("__LENGTH__:") {
            return Ok(Answer {
                text: cut.to_string(),
                finish: Finish::Length,
            });
        }
        Ok(Answer {
            text: answer,
            finish: Finish::Stop,
        })
    }
}

fn load() -> Suite {
    let path: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/meeting_analysis.json");
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_analysis_cases_hold() {
    let suite = load();
    for case in &suite.cases {
        let provider = Scripted {
            answers: Mutex::new(case.outputs.clone()),
            seen: Mutex::new(Vec::new()),
        };
        let settings = Settings {
            chunk_chars: case.chunk_chars.unwrap_or(12_000),
            timeout: Duration::from_secs(5),
        };
        let outcome = analysis::run(&case.transcript, &provider, &settings, None);
        let seen = provider.seen.lock().unwrap();
        let label = format!("{} / {}", suite.source, case.name);
        match (&case.expected, &case.failure) {
            (Some(want), _) => {
                let got = outcome.unwrap_or_else(|e| panic!("{label}: {e}"));
                assert_eq!(&got.analysis, want, "{label}");
                assert_eq!(got.model, "scripted");
                if let Some(parts) = case.parts {
                    assert_eq!(got.parts, parts, "{label}: parts");
                }
                if let Some(calls) = case.calls {
                    assert_eq!(got.calls, calls, "{label}: calls");
                }
            }
            (None, Some(code)) => {
                let err: Failure = outcome.expect_err(&label);
                assert_eq!(err.code(), code, "{label}: {err}");
                if let Some(needle) = &case.failure_contains {
                    assert!(err.to_string().contains(needle), "{label}: {err}");
                }
                if let Some(calls) = case.calls {
                    assert_eq!(seen.len() as u32, calls, "{label}: calls");
                }
            }
            (None, None) => panic!("{label}: a case states expected or failure"),
        }
        if let Some(needle) = &case.repair_prompt_contains {
            assert!(
                seen.iter()
                    .any(|r| r.system.contains("REPAIR PASS") && r.system.contains(needle)),
                "{label}: no repair prompt carrying {needle:?}"
            );
        }
        if let Some(needle) = &case.merge_prompt_contains {
            let last = seen.last().expect("a merge call");
            assert!(
                last.user.contains(needle),
                "{label}: the merge prompt lacks {needle:?}"
            );
            assert!(
                seen[..seen.len() - 1]
                    .iter()
                    .all(|r| r.user.contains("Part ")),
                "{label}: every earlier call is a part"
            );
        }
        if let Some(needle) = &case.merge_system_prompt_contains {
            let last = seen.last().expect("a merge call");
            assert!(
                last.system.contains(needle),
                "{label}: the merge system prompt lacks {needle:?}"
            );
            assert!(
                !last.system.contains("REPAIR PASS"),
                "{label}: a cut-off answer is never repaired"
            );
        }
        for (call, needle) in case.prompts_in_order.iter().enumerate() {
            let request = seen
                .get(call)
                .unwrap_or_else(|| panic!("{label}: no call {call}"));
            assert!(
                request.user.contains(needle),
                "{label}: call {call} lacks {needle:?}: {}",
                request.user
            );
        }
    }
    assert!(suite.cases.len() >= 12, "the golden set must not shrink");
}
