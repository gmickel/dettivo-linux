//! The QA mock provider: `DETTIVO_MOCK_LLM=echo` returns the transcript
//! unchanged (the Enhanced path runs end to end without a model), and
//! `DETTIVO_MOCK_LLM=fixture:<dir>` replays a recorded rewrite from that
//! directory, so a guard rejection, a degenerate rewrite, a thinking
//! block or a timeout can be staged deterministically.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::{Availability, LlmProvider, MOCK_ENV, ProviderError, RewriteRequest};
use crate::enhanced::prompt::{TRANSCRIPT_BEGIN, TRANSCRIPT_END};

/// What `DETTIVO_MOCK_LLM` asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// The transcript comes back unchanged.
    Echo,
    /// Rewrites are read from files in this directory.
    Fixture(PathBuf),
}

impl Mode {
    /// Parses `echo` or `fixture:<dir>`.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("echo") {
            return Some(Self::Echo);
        }
        value
            .strip_prefix("fixture:")
            .map(|dir| Self::Fixture(PathBuf::from(dir.trim())))
    }
}

/// The mock provider `DETTIVO_MOCK_LLM` names, or `None` when it is unset
/// or unreadable.
pub fn from_env() -> Option<Box<dyn LlmProvider>> {
    let value = std::env::var(MOCK_ENV).ok()?;
    let mode = Mode::parse(&value)?;
    Some(Box::new(MockProvider::new(mode)))
}

/// A provider that answers from the environment rather than a model.
#[derive(Debug, Clone)]
pub struct MockProvider {
    mode: Mode,
}

impl MockProvider {
    /// A mock in `mode`.
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }

    /// The file name a transcript maps to: the first sixteen hex
    /// characters of its SHA-256, so a fixture directory is stable and a
    /// recorded case is found without a manifest.
    pub fn fixture_name(transcript: &str) -> String {
        let digest = Sha256::digest(transcript.trim().as_bytes());
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            .chars()
            .take(16)
            .collect()
    }

    fn replay(dir: &Path, transcript: &str) -> Result<String, ProviderError> {
        let named = dir.join(format!("{}.txt", Self::fixture_name(transcript)));
        let fallback = dir.join("default.txt");
        for candidate in [named, fallback] {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if text.trim() == "__TIMEOUT__" {
                    return Err(ProviderError::Timeout);
                }
                return Ok(text.trim_end_matches('\n').to_string());
            }
        }
        Err(ProviderError::Unavailable(format!(
            "no fixture for this transcript under {}",
            dir.display()
        )))
    }
}

/// The transcript back out of the wrapped user message.
pub fn unwrap_transcript(user: &str) -> String {
    let Some(start) = user.find(TRANSCRIPT_BEGIN) else {
        return user.trim().to_string();
    };
    let start = start + TRANSCRIPT_BEGIN.len();
    let end = user[start..]
        .find(TRANSCRIPT_END)
        .map_or(user.len(), |i| start + i);
    user[start..end].trim().to_string()
}

impl LlmProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn model(&self) -> String {
        match &self.mode {
            Mode::Echo => "mock-echo".into(),
            Mode::Fixture(dir) => format!("mock-fixture:{}", dir.display()),
        }
    }

    fn probe(&self) -> Availability {
        match &self.mode {
            Mode::Echo => Availability::up("DETTIVO_MOCK_LLM=echo"),
            Mode::Fixture(dir) if dir.is_dir() => {
                Availability::up(format!("DETTIVO_MOCK_LLM=fixture:{}", dir.display()))
            }
            Mode::Fixture(dir) => Availability::down(
                format!("{} is not a directory", dir.display()),
                Some("point DETTIVO_MOCK_LLM at a fixture directory".into()),
            ),
        }
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        _timeout: Duration,
    ) -> Result<String, ProviderError> {
        let transcript = unwrap_transcript(&request.user);
        match &self.mode {
            Mode::Echo => Ok(transcript),
            Mode::Fixture(dir) => Self::replay(dir, &transcript),
        }
    }

    /// The mock answers the same whatever the model; the override is
    /// accepted so the policy and the request agree.
    fn with_model(&self, _model: &str) -> Option<Box<dyn LlmProvider>> {
        Some(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enhanced::prompt::wrap_transcript;

    fn request(text: &str) -> RewriteRequest {
        RewriteRequest {
            system: String::new(),
            user: wrap_transcript(text),
        }
    }

    #[test]
    fn echo_returns_the_transcript_unchanged() {
        assert_eq!(Mode::parse("echo"), Some(Mode::Echo));
        let provider = MockProvider::new(Mode::Echo);
        assert!(provider.probe().available);
        assert_eq!(provider.model(), "mock-echo");
        assert_eq!(
            provider
                .rewrite(&request("Hello there."), Duration::from_secs(1))
                .unwrap(),
            "Hello there."
        );
    }

    #[test]
    fn a_fixture_directory_replays_a_recorded_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let name = MockProvider::fixture_name("Hello there.");
        std::fs::write(dir.path().join(format!("{name}.txt")), "Hi there!\n").unwrap();
        std::fs::write(dir.path().join("default.txt"), "Fallback.\n").unwrap();
        assert_eq!(
            Mode::parse("fixture:/tmp/x"),
            Some(Mode::Fixture(PathBuf::from("/tmp/x")))
        );
        let provider = MockProvider::new(Mode::Fixture(dir.path().to_path_buf()));
        assert!(provider.probe().available);
        assert_eq!(
            provider
                .rewrite(&request("Hello there."), Duration::from_secs(1))
                .unwrap(),
            "Hi there!"
        );
        assert_eq!(
            provider
                .rewrite(&request("Something else."), Duration::from_secs(1))
                .unwrap(),
            "Fallback."
        );
    }

    #[test]
    fn a_fixture_can_stage_a_timeout_and_a_missing_directory_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("default.txt"), "__TIMEOUT__").unwrap();
        let provider = MockProvider::new(Mode::Fixture(dir.path().to_path_buf()));
        assert_eq!(
            provider.rewrite(&request("anything"), Duration::from_secs(1)),
            Err(ProviderError::Timeout)
        );
        let absent = MockProvider::new(Mode::Fixture(PathBuf::from("/nonexistent-fixture-dir")));
        assert!(!absent.probe().available);
        assert!(matches!(
            absent.rewrite(&request("anything"), Duration::from_secs(1)),
            Err(ProviderError::Unavailable(_))
        ));
        assert_eq!(Mode::parse("nonsense"), None);
    }
}
